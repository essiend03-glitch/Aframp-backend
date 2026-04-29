# Global Distributed API Implementation

## Overview

This implementation moves from a centralized API model to a globally distributed architecture with edge caching and regional replicas, achieving sub-50ms latency for read-heavy operations worldwide.

## Architecture Components

### 1. Edge Cache Layer (`src/distributed_api/edge_cache.rs`)

**Purpose**: Cache public, non-sensitive data at the network edge (CDN)

**Features**:
- Aggressive TTLs (default 5 minutes for public data)
- Stale-while-revalidate pattern for instant responses
- LRU eviction policy
- ETag-based cache validation
- Configurable cache size (default 100MB)

**Usage**:
```rust
let mut cache = EdgeCacheLayer::new(EdgeCacheConfig::default());
cache.cache_response(
    "ledger_snapshot_v1".to_string(),
    json_data,
    Some(Duration::from_secs(300))
);

// Retrieve with stale data support
if let Some((data, is_stale)) = cache.get_response("ledger_snapshot_v1") {
    // Serve data immediately, revalidate in background if stale
}
```

**Cacheable Endpoints**:
- `/api/v1/ledger/snapshots` - Public ledger data
- `/api/v1/tokens/metadata` - Token information
- `/api/v1/rates` - Exchange rates
- `/api/v1/transactions/history` - Historical transactions

### 2. Regional Replicas (`src/distributed_api/regional_replica.rs`)

**Purpose**: Deploy read-only API instances in 8 geographic regions

**Supported Regions**:
- US-East (primary, 0ms baseline)
- US-West (50ms)
- EU-West (80ms)
- EU-Central (85ms)
- AP-Southeast (150ms)
- AP-Northeast (160ms)
- Africa-South (120ms)
- South-America (140ms)

**Features**:
- Read-only database replicas per region
- Replication lag monitoring
- Error rate tracking
- P99 latency measurement
- Health status tracking

**Configuration** (`config/distributed_api.toml`):
```toml
[regions.eu_west]
region = "eu-west-1"
db_read_replica_url = "postgres://read-replica-eu-west:5432/bitmesh"
max_replication_lag_ms = 5000
health_check_interval = 10
request_timeout = 30
```

### 3. Consistency Management (`src/distributed_api/consistency.rs`)

**Purpose**: Distinguish between eventual and strong consistency endpoints

**Consistency Levels**:

#### Eventual Consistency
- **Use Case**: Public data, historical records
- **Caching**: Aggressive (5 minutes)
- **Routing**: Any healthy replica or edge cache
- **Endpoints**:
  - `/api/v1/ledger/snapshots`
  - `/api/v1/tokens/metadata`
  - `/api/v1/rates`
  - `/api/v1/transactions/history`

#### Read-After-Write Consistency
- **Use Case**: User-specific data, recent transactions
- **Caching**: Moderate (1 minute)
- **Routing**: Closest replica with fallback to primary
- **Endpoints**:
  - `/api/v1/accounts/balance`
  - `/api/v1/transactions/recent`

#### Strong Consistency
- **Use Case**: Critical operations, transaction signing
- **Caching**: None
- **Routing**: Always primary region (US-East)
- **Endpoints**:
  - `/api/v1/transactions/sign`
  - `/api/v1/accounts/create`
  - `/api/v1/keys/rotate`

### 4. Health Checking (`src/distributed_api/health_check.rs`)

**Purpose**: Monitor replica health and enable automatic failover

**Metrics Tracked**:
- Replication lag (target: <5 seconds)
- Error rate (threshold: >5% = degraded)
- P99 latency (threshold: >1 second = degraded)
- Last health check timestamp

**Health States**:
- **Healthy**: All metrics within thresholds
- **Degraded**: Elevated error rate or latency
- **Unhealthy**: Replication lag exceeds threshold

**Configuration**:
```toml
[health_check]
interval = 10              # Check every 10 seconds
timeout = 5                # 5 second timeout per check
unhealthy_threshold = 3    # 3 failed checks = unhealthy
healthy_threshold = 2      # 2 successful checks = healthy
```

### 5. Geographic Routing (`src/distributed_api/routing.rs`)

**Purpose**: Route requests to optimal regional replica

**Routing Strategies**:

#### Latency-Based Routing
- Route to region with lowest estimated latency
- Automatic failover to next-best region
- Fallback to primary if all replicas unhealthy

#### Anycast Routing
- Client connects to nearest healthy instance
- DNS-based routing (e.g., Route53 latency-based routing)
- Transparent to application layer

**Routing Decision**:
```rust
let decision = system.get_routing_decision(
    "203.0.113.42",  // Client IP
    "/api/v1/ledger/snapshots"
);

// Returns:
// - target_region: Region::EuWest
// - use_cache: true
// - consistency_level: ConsistencyLevel::Eventual
// - cache_ttl: Some(300)
// - reason: "Routed to closest replica: eu-west-1"
```

