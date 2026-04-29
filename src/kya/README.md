# KYA Module - Know Your Agent

## Overview

The KYA (Know Your Agent) module provides a decentralized identity and reputation system for autonomous agents, enabling trustless collaboration through cryptographic verification and portable reputation scores.

## Module Structure

```
src/kya/
├── mod.rs              # Module exports and public API
├── error.rs            # Error types and conversions
├── models.rs           # Core data structures (DID, profiles, etc.)
├── identity.rs         # DID-based identity registry
├── reputation.rs       # Reputation management and feedback
├── attestation.rs      # Cryptographically signed attestations
├── zkp.rs              # Zero-knowledge competence proofs
├── scoring.rs          # Modular scoring engine
├── registry.rs         # Central coordination layer
├── routes.rs           # REST API endpoints
└── README.md           # This file
```

## Core Components

### 1. Identity Registry (`identity.rs`)

Manages W3C-compliant Decentralized Identifiers (DIDs) for agents.

```rust
use kya::identity::{AgentIdentity, IdentityRegistry};

// Create new agent identity
let identity = AgentIdentity::new(
    "stellar",
    "mainnet",
    "MyAgent".to_string(),
    "GXXXXXXX".to_string()
)?;

// Register with registry
let registry = IdentityRegistry::new(pool);
registry.register(&identity).await?;
```

**Features:**
- Ed25519 key pair generation
- DID creation and management
- Public profile with capabilities
- Service endpoint registration

### 2. Reputation Manager (`reputation.rs`)

Tracks domain-specific reputation scores and interaction history.

```rust
use kya::reputation::ReputationManager;
use kya::models::ReputationDomain;

let manager = ReputationManager::new(pool);

// Record successful interaction
manager.record_interaction(
    &agent_did,
    &ReputationDomain::CodeAudit,
    true,  // success
    1.0    // weight
).await?;

// Get reputation score
let score = manager.get_domain_score(&agent_did, &ReputationDomain::CodeAudit).await?;
```

**Features:**
- Domain-specific scoring (0-100)
- Interaction tracking
- Success/failure rates
- Weighted reputation updates

### 3. Feedback Authorization (`reputation.rs`)

Prevents Sybil attacks through one-time feedback tokens.

```rust
use kya::reputation::FeedbackAuthorization;

let feedback_auth = FeedbackAuthorization::new(pool);

// Issue token after verified interaction
let token = feedback_auth.issue_token(
    &agent_did,
    &client_did,
    interaction_id,
    &domain,
    signature
).await?;

// Verify and consume token (one-time use)
let consumed = feedback_auth.verify_and_consume(token.id, &client_did).await?;
```

**Features:**
- One-time use tokens
- Interaction binding
- Sybil attack prevention
- Cryptographic signatures

### 4. Attestations (`attestation.rs`)

Cryptographically signed performance records from third parties.

```rust
use kya::attestation::{Attestation, AttestationVerifier};

let attestation_mgr = Attestation::new(pool);

// Create attestation
let attestation = attestation_mgr.create(
    &agent_did,
    &issuer_did,
    &domain,
    "Successfully completed 1000 audits".to_string(),
    Some("https://evidence.example.com".to_string()),
    signature,
    None  // no expiration
).await?;

// Verify attestation signature
let verifier = AttestationVerifier::new(pool);
let is_valid = verifier.verify(&attestation).await?;
```

**Features:**
- Cryptographic signatures
- Evidence linking
- Expiration support
- Batch verification

### 5. Zero-Knowledge Proofs (`zkp.rs`)

Privacy-preserving competence validation.

```rust
use kya::zkp::{CompetenceProof, ZKProofVerifier};

let proof_gen = CompetenceProof::new(pool);

// Generate proof
let proof = proof_gen.generate_proof(
    &agent_did,
    &domain,
    "Task completed",
    private_data,
    public_inputs
)?;

// Store proof
let record = proof_gen.store_proof(
    &agent_did,
    &domain,
    "Proof claim".to_string(),
    proof,
    public_inputs
).await?;

// Verify proof
let verifier = ZKProofVerifier::new(pool);
let is_valid = verifier.verify_proof(&record, &expected_inputs)?;
```

**Features:**
- Privacy-preserving validation
- Public parameter verification
- Proof storage and retrieval
- Batch verification

### 6. Scoring Engine (`scoring.rs`)

Multi-factor reputation scoring with rankings.

```rust
use kya::scoring::{DomainScore, ModularScoring};

let scorer = DomainScore::new(pool);

// Get detailed score
let detailed = scorer.get_detailed_score(&agent_did, &domain).await?;
println!("Score: {:.2}/100", detailed.overall_score);
println!("Success Rate: {:.2}%", detailed.success_rate * 100.0);

// Get composite score across all domains
let modular = ModularScoring::new(pool);
let composite = modular.calculate_composite_score(&agent_did).await?;

// Get ranking
let ranking = modular.get_domain_ranking(&agent_did, &domain).await?;
println!("Rank: {} / {} ({}th percentile)", 
    ranking.rank, ranking.total_agents, ranking.percentile);
```

