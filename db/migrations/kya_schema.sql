-- KYA (Know Your Agent) Database Schema
-- Decentralized Agent Identity & Reputation System

-- Agent Identity Registry
CREATE TABLE IF NOT EXISTS kya_agent_identities (
    did TEXT PRIMARY KEY,
    method TEXT NOT NULL,
    network TEXT NOT NULL,
    identifier TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    owner_address TEXT NOT NULL,
    public_key TEXT NOT NULL,
    capabilities JSONB NOT NULL DEFAULT '[]'::jsonb,
    service_endpoints JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(method, network, identifier)
);

CREATE INDEX idx_kya_identities_owner ON kya_agent_identities(owner_address);
CREATE INDEX idx_kya_identities_created ON kya_agent_identities(created_at DESC);

-- Reputation Scores (Domain-Specific)
CREATE TABLE IF NOT EXISTS kya_reputation_scores (
    id BIGSERIAL PRIMARY KEY,
    agent_did TEXT NOT NULL REFERENCES kya_agent_identities(did) ON DELETE CASCADE,
    domain TEXT NOT NULL,
    score DOUBLE PRECISION NOT NULL DEFAULT 50.0 CHECK (score >= 0 AND score <= 100),
    total_interactions BIGINT NOT NULL DEFAULT 0,
    successful_interactions BIGINT NOT NULL DEFAULT 0,
    failed_interactions BIGINT NOT NULL DEFAULT 0,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(agent_did, domain)
);

CREATE INDEX idx_kya_reputation_agent ON kya_reputation_scores(agent_did);
CREATE INDEX idx_kya_reputation_domain ON kya_reputation_scores(domain);
CREATE INDEX idx_kya_reputation_score ON kya_reputation_scores(score DESC);

-- Feedback Authorization Tokens (Sybil Resistance)
CREATE TABLE IF NOT EXISTS kya_feedback_tokens (
    id UUID PRIMARY KEY,
    agent_did TEXT NOT NULL REFERENCES kya_agent_identities(did) ON DELETE CASCADE,
    client_did TEXT NOT NULL,
    interaction_id UUID NOT NULL,
    domain TEXT NOT NULL,
    authorized_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    used BOOLEAN NOT NULL DEFAULT FALSE,
    signature TEXT NOT NULL,
    UNIQUE(interaction_id, client_did)
);

CREATE INDEX idx_kya_feedback_agent ON kya_feedback_tokens(agent_did);
CREATE INDEX idx_kya_feedback_client ON kya_feedback_tokens(client_did);
CREATE INDEX idx_kya_feedback_interaction ON kya_feedback_tokens(interaction_id);
CREATE INDEX idx_kya_feedback_used ON kya_feedback_tokens(used) WHERE NOT used;

-- Attestations (Cryptographically Signed Performance Records)
CREATE TABLE IF NOT EXISTS kya_attestations (
    id UUID PRIMARY KEY,
    agent_did TEXT NOT NULL REFERENCES kya_agent_identities(did) ON DELETE CASCADE,
    issuer_did TEXT NOT NULL,
    domain TEXT NOT NULL,
    claim TEXT NOT NULL,
    evidence_uri TEXT,
    signature TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    CHECK (expires_at IS NULL OR expires_at > issued_at)
);

CREATE INDEX idx_kya_attestations_agent ON kya_attestations(agent_did);
CREATE INDEX idx_kya_attestations_issuer ON kya_attestations(issuer_did);
CREATE INDEX idx_kya_attestations_domain ON kya_attestations(domain);
CREATE INDEX idx_kya_attestations_issued ON kya_attestations(issued_at DESC);
CREATE INDEX idx_kya_attestations_active ON kya_attestations(agent_did, domain) 
    WHERE expires_at IS NULL OR expires_at > NOW();

