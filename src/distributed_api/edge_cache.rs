use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use std::collections::HashMap;

/// Edge cache configuration with aggressive TTLs and stale-while-revalidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCacheConfig {
    /// Default TTL for public data (seconds)
    pub default_ttl: u64,
    /// Stale-while-revalidate window (seconds)
    pub stale_while_revalidate: u64,
    /// Maximum cache size in bytes
    pub max_size: usize,
    /// Enable compression for cached responses
    pub enable_compression: bool,
}

impl Default for EdgeCacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: 300,           // 5 minutes
            stale_while_revalidate: 60, // 1 minute
            max_size: 1024 * 1024 * 100, // 100MB
            enable_compression: true,
        }
    }
}

/// Cached response with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub data: String,
    pub created_at: SystemTime,
    pub ttl: Duration,
    pub etag: String,
    pub is_stale: bool,
}

impl CachedResponse {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().unwrap_or(Duration::from_secs(u64::MAX)) > self.ttl
    }

    pub fn is_stale_revalidatable(&self, stale_window: Duration) -> bool {
        let elapsed = self.created_at.elapsed().unwrap_or(Duration::from_secs(u64::MAX));
        elapsed > self.ttl && elapsed <= (self.ttl + stale_window)
    }
}

/// Edge cache layer for public API responses
pub struct EdgeCacheLayer {
    config: EdgeCacheConfig,
    cache: HashMap<String, CachedResponse>,
    current_size: usize,
}

impl EdgeCacheLayer {
    pub fn new(config: EdgeCacheConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
            current_size: 0,
        }
    }

    /// Cache a public API response with TTL
    pub fn cache_response(&mut self, key: String, data: String, ttl: Option<Duration>) {
        let ttl = ttl.unwrap_or(Duration::from_secs(self.config.default_ttl));
        let size = data.len();

        // Evict if necessary
        if self.current_size + size > self.config.max_size {
            self.evict_lru();
        }

        let etag = format!("{:x}", md5::compute(data.as_bytes()));
        let response = CachedResponse {
            data,
            created_at: SystemTime::now(),
            ttl,
            etag,
            is_stale: false,
        };

        self.current_size += size;
        self.cache.insert(key, response);
    }

    /// Get cached response, returning stale data if available
    pub fn get_response(&self, key: &str) -> Option<(CachedResponse, bool)> {
        self.cache.get(key).map(|resp| {
            let is_expired = resp.is_expired();
            let is_stale_revalidatable = resp.is_stale_revalidatable(
                Duration::from_secs(self.config.stale_while_revalidate)
            );

            if is_expired && !is_stale_revalidatable {
                None
            } else {
                Some((resp.clone(), is_expired))
            }
        }).flatten()
    }

    /// Invalidate cache entry
    pub fn invalidate(&mut self, key: &str) {
        if let Some(resp) = self.cache.remove(key) {
            self.current_size = self.current_size.saturating_sub(resp.data.len());
        }
    }

    /// Invalidate all cache entries matching pattern
    pub fn invalidate_pattern(&mut self, pattern: &str) {
        let keys: Vec<_> = self.cache.keys()
            .filter(|k| k.contains(pattern))
            .cloned()
            .collect();
        
        for key in keys {
            self.invalidate(&key);
        }
    }

    fn evict_lru(&mut self) {
        if let Some((key, _)) = self.cache.iter()
            .min_by_key(|(_, resp)| resp.created_at) {
            let key = key.clone();
            self.invalidate(&key);
        }
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            size_bytes: self.current_size,
            max_size_bytes: self.config.max_size,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
    pub entries: usize,
    pub size_bytes: usize,
    pub max_size_bytes: usize,
}