**Scoring Formula:**
- Success Rate: 0-40 points
- Volume Bonus: 0-20 points (logarithmic)
- Attestations: 0-20 points
- ZK Proofs: 0-20 points

### 7. KYA Registry (`registry.rs`)

Central coordination layer providing unified API.

```rust
use kya::registry::KYARegistry;

let registry = KYARegistry::new(pool);

// Register agent
registry.register_agent(&identity).await?;

// Record interaction
registry.record_interaction(&agent_did, &domain, true, 1.0).await?;

// Issue feedback token
let token = registry.issue_feedback_token(
    &agent_did, &client_did, interaction_id, &domain, signature
).await?;

// Create attestation
let attestation = registry.create_attestation(
    &agent_did, &issuer_did, &domain, claim, evidence, signature, expiry
).await?;

// Get full profile
let profile = registry.get_full_agent_profile(&agent_did).await?;
```

**Features:**
- Unified API for all operations
- Transaction coordination
- Cross-component integration
- Full profile aggregation

## Data Models

### DID (Decentralized Identifier)

```rust
pub struct DID {
    pub method: String,      // e.g., "stellar"
    pub network: String,     // e.g., "mainnet"
    pub identifier: String,  // unique agent ID
}

// Format: did:stellar:mainnet:abc123...
```

### Agent Profile

```rust
pub struct AgentProfile {
    pub did: DID,
    pub name: String,
    pub description: Option<String>,
    pub service_endpoints: Vec<ServiceEndpoint>,
    pub capabilities: Vec<Capability>,
    pub owner_address: String,
    pub public_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Reputation Domains

```rust
pub enum ReputationDomain {
    CodeAudit,
    FinancialAnalysis,
    ContentCreation,
    DataProcessing,
    SmartContractExecution,
    PaymentProcessing,
    Custom(String),
}
```

## API Routes

All routes are prefixed with `/kya`:

### Identity
- `POST /agents` - Register new agent
- `GET /agents` - List all agents
- `GET /agents/:did` - Get agent profile
- `PUT /agents/:did/profile` - Update profile

### Reputation
- `GET /agents/:did/reputation` - Get all scores
- `GET /agents/:did/reputation/:domain` - Get domain score
- `GET /agents/:did/scores` - Get detailed scores
- `GET /agents/:did/ranking/:domain` - Get ranking
- `POST /interactions` - Record interaction

### Feedback
- `POST /feedback/tokens` - Issue feedback token
- `POST /feedback/submit` - Submit feedback

### Attestations
- `POST /attestations` - Create attestation
- `GET /attestations/:did` - Get attestations

### Proofs
- `POST /proofs` - Store proof
- `GET /proofs/:did` - Get proofs

### Cross-Platform
- `POST /cross-platform/sync` - Sync reputation
- `GET /cross-platform/:did` - Get cross-platform data

## Error Handling

```rust
pub enum KYAError {
    IdentityNotFound(String),
    InvalidDID(String),
    SignatureVerificationFailed,
    AttestationVerificationFailed(String),
    ZKProofVerificationFailed,
    UnauthorizedFeedback,
    SybilAttackDetected,
    InvalidReputationScore,
    DomainNotSupported(String),
    DatabaseError(String),
    SerializationError(String),
    CryptoError(String),
    CrossPlatformVerificationFailed,
}
```

## Testing

Run integration tests:

```bash
cargo test --test kya_integration --features database
```

Test coverage includes:
- Agent registration and retrieval
- Reputation scoring
- Feedback token lifecycle
- Attestation verification
- ZK proof validation
- Cross-platform sync
- Full profile aggregation

## Security Considerations

1. **Private Keys**: Never stored in database, only in memory
2. **Signatures**: Ed25519 for all cryptographic operations
3. **Sybil Resistance**: One-time feedback tokens
4. **Privacy**: ZK proofs reveal competence without exposing data
5. **Audit Trail**: Complete interaction history
6. **Cross-Platform**: Verification proofs for trust portability

## Performance

- **Indexed Queries**: All common lookups optimized
- **Connection Pooling**: Efficient database connections
- **Async Operations**: Non-blocking I/O throughout
- **Batch Support**: Bulk operations where applicable

## Dependencies

- `sqlx` - Database operations
- `ed25519-dalek` - Cryptographic signatures
- `chrono` - Timestamp handling
- `uuid` - Unique identifiers
- `serde` - Serialization
- `axum` - HTTP routing

## Integration Example

```rust
use axum::Router;
use kya::routes::kya_routes;

let app = Router::new()
    .nest("/kya", kya_routes())
    .with_state(pool);
```

## Documentation

- **KYA_IMPLEMENTATION.md** - Complete technical documentation
- **KYA_QUICK_START.md** - Quick start guide
- **KYA_COMPLETION_SUMMARY.md** - Implementation summary
- Inline documentation in all source files

## Future Enhancements

1. Advanced ZK-SNARKs with arkworks/bellman
2. Reputation staking and slashing
3. Decentralized dispute resolution
4. ML-based anomaly detection
5. W3C Verifiable Credentials integration

## License

Part of the Aframp backend system.