-- Competence Proofs (Zero-Knowledge Proofs)
CREATE TABLE IF NOT EXISTS kya_competence_proofs (
    id UUID PRIMARY KEY,
    agent_did TEXT NOT NULL REFERENCES kya_agent_identities(did) ON DELETE CASCADE,
    domain TEXT NOT NULL,
    claim TEXT NOT NULL,
    proof BYTEA NOT NULL,
    public_inputs BYTEA NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_kya_proofs_agent ON kya_competence_proofs(agent_did);
CREATE INDEX idx_kya_proofs_domain ON kya_competence_proofs(domain);
CREATE INDEX idx_kya_proofs_verified ON kya_competence_proofs(verified);
CREATE INDEX idx_kya_proofs_created ON kya_competence_proofs(created_at DESC);

-- Cross-Platform Reputation Sync
CREATE TABLE IF NOT EXISTS kya_cross_platform_reputation (
    id BIGSERIAL PRIMARY KEY,
    agent_did TEXT NOT NULL REFERENCES kya_agent_identities(did) ON DELETE CASCADE,
    source_platform TEXT NOT NULL,
    target_platform TEXT NOT NULL,
    reputation_hash TEXT NOT NULL,
    verification_proof BYTEA NOT NULL,
    synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(agent_did, source_platform, target_platform)
);

CREATE INDEX idx_kya_cross_platform_agent ON kya_cross_platform_reputation(agent_did);
CREATE INDEX idx_kya_cross_platform_source ON kya_cross_platform_reputation(source_platform);
CREATE INDEX idx_kya_cross_platform_synced ON kya_cross_platform_reputation(synced_at DESC);

-- Interaction History (Audit Trail)
CREATE TABLE IF NOT EXISTS kya_interaction_history (
    id UUID PRIMARY KEY,
    agent_did TEXT NOT NULL REFERENCES kya_agent_identities(did) ON DELETE CASCADE,
    client_did TEXT NOT NULL,
    domain TEXT NOT NULL,
    interaction_type TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_kya_history_agent ON kya_interaction_history(agent_did);
CREATE INDEX idx_kya_history_client ON kya_interaction_history(client_did);
CREATE INDEX idx_kya_history_domain ON kya_interaction_history(domain);
CREATE INDEX idx_kya_history_created ON kya_interaction_history(created_at DESC);

-- Views for Analytics

-- Agent Reputation Summary
CREATE OR REPLACE VIEW kya_agent_reputation_summary AS
SELECT 
    ai.did,
    ai.name,
    ai.owner_address,
    COUNT(DISTINCT rs.domain) as active_domains,
    AVG(rs.score) as avg_score,
    SUM(rs.total_interactions) as total_interactions,
    SUM(rs.successful_interactions) as total_successful,
    COUNT(DISTINCT a.id) as attestation_count,
    COUNT(DISTINCT cp.id) FILTER (WHERE cp.verified) as verified_proof_count,
    ai.created_at
FROM kya_agent_identities ai
LEFT JOIN kya_reputation_scores rs ON ai.did = rs.agent_did
LEFT JOIN kya_attestations a ON ai.did = a.agent_did 
    AND (a.expires_at IS NULL OR a.expires_at > NOW())
LEFT JOIN kya_competence_proofs cp ON ai.did = cp.agent_did
GROUP BY ai.did, ai.name, ai.owner_address, ai.created_at;

-- Domain Leaderboard
CREATE OR REPLACE VIEW kya_domain_leaderboard AS
SELECT 
    domain,
    agent_did,
    score,
    total_interactions,
    successful_interactions,
    RANK() OVER (PARTITION BY domain ORDER BY score DESC) as rank,
    PERCENT_RANK() OVER (PARTITION BY domain ORDER BY score DESC) as percentile
FROM kya_reputation_scores
WHERE total_interactions > 0
ORDER BY domain, score DESC;

-- Functions

-- Update reputation score timestamp
CREATE OR REPLACE FUNCTION update_kya_reputation_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.last_updated = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_kya_reputation_timestamp
BEFORE UPDATE ON kya_reputation_scores
FOR EACH ROW
EXECUTE FUNCTION update_kya_reputation_timestamp();

-- Update agent profile timestamp
CREATE OR REPLACE FUNCTION update_kya_agent_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_kya_agent_timestamp
BEFORE UPDATE ON kya_agent_identities
FOR EACH ROW
EXECUTE FUNCTION update_kya_agent_timestamp();

-- Comments
COMMENT ON TABLE kya_agent_identities IS 'W3C DID-based agent identity registry';
COMMENT ON TABLE kya_reputation_scores IS 'Domain-specific reputation scores with interaction history';
COMMENT ON TABLE kya_feedback_tokens IS 'Sybil-resistant feedback authorization tokens';
COMMENT ON TABLE kya_attestations IS 'Cryptographically signed performance attestations';
COMMENT ON TABLE kya_competence_proofs IS 'Zero-knowledge proofs of agent competence';
COMMENT ON TABLE kya_cross_platform_reputation IS 'Cross-platform reputation synchronization';
COMMENT ON TABLE kya_interaction_history IS 'Complete audit trail of agent interactions';
