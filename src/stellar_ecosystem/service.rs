//! Orchestration service for Stellar Ecosystem Partner Integration (Issue #470).

#[cfg(feature = "database")]
use crate::stellar_ecosystem::{
    dex_pathfinding::{apply_slippage_buffer, enforce_slippage, DexPathfinder, DEFAULT_MAX_SLIPPAGE},
    metrics,
    models::*,
    repository,
    sep_client::SepClient,
    transaction_builder::StellarTransactionBuilder,
};
#[cfg(feature = "database")]
use anyhow::{anyhow, Context, Result};
#[cfg(feature = "database")]
use rust_decimal::prelude::*;
#[cfg(feature = "database")]
use sqlx::PgPool;
#[cfg(feature = "database")]
use std::sync::Arc;
#[cfg(feature = "database")]
use tokio::sync::RwLock;
#[cfg(feature = "database")]
use tracing::instrument;

/// Shared DEX configuration (updated via admin endpoint).
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct EcosystemConfig {
    pub horizon_url: String,
    pub max_slippage: Decimal,
    pub min_liquidity_depth: Decimal,
    pub monitored_pairs: Vec<AssetPair>,
    /// Stellar account used as the platform's sending account.
    pub platform_account: String,
    /// Stellar network passphrase.
    pub network_passphrase: String,
}

#[cfg(feature = "database")]
impl Default for EcosystemConfig {
    fn default() -> Self {
        Self {
            horizon_url: std::env::var("STELLAR_HORIZON_URL")
                .unwrap_or_else(|_| "https://horizon-testnet.stellar.org".into()),
            max_slippage: Decimal::from_str(DEFAULT_MAX_SLIPPAGE).unwrap(),
            min_liquidity_depth: Decimal::new(10_000, 0),
            monitored_pairs: vec![
                AssetPair {
                    base_asset: "cNGN".into(),
                    counter_asset: "USDC".into(),
                },
                AssetPair {
                    base_asset: "cNGN".into(),
                    counter_asset: "EURC".into(),
                },
            ],
            platform_account: std::env::var("STELLAR_PLATFORM_ACCOUNT").unwrap_or_default(),
            network_passphrase: std::env::var("STELLAR_NETWORK_PASSPHRASE")
                .unwrap_or_else(|_| "Test SDF Network ; September 2015".into()),
        }
    }
}

#[cfg(feature = "database")]
pub struct EcosystemService {
    pool: PgPool,
    config: Arc<RwLock<EcosystemConfig>>,
}

#[cfg(feature = "database")]
impl EcosystemService {
    pub fn new(pool: PgPool, config: EcosystemConfig) -> Self {
        Self {
            pool,
            config: Arc::new(RwLock::new(config)),
        }
    }

