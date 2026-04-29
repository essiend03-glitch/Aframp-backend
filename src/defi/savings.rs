use chrono::{DateTime, Utc, Duration};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::database::DbPool;
use crate::error::AppError;
use super::{
    CngnSavingsProduct, CngnSavingsAccount, YieldAccrualRecord, WithdrawalRequest,
    CreateSavingsAccountRequest, DepositRequest, WithdrawalRequest as SavingsWithdrawalRequest,
    SavingsAccountResponse, ProjectedYield, YieldRateHistory, RiskDisclosureAcceptance,
    SavingsProductType, SavingsAccountStatus, WithdrawalType,
};

/// cNGN Savings Product Service
pub struct SavingsService {
    db_pool: Arc<DbPool>,
    config: SavingsConfig,
}

impl SavingsService {
    pub fn new(db_pool: Arc<DbPool>, config: SavingsConfig) -> Self {
        Self { db_pool, config }
    }

    /// Create a new savings account
    pub async fn create_savings_account(
        &self,
        request: CreateSavingsAccountRequest,
        user_id: &str,
        ip_address: Option<&str>,
    ) -> Result<CngnSavingsAccount, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Validate product and user permissions
        let product = self.get_savings_product(request.product_id).await?;
        self.validate_savings_deposit_request(&request, &product, user_id).await?;

        // Check if user already has an account for this product
        if self.user_has_account_for_product(request.wallet_id, request.product_id).await? {
            return Err(AppError::BadRequest("User already has an account for this product".to_string()));
        }

        // Create savings account
        let account = CngnSavingsAccount {
            account_id: Uuid::new_v4(),
            wallet_id: request.wallet_id,
            product_id: request.product_id,
            deposited_amount: request.deposit_amount.clone(),
            current_balance: request.deposit_amount.clone(),
            accrued_yield_to_date: BigDecimal::from(0),
            current_yield_rate: product.target_yield_rate,
            deposit_timestamp: Utc::now(),
            last_yield_accrual_timestamp: Utc::now(),
            withdrawal_eligibility_timestamp: Utc::now() + Duration::hours(product.lock_up_period_hours),
            account_status: SavingsAccountStatus::Active,
            risk_disclosure_accepted_at: Utc::now(),
            risk_disclosure_ip_address: ip_address.map(|s| s.to_string()),
        };

        // Insert account
        sqlx::query!(
            r#"
            INSERT INTO cngn_savings_accounts (
                account_id, wallet_id, product_id, deposited_amount, current_balance,
                accrued_yield_to_date, current_yield_rate, deposit_timestamp,
                last_yield_accrual_timestamp, withdrawal_eligibility_timestamp,
                account_status, risk_disclosure_accepted_at, risk_disclosure_ip_address
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
            account.account_id,
            account.wallet_id,
            account.product_id,
            account.deposited_amount,
            account.current_balance,
            account.accrued_yield_to_date,
            account.current_yield_rate,
            account.deposit_timestamp,
            account.last_yield_accrual_timestamp,
            account.withdrawal_eligibility_timestamp,
            account.account_status as SavingsAccountStatus,
            account.risk_disclosure_accepted_at,
            account.risk_disclosure_ip_address,
        )
        .execute(&mut *tx)
        .await?;

        // Record risk disclosure acceptance
        self.record_risk_disclosure_acceptance(
            user_id,
            request.product_id,
            account.risk_disclosure_accepted_at,
            ip_address,
        ).await?;

        // Deploy funds to underlying strategy if configured
        if let Some(strategy_id) = product.underlying_strategy_id {
            self.deploy_savings_funds_to_strategy(account.account_id, strategy_id, &request.deposit_amount, &mut tx).await?;
        }

        tx.commit().await?;

        tracing::info!(
            account_id = %account.account_id,
            wallet_id = %account.wallet_id,
            product_id = %account.product_id,
            amount = %account.deposited_amount,
            user_id = %user_id,
            "cNGN savings account created"
        );

        Ok(account)
    }

    /// Process deposit to savings account
    pub async fn deposit_to_savings_account(
        &self,
        request: DepositRequest,
        user_id: &str,
    ) -> Result<CngnSavingsAccount, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Get account and validate ownership
        let mut account = self.get_savings_account(request.account_id).await?;
        self.validate_account_ownership(&account, user_id).await?;

        let product = self.get_savings_product(account.product_id).await?;
        self.validate_deposit_amount(&request.amount, &product).await?;