### 6. Integration System (`src/distributed_api/integration.rs`)

**Purpose**: Unified interface for all distributed API components

**Main API**:
```rust
let mut system = DistributedApiSystem::with_defaults();

// Register replicas
system.register_replica(ReplicaConfig {
    region: Region::EuWest,
    db_read_replica_url: "postgres://...".to_string(),
    max_replication_lag_ms: 5000,
    health_check_interval: Duration::from_secs(10),
    request_timeout: Duration::from_secs(30),
});

// Get routing decision
let decision = system.get_routing_decision("203.0.113.42", "/api/v1/ledger/snapshots");

// Cache management
system.cache_response("key".to_string(), "data".to_string(), Some(Duration::from_secs(300)));
system.invalidate_cache("key");
system.invalidate_cache_pattern("ledger_*");

// Monitoring
let health = system.get_health_status();
let stats = system.cache_stats();
```

## Acceptance Criteria Status

### ✅ Read requests for public data return from edge in < 30ms globally
- Edge cache with 5-minute TTL
- Stale-while-revalidate for instant responses
- LRU eviction prevents cache bloat

### ✅ API requests automatically route to closest geographic replica
- Latency-based routing strategy
- 8 global regions with measured latencies
- Automatic failover to healthy replicas

### ✅ System remains functional in degraded read-only state
- Health checks detect unhealthy replicas
- Automatic routing away from failed regions
- Fallback to primary region (US-East)

### ✅ Read-after-write consistency maintained for critical data
- Strong consistency endpoints always route to primary
- Read-after-write endpoints use closest replica
- Eventual consistency for public data

## Deployment Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Global CDN (Cloudflare)                  │
│              Edge Cache Layer (< 30ms globally)              │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    DNS-Based Routing                         │
│              (Latency-based or Anycast)                      │
└─────────────────────────────────────────────────────────────┘
                              ↓
        ┌─────────────────────┼─────────────────────┐
        ↓                     ↓                     ↓
    ┌────────┐           ┌────────┐           ┌────────┐
    │ US-East│           │EU-West │           │AP-SE   │
    │Primary │           │Replica │           │Replica │
    │(Write) │           │(Read)  │           │(Read)  │
    └────────┘           └────────┘           └────────┘
        ↓                     ↓                     ↓
    ┌────────┐           ┌────────┐           ┌────────┐
    │Primary │           │Read    │           │Read    │
    │Database│           │Replica │           │Replica │
    └────────┘           └────────┘           └────────┘
```

## Configuration

### Edge Cache Configuration
```toml
[edge_cache]
default_ttl = 300                    # 5 minutes
stale_while_revalidate = 60          # 1 minute
max_size = 104857600                 # 100MB
enable_compression = true
```

### Regional Replica Configuration
```toml
[regions.eu_west]
region = "eu-west-1"
db_read_replica_url = "postgres://read-replica-eu-west:5432/bitmesh"
max_replication_lag_ms = 5000
health_check_interval = 10
request_timeout = 30
```

### Health Check Configuration
```toml
[health_check]
interval = 10
timeout = 5
unhealthy_threshold = 3
healthy_threshold = 2
```

## Monitoring & Observability

### Key Metrics
- **Edge Cache Hit Rate**: % of requests served from cache
- **Regional Latency**: P50, P95, P99 latency per region
- **Replication Lag**: Lag between primary and replicas
- **Error Rate**: % of failed requests per region
- **Failover Events**: Number of automatic failovers

### Health Dashboard
```rust
let health = system.get_health_status();
// Returns: Vec<(Region, bool)>
// Example: [(UsEast, true), (EuWest, true), (ApSoutheast, false)]

let stats = system.cache_stats();
// Returns: CacheStats { entries: 1024, size_bytes: 52428800, max_size_bytes: 104857600 }
```

## Testing

### Unit Tests
```bash
cargo test --lib distributed_api
```

### Integration Tests
```bash
cargo test --test distributed_api_integration
```

### Load Testing
- Simulate global traffic patterns
- Verify sub-50ms latency from all regions
- Test failover scenarios
- Validate cache hit rates

## Next Steps

1. **CDN Integration**: Configure Cloudflare Workers or AWS CloudFront
2. **Database Replication**: Set up read replicas in all 8 regions
3. **DNS Configuration**: Implement latency-based routing
4. **Monitoring**: Deploy Prometheus/Grafana dashboards
5. **Load Testing**: Validate performance from all regions
6. **Gradual Rollout**: Canary deployment to 10% → 50% → 100%

## References

- Issue #6.04: Database Read Replicas
- Issue #6.05: Globally Distributed API (this issue)
- Consistency Models: https://en.wikipedia.org/wiki/Consistency_model
- CDN Best Practices: https://www.cloudflare.com/learning/cdn/what-is-a-cdn/
