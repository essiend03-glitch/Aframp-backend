//! Background worker: expires stale mint authorization requests.

use crate::mint_authorization::service::MintAuthService;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Spawn the expiry worker. Runs every `interval_secs` seconds.
pub fn spawn_expiry_worker(service: Arc<MintAuthService>, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            match service.expire_stale_requests().await {
                Ok(0) => {}
                Ok(n) => info!(expired = n, "Mint authorization expiry worker: expired {n} requests"),
                Err(e) => error!(error = %e, "Mint authorization expiry worker error"),
            }
        }
    });
}
