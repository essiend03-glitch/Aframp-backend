use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Raw price tick from an oracle provider node.
#[derive(Debug, Clone)]
pub struct OracleTick {
    pub node_id:   Uuid,
    pub pair:      String,
    /// Price with 18 decimal precision (stored as f64 for calc, persisted as NUMERIC(40,18)).
    pub price:     f64,
    pub weight:    u32,
    pub tick_at:   DateTime<Utc>,
}

/// Verified BFT price output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BftPrice {
    pub pair:         String,
    pub price:        f64,
    pub sources_used: usize,
    pub quorum_met:   bool,
    pub computed_at:  DateTime<Utc>,
}
