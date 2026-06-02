#!/bin/bash
# Quick start guide for Stellar submission engine

# 1. Run database migrations
echo "Running migrations..."
sqlx migrate run --database-url $DATABASE_URL

# 2. Set up environment variables
export STELLAR_NETWORK=testnet
export STELLAR_HORIZON_URL=https://horizon-testnet.stellar.org
export STELLAR_REQUEST_TIMEOUT=15
export STELLAR_MAX_RETRIES=3

# 3. Initialize channel accounts (5 channels for 50+ TPS)
# Run initialization script or use admin CLI
echo "Initialize stellar submission channels in database..."

# 4. Start the application
echo "Starting Aframp backend with Stellar submission engine..."
cargo run --release --features database

# 5. Monitor metrics
echo "Prometheus metrics available at http://localhost:9090"
echo "Key queries:"
echo "  stellar_tx_throughput_tps"
echo "  stellar_channel_pool_utilization_percent"
echo "  rate(stellar_tx_confirmed_total[1m])"
echo "  stellar_confirmation_delay_seconds"

# 6. Check admin endpoints
echo ""
echo "Admin endpoints:"
echo "  GET http://localhost:3000/api/v1/admin/infra/stellar/channels"
echo "  POST http://localhost:3000/api/v1/admin/infra/stellar/channels/0/top-up"
echo "    -H 'Content-Type: application/json'"
echo "    -d '{\"amount_xlm\": 500.0, \"description\": \"Balance replenishment\"}'"
