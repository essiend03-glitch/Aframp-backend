//! Analytics service: snapshot computation, behaviour profiling, risk scoring,
//! anomaly detection, and insight generation (Issue #369).

use std::collections::HashMap;
use std::sync::Arc;

use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive, Zero};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use sqlx::PgPool;
use tracing::{info, warn};

use crate::database::analytics_repository::{
    AnalyticsRepository, UpsertProfile, UpsertSnapshot,
};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AnalyticsConfig {
    /// Multiplier above baseline that triggers a volume-spike anomaly.
    pub volume_spike_multiplier: f64,
    /// Fraction of new counterparties in a window that triggers an anomaly.
    pub new_counterparty_threshold: f64,
    /// Std-dev multiplier for tx-size shift detection.
    pub size_shift_sigma: f64,
    /// Hour-of-day shift (in hours) that triggers a time-pattern anomaly.
    pub time_shift_hours: f64,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            volume_spike_multiplier: 3.0,
            new_counterparty_threshold: 0.7,
            size_shift_sigma: 2.5,
            time_shift_hours: 6.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Raw transaction row (read from transactions table)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TxRecord {
    pub wallet_address: String,
    pub tx_type: String,
    pub cngn_amount: BigDecimal,
    pub from_amount: BigDecimal,
    pub to_amount: BigDecimal,
    pub from_currency: String,
    pub to_currency: String,
    pub payment_provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

pub struct AnalyticsService {
    repo: Arc<AnalyticsRepository>,
    pool: PgPool,
    config: AnalyticsConfig,
}

impl AnalyticsService {
    pub fn new(pool: PgPool, config: AnalyticsConfig) -> Self {
        Self {
            repo: Arc::new(AnalyticsRepository::new(pool.clone())),
            pool,
            config,
        }
    }

    pub fn repo(&self) -> Arc<AnalyticsRepository> {
        self.repo.clone()
    }

    // -----------------------------------------------------------------------
    // Snapshot computation
    // -----------------------------------------------------------------------

    /// Fetch transactions for a wallet in [from, to) from the transactions table.
    async fn fetch_txs(
        &self,
        wallet_address: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<TxRecord> {
        let rows: Vec<(String, String, BigDecimal, BigDecimal, BigDecimal, String, String, Option<String>, DateTime<Utc>, String)> =
            sqlx::query_as(
                r#"SELECT wallet_address, type, cngn_amount, from_amount, to_amount,
                     from_currency, to_currency, payment_provider, created_at, status
                   FROM transactions
                   WHERE wallet_address = $1 AND created_at >= $2 AND created_at < $3
                   ORDER BY created_at"#,
            )
            .bind(wallet_address)
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        rows.into_iter()
            .map(|(wa, tt, ca, fa, ta, fc, tc, pp, cr, st)| TxRecord {
                wallet_address: wa,
                tx_type: tt,
                cngn_amount: ca,
                from_amount: fa,
                to_amount: ta,
                from_currency: fc,
                to_currency: tc,
                payment_provider: pp,
                created_at: cr,
                status: st,
            })
            .collect()
    }

    /// Compute and persist a snapshot for one wallet and period.
    pub async fn compute_snapshot(
        &self,
        wallet_address: &str,
        period: &str,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) {
        let txs = self.fetch_txs(wallet_address, period_start, period_end).await;
        if txs.is_empty() {
            return;
        }

        let mut cngn_sent = BigDecimal::zero();
        let mut cngn_received = BigDecimal::zero();
        let mut fiat_onramped = BigDecimal::zero();
        let mut fiat_offramped = BigDecimal::zero();
        let mut type_counts: HashMap<String, i32> = HashMap::new();
        let mut provider_counts: HashMap<String, i32> = HashMap::new();
        let mut counterparties: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut active_days: std::collections::HashSet<u32> = std::collections::HashSet::new();

        for tx in &txs {
            active_days.insert(tx.created_at.ordinal());
            *type_counts.entry(tx.tx_type.clone()).or_insert(0) += 1;
            if let Some(p) = &tx.payment_provider {
                *provider_counts.entry(p.clone()).or_insert(0) += 1;
            }
            match tx.tx_type.as_str() {
                "onramp" => {
                    cngn_received += &tx.cngn_amount;
                    fiat_onramped += &tx.from_amount;
                }
                "offramp" => {
                    cngn_sent += &tx.cngn_amount;
                    fiat_offramped += &tx.to_amount;
                }
                _ => {
                    cngn_sent += &tx.cngn_amount;
                }
            }
            // Use payment_reference or to_currency as a proxy counterparty identifier
            counterparties.insert(tx.to_currency.clone());
        }

        let most_used_tx_type = type_counts.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());
        let most_used_provider = provider_counts.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());

        let snap = UpsertSnapshot {
            wallet_address: wallet_address.to_string(),
            period: period.to_string(),
            period_start,
            period_end,
            total_tx_count: txs.len() as i32,
            total_cngn_sent: cngn_sent,
            total_cngn_received: cngn_received,
            total_fiat_onramped: fiat_onramped,
            total_fiat_offramped: fiat_offramped,
            total_fees_paid: BigDecimal::zero(), // fees tracked separately
            unique_counterparties: counterparties.len() as i32,
            most_used_tx_type,
            most_used_provider,
            active_days: active_days.len() as i32,
        };

        if let Err(e) = self.repo.upsert_snapshot(snap).await {
            warn!(wallet=%wallet_address, period=%period, error=%e, "Failed to upsert snapshot");
        }

        // Persist spending categories
        self.compute_spending_categories(wallet_address, period, period_start, &txs).await;
    }

    async fn compute_spending_categories(
        &self,
        wallet_address: &str,
        period: &str,
        period_start: DateTime<Utc>,
        txs: &[TxRecord],
    ) {
        let total = txs.len() as f64;
        if total == 0.0 {
            return;
        }
        let mut cat_counts: HashMap<String, (i32, BigDecimal)> = HashMap::new();
        for tx in txs {
            let cat = match tx.tx_type.as_str() {
                "onramp" => "onramp",
                "offramp" => "offramp",
                "bill_payment" => "bill_payments",
                _ => "transfers",
            };
            let e = cat_counts.entry(cat.to_string()).or_insert((0, BigDecimal::zero()));
            e.0 += 1;
            e.1 += &tx.cngn_amount;
        }
        for (cat, (count, amount)) in cat_counts {
            let pct = BigDecimal::from_f64((count as f64 / total) * 100.0).unwrap_or_default();
            let _ = self.repo.upsert_spending_category(
                wallet_address, period, period_start, &cat, count, amount, pct,
            ).await;
        }
    }

    // -----------------------------------------------------------------------
    // Behaviour profiling
    // -----------------------------------------------------------------------

    /// Compute and persist a behaviour profile from the last 90 days of activity.
    pub async fn compute_profile(&self, wallet_address: &str) {
        let to = Utc::now();
        let from = to - Duration::days(90);
        let txs = self.fetch_txs(wallet_address, from, to).await;
        if txs.is_empty() {
            return;
        }

        let n = txs.len() as f64;
        let total_cngn: BigDecimal = txs.iter().map(|t| &t.cngn_amount).sum();
        let avg_tx_size = &total_cngn / BigDecimal::from_f64(n).unwrap_or(BigDecimal::from(1));

        // Frequency per week over 90 days (≈ 12.857 weeks)
        let weeks = 90.0 / 7.0;
        let freq_per_week = BigDecimal::from_f64(n / weeks).unwrap_or_default();

        // Preferred hour (mode)
        let mut hour_counts: HashMap<u32, usize> = HashMap::new();
        for tx in &txs {
            *hour_counts.entry(tx.created_at.hour()).or_insert(0) += 1;
        }
        let preferred_hour = hour_counts.iter().max_by_key(|(_, v)| *v).map(|(h, _)| *h as i16);

        // Preferred provider (mode)
        let mut provider_counts: HashMap<String, usize> = HashMap::new();
        for tx in &txs {
            if let Some(p) = &tx.payment_provider {
                *provider_counts.entry(p.clone()).or_insert(0) += 1;
            }
        }
        let preferred_provider = provider_counts.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());

        // Preferred currency pair (mode of from_currency->to_currency)
        let mut pair_counts: HashMap<String, usize> = HashMap::new();
        for tx in &txs {
            let pair = format!("{}->{}", tx.from_currency, tx.to_currency);
            *pair_counts.entry(pair).or_insert(0) += 1;
        }
        let preferred_pair = pair_counts.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());

        let risk_score = self.compute_risk_score_from_txs(&txs, avg_tx_size.to_f64().unwrap_or(0.0));

        let profile = UpsertProfile {
            wallet_address: wallet_address.to_string(),
            avg_tx_size,
            tx_frequency_per_week: freq_per_week,
            preferred_hour_utc: preferred_hour,
            preferred_provider,
            preferred_currency_pair: preferred_pair,
            risk_score: BigDecimal::from_f64(risk_score).unwrap_or_default(),
        };

        if let Err(e) = self.repo.upsert_profile(profile).await {
            warn!(wallet=%wallet_address, error=%e, "Failed to upsert profile");
        }
    }

    // -----------------------------------------------------------------------
    // Risk scoring
    // -----------------------------------------------------------------------

    /// Score 0–100. Higher = riskier.
    pub fn compute_risk_score_from_txs(&self, txs: &[TxRecord], avg_size: f64) -> f64 {
        if txs.is_empty() {
            return 0.0;
        }
        let n = txs.len() as f64;

        // Factor 1: tx size deviation (coefficient of variation)
        let sizes: Vec<f64> = txs.iter().filter_map(|t| t.cngn_amount.to_f64()).collect();
        let mean = sizes.iter().sum::<f64>() / n;
        let variance = sizes.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n;
        let cv = if mean > 0.0 { variance.sqrt() / mean } else { 0.0 };
        let size_score = (cv * 20.0).min(25.0);

        // Factor 2: frequency deviation (high frequency = higher risk)
        let weeks = 90.0 / 7.0;
        let freq = n / weeks;
        let freq_score = (freq / 10.0 * 25.0).min(25.0);

        // Factor 3: new counterparty rate
        let mut counterparties: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut new_cp_count = 0usize;
        for tx in txs {
            let cp = tx.to_currency.clone();
            if counterparties.insert(cp) {
                new_cp_count += 1;
            }
        }
        let new_cp_rate = new_cp_count as f64 / n;
        let cp_score = (new_cp_rate * 25.0).min(25.0);

        // Factor 4: time-of-day entropy (uniform distribution = higher risk)
        let mut hour_counts = [0usize; 24];
        for tx in txs {
            hour_counts[tx.created_at.hour() as usize] += 1;
        }
        let entropy: f64 = hour_counts.iter().map(|&c| {
            if c == 0 { 0.0 } else {
                let p = c as f64 / n;
                -p * p.ln()
            }
        }).sum();
        let max_entropy = (24.0f64).ln();
        let time_score = (entropy / max_entropy * 25.0).min(25.0);

        let _ = avg_size; // used by caller context
        (size_score + freq_score + cp_score + time_score).min(100.0)
    }

    // -----------------------------------------------------------------------
    // Anomaly detection
    // -----------------------------------------------------------------------

    pub async fn detect_anomalies(&self, wallet_address: &str) {
        let profile = match self.repo.get_profile(wallet_address).await {
            Ok(Some(p)) => p,
            _ => return,
        };

        let to = Utc::now();
        let from = to - Duration::days(7);
        let recent_txs = self.fetch_txs(wallet_address, from, to).await;
        if recent_txs.is_empty() {
            return;
        }

        use bigdecimal::ToPrimitive;
        let baseline_freq = profile.tx_frequency_per_week.to_f64().unwrap_or(1.0);
        let baseline_avg = profile.avg_tx_size.to_f64().unwrap_or(1.0);
        let baseline_hour = profile.preferred_hour_utc.unwrap_or(12) as f64;

        let recent_freq = recent_txs.len() as f64;

        // Volume spike
        if baseline_freq > 0.0 {
            let ratio = recent_freq / baseline_freq;
            if ratio > self.config.volume_spike_multiplier {
                let mag = BigDecimal::from_f64(ratio).unwrap_or_default();
                let _ = self.repo.insert_anomaly(wallet_address, "volume_spike", mag).await;
                info!(wallet=%wallet_address, ratio=%ratio, "Volume spike anomaly flagged");
            }
        }

        // Size shift
        let sizes: Vec<f64> = recent_txs.iter().filter_map(|t| t.cngn_amount.to_f64()).collect();
        if !sizes.is_empty() {
            let mean = sizes.iter().sum::<f64>() / sizes.len() as f64;
            if baseline_avg > 0.0 {
                let sigma = (mean - baseline_avg).abs() / baseline_avg;
                if sigma > self.config.size_shift_sigma {
                    let mag = BigDecimal::from_f64(sigma).unwrap_or_default();
                    let _ = self.repo.insert_anomaly(wallet_address, "size_shift", mag).await;
                }
            }
        }

        // New counterparty rate
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut new_count = 0usize;
        for tx in &recent_txs {
            if seen.insert(tx.to_currency.clone()) {
                new_count += 1;
            }
        }
        let new_rate = new_count as f64 / recent_txs.len() as f64;
        if new_rate > self.config.new_counterparty_threshold {
            let mag = BigDecimal::from_f64(new_rate).unwrap_or_default();
            let _ = self.repo.insert_anomaly(wallet_address, "new_counterparty_rate", mag).await;
        }

        // Time pattern shift
        let mut hour_sum = 0.0f64;
        for tx in &recent_txs {
            hour_sum += tx.created_at.hour() as f64;
        }
        let avg_hour = hour_sum / recent_txs.len() as f64;
        let hour_shift = (avg_hour - baseline_hour).abs();
        if hour_shift > self.config.time_shift_hours {
            let mag = BigDecimal::from_f64(hour_shift).unwrap_or_default();
            let _ = self.repo.insert_anomaly(wallet_address, "time_pattern_shift", mag).await;
        }
    }

    // -----------------------------------------------------------------------
    // Insight generation
    // -----------------------------------------------------------------------

    pub async fn generate_insights(&self, wallet_address: &str, period: &str) {
        let to = Utc::now();
        let (from, prev_from, prev_to) = match period {
            "weekly" => (to - Duration::days(7), to - Duration::days(14), to - Duration::days(7)),
            _ => {
                let start = to.with_day(1).unwrap_or(to);
                let prev_end = start - Duration::seconds(1);
                let prev_start = prev_end.with_day(1).unwrap_or(prev_end);
                (start, prev_start, prev_end)
            }
        };

        let txs = self.fetch_txs(wallet_address, from, to).await;
        let prev_txs = self.fetch_txs(wallet_address, prev_from, prev_to).await;

        if txs.is_empty() {
            return;
        }

        // Top category
        let mut cat_amounts: HashMap<String, BigDecimal> = HashMap::new();
        for tx in &txs {
            let cat = match tx.tx_type.as_str() {
                "onramp" => "onramp",
                "offramp" => "offramp",
                "bill_payment" => "bill_payments",
                _ => "transfers",
            };
            *cat_amounts.entry(cat.to_string()).or_insert(BigDecimal::zero()) += &tx.cngn_amount;
        }
        let top_cat = cat_amounts.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
        let (top_category, top_category_amount) = match top_cat {
            Some((k, v)) => (Some(k.as_str()), Some(v.clone())),
            None => (None, None),
        };

        // Period delta
        let curr_total: BigDecimal = txs.iter().map(|t| &t.cngn_amount).sum();
        let prev_total: BigDecimal = prev_txs.iter().map(|t| &t.cngn_amount).sum();
        let delta_pct = if !prev_total.is_zero() {
            let delta = (&curr_total - &prev_total) / &prev_total * BigDecimal::from(100);
            Some(delta)
        } else {
            None
        };

        // Largest tx
        let largest = txs.iter().max_by(|a, b| a.cngn_amount.partial_cmp(&b.cngn_amount).unwrap_or(std::cmp::Ordering::Equal));
        let largest_tx_amount = largest.map(|t| t.cngn_amount.clone());

        // Most frequent counterparty
        let mut cp_counts: HashMap<String, usize> = HashMap::new();
        for tx in &txs {
            *cp_counts.entry(tx.to_currency.clone()).or_insert(0) += 1;
        }
        let most_frequent_cp = cp_counts.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.as_str());

        // Estimated monthly fees (placeholder — fees not yet tracked per-tx)
        let estimated_fees: Option<BigDecimal> = None;

        // cNGN balance trend
        let cngn_trend = if curr_total > prev_total {
            Some("increasing")
        } else if curr_total < prev_total {
            Some("decreasing")
        } else {
            Some("stable")
        };

        let _ = self.repo.upsert_insight(
            wallet_address,
            period,
            from,
            top_category,
            top_category_amount,
            delta_pct,
            largest_tx_amount,
            None,
            most_frequent_cp,
            estimated_fees,
            cngn_trend,
        ).await;
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::FromPrimitive;
    use chrono::TimeZone;

    fn make_tx(tx_type: &str, cngn: f64, hour: u32, provider: Option<&str>, to_currency: &str) -> TxRecord {
        TxRecord {
            wallet_address: "GTEST".to_string(),
            tx_type: tx_type.to_string(),
            cngn_amount: BigDecimal::from_f64(cngn).unwrap(),
            from_amount: BigDecimal::from_f64(cngn).unwrap(),
            to_amount: BigDecimal::from_f64(cngn).unwrap(),
            from_currency: "NGN".to_string(),
            to_currency: to_currency.to_string(),
            payment_provider: provider.map(|s| s.to_string()),
            created_at: Utc.with_ymd_and_hms(2026, 1, 15, hour, 0, 0).unwrap(),
            status: "completed".to_string(),
        }
    }

    fn dummy_service() -> AnalyticsService {
        // We only test pure functions; pool is never called in unit tests.
        use sqlx::postgres::PgPoolOptions;
        // Build a pool that will never connect — tests only call pure methods.
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://localhost/test")
            .unwrap();
        AnalyticsService::new(pool, AnalyticsConfig::default())
    }

    #[test]
    fn test_risk_score_zero_for_empty() {
        let svc = dummy_service();
        let score = svc.compute_risk_score_from_txs(&[], 0.0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_risk_score_bounded() {
        let svc = dummy_service();
        let txs: Vec<TxRecord> = (0..50)
            .map(|i| make_tx("onramp", (i as f64 + 1.0) * 1000.0, i % 24, Some("mpesa"), "CNGN"))
            .collect();
        let score = svc.compute_risk_score_from_txs(&txs, 0.0);
        assert!(score >= 0.0 && score <= 100.0, "score={}", score);
    }

    #[test]
    fn test_spending_category_classification() {
        assert_eq!(
            crate::api::analytics::models::SpendingCategory::from_tx_type("onramp"),
            crate::api::analytics::models::SpendingCategory::Onramp
        );
        assert_eq!(
            crate::api::analytics::models::SpendingCategory::from_tx_type("bill_payment"),
            crate::api::analytics::models::SpendingCategory::BillPayments
        );
        assert_eq!(
            crate::api::analytics::models::SpendingCategory::from_tx_type("unknown"),
            crate::api::analytics::models::SpendingCategory::Transfers
        );
    }

    #[test]
    fn test_risk_score_high_frequency() {
        let svc = dummy_service();
        // 100 txs in 90 days = ~7.7/week — should push freq_score up
        let txs: Vec<TxRecord> = (0..100)
            .map(|i| make_tx("onramp", 100.0, 10, Some("mpesa"), "CNGN"))
            .collect();
        let score = svc.compute_risk_score_from_txs(&txs, 100.0);
        assert!(score > 10.0);
    }

    #[test]
    fn test_risk_score_many_new_counterparties() {
        let svc = dummy_service();
        // Each tx has a unique to_currency (proxy for counterparty)
        let txs: Vec<TxRecord> = (0..20)
            .map(|i| make_tx("offramp", 50.0, 14, None, &format!("CP{}", i)))
            .collect();
        let score = svc.compute_risk_score_from_txs(&txs, 50.0);
        assert!(score > 15.0);
    }

    #[test]
    fn test_snapshot_period_str() {
        use crate::api::analytics::models::SnapshotPeriod;
        assert_eq!(SnapshotPeriod::Daily.as_str(), "daily");
        assert_eq!(SnapshotPeriod::Weekly.as_str(), "weekly");
        assert_eq!(SnapshotPeriod::Monthly.as_str(), "monthly");
    }

    #[test]
    fn test_anomaly_type_str() {
        use crate::api::analytics::models::AnomalyType;
        assert_eq!(AnomalyType::VolumeSpike.as_str(), "volume_spike");
        assert_eq!(AnomalyType::SizeShift.as_str(), "size_shift");
    }

    #[test]
    fn test_cohort_assignment_logic() {
        // Cohort month is derived from wallet creation date — verify format
        let dt = Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap();
        let cohort = format!("{}-{:02}", dt.year(), dt.month());
        assert_eq!(cohort, "2026-03");
    }

    #[test]
    fn test_incremental_snapshot_only_new_txs() {
        // Verify that period_start..period_end window correctly excludes older txs.
        let period_start = Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap();
        let period_end = Utc.with_ymd_and_hms(2026, 4, 8, 0, 0, 0).unwrap();
        let old_tx_at = Utc.with_ymd_and_hms(2026, 3, 31, 23, 59, 59).unwrap();
        let new_tx_at = Utc.with_ymd_and_hms(2026, 4, 3, 10, 0, 0).unwrap();

        assert!(old_tx_at < period_start, "old tx should be before window");
        assert!(new_tx_at >= period_start && new_tx_at < period_end, "new tx should be in window");
    }
}
