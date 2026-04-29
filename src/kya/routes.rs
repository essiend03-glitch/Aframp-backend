use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::kya::{
    error::KYAError,
    identity::AgentIdentity,
    models::*,
    registry::KYARegistry,
};

pub fn kya_routes() -> Router<PgPool> {
    Router::new()
        .route("/agents", axum::routing::post(register_agent))
        .route("/agents", axum::routing::get(list_agents))
        .route("/agents/:did", axum::routing::get(get_agent))
        .route("/agents/:did/profile", axum::routing::put(update_profile))
        .route("/agents/:did/reputation", axum::routing::get(get_reputation))
        .route("/agents/:did/reputation/:domain", axum::routing::get(get_domain_reputation))
        .route("/agents/:did/scores", axum::routing::get(get_all_scores))
        .route("/agents/:did/ranking/:domain", axum::routing::get(get_ranking))
        .route("/interactions", axum::routing::post(record_interaction))
        .route("/feedback/tokens", axum::routing::post(issue_feedback_token))
        .route("/feedback/submit", axum::routing::post(submit_feedback))
        .route("/attestations", axum::routing::post(create_attestation))
        .route("/attestations/:did", axum::routing::get(get_attestations))
        .route("/proofs", axum::routing::post(store_proof))
        .route("/proofs/:did", axum::routing::get(get_proofs))
        .route("/cross-platform/sync", axum::routing::post(sync_reputation))
        .route("/cross-platform/:did", axum::routing::get(get_cross_platform))
}

#[derive(Deserialize)]
struct RegisterAgentRequest {
    method: String,
    network: String,
    name: String,
    owner_address: String,
}

#[derive(Serialize)]
struct RegisterAgentResponse {
    did: String,
    public_key: String,
}

