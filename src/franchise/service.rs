//! Business logic for Multi-Store & Franchise Management (Issue #335).

use crate::error::Error;
use crate::franchise::{
    models::*,
    rbac::{can_access_branch, has_permission, permissions},
    repository::FranchiseRepository,
};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

pub struct FranchiseService {
    repo: Arc<FranchiseRepository>,
}

impl FranchiseService {
    pub fn new(repo: Arc<FranchiseRepository>) -> Self {
        Self { repo }
    }

    // -------------------------------------------------------------------------
    // Organizations
    // -------------------------------------------------------------------------

    pub async fn create_organization(
        &self,
        owner_user_id: Uuid,
        req: CreateOrganizationRequest,
    ) -> Result<Organization, Error> {
        if req.slug.is_empty() || req.name.is_empty() {
            return Err(Error::BadRequest("name and slug are required".into()));
        }
        let org = self
            .repo
            .create_organization(owner_user_id, &req)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Seed default roles
        let _ = self.repo.create_default_roles(org.id).await;

        // Audit
        let _ = self
            .repo
            .write_audit_log(
                org.id,
                None,
                Some(owner_user_id),
                "create",
                "organization",
                Some(&org.id.to_string()),
                None,
            )
            .await;

        info!(org_id = %org.id, owner = %owner_user_id, "Organization created");
        Ok(org)
    }

    pub async fn get_organization(&self, org_id: Uuid) -> Result<Organization, Error> {
        self.repo
            .get_organization(org_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("Organization not found".into()))
    }

