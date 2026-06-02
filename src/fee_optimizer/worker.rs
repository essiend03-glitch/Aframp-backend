//! #490 Gas & Fee Optimization — background telemetry worker.
//!
//! Polls fee telemetry every 2 s for fast chains (Stellar, Solana) and
//! every block (~12 s) for EVM chains. Implements multi-provider fallback.

use super::engine::FeeOptimizerEngine;
use super::models::ChainNetwork;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

const FAST_CHAIN_INTERVAL_SECS: u64 = 2;
const EVM_CHAIN_INTERVAL_SECS: u64 = 12;

/// RPC provider configuration (primary + fallbacks).
struct RpcProvider {
    name: &'static str,
    endpoint: &'static str,
}

const STELLAR_PROVIDERS: &[RpcProvider] = &[
    RpcProvider { name: "horizon-primary", endpoint: "https://horizon.stellar.org" },
    RpcProvider { name: "horizon-fallback", endpoint: "https://horizon-testnet.stellar.org" },
];

const ETH_PROVIDERS: &[RpcProvider] = &[
    RpcProvider { name: "infura-primary", endpoint: "https://mainnet.infura.io/v3/rpc" },
    RpcProvider { name: "alchemy-fallback", endpoint: "https://eth-mainnet.g.alchemy.com/v2/rpc" },
];

const SOLANA_PROVIDERS: &[RpcProvider] = &[
    RpcProvider { name: "solana-primary", endpoint: "https://api.mainnet-beta.solana.com" },
    RpcProvider { name: "solana-fallback", endpoint: "https://solana-api.projectserum.com" },
];

pub struct FeePollerWorker {
    engine: Arc<FeeOptimizerEngine>,
}

impl FeePollerWorker {
    pub fn new(engine: Arc<FeeOptimizerEngine>) -> Self {
        Self { engine }
    }

    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        info!("Fee poller worker started");

        let mut fast_ticker = interval(Duration::from_secs(FAST_CHAIN_INTERVAL_SECS));
        let mut evm_ticker = interval(Duration::from_secs(EVM_CHAIN_INTERVAL_SECS));

        loop {
            tokio::select! {
                _ = fast_ticker.tick() => {
                    // Stellar + Solana
                    for (network, providers) in [
                        (ChainNetwork::Stellar, STELLAR_PROVIDERS),
                        (ChainNetwork::Solana, SOLANA_PROVIDERS),
                    ] {
                        if let Err(e) = self.poll_with_fallback(network, providers).await {
                            error!(error = %e, "Fast chain fee poll failed");
                        }
                    }
                    // Escalate stalled transactions
                    if let Err(e) = self.engine.escalate_stalled_transactions().await {
                        error!(error = %e, "Transaction escalation failed");
                    }
                }
                _ = evm_ticker.tick() => {
                    // Ethereum + L2s
                    for (network, providers) in [
                        (ChainNetwork::Ethereum, ETH_PROVIDERS),
                        (ChainNetwork::Polygon, ETH_PROVIDERS),
                        (ChainNetwork::Arbitrum, ETH_PROVIDERS),
                    ] {
                        if let Err(e) = self.poll_with_fallback(network, providers).await {
                            error!(error = %e, "EVM fee poll failed");
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Fee poller worker shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Try each provider in order; return on first success.
    async fn poll_with_fallback(
        &self,
        network: ChainNetwork,
        providers: &[RpcProvider],
    ) -> Result<()> {
        for provider in providers {
            match self.fetch_fee(network.clone(), provider).await {
                Ok((base, priority, block)) => {
                    self.engine
                        .ingest_fee_snapshot(network, base, priority, provider.name, block)
                        .await?;
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        provider = provider.name,
                        error = %e,
                        "RPC provider returned stale/delayed fee — trying fallback"
                    );
                }
            }
        }
        Err(anyhow::anyhow!("all_rpc_providers_failed"))
    }

    /// Simulate fetching fee data from an RPC endpoint.
    /// Production: reqwest GET to provider endpoint, parse JSON response.
    async fn fetch_fee(
        &self,
        network: ChainNetwork,
        provider: &RpcProvider,
    ) -> Result<(u128, u128, Option<i64>)> {
        // Simulated fee values per network (production: real RPC calls)
        let (base, priority) = match network {
            ChainNetwork::Stellar  => (100_u128, 0_u128),
            ChainNetwork::Ethereum => (20_000_000_000, 1_500_000_000),
            ChainNetwork::Solana   => (5_000, 0),
            ChainNetwork::Polygon  => (30_000_000_000, 1_000_000_000),
            ChainNetwork::Arbitrum => (100_000_000, 100_000_000),
        };
        Ok((base, priority, Some(1_000_000)))
    }
}