        // Update account balance
        account.deposited_amount += &request.amount;
        account.current_balance += &request.amount;

        sqlx::query!(
            r#"
            UPDATE cngn_savings_accounts 
            SET deposited_amount = $1, current_balance = $2, updated_at = NOW()
            WHERE account_id = $3
            "#,
            account.deposited_amount,
            account.current_balance,
            account.account_id,
        )
        .execute(&mut *tx)
        .await?;

        // Deploy additional funds to strategy
        if let Some(strategy_id) = product.underlying_strategy_id {
            self.deploy_savings_funds_to_strategy(account.account_id, strategy_id, &request.amount, &mut tx).await?;
        }

        tx.commit().await?;

        tracing::info!(
            account_id = %account.account_id,
            amount = %request.amount,
            user_id = %user_id,
            "Deposit processed to cNGN savings account"
        );

        Ok(account)
    }

    /// Process withdrawal from savings account
    pub async fn withdraw_from_savings_account(
        &self,
        request: SavingsWithdrawalRequest,
        user_id: &str,
    ) -> Result<WithdrawalRequest, AppError> {
        let mut tx = self.db_pool.begin().await?;

        // Get account and validate ownership
        let account = self.get_savings_account(request.account_id).await?;
        self.validate_account_ownership(&account, user_id).await?;

        let product = self.get_savings_product(account.product_id).await?;
        self.validate_withdrawal_request(&request, &account, &product).await?;

        // Calculate penalty if early withdrawal
        let penalty_amount = self.calculate_early_withdrawal_penalty(&request, &account, &product)?;
        let net_amount = &request.amount - &penalty_amount;

        // Create withdrawal request
        let withdrawal = WithdrawalRequest {
            request_id: Uuid::new_v4(),
            account_id: request.account_id,
            requested_amount: request.amount,
            withdrawal_type: request.withdrawal_type,
            early_withdrawal_flag: self.is_early_withdrawal(&account, &product),
            penalty_amount: penalty_amount.clone(),
            net_withdrawal_amount: net_amount,
            request_timestamp: Utc::now(),
            settlement_timestamp: None,
            status: "pending".to_string(),
            transaction_hash: None,
        };

        // Insert withdrawal request
        sqlx::query!(
            r#"
            INSERT INTO withdrawal_requests (
                request_id, account_id, requested_amount, withdrawal_type,
                early_withdrawal_flag, penalty_amount, net_withdrawal_amount,
                request_timestamp, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            withdrawal.request_id,
            withdrawal.account_id,
            withdrawal.requested_amount,
            withdrawal.withdrawal_type as WithdrawalType,
            withdrawal.early_withdrawal_flag,
            withdrawal.penalty_amount,
            withdrawal.net_withdrawal_amount,
            withdrawal.request_timestamp,
            withdrawal.status,
        )
        .execute(&mut *tx)
        .await?;

        // Update account balance
        let new_balance = &account.current_balance - &request.amount;
        let new_status = if new_balance == BigDecimal::from(0) {
            SavingsAccountStatus::Closed
        } else {
            SavingsAccountStatus::Active
        };

        sqlx::query!(
            r#"
            UPDATE cngn_savings_accounts 
            SET current_balance = $1, account_status = $2, updated_at = NOW()
            WHERE account_id = $3
            "#,
            new_balance,
            new_status as SavingsAccountStatus,
            account.account_id,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            withdrawal_id = %withdrawal.request_id,
            account_id = %withdrawal.account_id,
            amount = %withdrawal.requested_amount,
            net_amount = %withdrawal.net_withdrawal_amount,
            user_id = %user_id,
            "Withdrawal processed from cNGN savings account"
        );

        Ok(withdrawal)
    }

    /// Calculate projected yield for an account
    pub async fn calculate_projected_yield(
        &self,
        account_id: Uuid,
        period_days: u32,
        user_id: &str,
    ) -> Result<ProjectedYield, AppError> {
        let account = self.get_savings_account(account_id).await?;
        self.validate_account_ownership(&account, user_id).await?;

        let opening_balance = account.current_balance.clone();
        let daily_rate = account.current_yield_rate / 365.0;
        let yield_amount = &opening_balance * BigDecimal::from(daily_rate * period_days as f64);
        let projected_end_balance = &opening_balance + &yield_amount;

        Ok(ProjectedYield {
            period_days,
            opening_balance,
            projected_yield_amount: yield_amount,
            projected_yield_rate: account.current_yield_rate,
            projected_end_balance,
        })
    }

    /// Reinvest accrued yield back into principal
    pub async fn reinvest_yield(
        &self,
        account_id: Uuid,
        user_id: &str,
    ) -> Result<CngnSavingsAccount, AppError> {
        let mut tx = self.db_pool.begin().await?;

        let mut account = self.get_savings_account(account_id).await?;
        self.validate_account_ownership(&account, user_id).await?;

        if account.accrued_yield_to_date == BigDecimal::from(0) {
            return Err(AppError::BadRequest("No accrued yield to reinvest".to_string()));
        }

        // Add accrued yield to principal
        let reinvest_amount = account.accrued_yield_to_date.clone();
        account.current_balance += &reinvest_amount;
        account.deposited_amount += &reinvest_amount;
        account.accrued_yield_to_date = BigDecimal::from(0);

        sqlx::query!(
            r#"
            UPDATE cngn_savings_accounts 
            SET current_balance = $1, deposited_amount = $2, accrued_yield_to_date = $3, updated_at = NOW()
            WHERE account_id = $4
            "#,
            account.current_balance,
            account.deposited_amount,
            account.accrued_yield_to_date,
            account.account_id,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            account_id = %account.account_id,
            reinvest_amount = %reinvest_amount,
            user_id = %user_id,
            "Yield reinvested into cNGN savings account"
        );

        Ok(account)
    }

    /// Get yield rate history for a product
    pub async fn get_yield_rate_history(
        &self,
        product_id: Uuid,
        days: Option<i64>,
    ) -> Result<Vec<YieldRateHistory>, AppError> {
        let history = sqlx::query_as!(
            YieldRateHistory,
            r#"
            SELECT 
                history_id, product_id, yield_rate, rate_source, recorded_at
            FROM yield_rate_history
            WHERE product_id = $1
            "#,
            product_id
        )
        .fetch_all(&*self.db_pool)
        .await?;

        Ok(history)
    }

    /// Accept risk disclosure
    pub async fn accept_risk_disclosure(
        &self,
        user_id: &str,
        product_id: Uuid,
        ip_address: Option<&str>,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        self.record_risk_disclosure_acceptance(user_id, product_id, now, ip_address).await?;
        Ok(())
    }

    /// Background job: Accrue yield for all active accounts
    pub async fn accrue_yield_for_all_accounts(&self) -> Result<u64, AppError> {
        let accounts = sqlx::query!(
            r#"
            SELECT account_id, current_balance, current_yield_rate, last_yield_accrual_timestamp
            FROM cngn_savings_accounts
            WHERE account_status = 'active' AND current_balance > 0
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        let mut processed_count = 0;

        for account in accounts {
            if let Ok(()) = self.accrue_yield_for_account(
                account.account_id,
                account.current_balance.unwrap_or_else(|| BigDecimal::from(0)),
                account.current_yield_rate.unwrap_or(0.0),
                account.last_yield_accrual_timestamp.unwrap_or_else(Utc::now),
            ).await {
                processed_count += 1;
            }
        }

        tracing::info!(
            processed_accounts = processed_count,
            total_accounts = accounts.len(),
            "Yield accrual job completed"
        );

        Ok(processed_count)
    }

    /// Background job: Update variable yield rates from underlying strategies
    pub async fn update_variable_yield_rates(&self) -> Result<u64, AppError> {
        let products = sqlx::query!(
            r#"
            SELECT product_id, underlying_strategy_id, yield_rate_source
            FROM cngn_savings_products
            WHERE yield_rate_source = 'variable' AND product_status = 'active'
            "#
        )
        .fetch_all(&*self.db_pool)
        .await?;

        let mut updated_count = 0;

        for product in products {
            if let Some(strategy_id) = product.underlying_strategy_id {
                if let Ok(new_rate) = self.get_strategy_yield_rate(strategy_id).await {
                    if let Ok(()) = self.update_product_yield_rate(product.product_id, new_rate).await {
                        updated_count += 1;
                    }
                }
            }
        }

        tracing::info!(
            updated_products = updated_count,
            total_products = products.len(),
            "Variable yield rate update job completed"
        );

        Ok(updated_count)
    }

    // ── Helper Methods ────────────────────────────────────────────────────────

    async fn get_savings_product(&self, product_id: Uuid) -> Result<CngnSavingsProduct, AppError> {
        let product = sqlx::query_as!(
            CngnSavingsProduct,
            r#"
            SELECT 
                product_id, product_name, description, product_type as "product_type: SavingsProductType",
                minimum_deposit_amount, maximum_deposit_amount, lock_up_period_hours,
                early_withdrawal_penalty_pct, target_yield_rate, yield_rate_source,
                underlying_strategy_id, yield_rate_floor, yield_rate_ceil,
                product_status, risk_disclosure_version, created_at, updated_at
            FROM cngn_savings_products
            WHERE product_id = $1
            "#,
            product_id
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(product)
    }

    async fn get_savings_account(&self, account_id: Uuid) -> Result<CngnSavingsAccount, AppError> {
        let account = sqlx::query_as!(
            CngnSavingsAccount,
            r#"
            SELECT 
                account_id, wallet_id, product_id, deposited_amount, current_balance,
                accrued_yield_to_date, current_yield_rate, deposit_timestamp,
                last_yield_accrual_timestamp, withdrawal_eligibility_timestamp,
                account_status as "account_status: SavingsAccountStatus",
                risk_disclosure_accepted_at, risk_disclosure_ip_address
            FROM cngn_savings_accounts
            WHERE account_id = $1
            "#,
            account_id
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(account)
    }

    async fn validate_savings_deposit_request(
        &self,
        request: &CreateSavingsAccountRequest,
        product: &CngnSavingsProduct,
        user_id: &str,
    ) -> Result<(), AppError> {
        // Validate deposit amount limits
        if request.deposit_amount < product.minimum_deposit_amount {
            return Err(AppError::BadRequest(format!(
                "Deposit amount {} is below minimum {}",
                request.deposit_amount, product.minimum_deposit_amount
            )));
        }

        if request.deposit_amount > product.maximum_deposit_amount {
            return Err(AppError::BadRequest(format!(
                "Deposit amount {} exceeds maximum {}",
                request.deposit_amount, product.maximum_deposit_amount
            )));
        }

        // Validate risk disclosure acceptance
        if !request.risk_disclosure_accepted {
            return Err(AppError::BadRequest("Risk disclosure must be accepted".to_string()));
        }

        // Check user KYC status (placeholder - would integrate with KYC service)
        if !self.user_kyc_eligible(user_id).await? {
            return Err(AppError::Forbidden("User not eligible for savings products".to_string()));
        }

        Ok(())
    }

    async fn validate_deposit_amount(
        &self,
        amount: &BigDecimal,
        product: &CngnSavingsProduct,
    ) -> Result<(), AppError> {
        if amount < &product.minimum_deposit_amount {
            return Err(AppError::BadRequest(format!(
                "Deposit amount {} is below minimum {}",
                amount, product.minimum_deposit_amount
            )));
        }

        Ok(())
    }

    async fn validate_withdrawal_request(
        &self,
        request: &SavingsWithdrawalRequest,
        account: &CngnSavingsAccount,
        product: &CngnSavingsProduct,
    ) -> Result<(), AppError> {
        // Validate sufficient balance
        if request.amount > account.current_balance {
            return Err(AppError::BadRequest("Insufficient balance for withdrawal".to_string()));
        }

        // Check lock-up period for fixed-term products
        if product.product_type == SavingsProductType::FixedTerm {
            if Utc::now() < account.withdrawal_eligibility_timestamp {
                return Err(AppError::BadRequest("Withdrawal not permitted during lock-up period".to_string()));
            }
        }

        // Validate partial withdrawal amount
        if matches!(request.withdrawal_type, WithdrawalType::Partial) {
            let remaining_balance = &account.current_balance - &request.amount;
            if remaining_balance < product.minimum_deposit_amount {
                return Err(AppError::BadRequest("Partial withdrawal would leave balance below minimum".to_string()));
            }
        }

        Ok(())
    }

    fn calculate_early_withdrawal_penalty(
        &self,
        request: &SavingsWithdrawalRequest,
        account: &CngnSavingsAccount,
        product: &CngnSavingsProduct,
    ) -> Result<BigDecimal, AppError> {
        if self.is_early_withdrawal(account, product) {
            let penalty = &request.amount * (product.early_withdrawal_penalty_pct / 100.0);
            Ok(penalty)
        } else {
            Ok(BigDecimal::from(0))
        }
    }

    fn is_early_withdrawal(&self, account: &CngnSavingsAccount, product: &CngnSavingsProduct) -> bool {
        product.product_type == SavingsProductType::FixedTerm &&
        Utc::now() < account.withdrawal_eligibility_timestamp
    }

    async fn validate_account_ownership(&self, account: &CngnSavingsAccount, user_id: &str) -> Result<(), AppError> {
        // In a real implementation, would verify wallet ownership
        // For now, assume the check passes
        Ok(())
    }

    async fn user_has_account_for_product(&self, wallet_id: Uuid, product_id: Uuid) -> Result<bool, AppError> {
        let count = sqlx::query!(
            "SELECT COUNT(*) as count FROM cngn_savings_accounts WHERE wallet_id = $1 AND product_id = $2",
            wallet_id,
            product_id
        )
        .fetch_one(&*self.db_pool)
        .await?;

        Ok(count.count.unwrap_or(0) > 0)
    }

    async fn record_risk_disclosure_acceptance(
        &self,
        user_id: &str,
        product_id: Uuid,
        accepted_at: DateTime<Utc>,
        ip_address: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"
            INSERT INTO risk_disclosure_acceptances (
                acceptance_id, user_id, product_id, disclosure_version,
                accepted_at, ip_address
            ) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (user_id, product_id) DO UPDATE SET
                accepted_at = EXCLUDED.accepted_at,
                ip_address = EXCLUDED.ip_address
            "#,
            Uuid::new_v4(),
            user_id,
            product_id,
            "1.0", // Would get current version
            accepted_at,
            ip_address,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    async fn deploy_savings_funds_to_strategy(
        &self,
        _account_id: Uuid,
        _strategy_id: Uuid,
        _amount: &BigDecimal,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), AppError> {
        // Implementation would deploy funds to the underlying DeFi strategy
        // For now, this is a placeholder
        Ok(())
    }

    async fn accrue_yield_for_account(
        &self,
        account_id: Uuid,
        balance: BigDecimal,
        yield_rate: f64,
        last_accrual: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        let hours_elapsed = (now - last_accrual).num_hours();
        
        if hours_elapsed < 1 {
            return Ok(()); // Don't accrue if less than 1 hour has passed
        }

        let hourly_rate = yield_rate / (365.0 * 24.0);
        let yield_amount = &balance * BigDecimal::from(hourly_rate * hours_elapsed as f64);

        if yield_amount > BigDecimal::from(0) {
            // Update account
            sqlx::query!(
                r#"
                UPDATE cngn_savings_accounts 
                SET accrued_yield_to_date = accrued_yield_to_date + $1,
                    last_yield_accrual_timestamp = $2,
                    updated_at = NOW()
                WHERE account_id = $3
                "#,
                yield_amount,
                now,
                account_id,
            )
            .execute(&*self.db_pool)
            .await?;

            // Record accrual
            sqlx::query!(
                r#"
                INSERT INTO yield_accrual_records (
                    accrual_id, account_id, accrual_period_start, accrual_period_end,
                    opening_balance, yield_rate_applied, yield_amount_earned, accrual_timestamp
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                Uuid::new_v4(),
                account_id,
                last_accrual,
                now,
                balance,
                yield_rate,
                yield_amount,
                now,
            )
            .execute(&*self.db_pool)
            .await?;
        }

        Ok(())
    }

    async fn get_strategy_yield_rate(&self, _strategy_id: Uuid) -> Result<f64, AppError> {
        // Implementation would get current yield rate from strategy
        // For now, return placeholder
        Ok(0.08) // 8%
    }

    async fn update_product_yield_rate(&self, product_id: Uuid, new_rate: f64) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE cngn_savings_products SET target_yield_rate = $1, updated_at = NOW() WHERE product_id = $2",
            new_rate,
            product_id,
        )
        .execute(&*self.db_pool)
        .await?;

        Ok(())
    }

    async fn user_kyc_eligible(&self, _user_id: &str) -> Result<bool, AppError> {
        // Implementation would check KYC status
        // For now, assume eligible
        Ok(true)
    }
}

/// Savings configuration
#[derive(Debug, Clone)]
pub struct SavingsConfig {
    pub yield_accrual_interval_hours: u64,
    pub yield_rate_update_interval_hours: u64,
    pub min_account_balance: BigDecimal,
    pub max_withdrawal_frequency_hours: u64,
}

impl Default for SavingsConfig {
    fn default() -> Self {
        Self {
            yield_accrual_interval_hours: 1,
            yield_rate_update_interval_hours: 24,
            min_account_balance: BigDecimal::from(100),
            max_withdrawal_frequency_hours: 24,
        }
    }
}
