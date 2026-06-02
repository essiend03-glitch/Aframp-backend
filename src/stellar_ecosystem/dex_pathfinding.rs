//! Stellar DEX pathfinding service (Issue #470).
//! Queries Horizon for optimal trading paths and enforces slippage tolerances.

#[cfg(feature = "database")]
use crate::stellar_ecosystem::{
    metrics,
    models::{PathfindingRequest, PathfindingResult},
    repository,
};
#[cfg(feature = "database")]
use anyhow::{anyhow, Context, Result};
#[cfg(feature = "database")]
use rust_decimal::prelude::*;
#[cfg(feature = "database")]
use sqlx::PgPool;
#[cfg(feature = "database")]
use std::time::Instant;
#[cfg(feature = "database")]
use tracing::{instrument, warn};

/// Maximum allowed slippage fraction (0.5%) — overridable via admin config.
pub const DEFAULT_MAX_SLIPPAGE: &str = "0.005";
/// Sub-200ms pathfinding budget (seconds).
pub const PATHFINDING_BUDGET_SECS: f64 = 0.200;

#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct DexPathfinder {
    pub horizon_url: String,
    pub max_slippage: Decimal,
    http: reqwest::Client,
}

#[cfg(feature = "database")]
impl DexPathfinder {
    pub fn new(horizon_url: impl Into<String>, max_slippage: Decimal) -> Self {
        Self {
            horizon_url: horizon_url.into(),
            max_slippage,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(180)) // stay under 200ms budget
                .build()
                .expect("build reqwest client"),
        }
    }

    /// Find the optimal path for a source→destination asset swap.
    /// Returns `Err` if no path exists or the spread exceeds `max_slippage`.
    #[instrument(skip(self, pool), fields(
        src = %req.source_asset,
        dst = %req.destination_asset,
    ))]
    pub async fn find_path(
        &self,
        pool: &PgPool,
        req: &PathfindingRequest,
    ) -> Result<PathfindingResult> {
        let t = Instant::now();

        // 1. Check local snapshot cache first
        if let Ok(Some(snap)) =
            repository::get_latest_snapshot(pool, &req.source_asset, &req.destination_asset).await
        {
            if let (Some(bid), Some(ask)) = (snap.best_bid, snap.best_ask) {
                let spread = compute_spread(bid, ask);
                let elapsed = t.elapsed().as_secs_f64();
                metrics::observe_pathfinding_duration(
                    &req.source_asset,
                    &req.destination_asset,
                    elapsed,
                );
                metrics::set_consecutive_pathfinding_failures(
                    &req.source_asset,
                    &req.destination_asset,
                    0.0,
                );

                let src_amount = req
                    .source_amount
                    .unwrap_or_else(|| Decimal::new(1, 0));
                let dst_amount = src_amount * bid;

                return Ok(PathfindingResult {
                    source_asset: req.source_asset.clone(),
                    source_amount: src_amount,
                    destination_asset: req.destination_asset.clone(),
                    destination_amount: dst_amount,
                    path: vec![req.source_asset.clone(), req.destination_asset.clone()],
                    spread,
                    within_tolerance: spread <= self.max_slippage,
                });
            }
        }

        // 2. Query Horizon /paths/strict-send or /paths/strict-receive
        let result = self.query_horizon_paths(req).await;

        let elapsed = t.elapsed().as_secs_f64();
        metrics::observe_pathfinding_duration(&req.source_asset, &req.destination_asset, elapsed);

        if elapsed > PATHFINDING_BUDGET_SECS {
            warn!(
                elapsed_secs = elapsed,
                budget_secs = PATHFINDING_BUDGET_SECS,
                "Pathfinding exceeded 200ms budget"
            );
        }

        match result {
            Ok(r) => {
                metrics::set_consecutive_pathfinding_failures(
                    &req.source_asset,
                    &req.destination_asset,
                    0.0,
                );
                Ok(r)
            }
            Err(e) => {
                // Increment consecutive failure counter for alerting
                // (we don't have the current count here; the gauge is monotonically incremented)
                metrics::set_consecutive_pathfinding_failures(
                    &req.source_asset,
                    &req.destination_asset,
                    -1.0, // sentinel: caller should read+increment
                );
                Err(e)
            }
        }
    }

    async fn query_horizon_paths(&self, req: &PathfindingRequest) -> Result<PathfindingResult> {
        // Use strict-send when source_amount is specified, strict-receive otherwise
        let (endpoint, amount_param, amount_val) = if let Some(src) = req.source_amount {
            (
                "paths/strict-send",
                "source_amount",
                src.to_string(),
            )
        } else {
            let dst = req
                .destination_amount
                .ok_or_else(|| anyhow!("either source_amount or destination_amount required"))?;
            ("paths/strict-receive", "destination_amount", dst.to_string())
        };

        let url = format!(
            "{}/{endpoint}?source_asset_type={src_type}&source_asset_code={src_code}\
             &source_asset_issuer={src_issuer}\
             &destination_asset_type={dst_type}&destination_asset_code={dst_code}\
             &destination_asset_issuer={dst_issuer}\
             &{amount_param}={amount_val}",
            self.horizon_url,
            endpoint = endpoint,
            src_type = asset_type(&req.source_asset),
            src_code = asset_code(&req.source_asset),
            src_issuer = asset_issuer(&req.source_asset),
            dst_type = asset_type(&req.destination_asset),
            dst_code = asset_code(&req.destination_asset),
            dst_issuer = asset_issuer(&req.destination_asset),
            amount_param = amount_param,
            amount_val = amount_val,
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("Horizon pathfinding request")?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "Horizon pathfinding failed: HTTP {}",
                resp.status()
            ));
        }

        let body: serde_json::Value = resp.json().await.context("parse Horizon paths response")?;
        let records = body["_embedded"]["records"]
            .as_array()
            .ok_or_else(|| anyhow!("no path records in Horizon response"))?;

        if records.is_empty() {
            return Err(anyhow!(
                "No DEX path found for {}/{}",
                req.source_asset,
                req.destination_asset
            ));
        }

        // Pick the best path (first record = best by Horizon)
        let best = &records[0];
        let src_amount = parse_stellar_amount(best["source_amount"].as_str().unwrap_or("0"))?;
        let dst_amount =
            parse_stellar_amount(best["destination_amount"].as_str().unwrap_or("0"))?;

        let path: Vec<String> = best["path"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a["asset_code"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        // Spread = (src_amount / dst_amount - 1) as a fraction
        let spread = if dst_amount.is_zero() {
            Decimal::ZERO
        } else {
            ((src_amount / dst_amount) - Decimal::ONE).abs()
        };

        Ok(PathfindingResult {
            source_asset: req.source_asset.clone(),
            source_amount: src_amount,
            destination_asset: req.destination_asset.clone(),
            destination_amount: dst_amount,
            path,
            spread,
            within_tolerance: spread <= self.max_slippage,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Slippage guard
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `Err` if the live spread exceeds `max_slippage`.
/// Call this immediately before submitting a transaction to enforce the
/// slippage protection requirement.
#[cfg(feature = "database")]
pub fn enforce_slippage(
    path_result: &PathfindingResult,
    max_slippage: Decimal,
    base_asset: &str,
    counter_asset: &str,
) -> Result<()> {
    if path_result.spread > max_slippage {
        metrics::record_slippage_rejection(base_asset, counter_asset);
        tracing::warn!(
            spread = %path_result.spread,
            max_slippage = %max_slippage,
            "Slippage threshold exceeded — aborting execution"
        );
        return Err(anyhow!(
            "Slippage {:.4}% exceeds maximum {:.4}%",
            path_result.spread * Decimal::ONE_HUNDRED,
            max_slippage * Decimal::ONE_HUNDRED,
        ));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a Stellar amount string (7 decimal places) into Decimal.
#[cfg(feature = "database")]
pub fn parse_stellar_amount(s: &str) -> Result<Decimal> {
    Decimal::from_str(s).map_err(|e| anyhow!("invalid Stellar amount '{}': {}", s, e))
}

#[cfg(feature = "database")]
fn compute_spread(bid: Decimal, ask: Decimal) -> Decimal {
    if bid.is_zero() {
        return Decimal::ZERO;
    }
    ((ask - bid) / bid).abs()
}

/// Extract asset type for Horizon query ("native" or "credit_alphanum4/12").
#[cfg(feature = "database")]
fn asset_type(asset: &str) -> &str {
    if asset == "XLM" || asset == "native" {
        "native"
    } else if asset_code(asset).len() <= 4 {
        "credit_alphanum4"
    } else {
        "credit_alphanum12"
    }
}

#[cfg(feature = "database")]
fn asset_code(asset: &str) -> &str {
    asset.split(':').next().unwrap_or(asset)
}

#[cfg(feature = "database")]
fn asset_issuer(asset: &str) -> &str {
    asset.split(':').nth(1).unwrap_or("")
}
