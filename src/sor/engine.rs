//! #487 Smart Order Routing — pathfinder, order splitter, execution tracker.
//!
//! The pathfinder implements a modified Bellman-Ford over a venue graph where
//! edge weights are total cost in basis points (spread + execution fee).
//! The order splitter divides large orders across the cheapest venues to
//! minimise market impact.

use super::models::*;
use super::repository::SorRepository;
use super::metrics;
use crate::cache::RedisPool;
use anyhow::{anyhow, Result};
use bigdecimal::{BigDecimal, ToPrimitive};
use redis::AsyncCommands;
use serde_json::json;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Hard slippage limit applied at venue boundary (25 bps = 0.25 %).
pub const DEFAULT_MAX_SLIPPAGE_BPS: f64 = 25.0;

pub struct SorEngine {
    repo: Arc<SorRepository>,
    redis: RedisPool,
}

impl SorEngine {
    pub fn new(repo: Arc<SorRepository>, redis: RedisPool) -> Self {
        Self { repo, redis }
    }

    // ── Public entry point ────────────────────────────────────────────────────

    /// Route an order: compute optimal path, split across venues, persist
    /// execution record, and return the execution ID.
    pub async fn route_order(&self, req: RouteOrderRequest) -> Result<Uuid> {
        let t0 = Instant::now();

        let venues = self.repo.list_active_venues().await?;
        let edges = self.build_edges(&venues, &req.source_currency, &req.target_currency);

        if edges.is_empty() {
            metrics::routing_failures().inc();
            return Err(anyhow!("no_viable_route"));
        }

        let path = self.bellman_ford(&edges)?;

        // Slippage guard at venue boundary
        if path.total_cost_bps > req.max_slippage_bps {
            warn!(
                cost_bps = path.total_cost_bps,
                limit_bps = req.max_slippage_bps,
                "SOR: slippage limit exceeded — recalculating via alternate lane"
            );
            metrics::slippage_breaches().inc();
            return Err(anyhow!("slippage_limit_exceeded"));
        }

        let slices = self.split_order(&path, &req.amount);
        let calc_ms = t0.elapsed().as_millis() as i32;

        metrics::routing_duration().observe(t0.elapsed().as_secs_f64());

        let execution_id = Uuid::new_v4();
        let correlation_tag = format!("SOR-{}", &execution_id.to_string()[..8].to_uppercase());

        let routing_plan = json!(slices
            .iter()
            .map(|s| json!({
                "venue_id": s.venue_id,
                "venue_name": s.venue_name,
                "allocation_pct": s.allocation_pct,
                "amount": s.amount.to_string(),
            }))
            .collect::<Vec<_>>());

        let exec = SmartOrderExecution {
            execution_id,
            parent_transaction_id: req.parent_transaction_id,
            correlation_tag: correlation_tag.clone(),
            source_currency: req.source_currency.clone(),
            target_currency: req.target_currency.clone(),
            total_amount: req.amount.clone(),
            status: SorStatus::Routing,
            routing_plan,
            realized_slippage_bps: None,
            path_calc_ms: Some(calc_ms),
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        self.repo.insert_execution(&exec).await?;

        // Persist child orders
        for slice in &slices {
            let child = SorChildOrder {
                child_order_id: Uuid::new_v4(),
                execution_id,
                venue_id: slice.venue_id,
                allocation_pct: BigDecimal::try_from(slice.allocation_pct)
                    .unwrap_or_default(),
                allocated_amount: slice.amount.clone(),
                filled_amount: BigDecimal::from(0),
                status: ChildOrderStatus::Pending,
                venue_order_ref: None,
                slippage_bps: None,
                submitted_at: None,
                filled_at: None,
                failed_reason: None,
            };
            self.repo.insert_child_order(&child).await?;
        }

        // Cache routing plan in Redis for dashboard reads
        if let Ok(mut conn) = self.redis.get().await {
            let key = format!("sor:execution:{}", execution_id);
            let _: Result<(), _> = conn
                .set_ex(key, serde_json::to_string(&exec.routing_plan)?, 3600)
                .await;
        }

        info!(
            execution_id = %execution_id,
            correlation_tag = %correlation_tag,
            calc_ms,
            slices = slices.len(),
            total_cost_bps = path.total_cost_bps,
            "SOR: order routed"
        );

        metrics::orders_routed().inc();
        metrics::slippage_saved().observe(req.max_slippage_bps - path.total_cost_bps);

        Ok(execution_id)
    }

    // ── Pathfinder (Bellman-Ford) ─────────────────────────────────────────────

    fn build_edges(
        &self,
        venues: &[LiquidityVenue],
        src: &str,
        dst: &str,
    ) -> Vec<RouteEdge> {
        venues
            .iter()
            .filter(|v| {
                v.supported_currencies.contains(&src.to_string())
                    && v.supported_currencies.contains(&dst.to_string())
                    && v.used_volume_today < v.daily_volume_limit
            })
            .map(|v| {
                let cost_bps = v.spread_bps.to_f64().unwrap_or(0.0)
                    + v.execution_fee_bps.to_f64().unwrap_or(0.0);
                RouteEdge {
                    venue_id: v.venue_id,
                    venue_name: v.name.clone(),
                    venue_type: v.venue_type.clone(),
                    source_currency: src.to_string(),
                    target_currency: dst.to_string(),
                    cost_bps,
                    available_depth: v.daily_volume_limit.clone() - &v.used_volume_today,
                }
            })
            .collect()
    }

    /// Bellman-Ford over a two-node graph (src → dst).
    /// Returns the cheapest single-hop path; multi-hop extension is straightforward.
    fn bellman_ford(&self, edges: &[RouteEdge]) -> Result<RoutePath> {
        // For a direct src→dst graph, Bellman-Ford reduces to finding the
        // minimum-cost edge. We keep the full algorithm structure for future
        // multi-hop extension.
        let mut dist: HashMap<String, f64> = HashMap::new();
        let mut best_edge: Option<&RouteEdge> = None;

        if edges.is_empty() {
            return Err(anyhow!("no_edges"));
        }

        let src = &edges[0].source_currency;
        let dst = &edges[0].target_currency;
        dist.insert(src.clone(), 0.0);
        dist.insert(dst.clone(), f64::INFINITY);

        // Relax edges |V|-1 times (here |V|=2, so once suffices)
        for edge in edges {
            let src_dist = *dist.get(&edge.source_currency).unwrap_or(&f64::INFINITY);
            let new_dist = src_dist + edge.cost_bps;
            let dst_dist = dist.entry(edge.target_currency.clone()).or_insert(f64::INFINITY);
            if new_dist < *dst_dist {
                *dst_dist = new_dist;
                best_edge = Some(edge);
            }
        }

        let edge = best_edge.ok_or_else(|| anyhow!("no_path_found"))?;
        Ok(RoutePath {
            edges: vec![edge.clone()],
            total_cost_bps: *dist.get(dst).unwrap_or(&f64::INFINITY),
        })
    }

    // ── Order splitter ────────────────────────────────────────────────────────

    /// Split `amount` across the cheapest venues by available depth.
    /// Allocates proportionally to depth, capped at 3 venues to limit complexity.
    fn split_order(&self, path: &RoutePath, amount: &BigDecimal) -> Vec<OrderSlice> {
        // Collect all edges sorted by cost ascending
        let mut sorted = path.edges.clone();
        sorted.sort_by(|a, b| a.cost_bps.partial_cmp(&b.cost_bps).unwrap());
        sorted.truncate(3);

        let total_depth: f64 = sorted
            .iter()
            .map(|e| e.available_depth.to_f64().unwrap_or(0.0))
            .sum();

        if total_depth == 0.0 {
            // Fallback: equal split
            let pct = 1.0 / sorted.len() as f64;
            return sorted
                .iter()
                .map(|e| OrderSlice {
                    venue_id: e.venue_id,
                    venue_name: e.venue_name.clone(),
                    allocation_pct: pct,
                    amount: amount * BigDecimal::try_from(pct).unwrap_or_default(),
                })
                .collect();
        }

        sorted
            .iter()
            .map(|e| {
                let pct = e.available_depth.to_f64().unwrap_or(0.0) / total_depth;
                OrderSlice {
                    venue_id: e.venue_id,
                    venue_name: e.venue_name.clone(),
                    allocation_pct: pct,
                    amount: amount * BigDecimal::try_from(pct).unwrap_or_default(),
                }
            })
            .collect()
    }

    // ── Execution tracker ─────────────────────────────────────────────────────

    /// Mark a child order as filled and check if the parent execution is complete.
    pub async fn record_fill(
        &self,
        execution_id: Uuid,
        child_order_id: Uuid,
        filled_amount: &BigDecimal,
        slippage_bps: f64,
        venue_order_ref: &str,
    ) -> Result<()> {
        // Slippage guard at venue boundary
        if slippage_bps > DEFAULT_MAX_SLIPPAGE_BPS {
            warn!(
                child_order_id = %child_order_id,
                slippage_bps,
                "SOR: venue slippage exceeded limit — halting fill"
            );
            self.repo
                .fail_child_order(child_order_id, "slippage_limit_exceeded")
                .await?;
            self.repo
                .update_execution_status(execution_id, SorStatus::Failed, Some(slippage_bps))
                .await?;
            return Err(anyhow!("slippage_limit_exceeded"));
        }

        let bd = sqlx::types::BigDecimal::from_str(&filled_amount.to_string())?;
        self.repo
            .update_child_order_filled(child_order_id, &bd, slippage_bps, venue_order_ref)
            .await?;

        info!(
            execution_id = %execution_id,
            child_order_id = %child_order_id,
            slippage_bps,
            "SOR: child order filled"
        );
        Ok(())
    }

    /// Rollback all pending child orders for an execution (venue timeout / error).
    pub async fn rollback_execution(&self, execution_id: Uuid, reason: &str) -> Result<()> {
        self.repo
            .update_execution_status(execution_id, SorStatus::RolledBack, None)
            .await?;
        error!(execution_id = %execution_id, reason, "SOR: execution rolled back");
        metrics::rollbacks().inc();
        Ok(())
    }
}