async fn register_agent(
    State(pool): State<PgPool>,
    Json(req): Json<RegisterAgentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let identity = AgentIdentity::new(&req.method, &req.network, req.name, req.owner_address)?;
    let registry = KYARegistry::new(pool);
    
    registry.register_agent(&identity).await?;
    
    Ok(Json(RegisterAgentResponse {
        did: identity.profile.did.to_string(),
        public_key: identity.profile.public_key.clone(),
    }))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

async fn list_agents(
    State(pool): State<PgPool>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let registry = KYARegistry::new(pool);
    let agents = registry.list_agents(query.limit, query.offset).await?;
    
    let profiles: Vec<AgentProfile> = agents.iter().map(|a| a.export_profile()).collect();
    Ok(Json(profiles))
}

async fn get_agent(
    State(pool): State<PgPool>,
    Path(did_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let registry = KYARegistry::new(pool);
    let profile = registry.get_full_agent_profile(&did).await?;
    
    Ok(Json(profile))
}

async fn update_profile(
    State(pool): State<PgPool>,
    Path(did_str): Path<String>,
    Json(profile): Json<AgentProfile>,
) -> Result<impl IntoResponse, AppError> {
    let registry = KYARegistry::new(pool);
    registry.update_agent_profile(&profile).await?;
    
    Ok(StatusCode::OK)
}

async fn get_reputation(
    State(pool): State<PgPool>,
    Path(did_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let registry = KYARegistry::new(pool);
    let reputations = registry.get_all_reputations(&did).await?;
    
    Ok(Json(reputations))
}

async fn get_domain_reputation(
    State(pool): State<PgPool>,
    Path((did_str, domain_str)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let domain = parse_domain(&domain_str)?;
    let registry = KYARegistry::new(pool);
    let reputation = registry.get_reputation(&did, &domain).await?;
    
    Ok(Json(reputation))
}

async fn get_all_scores(
    State(pool): State<PgPool>,
    Path(did_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let registry = KYARegistry::new(pool);
    let scores = registry.get_all_scores(&did).await?;
    
    Ok(Json(scores))
}

async fn get_ranking(
    State(pool): State<PgPool>,
    Path((did_str, domain_str)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let domain = parse_domain(&domain_str)?;
    let registry = KYARegistry::new(pool);
    let ranking = registry.get_ranking(&did, &domain).await?;
    
    Ok(Json(ranking))
}

#[derive(Deserialize)]
struct RecordInteractionRequest {
    agent_did: String,
    domain: String,
    success: bool,
    weight: f64,
}

async fn record_interaction(
    State(pool): State<PgPool>,
    Json(req): Json<RecordInteractionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&req.agent_did)?;
    let domain = parse_domain(&req.domain)?;
    let registry = KYARegistry::new(pool);
    
    registry.record_interaction(&did, &domain, req.success, req.weight).await?;
    
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct IssueFeedbackTokenRequest {
    agent_did: String,
    client_did: String,
    interaction_id: Uuid,
    domain: String,
    signature: String,
}

async fn issue_feedback_token(
    State(pool): State<PgPool>,
    Json(req): Json<IssueFeedbackTokenRequest>,
) -> Result<impl IntoResponse, AppError> {
    let agent_did = DID::from_string(&req.agent_did)?;
    let client_did = DID::from_string(&req.client_did)?;
    let domain = parse_domain(&req.domain)?;
    let registry = KYARegistry::new(pool);
    
    let token = registry.issue_feedback_token(
        &agent_did,
        &client_did,
        req.interaction_id,
        &domain,
        req.signature,
    ).await?;
    
    Ok(Json(token))
}

#[derive(Deserialize)]
struct SubmitFeedbackRequest {
    token_id: Uuid,
    client_did: String,
    success: bool,
    weight: f64,
}

async fn submit_feedback(
    State(pool): State<PgPool>,
    Json(req): Json<SubmitFeedbackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let client_did = DID::from_string(&req.client_did)?;
    let registry = KYARegistry::new(pool);
    
    registry.submit_feedback(req.token_id, &client_did, req.success, req.weight).await?;
    
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct CreateAttestationRequest {
    agent_did: String,
    issuer_did: String,
    domain: String,
    claim: String,
    evidence_uri: Option<String>,
    signature: String,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn create_attestation(
    State(pool): State<PgPool>,
    Json(req): Json<CreateAttestationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let agent_did = DID::from_string(&req.agent_did)?;
    let issuer_did = DID::from_string(&req.issuer_did)?;
    let domain = parse_domain(&req.domain)?;
    let registry = KYARegistry::new(pool);
    
    let attestation = registry.create_attestation(
        &agent_did,
        &issuer_did,
        &domain,
        req.claim,
        req.evidence_uri,
        req.signature,
        req.expires_at,
    ).await?;
    
    Ok(Json(attestation))
}

async fn get_attestations(
    State(pool): State<PgPool>,
    Path(did_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let registry = KYARegistry::new(pool);
    let attestations = registry.get_attestations(&did).await?;
    
    Ok(Json(attestations))
}

#[derive(Deserialize)]
struct StoreProofRequest {
    agent_did: String,
    domain: String,
    claim: String,
    proof: String,  // hex-encoded
    public_inputs: String,  // hex-encoded
}

async fn store_proof(
    State(pool): State<PgPool>,
    Json(req): Json<StoreProofRequest>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&req.agent_did)?;
    let domain = parse_domain(&req.domain)?;
    let proof = hex::decode(&req.proof).map_err(|_| KYAError::CryptoError("Invalid proof hex".to_string()))?;
    let public_inputs = hex::decode(&req.public_inputs).map_err(|_| KYAError::CryptoError("Invalid public inputs hex".to_string()))?;
    
    let registry = KYARegistry::new(pool);
    let proof_record = registry.store_competence_proof(&did, &domain, req.claim, proof, public_inputs).await?;
    
    Ok(Json(proof_record))
}

async fn get_proofs(
    State(pool): State<PgPool>,
    Path(did_str): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let registry = KYARegistry::new(pool);
    let proofs = registry.get_competence_proofs(&did).await?;
    
    Ok(Json(proofs))
}

#[derive(Deserialize)]
struct SyncReputationRequest {
    agent_did: String,
    source_platform: String,
    target_platform: String,
    reputation_hash: String,
    verification_proof: String,  // hex-encoded
}

async fn sync_reputation(
    State(pool): State<PgPool>,
    Json(req): Json<SyncReputationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&req.agent_did)?;
    let proof = hex::decode(&req.verification_proof).map_err(|_| KYAError::CryptoError("Invalid proof hex".to_string()))?;
    
    let registry = KYARegistry::new(pool);
    registry.sync_cross_platform_reputation(
        &did,
        req.source_platform,
        req.target_platform,
        req.reputation_hash,
        proof,
    ).await?;
    
    Ok(StatusCode::OK)
}

async fn get_cross_platform(
    State(pool): State<PgPool>,
    Path(did_str): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let did = DID::from_string(&did_str)?;
    let source_platform = query.get("source").ok_or(KYAError::InvalidDID("Missing source parameter".to_string()))?;
    
    let registry = KYARegistry::new(pool);
    let reputations = registry.get_cross_platform_reputation(&did, source_platform).await?;
    
    Ok(Json(reputations))
}

fn parse_domain(domain_str: &str) -> Result<ReputationDomain, KYAError> {
    match domain_str {
        "code_audit" => Ok(ReputationDomain::CodeAudit),
        "financial_analysis" => Ok(ReputationDomain::FinancialAnalysis),
        "content_creation" => Ok(ReputationDomain::ContentCreation),
        "data_processing" => Ok(ReputationDomain::DataProcessing),
        "smart_contract_execution" => Ok(ReputationDomain::SmartContractExecution),
        "payment_processing" => Ok(ReputationDomain::PaymentProcessing),
        custom => Ok(ReputationDomain::Custom(custom.to_string())),
    }
}

struct AppError(KYAError);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self.0 {
            KYAError::IdentityNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            KYAError::InvalidDID(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            KYAError::UnauthorizedFeedback => (StatusCode::UNAUTHORIZED, self.0.to_string()),
            KYAError::SybilAttackDetected => (StatusCode::FORBIDDEN, self.0.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
        };
        
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl From<KYAError> for AppError {
    fn from(err: KYAError) -> Self {
        AppError(err)
    }
}