    pub async fn update_settlement(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
        req: UpdateSettlementRequest,
    ) -> Result<Organization, Error> {
        // Verify actor has settlement:manage permission
        self.require_permission(org_id, actor_user_id, permissions::SETTLEMENT_MANAGE)
            .await?;

        let valid_modes = ["individual", "centralized"];
        if !valid_modes.contains(&req.settlement_mode.as_str()) {
            return Err(Error::BadRequest(
                "settlement_mode must be 'individual' or 'centralized'".into(),
            ));
        }

        let org = self
            .repo
            .update_settlement(
                org_id,
                &req.settlement_mode,
                req.centralized_bank_account_id.as_deref(),
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let _ = self
            .repo
            .write_audit_log(
                org_id,
                None,
                Some(actor_user_id),
                "update_settlement",
                "organization",
                Some(&org_id.to_string()),
                Some(&serde_json::json!({ "settlement_mode": req.settlement_mode })),
            )
            .await;

        Ok(org)
    }

    // -------------------------------------------------------------------------
    // Regions
    // -------------------------------------------------------------------------

    pub async fn create_region(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
        req: CreateRegionRequest,
    ) -> Result<OrganizationRegion, Error> {
        self.require_permission(org_id, actor_user_id, permissions::ORG_ADMIN)
            .await?;

        let region = self
            .repo
            .create_region(org_id, &req)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let _ = self
            .repo
            .write_audit_log(
                org_id,
                None,
                Some(actor_user_id),
                "create",
                "region",
                Some(&region.id.to_string()),
                None,
            )
            .await;

        Ok(region)
    }

    pub async fn list_regions(&self, org_id: Uuid) -> Result<Vec<OrganizationRegion>, Error> {
        self.repo
            .list_regions(org_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }

    // -------------------------------------------------------------------------
    // Branches
    // -------------------------------------------------------------------------

    pub async fn create_branch(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
        req: CreateBranchRequest,
    ) -> Result<OrganizationBranch, Error> {
        self.require_permission(org_id, actor_user_id, permissions::BRANCH_MANAGE)
            .await?;

        let branch = self
            .repo
            .create_branch(org_id, &req)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let _ = self
            .repo
            .write_audit_log(
                org_id,
                Some(branch.id),
                Some(actor_user_id),
                "create",
                "branch",
                Some(&branch.id.to_string()),
                None,
            )
            .await;

        info!(org_id = %org_id, branch_id = %branch.id, "Branch created");
        Ok(branch)
    }

    pub async fn list_branches(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<Vec<OrganizationBranch>, Error> {
        let member = self.repo.get_member(org_id, actor_user_id).await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let branches = self
            .repo
            .list_branches(org_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Filter to only branches the actor can see
        if let Some(m) = member {
            if m.branch_id.is_some() {
                return Ok(branches
                    .into_iter()
                    .filter(|b| can_access_branch(&m, b.id))
                    .collect());
            }
        }
        Ok(branches)
    }

    pub async fn remove_branch(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
        branch_id: Uuid,
    ) -> Result<(), Error> {
        self.require_permission(org_id, actor_user_id, permissions::BRANCH_MANAGE)
            .await?;

        self.repo
            .deactivate_branch(org_id, branch_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let _ = self
            .repo
            .write_audit_log(
                org_id,
                Some(branch_id),
                Some(actor_user_id),
                "deactivate",
                "branch",
                Some(&branch_id.to_string()),
                None,
            )
            .await;

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Members
    // -------------------------------------------------------------------------

    pub async fn add_member(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
        req: AddMemberRequest,
    ) -> Result<OrganizationMember, Error> {
        self.require_permission(org_id, actor_user_id, permissions::MEMBER_MANAGE)
            .await?;

        let member = self
            .repo
            .add_member(org_id, &req)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let _ = self
            .repo
            .write_audit_log(
                org_id,
                req.branch_id,
                Some(actor_user_id),
                "add_member",
                "member",
                Some(&req.user_id.to_string()),
                None,
            )
            .await;

        Ok(member)
    }

    pub async fn remove_member(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
        target_user_id: Uuid,
    ) -> Result<(), Error> {
        self.require_permission(org_id, actor_user_id, permissions::MEMBER_MANAGE)
            .await?;

        self.repo
            .remove_member(org_id, target_user_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let _ = self
            .repo
            .write_audit_log(
                org_id,
                None,
                Some(actor_user_id),
                "remove_member",
                "member",
                Some(&target_user_id.to_string()),
                None,
            )
            .await;

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Cross-store analytics
    // -------------------------------------------------------------------------

    pub async fn get_cross_store_revenue(
        &self,
        org_id: Uuid,
        actor_user_id: Uuid,
        query: RevenueReportQuery,
    ) -> Result<CrossStoreRevenueReport, Error> {
        self.require_permission(org_id, actor_user_id, permissions::REPORTS_VIEW)
            .await?;

        let rows = self
            .repo
            .get_cross_store_revenue(org_id, query.period_start, query.period_end, query.branch_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let total_revenue: f64 = rows
            .iter()
            .map(|(_, _, rev, _)| {
                rev.to_string().parse::<f64>().unwrap_or(0.0)
            })
            .sum();

        let total_transactions: i64 = rows.iter().map(|(_, _, _, cnt)| cnt).sum();

        let branches = rows
            .into_iter()
            .map(|(branch_id, branch_name, revenue, count)| BranchRevenueSummary {
                branch_id,
                branch_name,
                total_revenue: revenue.to_string(),
                transaction_count: count,
            })
            .collect();

        Ok(CrossStoreRevenueReport {
            organization_id: org_id,
            period_start: query.period_start,
            period_end: query.period_end,
            total_revenue: format!("{:.2}", total_revenue),
            total_transactions,
            branches,
        })
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    async fn require_permission(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<(), Error> {
        let member = self
            .repo
            .get_member(org_id, user_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::Forbidden("Not a member of this organization".into()))?;

        let role = self
            .repo
            .get_role(member.role_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::Internal("Role not found".into()))?;

        if !has_permission(&role, permission) {
            return Err(Error::Forbidden(format!(
                "Missing permission: {}",
                permission
            )));
        }
        Ok(())
    }
}