    pub async fn config(&self) -> EcosystemConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, req: UpdateDexConfigRequest) {
        let mut cfg = self.config.write().await;
        if let Some(s) = req.max_slippage {
            cfg.max_slippage = s;
        }
        if let Some(d) = req.min_liquidity_depth {
            cfg.min_liquidity_depth = d;
        }
        if let Some(p) = req.monitored_pairs {
            cfg.monitored_pairs = p;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Anchor management
    // ─────────────────────────────────────────────────────────────────────────

    #[instrument(skip(self))]
    pub async fn register_anchor(
        &self,
        req: CreateAnchorConnectionRequest,
    ) -> Result<AnchorConnection> {
        repository::insert_anchor_connection(&self.pool, &req).await
    }

    #[instrument(skip(self))]
    pub async fn list_anchors(&self) -> Result<Vec<AnchorConnection>> {
        repository::list_anchor_connections(&self.pool).await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DEX pathfinding
    // ─────────────────────────────────────────────────────────────────────────

    #[instrument(skip(self))]
    pub async fn find_path(&self, req: PathfindingRequest) -> Result<PathfindingResult> {
        let cfg = self.config.read().await;
        let pathfinder = DexPathfinder::new(&cfg.horizon_url, cfg.max_slippage);
        drop(cfg);
        pathfinder.find_path(&self.pool, &req).await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cross-anchor transfer (SEP-31)
    // ─────────────────────────────────────────────────────────────────────────

    /// Initiate a cross-anchor SEP-31 transfer with slippage protection.
    #[instrument(skip(self), fields(
        send_asset = %req.send_asset,
        receive_asset = %req.receive_asset,
        amount = %req.send_amount,
    ))]
    pub async fn initiate_transfer(
        &self,
        req: InitiateTransferRequest,
    ) -> Result<CrossAnchorTransfer> {
        let cfg = self.config.read().await.clone();

        // 1. Resolve anchor
        let anchor = repository::get_anchor_by_domain(&self.pool, &req.receiving_anchor_domain)
            .await?
            .ok_or_else(|| anyhow!("anchor '{}' not found", req.receiving_anchor_domain))?;

        if anchor.status != "active" {
            return Err(anyhow!("anchor '{}' is not active", anchor.domain));
        }
        if !anchor.sep31_enabled {
            return Err(anyhow!("anchor '{}' does not support SEP-31", anchor.domain));
        }

        // 2. Pathfinding + slippage check
        let path_req = PathfindingRequest {
            source_asset: req.send_asset.clone(),
            destination_asset: req.receive_asset.clone(),
            source_amount: Some(req.send_amount),
            destination_amount: None,
        };
        let pathfinder = DexPathfinder::new(&cfg.horizon_url, cfg.max_slippage);
        let path_result = pathfinder.find_path(&self.pool, &path_req).await?;

        let max_slippage = req.max_slippage.unwrap_or(cfg.max_slippage);
        enforce_slippage(&path_result, max_slippage, &req.send_asset, &req.receive_asset)?;

        // 3. Persist transfer record
        let transfer = repository::insert_cross_anchor_transfer(&self.pool, &req, anchor.id).await?;

        // 4. Call SEP-31 on the receiving anchor
        let horizon_url = anchor
            .horizon_url
            .as_deref()
            .unwrap_or(&cfg.horizon_url)
            .to_string();
        let sep_client = SepClient::new(&anchor.domain, &horizon_url);

        let jwt = anchor
            .jwt_token
            .as_deref()
            .ok_or_else(|| anyhow!("no JWT for anchor '{}' — run SEP-10 auth first", anchor.domain))?;

        // Discover SEP-31 endpoint from stellar.toml
        let toml = sep_client.discover_sep_endpoints().await.context("discover SEP endpoints")?;
        let sep31_endpoint = toml
            .direct_payment_server
            .ok_or_else(|| anyhow!("anchor '{}' has no DIRECT_PAYMENT_SERVER", anchor.domain))?;

        let sep31_req = Sep31SendRequest {
            amount: req.send_amount,
            asset_code: req.send_asset.split(':').next().unwrap_or(&req.send_asset).to_string(),
            asset_issuer: req.send_asset.split(':').nth(1).map(str::to_string),
            destination_asset: Some(req.receive_asset.clone()),
            sender_id: None,
            receiver_id: None,
            fields: None,
        };

        let sep31_resp = sep_client.sep31_send(&sep31_endpoint, jwt, &sep31_req).await?;

        repository::update_transfer_sep31_id(
            &self.pool,
            transfer.id,
            &sep31_resp.id,
            None,
        )
        .await?;

        // 5. Build and submit the Stellar transaction
        let send_max = apply_slippage_buffer(req.send_amount, max_slippage);
        let built_tx = StellarTransactionBuilder::new(
            &req.sender_account,
            0, // sequence fetched from Horizon in production
            &cfg.network_passphrase,
        )
        .add_path_payment_strict_receive(
            &req.send_asset,
            send_max,
            &sep31_resp.stellar_account_id,
            &req.receive_asset,
            path_result.destination_amount,
            path_result.path.clone(),
        )?
        .build()?;

        tracing::info!(
            transfer_id = %transfer.id,
            sep31_id = %sep31_resp.id,
            tx_xdr = %built_tx.xdr_base64,
            stellar_account = %sep31_resp.stellar_account_id,
            spread = %path_result.spread,
            "Cross-anchor transfer transaction built"
        );

        // Record submission (mock hash — real submission would call Horizon /transactions)
        let mock_hash = format!("PENDING_{}", transfer.id);
        repository::update_transfer_submitted(
            &self.pool,
            transfer.id,
            &mock_hash,
            &built_tx.xdr_base64,
            0,
            path_result.spread,
        )
        .await?;

        metrics::record_transfer_outcome(&anchor.domain, "pending_receiver");

        // Return updated transfer
        repository::get_transfer_by_id(&self.pool, transfer.id)
            .await?
            .ok_or_else(|| anyhow!("transfer not found after insert"))
    }
}
