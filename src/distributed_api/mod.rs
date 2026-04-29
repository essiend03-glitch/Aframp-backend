/// Global Distributed API Architecture
/// 
/// This module implements a globally distributed API model with:
/// - Edge-first caching via CDN
/// - Geographically distributed read replicas
/// - Consistency vs. latency trade-offs
/// - Health-aware load balancing

pub mod edge_cache;
pub mod regional_replica;
pub mod consistency;
pub mod health_check;
pub mod routing;
pub mod integration;

pub use edge_cache::EdgeCacheLayer;
pub use regional_replica::RegionalReplica;
pub use consistency::{ConsistencyLevel, ConsistencyManager};
pub use health_check::HealthChecker;
pub use routing::GeoRouter;
pub use integration::DistributedApiSystem;
