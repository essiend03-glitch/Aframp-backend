-- =============================================================================
-- Issue #335: Multi-Store & Franchise Management
-- =============================================================================

-- Organizations (top-level parent)
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    logo_url TEXT,
    -- Settlement preference
    settlement_mode TEXT NOT NULL DEFAULT 'individual'
        CHECK (settlement_mode IN ('individual', 'centralized')),
    centralized_bank_account_id TEXT,
    -- Global policy overrides (JSONB)
    global_policies JSONB NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE organizations IS 'Top-level franchise / corporate entity owning multiple stores.';

-- Regions (optional middle tier)
CREATE TABLE organization_regions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    code TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, code)
);

COMMENT ON TABLE organization_regions IS 'Optional regional grouping within an organization.';

-- Store / Branch (leaf node)
CREATE TABLE organization_branches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    region_id UUID REFERENCES organization_regions(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    branch_code TEXT NOT NULL,
    address TEXT,
    -- Branch-level policy overrides (null = inherit from org)
    local_policies JSONB NOT NULL DEFAULT '{}',
    -- Each branch has its own wallet address for settlement
    wallet_address VARCHAR(255),
    -- Settlement override: null = use org default
    settlement_mode TEXT CHECK (settlement_mode IN ('individual', 'centralized')),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, branch_code)
);

COMMENT ON TABLE organization_branches IS 'Individual store/branch within an organization.';

-- RBAC: roles within an organization
CREATE TABLE organization_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- Permissions as a JSONB set of strings
    permissions JSONB NOT NULL DEFAULT '[]',
    scope TEXT NOT NULL DEFAULT 'branch' CHECK (scope IN ('organization', 'region', 'branch')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, name)
);

COMMENT ON TABLE organization_roles IS 'RBAC roles scoped to organization, region, or branch.';

-- User memberships in organizations
CREATE TABLE organization_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES organization_roles(id) ON DELETE RESTRICT,
    -- Null branch_id = org-wide access; set = scoped to that branch
    branch_id UUID REFERENCES organization_branches(id) ON DELETE CASCADE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    invited_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    accepted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, user_id, branch_id)
);

COMMENT ON TABLE organization_members IS 'User membership and RBAC assignments within an organization.';

-- Audit log for all org-level actions
CREATE TABLE organization_audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    branch_id UUID REFERENCES organization_branches(id) ON DELETE SET NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    old_value JSONB,
    new_value JSONB,
    ip_address INET,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE organization_audit_logs IS 'Immutable audit trail for all franchise management actions.';

-- Cross-store revenue snapshots for consolidated reporting
CREATE TABLE organization_revenue_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    branch_id UUID REFERENCES organization_branches(id) ON DELETE CASCADE,
    snapshot_date DATE NOT NULL,
    total_revenue NUMERIC(36, 18) NOT NULL DEFAULT 0,
    transaction_count INTEGER NOT NULL DEFAULT 0,
    avg_transaction_value NUMERIC(36, 18),
    currency TEXT NOT NULL DEFAULT 'cNGN',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, branch_id, snapshot_date)
);

COMMENT ON TABLE organization_revenue_snapshots IS 'Daily revenue snapshots per branch for cross-store analytics.';

-- Indexes
CREATE INDEX idx_org_owner ON organizations(owner_user_id);
CREATE INDEX idx_branch_org ON organization_branches(organization_id);
CREATE INDEX idx_branch_region ON organization_branches(region_id);
CREATE INDEX idx_member_user ON organization_members(user_id);
CREATE INDEX idx_member_org ON organization_members(organization_id);
CREATE INDEX idx_member_branch ON organization_members(branch_id);
CREATE INDEX idx_org_audit_org ON organization_audit_logs(organization_id);
CREATE INDEX idx_org_audit_branch ON organization_audit_logs(branch_id);
CREATE INDEX idx_org_audit_user ON organization_audit_logs(user_id);
CREATE INDEX idx_org_revenue_org_date ON organization_revenue_snapshots(organization_id, snapshot_date);
CREATE INDEX idx_org_revenue_branch_date ON organization_revenue_snapshots(branch_id, snapshot_date);

-- Triggers
CREATE TRIGGER set_updated_at_organizations
    BEFORE UPDATE ON organizations
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_organization_regions
    BEFORE UPDATE ON organization_regions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_organization_branches
    BEFORE UPDATE ON organization_branches
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_organization_roles
    BEFORE UPDATE ON organization_roles
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER set_updated_at_organization_members
    BEFORE UPDATE ON organization_members
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Seed default roles
-- (These will be inserted per-organization at creation time via application logic)
