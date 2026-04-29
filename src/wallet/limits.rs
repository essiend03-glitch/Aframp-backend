use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const USER_LIMIT_COOLING_OFF_HOURS: i64 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitSource {
    PlatformKycTier,
    UserDefined,
    RiskHold,
    AdminOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitScope {
    PerTransaction,
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitStatus {
    Active,
    PendingCoolingOff,
    Suspended,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideStatus {
    Pending,
    Approved,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionDirection {
    Credit,
    Debit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionControlType {
    TransactionTypeBlock,
    RecipientBlock,
    GeographyRestriction,
    TimeOfDayRestriction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionControlAction {
    Block,
    RequireApproval,
    NotifyOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendingLimit {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub source: LimitSource,
    pub scope: LimitScope,
    pub amount_minor: i64,
    pub currency: String,
    pub status: LimitStatus,
    pub starts_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub cooling_off_until: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitUsageSnapshot {
    pub limit_id: Uuid,
    pub wallet_id: Uuid,
    pub scope: LimitScope,
    pub currency: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub used_minor: i64,
    pub reserved_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitOverride {
    pub id: Uuid,
    pub limit_id: Uuid,
    pub wallet_id: Uuid,
    pub approved_amount_minor: i64,
    pub status: OverrideStatus,
    pub starts_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub approved_by: Option<Uuid>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionControl {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub control_type: TransactionControlType,
    pub action: TransactionControlAction,
    pub value: String,
    pub enabled: bool,
    pub starts_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionIntent {
    pub wallet_id: Uuid,
    pub amount_minor: i64,
    pub currency: String,
    pub direction: TransactionDirection,
    pub transaction_type: String,
    pub recipient: Option<String>,
    pub country: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitViolation {
    pub limit_id: Uuid,
    pub scope: LimitScope,
    pub limit_amount_minor: i64,
    pub projected_amount_minor: i64,
    pub remaining_minor: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitCheckResult {
    pub allowed: bool,
    pub wallet_id: Uuid,
    pub amount_minor: i64,
    pub currency: String,
    pub evaluated_limit_ids: Vec<Uuid>,
    pub violations: Vec<LimitViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlViolation {
    pub control_id: Uuid,
    pub control_type: TransactionControlType,
    pub action: TransactionControlAction,
    pub value: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlCheckResult {
    pub allowed: bool,
    pub requires_approval: bool,
    pub notifications_only: bool,
    pub violations: Vec<ControlViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicUsageReservation {
    pub limit_id: Uuid,
    pub wallet_id: Uuid,
    pub scope: LimitScope,
    pub currency: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub reserve_minor: i64,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitSummary {
    pub wallet_id: Uuid,
    pub currency: String,
    pub scope: LimitScope,
    pub effective_limit_minor: i64,
    pub used_minor: i64,
    pub reserved_minor: i64,
    pub remaining_minor: i64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

pub fn platform_limits_for_kyc_tier(
    wallet_id: Uuid,
    kyc_tier: i32,
    currency: &str,
    now: DateTime<Utc>,
) -> Vec<SpendingLimit> {
    let (single, daily, weekly, monthly) = match kyc_tier {
        i32::MIN..=0 => (5_000_000, 20_000_000, 100_000_000, 300_000_000),
        1 => (25_000_000, 100_000_000, 500_000_000, 2_000_000_000),
        2 => (100_000_000, 500_000_000, 2_500_000_000, 10_000_000_000),
        _ => (1_000_000_000, 5_000_000_000, 25_000_000_000, 100_000_000_000),
    };

    [
        (LimitScope::PerTransaction, single),
        (LimitScope::Daily, daily),
        (LimitScope::Weekly, weekly),
        (LimitScope::Monthly, monthly),
    ]
    .into_iter()
    .map(|(scope, amount_minor)| SpendingLimit {
        id: Uuid::new_v4(),
        wallet_id,
        source: LimitSource::PlatformKycTier,
        scope,
        amount_minor,
        currency: currency.to_ascii_uppercase(),
        status: LimitStatus::Active,
        starts_at: now,
        expires_at: None,
        cooling_off_until: None,
        created_by: None,
    })
    .collect()
}

pub fn period_window_for_scope(
    scope: LimitScope,
    at: DateTime<Utc>,
) -> (DateTime<Utc>, DateTime<Utc>) {
    match scope {
        LimitScope::PerTransaction => (at, at),
        LimitScope::Daily => {
            let start = start_of_day(at.date_naive());
            (start, start + Duration::days(1))
        }
        LimitScope::Weekly => {
            let date = at.date_naive();
            let monday = date - Duration::days(i64::from(date.weekday().num_days_from_monday()));
            let start = start_of_day(monday);
            (start, start + Duration::days(7))
        }
        LimitScope::Monthly => {
            let date = at.date_naive();
            let start_date = NaiveDate::from_ymd_opt(date.year(), date.month(), 1)
                .expect("valid first day for month");
            let (next_year, next_month) = if date.month() == 12 {
                (date.year() + 1, 1)
            } else {
                (date.year(), date.month() + 1)
            };
            let next_date = NaiveDate::from_ymd_opt(next_year, next_month, 1)
                .expect("valid first day for next month");
            (start_of_day(start_date), start_of_day(next_date))
        }
    }
}

pub fn is_limit_active(limit: &SpendingLimit, at: DateTime<Utc>) -> bool {
    limit.status == LimitStatus::Active
        && limit.starts_at <= at
        && limit.expires_at.map_or(true, |expires_at| expires_at > at)
        && limit
            .cooling_off_until
            .map_or(true, |cooling_off_until| cooling_off_until <= at)
}

pub fn override_is_active(limit_override: &LimitOverride, at: DateTime<Utc>) -> bool {
    limit_override.status == OverrideStatus::Approved
        && limit_override.starts_at <= at
        && limit_override.expires_at > at
}

pub fn apply_override_amount(
    limit: &SpendingLimit,
    overrides: &[LimitOverride],
    at: DateTime<Utc>,
) -> i64 {
    overrides
        .iter()
        .filter(|candidate| candidate.limit_id == limit.id)
        .filter(|candidate| override_is_active(candidate, at))
        .map(|candidate| candidate.approved_amount_minor)
        .max()
        .unwrap_or(limit.amount_minor)
}

pub fn select_most_restrictive_limit<'a>(
    limits: &'a [SpendingLimit],
    scope: LimitScope,
    currency: &str,
    at: DateTime<Utc>,
) -> Option<&'a SpendingLimit> {
    let normalized_currency = currency.to_ascii_uppercase();
    limits
        .iter()
        .filter(|limit| limit.scope == scope)
        .filter(|limit| limit.currency.eq_ignore_ascii_case(&normalized_currency))
        .filter(|limit| is_limit_active(limit, at))
        .min_by_key(|limit| limit.amount_minor)
}

pub fn evaluate_spending_limits(
    intent: &TransactionIntent,
    limits: &[SpendingLimit],
    usage: &[LimitUsageSnapshot],
    overrides: &[LimitOverride],
) -> LimitCheckResult {
    if intent.direction != TransactionDirection::Debit || intent.amount_minor <= 0 {
        return LimitCheckResult {
            allowed: true,
            wallet_id: intent.wallet_id,
            amount_minor: intent.amount_minor,
            currency: intent.currency.to_ascii_uppercase(),
            evaluated_limit_ids: Vec::new(),
            violations: Vec::new(),
        };
    }

    let mut evaluated_limit_ids = Vec::new();
    let mut violations = Vec::new();

    for limit in limits
        .iter()
        .filter(|limit| limit.wallet_id == intent.wallet_id)
        .filter(|limit| limit.currency.eq_ignore_ascii_case(&intent.currency))
        .filter(|limit| is_limit_active(limit, intent.occurred_at))
    {
        evaluated_limit_ids.push(limit.id);
        let effective_amount = apply_override_amount(limit, overrides, intent.occurred_at);
        let projected_amount = if limit.scope == LimitScope::PerTransaction {
            intent.amount_minor
        } else {
            let (period_start, period_end) =
                period_window_for_scope(limit.scope, intent.occurred_at);
            let snapshot = usage.iter().find(|snapshot| {
                snapshot.limit_id == limit.id
                    && snapshot.period_start == period_start
                    && snapshot.period_end == period_end
            });
            snapshot
                .map(|snapshot| snapshot.used_minor + snapshot.reserved_minor)
                .unwrap_or(0)
                + intent.amount_minor
        };

        if projected_amount > effective_amount {
            let already_committed = (projected_amount - intent.amount_minor).max(0);
            violations.push(LimitViolation {
                limit_id: limit.id,
                scope: limit.scope,
                limit_amount_minor: effective_amount,
                projected_amount_minor: projected_amount,
                remaining_minor: (effective_amount - already_committed).max(0),
                reason: format!(
                    "{:?} limit exceeded for {}",
                    limit.scope,
                    intent.currency.to_ascii_uppercase()
                ),
            });
        }
    }

    LimitCheckResult {
        allowed: violations.is_empty(),
        wallet_id: intent.wallet_id,
        amount_minor: intent.amount_minor,
        currency: intent.currency.to_ascii_uppercase(),
        evaluated_limit_ids,
        violations,
    }
}

pub fn build_atomic_usage_reservations(
    intent: &TransactionIntent,
    limits: &[SpendingLimit],
    usage: &[LimitUsageSnapshot],
    overrides: &[LimitOverride],
) -> Result<Vec<AtomicUsageReservation>, LimitCheckResult> {
    let check = evaluate_spending_limits(intent, limits, usage, overrides);
    if !check.allowed {
        return Err(check);
    }

    let mut reservations: Vec<_> = limits
        .iter()
        .filter(|limit| limit.wallet_id == intent.wallet_id)
        .filter(|limit| limit.currency.eq_ignore_ascii_case(&intent.currency))
        .filter(|limit| limit.scope != LimitScope::PerTransaction)
        .filter(|limit| is_limit_active(limit, intent.occurred_at))
        .map(|limit| {
            let (period_start, period_end) =
                period_window_for_scope(limit.scope, intent.occurred_at);
            AtomicUsageReservation {
                limit_id: limit.id,
                wallet_id: intent.wallet_id,
                scope: limit.scope,
                currency: intent.currency.to_ascii_uppercase(),
                period_start,
                period_end,
                reserve_minor: intent.amount_minor,
                idempotency_key: intent.idempotency_key.clone(),
            }
        })
        .collect();

    reservations.sort_by_key(|reservation| {
        (
            reservation.wallet_id,
            reservation.currency.clone(),
            scope_lock_rank(reservation.scope),
            reservation.period_start,
            reservation.limit_id,
        )
    });
    Ok(reservations)
}

pub fn evaluate_transaction_controls(
    intent: &TransactionIntent,
    controls: &[TransactionControl],
) -> ControlCheckResult {
    let mut requires_approval = false;
    let mut notifications_only = false;
    let mut violations = Vec::new();

    for control in controls
        .iter()
        .filter(|control| control.wallet_id == intent.wallet_id)
        .filter(|control| control.enabled)
        .filter(|control| control.starts_at <= intent.occurred_at)
        .filter(|control| control.expires_at.map_or(true, |expires_at| expires_at > intent.occurred_at))
        .filter(|control| control_matches_intent(control, intent))
    {
        match control.action {
            TransactionControlAction::Block => {}
            TransactionControlAction::RequireApproval => requires_approval = true,
            TransactionControlAction::NotifyOnly => notifications_only = true,
        }

        violations.push(ControlViolation {
            control_id: control.id,
            control_type: control.control_type,
            action: control.action,
            value: control.value.clone(),
            reason: control
                .reason
                .clone()
                .unwrap_or_else(|| "transaction control matched".to_string()),
        });
    }

    let hard_blocked = violations
        .iter()
        .any(|violation| violation.action == TransactionControlAction::Block);

    ControlCheckResult {
        allowed: !hard_blocked,
        requires_approval,
        notifications_only,
        violations,
    }
}

pub fn requires_cooling_off_for_update(
    existing: &SpendingLimit,
    requested_amount_minor: i64,
    requested_source: LimitSource,
) -> bool {
    existing.source == LimitSource::UserDefined
        && requested_source == LimitSource::UserDefined
        && requested_amount_minor > existing.amount_minor
}

pub fn cooling_off_until(now: DateTime<Utc>) -> DateTime<Utc> {
    now + Duration::hours(USER_LIMIT_COOLING_OFF_HOURS)
}

pub fn summarize_limits(
    wallet_id: Uuid,
    currency: &str,
    limits: &[SpendingLimit],
    usage: &[LimitUsageSnapshot],
    overrides: &[LimitOverride],
    at: DateTime<Utc>,
) -> Vec<LimitSummary> {
    let normalized_currency = currency.to_ascii_uppercase();

    [
        LimitScope::PerTransaction,
        LimitScope::Daily,
        LimitScope::Weekly,
        LimitScope::Monthly,
    ]
    .into_iter()
    .filter_map(|scope| {
        let limit = select_most_restrictive_limit(limits, scope, &normalized_currency, at)?;
        if limit.wallet_id != wallet_id {
            return None;
        }
        let effective_limit_minor = apply_override_amount(limit, overrides, at);
        let (period_start, period_end) = period_window_for_scope(scope, at);
        let snapshot = usage.iter().find(|snapshot| {
            snapshot.limit_id == limit.id
                && snapshot.period_start == period_start
                && snapshot.period_end == period_end
        });
        let used_minor = snapshot.map(|snapshot| snapshot.used_minor).unwrap_or(0);
        let reserved_minor = snapshot.map(|snapshot| snapshot.reserved_minor).unwrap_or(0);

        Some(LimitSummary {
            wallet_id,
            currency: normalized_currency.clone(),
            scope,
            effective_limit_minor,
            used_minor,
            reserved_minor,
            remaining_minor: (effective_limit_minor - used_minor - reserved_minor).max(0),
            period_start,
            period_end,
        })
    })
    .collect()
}

fn control_matches_intent(control: &TransactionControl, intent: &TransactionIntent) -> bool {
    match control.control_type {
        TransactionControlType::TransactionTypeBlock => {
            intent.transaction_type.eq_ignore_ascii_case(&control.value)
        }
        TransactionControlType::RecipientBlock => intent
            .recipient
            .as_deref()
            .map(|recipient| recipient.eq_ignore_ascii_case(&control.value))
            .unwrap_or(false),
        TransactionControlType::GeographyRestriction => intent
            .country
            .as_deref()
            .map(|country| country.eq_ignore_ascii_case(&control.value))
            .unwrap_or(false),
        TransactionControlType::TimeOfDayRestriction => {
            time_range_contains(&control.value, intent.occurred_at)
        }
    }
}

fn time_range_contains(range: &str, at: DateTime<Utc>) -> bool {
    let Some((start, end)) = range.split_once('-') else {
        return false;
    };
    let Some(start_minutes) = parse_hhmm(start) else {
        return false;
    };
    let Some(end_minutes) = parse_hhmm(end) else {
        return false;
    };
    let current_minutes = at.hour() * 60 + at.minute();

    if start_minutes <= end_minutes {
        current_minutes >= start_minutes && current_minutes < end_minutes
    } else {
        current_minutes >= start_minutes || current_minutes < end_minutes
    }
}

fn parse_hhmm(value: &str) -> Option<u32> {
    let (hour, minute) = value.trim().split_once(':')?;
    let hour: u32 = hour.parse().ok()?;
    let minute: u32 = minute.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

fn start_of_day(date: NaiveDate) -> DateTime<Utc> {
    DateTime::<Utc>::from_naive_utc_and_offset(
        date.and_hms_opt(0, 0, 0).expect("midnight is valid"),
        Utc,
    )
}

fn scope_lock_rank(scope: LimitScope) -> u8 {
    match scope {
        LimitScope::PerTransaction => 0,
        LimitScope::Daily => 1,
        LimitScope::Weekly => 2,
        LimitScope::Monthly => 3,
    }
}
