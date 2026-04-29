//! RBAC helpers for franchise access control.
//!
//! Permissions are stored as a JSON array of strings on each role.
//! Scope determines whether a member can see org-wide, region, or branch data.

use crate::franchise::models::{OrganizationMember, OrganizationRole};
use uuid::Uuid;

/// Well-known permission strings.
pub mod permissions {
    pub const ORG_ADMIN: &str = "org:admin";
    pub const ORG_VIEW: &str = "org:view";
    pub const BRANCH_MANAGE: &str = "branch:manage";
    pub const BRANCH_VIEW: &str = "branch:view";
    pub const MEMBER_MANAGE: &str = "member:manage";
    pub const SETTLEMENT_MANAGE: &str = "settlement:manage";
    pub const REPORTS_VIEW: &str = "reports:view";
}

/// Check whether a member has a specific permission.
pub fn has_permission(role: &OrganizationRole, permission: &str) -> bool {
    role.permissions
        .as_array()
        .map(|arr| arr.iter().any(|p| p.as_str() == Some(permission)))
        .unwrap_or(false)
}

/// Check whether a member can access data for a given branch.
///
/// - Org-scope members (branch_id = None) can access all branches.
/// - Branch-scope members can only access their assigned branch.
pub fn can_access_branch(member: &OrganizationMember, branch_id: Uuid) -> bool {
    match member.branch_id {
        None => true,                          // org-wide access
        Some(assigned) => assigned == branch_id,
    }
}

/// Default permissions for built-in roles.
pub fn corporate_admin_permissions() -> Vec<&'static str> {
    vec![
        permissions::ORG_ADMIN,
        permissions::ORG_VIEW,
        permissions::BRANCH_MANAGE,
        permissions::BRANCH_VIEW,
        permissions::MEMBER_MANAGE,
        permissions::SETTLEMENT_MANAGE,
        permissions::REPORTS_VIEW,
    ]
}

pub fn branch_manager_permissions() -> Vec<&'static str> {
    vec![
        permissions::BRANCH_VIEW,
        permissions::REPORTS_VIEW,
    ]
}
