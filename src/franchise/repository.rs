//! Database access layer for franchise management.

use crate::database::error::DatabaseError;
use crate::franchise::models::*;
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

pub struct FranchiseRepository {
    pool: PgPool,
}

impl FranchiseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -------------------------------------------------------------------------
    // Organizations
    // -------------------------------------------------------------------------

    pub async fn create_organization(
        &self,
        owner_user_id: Uuid,
        req: &CreateOrganizationRequest,
    ) -> Result<Organization, DatabaseError> {
        let settlement_mode = req.settlement_mode.as_deref().unwrap_or("individual");
        sqlx::query_as::<_, Organization>(
            r#"
            INSERT INTO organizations
                (owner_user_id, name, slug, logo_url, settlement_mode, centralized_bank_account_id)
            VALUES ($1,$2,$3,$4,$5,$6)
            RETURNING *
            "#,
        )
        .bind(owner_user_id)
        .bind(&req.name)
        .bind(&req.slug)
        .bind(&req.logo_url)
        .bind(settlement_mode)
        .bind(&req.centralized_bank_account_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_organization(&self, org_id: Uuid) -> Result<Option<Organization>, DatabaseError> {
        sqlx::query_as::<_, Organization>(
            "SELECT * FROM organizations WHERE id = $1",
        )
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn update_settlement(
        &self,
        org_id: Uuid,
        settlement_mode: &str,
        bank_account_id: Option<&str>,
    ) -> Result<Organization, DatabaseError> {
        sqlx::query_as::<_, Organization>(
            r#"
            UPDATE organizations
            SET settlement_mode = $2,
                centralized_bank_account_id = $3,
                updated_at = now()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(org_id)
        .bind(settlement_mode)
        .bind(bank_account_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Regions
    // -------------------------------------------------------------------------

    pub async fn create_region(
        &self,
        org_id: Uuid,
        req: &CreateRegionRequest,
    ) -> Result<OrganizationRegion, DatabaseError> {
        sqlx::query_as::<_, OrganizationRegion>(
            r#"
            INSERT INTO organization_regions (organization_id, name, code)
            VALUES ($1,$2,$3)
            RETURNING *
            "#,
        )
        .bind(org_id)
        .bind(&req.name)
        .bind(&req.code)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_regions(
        &self,
        org_id: Uuid,
    ) -> Result<Vec<OrganizationRegion>, DatabaseError> {
        sqlx::query_as::<_, OrganizationRegion>(
            "SELECT * FROM organization_regions WHERE organization_id = $1 ORDER BY name",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Branches
    // -------------------------------------------------------------------------

    pub async fn create_branch(
        &self,
        org_id: Uuid,
        req: &CreateBranchRequest,
    ) -> Result<OrganizationBranch, DatabaseError> {
        let policies = req
            .local_policies
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        sqlx::query_as::<_, OrganizationBranch>(
            r#"
            INSERT INTO organization_branches
                (organization_id, region_id, name, branch_code, address,
                 wallet_address, settlement_mode, local_policies)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            RETURNING *
            "#,
        )
        .bind(org_id)
        .bind(req.region_id)
        .bind(&req.name)
        .bind(&req.branch_code)
        .bind(&req.address)
        .bind(&req.wallet_address)
        .bind(&req.settlement_mode)
        .bind(policies)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_branches(
        &self,
        org_id: Uuid,
    ) -> Result<Vec<OrganizationBranch>, DatabaseError> {
        sqlx::query_as::<_, OrganizationBranch>(
            "SELECT * FROM organization_branches WHERE organization_id = $1 ORDER BY name",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn deactivate_branch(
        &self,
        org_id: Uuid,
        branch_id: Uuid,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE organization_branches SET is_active = FALSE, updated_at = now()
             WHERE id = $1 AND organization_id = $2",
        )
        .bind(branch_id)
        .bind(org_id)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Members
    // -------------------------------------------------------------------------

    pub async fn add_member(
        &self,
        org_id: Uuid,
        req: &AddMemberRequest,
    ) -> Result<OrganizationMember, DatabaseError> {
        sqlx::query_as::<_, OrganizationMember>(
            r#"
            INSERT INTO organization_members
                (organization_id, user_id, role_id, branch_id)
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (organization_id, user_id, branch_id) DO UPDATE SET
                role_id    = EXCLUDED.role_id,
                is_active  = TRUE,
                updated_at = now()
            RETURNING *
            "#,
        )
        .bind(org_id)
        .bind(req.user_id)
        .bind(req.role_id)
        .bind(req.branch_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn remove_member(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE organization_members SET is_active = FALSE, updated_at = now()
             WHERE organization_id = $1 AND user_id = $2",
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    pub async fn get_member(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrganizationMember>, DatabaseError> {
        sqlx::query_as::<_, OrganizationMember>(
            "SELECT * FROM organization_members
             WHERE organization_id = $1 AND user_id = $2 AND is_active = TRUE
             LIMIT 1",
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Roles
    // -------------------------------------------------------------------------

    pub async fn create_default_roles(
        &self,
        org_id: Uuid,
    ) -> Result<Vec<OrganizationRole>, DatabaseError> {
        let corporate_perms = serde_json::json!([
            "org:admin","org:view","branch:manage","branch:view",
            "member:manage","settlement:manage","reports:view"
        ]);
        let manager_perms = serde_json::json!(["branch:view","reports:view"]);

        sqlx::query(
            r#"
            INSERT INTO organization_roles (organization_id, name, permissions, scope)
            VALUES
                ($1, 'Corporate Admin', $2, 'organization'),
                ($1, 'Branch Manager',  $3, 'branch')
            ON CONFLICT (organization_id, name) DO NOTHING
            "#,
        )
        .bind(org_id)
        .bind(corporate_perms)
        .bind(manager_perms)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        sqlx::query_as::<_, OrganizationRole>(
            "SELECT * FROM organization_roles WHERE organization_id = $1",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_role(&self, role_id: Uuid) -> Result<Option<OrganizationRole>, DatabaseError> {
        sqlx::query_as::<_, OrganizationRole>(
            "SELECT * FROM organization_roles WHERE id = $1",
        )
        .bind(role_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Audit log
    // -------------------------------------------------------------------------

    pub async fn write_audit_log(
        &self,
        org_id: Uuid,
        branch_id: Option<Uuid>,
        user_id: Option<Uuid>,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        new_value: Option<&serde_json::Value>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO organization_audit_logs
                (organization_id, branch_id, user_id, action, resource_type, resource_id, new_value)
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            "#,
        )
        .bind(org_id)
        .bind(branch_id)
        .bind(user_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(new_value)
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Cross-store revenue
    // -------------------------------------------------------------------------

    pub async fn get_cross_store_revenue(
        &self,
        org_id: Uuid,
        period_start: NaiveDate,
        period_end: NaiveDate,
        branch_id: Option<Uuid>,
    ) -> Result<Vec<(Option<Uuid>, Option<String>, sqlx::types::BigDecimal, i64)>, DatabaseError> {
        sqlx::query_as::<_, (Option<Uuid>, Option<String>, sqlx::types::BigDecimal, i64)>(
            r#"
            SELECT
                s.branch_id,
                b.name AS branch_name,
                COALESCE(SUM(s.total_revenue), 0) AS total_revenue,
                COALESCE(SUM(s.transaction_count), 0) AS transaction_count
            FROM organization_revenue_snapshots s
            LEFT JOIN organization_branches b ON b.id = s.branch_id
            WHERE s.organization_id = $1
              AND s.snapshot_date BETWEEN $2 AND $3
              AND ($4::uuid IS NULL OR s.branch_id = $4)
            GROUP BY s.branch_id, b.name
            ORDER BY total_revenue DESC
            "#,
        )
        .bind(org_id)
        .bind(period_start)
        .bind(period_end)
        .bind(branch_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }
}
