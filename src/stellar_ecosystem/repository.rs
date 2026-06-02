//! Database repository for Stellar Ecosystem Partner Integration (Issue #470).

#[cfg(feature = "database")]
use crate::stellar_ecosystem::models::*;
#[cfg(feature = "database")]
use anyhow::Result;
#[cfg(feature = "database")]
use rust_decimal::Decimal;
#[cfg(feature = "database")]
use sqlx::PgPool;
#[cfg(feature = "database")]
use tracing::instrument;
#[cfg(feature = "database")]
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Anchor connections
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn insert_anchor_connection(
    pool: &PgPool,
    req: &CreateAnchorConnectionRequest,
) -> Result<AnchorConnection> {
    let row = sqlx::query_as!(
        AnchorConnection,
        r#"
        INSERT INTO stellar_anchor_connections
            (domain, display_name, supported_assets, sep24_enabled, sep31_enabled, signing_key, horizon_url)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id, domain, display_name, status, supported_assets,
            sep24_enabled, sep31_enabled, signing_key, jwt_token, jwt_expires_at,
            horizon_url, total_transfers, total_volume_usd, last_connected_at,
            created_at, updated_at
        "#,
        req.domain,
        req.display_name,
        &req.supported_assets,
        req.sep24_enabled,
        req.sep31_enabled,
        req.signing_key,
        req.horizon_url,
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn get_anchor_by_domain(pool: &PgPool, domain: &str) -> Result<Option<AnchorConnection>> {
    let row = sqlx::query_as!(
        AnchorConnection,
        r#"
        SELECT id, domain, display_name, status, supported_assets,
               sep24_enabled, sep31_enabled, signing_key, jwt_token, jwt_expires_at,
               horizon_url, total_transfers, total_volume_usd, last_connected_at,
               created_at, updated_at
        FROM stellar_anchor_connections
        WHERE domain = $1
        "#,
        domain
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn list_anchor_connections(pool: &PgPool) -> Result<Vec<AnchorConnection>> {
    let rows = sqlx::query_as!(
        AnchorConnection,
        r#"
        SELECT id, domain, display_name, status, supported_assets,
               sep24_enabled, sep31_enabled, signing_key, jwt_token, jwt_expires_at,
               horizon_url, total_transfers, total_volume_usd, last_connected_at,
               created_at, updated_at
        FROM stellar_anchor_connections
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn update_anchor_jwt(
    pool: &PgPool,
    anchor_id: Uuid,
    jwt_token: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE stellar_anchor_connections
        SET jwt_token = $1, jwt_expires_at = $2, last_connected_at = NOW()
        WHERE id = $3
        "#,
        jwt_token,
        expires_at,
        anchor_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn increment_anchor_stats(
    pool: &PgPool,
    anchor_id: Uuid,
    volume_usd: Decimal,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE stellar_anchor_connections
        SET total_transfers = total_transfers + 1,
            total_volume_usd = total_volume_usd + $1
        WHERE id = $2
        "#,
        volume_usd,
        anchor_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// DEX order book snapshots
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn upsert_order_book_snapshot(
    pool: &PgPool,
    base_asset: &str,
    counter_asset: &str,
    best_bid: Option<Decimal>,
    best_ask: Option<Decimal>,
    mid_price: Option<Decimal>,
    spread_pct: Option<Decimal>,
    bids: serde_json::Value,
    asks: serde_json::Value,
    depth_1pct_base: Decimal,
    depth_1pct_counter: Decimal,
) -> Result<DexOrderBookSnapshot> {
    let row = sqlx::query_as!(
        DexOrderBookSnapshot,
        r#"
        INSERT INTO dex_order_book_snapshots
            (base_asset, counter_asset, best_bid, best_ask, mid_price, spread_pct,
             bids, asks, depth_1pct_base, depth_1pct_counter,
             expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW() + INTERVAL '5 minutes')
        RETURNING
            id, base_asset, counter_asset, best_bid, best_ask, mid_price, spread_pct,
            bids, asks, depth_1pct_base, depth_1pct_counter, snapshotted_at, expires_at
        "#,
        base_asset,
        counter_asset,
        best_bid,
        best_ask,
        mid_price,
        spread_pct,
        bids,
        asks,
        depth_1pct_base,
        depth_1pct_counter,
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn get_latest_snapshot(
    pool: &PgPool,
    base_asset: &str,
    counter_asset: &str,
) -> Result<Option<DexOrderBookSnapshot>> {
    let row = sqlx::query_as!(
        DexOrderBookSnapshot,
        r#"
        SELECT id, base_asset, counter_asset, best_bid, best_ask, mid_price, spread_pct,
               bids, asks, depth_1pct_base, depth_1pct_counter, snapshotted_at, expires_at
        FROM dex_order_book_snapshots
        WHERE base_asset = $1 AND counter_asset = $2 AND expires_at > NOW()
        ORDER BY snapshotted_at DESC
        LIMIT 1
        "#,
        base_asset,
        counter_asset,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-anchor transfers
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn insert_cross_anchor_transfer(
    pool: &PgPool,
    req: &InitiateTransferRequest,
    receiving_anchor_id: Uuid,
) -> Result<CrossAnchorTransfer> {
    let row = sqlx::query_as!(
        CrossAnchorTransfer,
        r#"
        INSERT INTO cross_anchor_transfers
            (receiving_anchor_id, send_asset, receive_asset, send_amount, sender_account)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id, reference_id, receiving_anchor_id, sep31_transaction_id,
            compliance_tracking_id, status, send_asset, receive_asset,
            send_amount, receive_amount, execution_spread,
            stellar_tx_hash, stellar_tx_xdr, stellar_ledger,
            sender_account, receiver_account,
            error_code, error_message,
            submitted_at, completed_at, expires_at, created_at, updated_at
        "#,
        receiving_anchor_id,
        req.send_asset,
        req.receive_asset,
        req.send_amount,
        req.sender_account,
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn update_transfer_sep31_id(
    pool: &PgPool,
    transfer_id: Uuid,
    sep31_id: &str,
    compliance_id: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE cross_anchor_transfers
        SET sep31_transaction_id = $1,
            compliance_tracking_id = $2,
            status = 'pending_stellar'
        WHERE id = $3
        "#,
        sep31_id,
        compliance_id,
        transfer_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn update_transfer_submitted(
    pool: &PgPool,
    transfer_id: Uuid,
    tx_hash: &str,
    tx_xdr: &str,
    ledger: i64,
    spread: Decimal,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE cross_anchor_transfers
        SET stellar_tx_hash = $1,
            stellar_tx_xdr = $2,
            stellar_ledger = $3,
            execution_spread = $4,
            status = 'pending_receiver',
            submitted_at = NOW()
        WHERE id = $5
        "#,
        tx_hash,
        tx_xdr,
        ledger,
        spread,
        transfer_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn update_transfer_status(
    pool: &PgPool,
    transfer_id: Uuid,
    status: &str,
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE cross_anchor_transfers
        SET status = $1,
            error_code = $2,
            error_message = $3,
            completed_at = CASE WHEN $1 IN ('completed', 'refunded', 'error') THEN NOW() ELSE NULL END
        WHERE id = $4
        "#,
        status,
        error_code,
        error_message,
        transfer_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(feature = "database")]
#[instrument(skip(pool))]
pub async fn get_transfer_by_id(
    pool: &PgPool,
    transfer_id: Uuid,
) -> Result<Option<CrossAnchorTransfer>> {
    let row = sqlx::query_as!(
        CrossAnchorTransfer,
        r#"
        SELECT id, reference_id, receiving_anchor_id, sep31_transaction_id,
               compliance_tracking_id, status, send_asset, receive_asset,
               send_amount, receive_amount, execution_spread,
               stellar_tx_hash, stellar_tx_xdr, stellar_ledger,
               sender_account, receiver_account,
               error_code, error_message,
               submitted_at, completed_at, expires_at, created_at, updated_at
        FROM cross_anchor_transfers
        WHERE id = $1
        "#,
        transfer_id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
