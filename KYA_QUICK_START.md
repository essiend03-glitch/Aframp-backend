# KYA Quick Start Guide

## What is KYA?

KYA (Know Your Agent) is a decentralized identity and reputation system for autonomous agents. It enables trustless collaboration by providing:

- **Sovereign Identity**: W3C DID-based agent identities
- **Portable Reputation**: Scores that travel across platforms
- **Sybil Resistance**: Cryptographic feedback authorization
- **Zero-Knowledge Proofs**: Privacy-preserving competence validation
- **Cross-Platform Trust**: Reputation synchronization across networks

## Installation

### 1. Database Setup

```bash
# Run the KYA schema migration
psql -U postgres -d your_database -f db/migrations/kya_schema.sql
```

### 2. Add to Your Application

The KYA module is already integrated into the codebase at `src/kya/`.

### 3. Configure Routes

```rust
use axum::Router;
use kya::routes::kya_routes;

let app = Router::new()
    .nest("/kya", kya_routes())
    .with_state(pool);
```

## Basic Usage

### Register an Agent

```bash
curl -X POST http://localhost:8080/kya/agents \
  -H "Content-Type: application/json" \
  -d '{
    "method": "stellar",
    "network": "mainnet",
    "name": "MyAgent",
    "owner_address": "GXXXXXXX..."
  }'
```

Response:
```json
{
  "did": "did:stellar:mainnet:abc123...",
  "public_key": "ed25519_public_key_hex"
}
```

### Record an Interaction

```bash
curl -X POST http://localhost:8080/kya/interactions \
  -H "Content-Type: application/json" \
  -d '{
    "agent_did": "did:stellar:mainnet:abc123",
    "domain": "code_audit",
    "success": true,
    "weight": 1.0
  }'
```

### Get Agent Reputation

```bash
curl http://localhost:8080/kya/agents/did:stellar:mainnet:abc123/reputation
```

Response:
```json
[
  {
    "domain": "code_audit",
    "score": 75.5,
    "total_interactions": 100,
    "successful_interactions": 95,
    "failed_interactions": 5,
    "last_updated": "2026-04-24T10:30:00Z"
  }
]
```

### Issue Feedback Token (Sybil Resistance)

```bash
curl -X POST http://localhost:8080/kya/feedback/tokens \
  -H "Content-Type: application/json" \
  -d '{
    "agent_did": "did:stellar:mainnet:abc123",
    "client_did": "did:stellar:mainnet:xyz789",
    "interaction_id": "uuid-here",
    "domain": "code_audit",
    "signature": "signature_hex"
  }'
```

### Submit Feedback

```bash
curl -X POST http://localhost:8080/kya/feedback/submit \
  -H "Content-Type: application/json" \
  -d '{
    "token_id": "token-uuid",
    "client_did": "did:stellar:mainnet:xyz789",
    "success": true,
    "weight": 1.0
  }'
```

### Create Attestation

```bash
curl -X POST http://localhost:8080/kya/attestations \
  -H "Content-Type: application/json" \
  -d '{
    "agent_did": "did:stellar:mainnet:abc123",
    "issuer_did": "did:stellar:mainnet:issuer",
    "domain": "code_audit",
    "claim": "Successfully completed 1000 audits",
    "evidence_uri": "https://evidence.example.com",
    "signature": "signature_hex"
  }'
```

## Reputation Domains

Available domains for scoring:

- `code_audit` - Code review and security auditing
- `financial_analysis` - Financial calculations
- `content_creation` - Content generation
- `data_processing` - Data transformation
- `smart_contract_execution` - Blockchain operations
- `payment_processing` - Payment handling
- Custom domains supported

## Scoring System

Reputation scores (0-100) are calculated from:

1. **Success Rate** (40 points max)
2. **Volume Bonus** (20 points max) - logarithmic
3. **Attestations** (20 points max)
4. **ZK Proofs** (20 points max)

## Security Features

✅ **Sybil Resistance**: One-time feedback tokens  
✅ **Cryptographic Verification**: Ed25519 signatures  
✅ **Privacy Protection**: Zero-knowledge proofs  
✅ **Cross-Platform**: Reputation portability  

## Code Examples

### Rust Integration

```rust
use kya::{AgentIdentity, KYARegistry, ReputationDomain};

// Create agent
let identity = AgentIdentity::new(
    "stellar", "mainnet", 
    "MyAgent".to_string(),
    "GXXXXXXX".to_string()
)?;

// Register
let registry = KYARegistry::new(pool);
registry.register_agent(&identity).await?;

// Record interaction
registry.record_interaction(
    &identity.profile.did,
    &ReputationDomain::CodeAudit,
    true,  // success
    1.0    // weight
).await?;

// Get reputation
let score = registry.get_composite_score(&identity.profile.did).await?;
println!("Trust Score: {:.2}/100", score);
```

## Testing

Run integration tests:

```bash
cargo test --test kya_integration --features database
```

## Documentation

For detailed documentation, see:
- `KYA_IMPLEMENTATION.md` - Complete implementation guide
- `src/kya/` - Source code with inline documentation
- `db/migrations/kya_schema.sql` - Database schema

## Support

For issues or questions:
1. Check the implementation documentation
2. Review the test suite for examples
3. Examine the API routes in `src/kya/routes.rs`

## Next Steps

1. ✅ Database migration completed
2. ✅ Module integrated into codebase
3. ✅ API endpoints available
4. 🔄 Configure routes in your application
5. 🔄 Test with sample agents
6. 🔄 Integrate with AI agent SDK

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│              KYA Registry                       │
│  (Central coordination of all components)      │
└─────────────────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        │             │             │
┌───────▼──────┐ ┌───▼────┐ ┌─────▼──────┐
│   Identity   │ │Reputation│ │Attestation│
│   Registry   │ │ Manager  │ │  System   │
└──────────────┘ └──────────┘ └────────────┘
        │             │             │
┌───────▼──────┐ ┌───▼────┐ ┌─────▼──────┐
│  Feedback    │ │   ZK   │ │  Scoring   │
│Authorization │ │ Proofs │ │  Engine    │
└──────────────┘ └────────┘ └────────────┘
```

## Status

✅ **COMPLETE** - All acceptance criteria met:
- DID-based identity registry
- On-chain reputation system
- Sybil-resistant feedback
- Zero-knowledge proofs
- Cross-platform support
- Modular domain scoring
- API endpoints
- Database schema
- Integration tests
- Documentation
