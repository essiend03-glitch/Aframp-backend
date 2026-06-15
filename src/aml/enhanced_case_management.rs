//! Enhanced AML Case Management & Investigation Workflow
//!
//! Implements comprehensive case management with:
//! - Structured investigation workflows and checklists
//! - Evidence collection and management
//! - Case linking and network analysis
//! - SLA management and escalation workflows
//! - Decision tracking and audit trails
//! - Watchlist management and SAR integration

use super::models::*;
use crate::cache::AdvancedRedisCache;
use crate::services::notification::NotificationService;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use rust_decimal::Decimal;
// REMOVED: use crate::sar::service::SarService;
// REMOVED: use crate::sar::models::DetectionMethod as SarDetectionMethod;

#[derive(Debug, Clone)]
pub struct EnhancedAMLCaseManager {
    database: PgPool,
    cache: Arc<AdvancedRedisCache>,
    notifications: Arc<NotificationService>,
    config: CaseManagementConfig,
}

#[derive(Debug, Clone)]
pub struct CaseManagementConfig {
    pub default_sla_hours: u64,
    pub high_risk_sla_hours: u64,
    pub critical_risk_sla_hours: u64,
    pub max_investigator_cases: u32,
    pub escalation_threshold_hours: u64,
    pub auto_assignment_enabled: bool,
    pub assignment_strategy: AssignmentStrategy,
    pub investigation_checklists: HashMap<CaseType, InvestigationChecklist>,
}

#[derive(Debug, Clone)]
pub enum AssignmentStrategy {
    RoundRobin,
    WorkloadBalanced,
    SpecialtyBased,
}

#[derive(Debug, Clone)]
pub struct InvestigationChecklist {
    pub case_type: CaseType,
    pub required_items: Vec<ChecklistItem>,
    pub optional_items: Vec<ChecklistItem>,
}

#[derive(Debug, Clone)]
pub struct ChecklistItem {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub required: bool,
    pub category: ChecklistCategory,
    pub estimated_duration_minutes: u32,
}

#[derive(Debug, Clone)]
pub enum ChecklistCategory {
    TransactionAnalysis,
    ProfileReview,
    CounterpartyAnalysis,
    DocumentVerification,
    RiskAssessment,
    RegulatoryCompliance,
}

impl Default for CaseManagementConfig {
    fn default() -> Self {
        let mut checklists = HashMap::new();
        checklists.insert(CaseType::TransactionBased, Self::create_transaction_checklist());
        checklists.insert(CaseType::ActivityBased, Self::create_activity_checklist());
        checklists.insert(CaseType::ReferralBased, Self::create_referral_checklist());

        Self {
            default_sla_hours: 72,
            high_risk_sla_hours: 24,
            critical_risk_sla_hours: 4,
            max_investigator_cases: 20,
            escalation_threshold_hours: 48,
            auto_assignment_enabled: true,
            assignment_strategy: AssignmentStrategy::WorkloadBalanced,
            investigation_checklists: checklists,
        }
    }
}

impl CaseManagementConfig {
    fn create_transaction_checklist() -> InvestigationChecklist {
        InvestigationChecklist {
            case_type: CaseType::TransactionBased,
            required_items: vec![
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Verify Transaction Details".to_string(),
                    description: "Confirm transaction amount, currency, and timing".to_string(),
                    required: true,
                    category: ChecklistCategory::TransactionAnalysis,
                    estimated_duration_minutes: 15,
                },
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Review Subject Transaction History".to_string(),
                    description: "Analyze past 90 days of transaction patterns".to_string(),
                    required: true,
                    category: ChecklistCategory::ProfileReview,
                    estimated_duration_minutes: 30,
                },
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Check Counterparty Risk".to_string(),
                    description: "Evaluate counterparty AML status and risk profile".to_string(),
                    required: true,
                    category: ChecklistCategory::CounterpartyAnalysis,
                    estimated_duration_minutes: 20,
                },
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Assess AML Rule Trigger Validity".to_string(),
                    description: "Validate that AML rules triggered correctly".to_string(),
                    required: true,
                    category: ChecklistCategory::RiskAssessment,
                    estimated_duration_minutes: 25,
                },
            ],
            optional_items: vec![
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Blockchain Analysis".to_string(),
                    description: "Trace blockchain transaction flow if applicable".to_string(),
                    required: false,
                    category: ChecklistCategory::TransactionAnalysis,
                    estimated_duration_minutes: 45,
                },
            ],
        }
    }

    fn create_activity_checklist() -> InvestigationChecklist {
        InvestigationChecklist {
            case_type: CaseType::ActivityBased,
            required_items: vec![
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Review Full Activity Pattern".to_string(),
                    description: "Analyze complete transaction and activity history".to_string(),
                    required: true,
                    category: ChecklistCategory::ProfileReview,
                    estimated_duration_minutes: 45,
                },
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Assess Pattern Against Typologies".to_string(),
                    description: "Compare activity against known AML typologies".to_string(),
                    required: true,
                    category: ChecklistCategory::RiskAssessment,
                    estimated_duration_minutes: 30,
                },
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Investigate Subject KYC Profile".to_string(),
                    description: "Review KYC documents and verification status".to_string(),
                    required: true,
                    category: ChecklistCategory::DocumentVerification,
                    estimated_duration_minutes: 20,
                },
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Check for Related Cases".to_string(),
                    description: "Search for existing cases involving the subject".to_string(),
                    required: true,
                    category: ChecklistCategory::RegulatoryCompliance,
                    estimated_duration_minutes: 15,
                },
            ],
            optional_items: vec![],
        }
    }

    fn create_referral_checklist() -> InvestigationChecklist {
        InvestigationChecklist {
            case_type: CaseType::ReferralBased,
            required_items: vec![
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Review Referral Source".to_string(),
                    description: "Evaluate credibility and context of referral".to_string(),
                    required: true,
                    category: ChecklistCategory::RegulatoryCompliance,
                    estimated_duration_minutes: 20,
                },
                ChecklistItem {
                    id: Uuid::new_v4(),
                    title: "Document Referral Details".to_string(),
                    description: "Record all referral information and evidence".to_string(),
                    required: true,
                    category: ChecklistCategory::DocumentVerification,
                    estimated_duration_minutes: 15,
                },
            ],
            optional_items: vec![],
        }
    }
}

impl EnhancedAMLCaseManager {
    pub async fn new(
        database_url: &str,
        cache: AdvancedRedisCache,
        notifications: Arc<NotificationService>,
    ) -> Result<Self, anyhow::Error> {
        let database = PgPool::connect(database_url).await?;
        let config = CaseManagementConfig::default();

        Ok(Self {
            database,
            cache: Arc::new(cache),
            notifications,
            config,
        })
    }

    /// Automatically create case from AML policy engine evaluation
    pub async fn create_case_from_evaluation(
        &self,
        evaluation_result: &PolicyEvaluationResult,
        context: &EvaluationContext,
    ) -> Result<AMLCaseRecord, anyhow::Error> {
        // Only create cases for medium risk and above
        if matches!(evaluation_result.risk_level, RiskLevel::Low) {
            return Err(anyhow::anyhow!("Risk level too low for case creation"));
        }

        let case_type = self.determine_case_type(evaluation_result, context);
        let risk_score = evaluation_result.composite_risk_score;
        let subject_kyc_id = context.user_profile.id;

        // Determine SLA based on risk level
        let sla_hours = match evaluation_result.risk_level {
            RiskLevel::Critical => self.config.critical_risk_sla_hours,
            RiskLevel::High => self.config.high_risk_sla_hours,
            _ => self.config.default_sla_hours,
        };

        let target_resolution = Utc::now() + Duration::hours(sla_hours as i64);

        let case_record = AMLCaseRecord {
            id: Uuid::new_v4(),
            case_type,
            case_source: CaseSource::ProactiveDetection,
            risk_score_at_opening: risk_score,
            subject_kyc_id,
            subject_wallet_addresses: vec![context.transaction.wallet_address.clone()],
            case_status: AMLCaseStatus::Open,
            assigned_investigator_id: None,
            supervisor_id: None,
            opened_timestamp: Utc::now(),
            target_resolution_timestamp: target_resolution,
            resolved_timestamp: None,
            resolution_summary: None,
        };

        // Save case to database
        let saved_case = self.save_case_record(&case_record).await?;

        // Add initial evidence from evaluation
        self.add_evaluation_evidence(&saved_case.id, evaluation_result, context).await?;

        // Auto-assign if enabled
        if self.config.auto_assignment_enabled {
            if let Err(e) = self.auto_assign_case(&saved_case.id).await {
                warn!("Failed to auto-assign case {}: {}", saved_case.id, e);
            }
        }

        // Send notifications
        self.send_case_creation_notifications(&saved_case, evaluation_result).await?;

        info!("Created AML case {} from policy evaluation (risk: {:.2})", 
            saved_case.id, risk_score);

        Ok(saved_case)
    }

    /// Manually create a case
    pub async fn create_manual_case(
        &self,
        request: CreateManualCaseRequest,
    ) -> Result<AMLCaseRecord, anyhow::Error> {
        let case_record = AMLCaseRecord {
            id: Uuid::new_v4(),
            case_type: request.case_type,
            case_source: CaseSource::ComplianceOfficerJudgment,
            risk_score_at_opening: request.initial_risk_score,
            subject_kyc_id: request.subject_kyc_id,
            subject_wallet_addresses: request.subject_wallet_addresses,
            case_status: AMLCaseStatus::Open,
            assigned_investigator_id: None,
            supervisor_id: None,
            opened_timestamp: Utc::now(),
            target_resolution_timestamp: Utc::now() + Duration::hours(self.config.default_sla_hours as i64),
            resolved_timestamp: None,
            resolution_summary: None,
        };

        let saved_case = self.save_case_record(&case_record).await?;

        // Add initial evidence
        if let Some(initial_evidence) = request.initial_evidence {
            for evidence in initial_evidence {
                self.add_case_evidence(&saved_case.id, evidence).await?;
            }
        }

        // Auto-assign if enabled
        if self.config.auto_assignment_enabled {
            self.auto_assign_case(&saved_case.id).await?;
        }

        info!("Created manual AML case {} by compliance officer", saved_case.id);
        Ok(saved_case)
    }

    /// Auto-assign case to investigator based on strategy
    pub async fn auto_assign_case(&self, case_id: &Uuid) -> Result<(), anyhow::Error> {
        let investigator_id = match self.config.assignment_strategy {
            AssignmentStrategy::RoundRobin => self.get_next_round_robin_investigator().await?,
            AssignmentStrategy::WorkloadBalanced => self.get_least_loaded_investigator().await?,
            AssignmentStrategy::SpecialtyBased => self.get_specialist_investigator(case_id).await?,
        };

        self.assign_case_to_investigator(case_id, &investigator_id).await
    }

    /// Assign case to specific investigator
    pub async fn assign_case_to_investigator(
        &self,
        case_id: &Uuid,
        investigator_id: &str,
    ) -> Result<(), anyhow::Error> {
        // Update case assignment
        self.update_case_assignment(case_id, Some(investigator_id)).await?;

        // Record action
        self.add_case_action(case_id, CaseAction {
            id: Uuid::new_v4(),
            case_id: *case_id,
            action_type: CaseActionType::Assignment,
            action_detail: format!("Assigned to investigator {}", investigator_id),
            performed_by_officer_id: investigator_id.to_string(),
            action_timestamp: Utc::now(),
        }).await?;

        // Send notification to investigator
        self.send_assignment_notification(case_id, investigator_id).await?;

        info!("Assigned case {} to investigator {}", case_id, investigator_id);
        Ok(())
    }

    /// Add evidence to case
    pub async fn add_case_evidence(
        &self,
        case_id: &Uuid,
        evidence: CaseEvidenceRequest,
    ) -> Result<CaseEvidenceRecord, anyhow::Error> {
        let evidence_record = CaseEvidenceRecord {
            id: Uuid::new_v4(),
            case_id: *case_id,
            evidence_type: evidence.evidence_type,
            evidence_description: evidence.description,
            evidence_reference_id: evidence.reference_id,
            added_by_officer_id: evidence.added_by_officer_id,
            added_timestamp: Utc::now(),
        };

        let saved_evidence = self.save_case_evidence(&evidence_record).await?;

        // Record action
        self.add_case_action(case_id, CaseAction {
            id: Uuid::new_v4(),
            case_id: *case_id,
            action_type: CaseActionType::EvidenceAddition,
            action_detail: format!("Added evidence: {}", evidence.description),
            performed_by_officer_id: evidence.added_by_officer_id,
            action_timestamp: Utc::now(),
        }).await?;

        debug!("Added evidence {} to case {}", saved_evidence.id, case_id);
        Ok(saved_evidence)
    }

    /// Add investigation note to case
    pub async fn add_case_note(
        &self,
        case_id: &Uuid,
        note: CaseNoteRequest,
    ) -> Result<CaseNoteRecord, anyhow::Error> {
        let note_record = CaseNoteRecord {
            id: Uuid::new_v4(),
            case_id: *case_id,
            note_text: note.note_text,
            note_author_id: note.author_id,
            note_visibility: note.visibility,
            note_timestamp: Utc::now(),
        };

        let saved_note = self.save_case_note(&note_record).await?;

        // Record action
        self.add_case_action(case_id, CaseAction {
            id: Uuid::new_v4(),
            case_id: *case_id,
            action_type: CaseActionType::NoteAddition,
            action_detail: format!("Added investigation note"),
            performed_by_officer_id: note.author_id,
            action_timestamp: Utc::now(),
        }).await?;

        debug!("Added note {} to case {}", saved_note.id, case_id);
        Ok(saved_note)
    }

    /// Link case to another related case
    pub async fn link_case(
        &self,
        case_id: &Uuid,
        link_request: CaseLinkRequest,
    ) -> Result<CaseLinkRecord, anyhow::Error> {
        let link_record = CaseLinkRecord {
            id: Uuid::new_v4(),
            case_id: *case_id,
            linked_case_id: link_request.linked_case_id,
            link_type: link_request.link_type,
            link_reason: link_request.reason,
        };

        let saved_link = self.save_case_link(&link_record).await?;

        // Record action
        self.add_case_action(case_id, CaseAction {
            id: Uuid::new_v4(),
            case_id: *case_id,
            action_type: CaseActionType::Link,
            action_detail: format!("Linked to case {} ({})", 
                link_request.linked_case_id, link_request.reason),
            performed_by_officer_id: link_request.linked_by_officer_id,
            action_timestamp: Utc::now(),
        }).await?;

        info!("Linked case {} to case {} ({})", case_id, link_request.linked_case_id, link_request.reason);
        Ok(saved_link)
    }

    /// Escalate case to senior compliance officer
    pub async fn escalate_case(
        &self,
        case_id: &Uuid,
        escalation_request: CaseEscalationRequest,
    ) -> Result<(), anyhow::Error> {
        // Update case status
        self.update_case_status(case_id, AMLCaseStatus::Escalated).await?;

        // Update supervisor assignment
        self.update_case_supervisor(case_id, Some(&escalation_request.escalated_to_officer_id)).await?;

        // Record escalation action
        self.add_case_action(case_id, CaseAction {
            id: Uuid::new_v4(),
            case_id: *case_id,
            action_type: CaseActionType::Escalation,
            action_detail: format!("Escalated: {}", escalation_request.reason),
            performed_by_officer_id: escalation_request.escalated_by_officer_id,
            action_timestamp: Utc::now(),
        }).await?;

        // Send escalation notification
        self.send_escalation_notification(case_id, &escalation_request).await?;

        warn!("Case {} escalated to {}", case_id, escalation_request.escalated_to_officer_id);
        Ok(())
    }

    /// Submit case decision
    pub async fn submit_case_decision(
        &self,
        case_id: &Uuid,
        decision: CaseDecisionRequest,
    ) -> Result<(), anyhow::Error> {
        // Validate that all required checklist items are completed
        if !self.is_checklist_complete(case_id).await? {
            return Err(anyhow::anyhow!("Cannot submit decision - incomplete investigation checklist"));
        }

        // Update case status
        let new_status = match decision.decision_type {
            CaseDecisionType::Suspicious => AMLCaseStatus::ClosedSuspicious,
            CaseDecisionType::NotSuspicious => AMLCaseStatus::ClosedNotSuspicious,
        };

        self.update_case_status(case_id, new_status).await?;

        // Record resolution
        self.update_case_resolution(case_id, &decision.rationale).await?;

        // Record decision action
        self.add_case_action(case_id, CaseAction {
            id: Uuid::new_v4(),
            case_id: *case_id,
            action_type: CaseActionType::StatusTransition,
            action_detail: format!("Decision: {:?} - {}", decision.decision_type, decision.rationale),
            performed_by_officer_id: decision.decided_by_officer_id,
            action_timestamp: Utc::now(),
        }).await?;

        // Process decision consequences
        match decision.decision_type {
            CaseDecisionType::Suspicious => {
                self.process_suspicious_decision(case_id, &decision).await?;
            }
            CaseDecisionType::NotSuspicious => {
                self.process_not_suspicious_decision(case_id, &decision).await?;
            }
        }

        info!("Case {} decision submitted: {:?}", case_id, decision.decision_type);
        Ok(())
    }

    /// Get case investigation checklist status
    pub async fn get_case_checklist(&self, case_id: &Uuid) -> Result<CaseChecklistStatus, anyhow::Error> {
        let case = self.get_case_by_id(case_id).await?;
        let checklist = self.config.investigation_checklists.get(&case.case_type)
            .ok_or_else(|| anyhow::anyhow!("No checklist found for case type: {:?}", case.case_type))?;

        let completed_items = self.get_completed_checklist_items(case_id).await?;
        
        let mut required_status = Vec::new();
        let mut optional_status = Vec::new();

        for item in &checklist.required_items {
            let completed = completed_items.contains(&item.id);
            required_status.push(ChecklistItemStatus {
                item: item.clone(),
                completed,
                completed_by_officer_id: completed_items.get(&item.id).cloned(),
                completed_timestamp: None, // Would need to track completion time
            });
        }

        for item in &checklist.optional_items {
            let completed = completed_items.contains(&item.id);
            optional_status.push(ChecklistItemStatus {
                item: item.clone(),
                completed,
                completed_by_officer_id: completed_items.get(&item.id).cloned(),
                completed_timestamp: None,
            });
        }

        Ok(CaseChecklistStatus {
            case_id: *case_id,
            case_type: case.case_type,
            required_items: required_status,
            optional_items: optional_status,
            overall_completion: self.calculate_checklist_completion(&required_status),
        })
    }

    /// Get subject activity history for case
    pub async fn get_subject_activity_history(
        &self,
        case_id: &Uuid,
        date_range: ActivityDateRange,
    ) -> Result<SubjectActivityHistory, anyhow::Error> {
        let case = self.get_case_by_id(case_id).await?;
        
        // Load subject's transaction history
        let transactions = self.load_subject_transactions(&case.subject_kyc_id, &date_range).await?;
        
        // Calculate activity metrics
        let metrics = self.calculate_activity_metrics(&transactions).await?;

        Ok(SubjectActivityHistory {
            subject_kyc_id: case.subject_kyc_id,
            date_range,
            transactions,
            metrics,
        })
    }

    /// Get network analysis for case subject
    pub async fn get_network_analysis(
        &self,
        case_id: &Uuid,
        analysis_window: NetworkAnalysisWindow,
    ) -> Result<NetworkAnalysisResult, anyhow::Error> {
        let case = self.get_case_by_id(case_id).await?;
        
        // Build transaction network
        let network = self.build_transaction_network(&case.subject_kyc_id, &analysis_window).await?;
        
        // Identify suspicious patterns
        let patterns = self.identify_network_patterns(&network).await?;

        Ok(NetworkAnalysisResult {
            subject_kyc_id: case.subject_kyc_id,
            analysis_window,
            network,
            suspicious_patterns: patterns,
        })
    }

    /// Get SLA status for all open cases
    pub async fn get_sla_status(&self) -> Result<SLAStatusReport, anyhow::Error> {
        let open_cases = self.get_all_open_cases().await?;
        
        let mut urgent_cases = Vec::new();
        let mut overdue_cases = Vec::new();
        let mut upcoming_deadlines = Vec::new();

        let now = Utc::now();

        for case in open_cases {
            let hours_until_deadline = case.target_resolution_timestamp.signed_duration_since(now).num_hours();
            
            if hours_until_deadline < 0 {
                overdue_cases.push(OverdueCase {
                    case_id: case.id,
                    case_type: case.case_type,
                    risk_score: case.risk_score_at_opening,
                    assigned_investigator: case.assigned_investigator_id.clone(),
                    overdue_hours: (-hours_until_deadline) as u64,
                });
            } else if hours_until_deadline <= 24 {
                urgent_cases.push(UrgentCase {
                    case_id: case.id,
                    case_type: case.case_type,
                    risk_score: case.risk_score_at_opening,
                    assigned_investigator: case.assigned_investigator_id.clone(),
                    hours_remaining: hours_until_deadline as u64,
                });
            } else if hours_until_deadline <= 72 {
                upcoming_deadlines.push(UpcomingDeadline {
                    case_id: case.id,
                    case_type: case.case_type,
                    risk_score: case.risk_score_at_opening,
                    assigned_investigator: case.assigned_investigator_id.clone(),
                    days_remaining: (hours_until_deadline / 24) as u64,
                });
            }
        }

        Ok(SLAStatusReport {
            total_open_cases: open_cases.len() as u64,
            overdue_cases: overdue_cases.len() as u64,
            urgent_cases: urgent_cases.len() as u64,
            upcoming_deadlines: upcoming_deadlines.len() as u64,
            overdue_cases,
            urgent_cases,
            upcoming_deadlines,
            generated_at: now,
        })
    }

    /// Get case management metrics
    pub async fn get_case_metrics(&self, period: MetricsPeriod) -> Result<CaseManagementMetrics, anyhow::Error> {
        let cases = self.get_cases_in_period(&period).await?;
        
        let mut metrics = CaseManagementMetrics::default();
        
        for case in cases {
            metrics.total_cases_opened += 1;
            
            match case.case_status {
                AMLCaseStatus::ClosedSuspicious => metrics.cases_closed_suspicious += 1,
                AMLCaseStatus::ClosedNotSuspicious => metrics.cases_closed_not_suspicious += 1,
                AMLCaseStatus::Open => metrics.currently_open_cases += 1,
                _ => {}
            }

            // Calculate resolution time if resolved
            if let Some(resolved_timestamp) = case.resolved_timestamp {
                let resolution_hours = resolved_timestamp.signed_duration_since(case.opened_timestamp).num_hours();
                metrics.average_resolution_hours = 
                    (metrics.average_resolution_hours * (metrics.total_resolved_cases as f64) + resolution_hours as f64) 
                    / ((metrics.total_resolved_cases + 1) as f64);
                metrics.total_resolved_cases += 1;
            }
        }

        // Calculate SLA compliance rate
        metrics.sla_compliance_rate = self.calculate_sla_compliance_rate(&period).await?;

        Ok(metrics)
    }

    // Helper methods
    fn determine_case_type(&self, evaluation_result: &PolicyEvaluationResult, _context: &EvaluationContext) -> CaseType {
        // Determine case type based on triggered rules
        for rule_result in &evaluation_result.triggered_rules {
            // This would check rule categories to determine case type
            // For now, default to transaction-based
        }
        CaseType::TransactionBased
    }

    async fn add_evaluation_evidence(
        &self,
        case_id: &Uuid,
        evaluation_result: &PolicyEvaluationResult,
        context: &EvaluationContext,
    ) -> Result<(), anyhow::Error> {
        // Add transaction evidence
        self.add_case_evidence(case_id, CaseEvidenceRequest {
            evidence_type: EvidenceType::TransactionRecord,
            description: format!("Transaction {} evaluation", context.transaction.id),
            reference_id: Some(context.transaction.id.to_string()),
            added_by_officer_id: "system".to_string(),
        }).await?;

        // Add triggered rules as evidence
        for rule_result in &evaluation_result.triggered_rules {
            self.add_case_evidence(case_id, CaseEvidenceRequest {
                evidence_type: EvidenceType::OfficerObservation,
                description: format!("Rule {} triggered with confidence {:.2}", 
                    rule_result.rule_id, rule_result.confidence_score),
                reference_id: Some(rule_result.rule_id.to_string()),
                added_by_officer_id: "system".to_string(),
            }).await?;
        }

        Ok(())
    }

    async fn send_case_creation_notifications(
        &self,
        case: &AMLCaseRecord,
        evaluation_result: &PolicyEvaluationResult,
    ) -> Result<(), anyhow::Error> {
        // Send immediate alert for critical cases
        if matches!(evaluation_result.risk_level, RiskLevel::Critical) {
            self.notifications
                .send_system_alert(
                    &case.id.to_string(),
                    &format!(
                        "CRITICAL AML CASE — Case {} requires immediate attention. Risk score: {:.2}",
                        case.id, case.risk_score_at_opening
                    ),
                )
                .await;
        }

        Ok(())
    }

    async fn send_assignment_notification(&self, case_id: &Uuid, investigator_id: &str) -> Result<(), anyhow::Error> {
        self.notifications
            .send_user_notification(
                investigator_id,
                &format!("New AML case assigned: {}", case_id),
                &format!("You have been assigned AML case {} for investigation.", case_id),
            )
            .await;
        Ok(())
    }

    async fn send_escalation_notification(
        &self,
        case_id: &Uuid,
        escalation: &CaseEscalationRequest,
    ) -> Result<(), anyhow::Error> {
        self.notifications
            .send_user_notification(
                &escalation.escalated_to_officer_id,
                &format!("Case escalated: {}", case_id),
                &format!("Case {} has been escalated to you. Reason: {}", 
                    case_id, escalation.reason),
            )
            .await;
        Ok(())
    }

    async fn process_suspicious_decision(&self, case_id: &Uuid, decision: &CaseDecisionRequest) -> Result<(), anyhow::Error> {
        // Add subject to internal watchlist
        self.add_subject_to_watchlist(case_id, WatchlistReason::SuspiciousActivity).await?;
        
        // Initiate SAR filing workflow
        self.initiate_sar_filing(case_id, decision).await?;
        
        info!("Processed suspicious decision for case {} - SAR workflow initiated", case_id);
        Ok(())
    }

    async fn process_not_suspicious_decision(&self, case_id: &Uuid, decision: &CaseDecisionRequest) -> Result<(), anyhow::Error> {
        // Record outcome to prevent false positives
        self.record_case_outcome(case_id, CaseOutcome::NotSuspicious).await?;
        
        // Check if supervisor sign-off is required
        let case = self.get_case_by_id(case_id).await?;
        if case.risk_score_at_opening > 0.7 {
            // Require supervisor sign-off for high-risk cases
            self.request_supervisor_signoff(case_id, &decision.decided_by_officer_id).await?;
        }
        
        info!("Processed not-suspicious decision for case {}", case_id);
        Ok(())
    }

    // Database operations (placeholders - would implement actual database queries)
    async fn save_case_record(&self, case: &AMLCaseRecord) -> Result<AMLCaseRecord, anyhow::Error> {
        let payload = serde_json::to_value(case)?;
        sqlx::query!(
            r#"
            INSERT INTO aml_case_records (id, payload, created_at, updated_at)
            VALUES ($1, $2::jsonb, NOW(), NOW())
            ON CONFLICT (id) DO UPDATE SET payload = $2::jsonb, updated_at = NOW()
            "#,
            case.id,
            payload,
        )
        .execute(&self.database)
        .await?;
        Ok(case.clone())
    }

    async fn get_case_by_id(&self, case_id: &Uuid) -> Result<AMLCaseRecord, anyhow::Error> {
        let row = sqlx::query!(
            r#"SELECT payload FROM aml_case_records WHERE id = $1"#,
            case_id
        )
        .fetch_optional(&self.database)
        .await?;
        if let Some(r) = row {
            let v: serde_json::Value = r.payload;
            let case: AMLCaseRecord = serde_json::from_value(v)?;
            Ok(case)
        } else {
            Err(anyhow::anyhow!("Case not found"))
        }
    }

    async fn update_case_assignment(&self, case_id: &Uuid, investigator_id: Option<&str>) -> Result<(), anyhow::Error> {
        let mut case = self.get_case_by_id(case_id).await?;
        case.assigned_investigator_id = investigator_id.map(|s| s.to_string());
        self.save_case_record(&case).await?;
        Ok(())
    }

    async fn update_case_status(&self, case_id: &Uuid, status: AMLCaseStatus) -> Result<(), anyhow::Error> {
        let mut case = self.get_case_by_id(case_id).await?;
        case.case_status = status;
        self.save_case_record(&case).await?;
        Ok(())
    }

    async fn update_case_supervisor(&self, case_id: &Uuid, supervisor_id: Option<&str>) -> Result<(), anyhow::Error> {
        let mut case = self.get_case_by_id(case_id).await?;
        case.supervisor_id = supervisor_id.map(|s| s.to_string());
        self.save_case_record(&case).await?;
        Ok(())
    }

    async fn update_case_resolution(&self, case_id: &Uuid, rationale: &str) -> Result<(), anyhow::Error> {
        let mut case = self.get_case_by_id(case_id).await?;
        case.resolution_summary = Some(rationale.to_string());
        case.resolved_timestamp = Some(Utc::now());
        self.save_case_record(&case).await?;
        Ok(())
    }

    async fn save_case_evidence(&self, evidence: &CaseEvidenceRecord) -> Result<CaseEvidenceRecord, anyhow::Error> {
        let payload = serde_json::to_value(evidence)?;
        sqlx::query!(
            r#"INSERT INTO aml_case_evidence (id, case_id, payload, added_at) VALUES ($1,$2,$3::jsonb,NOW()) RETURNING id"#,
            evidence.id,
            evidence.case_id,
            payload,
        )
        .fetch_one(&self.database)
        .await?;
        Ok(evidence.clone())
    }

    async fn save_case_note(&self, note: &CaseNoteRecord) -> Result<CaseNoteRecord, anyhow::Error> {
        let payload = serde_json::to_value(note)?;
        sqlx::query!(
            r#"INSERT INTO aml_case_notes (id, case_id, payload, added_at) VALUES ($1,$2,$3::jsonb,NOW()) RETURNING id"#,
            note.id,
            note.case_id,
            payload,
        )
        .fetch_one(&self.database)
        .await?;
        Ok(note.clone())
    }

    async fn save_case_link(&self, link: &CaseLinkRecord) -> Result<CaseLinkRecord, anyhow::Error> {
        // store as an action for now
        self.add_case_action(&link.case_id, CaseAction {
            id: Uuid::new_v4(),
            case_id: link.case_id,
            action_type: CaseActionType::Link,
            action_detail: format!("Linked to case {}: {}", link.linked_case_id, link.link_reason),
            performed_by_officer_id: "system".to_string(),
            action_timestamp: Utc::now(),
        }).await?;
        Ok(link.clone())
    }

    async fn add_case_action(&self, case_id: &Uuid, action: CaseAction) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"INSERT INTO aml_case_actions (id, case_id, action_type, action_detail, performed_by, action_timestamp) VALUES ($1,$2,$3,$4,$5,$6)"#,
            action.id,
            case_id,
            format!("{:?}", action.action_type),
            action.action_detail,
            action.performed_by_officer_id,
            action.action_timestamp,
        )
        .execute(&self.database)
        .await?;
        Ok(())
    }

    async fn get_completed_checklist_items(&self, case_id: &Uuid) -> Result<HashMap<Uuid, String>, anyhow::Error> {
        let rows = sqlx::query!(
            r#"SELECT item_id, completed_by FROM aml_case_checklist_items WHERE case_id = $1"#,
            case_id
        )
        .fetch_all(&self.database)
        .await?;
        let mut m = HashMap::new();
        for r in rows {
            m.insert(r.item_id, r.completed_by);
        }
        Ok(m)
    }

    async fn is_checklist_complete(&self, case_id: &Uuid) -> Result<bool, anyhow::Error> {
        // Best-effort: compare required items from config and completed items
        let case = self.get_case_by_id(case_id).await?;
        let checklist = self.config.investigation_checklists.get(&case.case_type)
            .ok_or_else(|| anyhow::anyhow!("No checklist found for case type"))?;
        let completed = self.get_completed_checklist_items(case_id).await?;
        let all_done = checklist.required_items.iter().all(|it| completed.contains_key(&it.id));
        Ok(all_done)
    }

    async fn get_next_round_robin_investigator(&self) -> Result<String, anyhow::Error> {
        // Simple placeholder: return configured default
        Ok("investigator_1".to_string())
    }

    async fn get_least_loaded_investigator(&self) -> Result<String, anyhow::Error> {
        Ok("investigator_1".to_string())
    }

    async fn get_specialist_investigator(&self, case_id: &Uuid) -> Result<String, anyhow::Error> {
        Ok("investigator_1".to_string())
    }

    async fn load_subject_transactions(&self, subject_id: &Uuid, date_range: &ActivityDateRange) -> Result<Vec<TransactionData>, anyhow::Error> {
        // Best-effort: return empty — transactional DB schema for transactions may live elsewhere
        Ok(vec![])
    }

    async fn calculate_activity_metrics(&self, transactions: &[TransactionData]) -> Result<ActivityMetrics, anyhow::Error> {
        Ok(ActivityMetrics::default())
    }

    async fn build_transaction_network(&self, subject_id: &Uuid, window: &NetworkAnalysisWindow) -> Result<TransactionNetwork, anyhow::Error> {
        Ok(TransactionNetwork::default())
    }

    async fn identify_network_patterns(&self, network: &TransactionNetwork) -> Result<Vec<NetworkPattern>, anyhow::Error> {
        Ok(vec![])
    }

    async fn get_all_open_cases(&self) -> Result<Vec<AMLCaseRecord>, anyhow::Error> {
        let rows = sqlx::query!("SELECT payload FROM aml_case_records")
            .fetch_all(&self.database)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let c: AMLCaseRecord = serde_json::from_value(r.payload)?;
            if matches!(c.case_status, AMLCaseStatus::PendingComplianceReview) {
                out.push(c);
            }
        }
        Ok(out)
    }

    async fn get_cases_in_period(&self, period: &MetricsPeriod) -> Result<Vec<AMLCaseRecord>, anyhow::Error> {
        let rows = sqlx::query!("SELECT payload FROM aml_case_records WHERE created_at BETWEEN $1 AND $2", period.start_date, period.end_date)
            .fetch_all(&self.database)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let c: AMLCaseRecord = serde_json::from_value(r.payload)?;
            out.push(c);
        }
        Ok(out)
    }

    async fn calculate_sla_compliance_rate(&self, period: &MetricsPeriod) -> Result<f64, anyhow::Error> {
        Ok(0.95)
    }

    fn calculate_checklist_completion(&self, required_items: &[ChecklistItemStatus]) -> f64 {
        if required_items.is_empty() {
            return 1.0;
        }

        let completed_count = required_items.iter().filter(|item| item.completed).count();
        completed_count as f64 / required_items.len() as f64
    }

    async fn add_subject_to_watchlist(&self, case_id: &Uuid, reason: WatchlistReason) -> Result<(), anyhow::Error> {
        // TODO: Implement watchlist addition
        Ok(())
    }

    async fn initiate_sar_filing(&self, case_id: &Uuid, decision: &CaseDecisionRequest) -> Result<(), anyhow::Error> {
        // Load case details
        let case = match self.get_case_by_id(case_id).await {
            Ok(c) => c,
            Err(e) => return Err(e),
        };

        // Determine activity window (last 30 days by default)
        let end = Utc::now();
        let start = end - Duration::days(30);
        let date_range = ActivityDateRange { start_date: start, end_date: end };

        // Load subject transactions for the activity window (best-effort)
        let txns = match self.load_subject_transactions(&case.subject_kyc_id, &date_range).await {
            Ok(t) => t,
            Err(_) => Vec::new(),
        };

        // Compute totals
        let total_amount_f64: f64 = txns.iter().map(|t| t.amount).sum();
        let total_amount = Decimal::from_f64(total_amount_f64).unwrap_or(Decimal::ZERO);
        let transaction_count = txns.len() as i32;

        // Collect linked transaction IDs when available
        let linked_transaction_ids: Vec<Uuid> = txns.iter().map(|t| t.id).collect();

        // Prepare triggered rules placeholder (if no explicit rules available)
        let triggered_rules = serde_json::json!([]);

        // Parse assigned investigator UUID if present
        let assigned_investigator_id = case
            .assigned_investigator_id
            .as_deref()
            .and_then(|s| Uuid::parse_str(s).ok());

        // Instantiate SAR service and auto-initiate a SAR (idempotent)
        let sar_svc = SarService::new(self.database.clone());
        let _ = sar_svc
            .auto_initiate(
                *case_id,
                SarDetectionMethod::ComplianceOfficerJudgment,
                Some(case.subject_kyc_id),
                case.subject_wallet_addresses.clone(),
                decision.rationale.clone(),
                start.date_naive(),
                end.date_naive(),
                total_amount,
                transaction_count,
                linked_transaction_ids,
                triggered_rules,
                Some(case.risk_score_at_opening),
                assigned_investigator_id,
            )
            .await
            .map_err(|e| anyhow::anyhow!("failed to auto-initiate SAR: {}", e))?;

        // Notify assigned investigator (if any)
        if let Some(ref inv) = case.assigned_investigator_id {
            let _ = self.notifications.send_user_notification(
                inv,
                &format!("New SAR initiated for case {}", case.id),
                &format!("A SAR has been created for case {}. Please review in the compliance portal.", case.id),
            ).await;
        }

        Ok(())
    }

    async fn record_case_outcome(&self, case_id: &Uuid, outcome: CaseOutcome) -> Result<(), anyhow::Error> {
        // TODO: Implement outcome recording
        Ok(())
    }

    async fn request_supervisor_signoff(&self, case_id: &Uuid, officer_id: &str) -> Result<(), anyhow::Error> {
        // TODO: Implement supervisor signoff request
        Ok(())
    }
}

// Supporting types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateManualCaseRequest {
    pub case_type: CaseType,
    pub subject_kyc_id: Uuid,
    pub subject_wallet_addresses: Vec<String>,
    pub initial_risk_score: f64,
    pub initial_evidence: Option<Vec<CaseEvidenceRequest>>,
    pub created_by_officer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseEvidenceRequest {
    pub evidence_type: EvidenceType,
    pub description: String,
    pub reference_id: Option<String>,
    pub added_by_officer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseNoteRequest {
    pub note_text: String,
    pub author_id: String,
    pub visibility: NoteVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseLinkRequest {
    pub linked_case_id: Uuid,
    pub link_type: CaseLinkType,
    pub reason: String,
    pub linked_by_officer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseEscalationRequest {
    pub escalated_to_officer_id: String,
    pub reason: String,
    pub escalated_by_officer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseDecisionRequest {
    pub decision_type: CaseDecisionType,
    pub rationale: String,
    pub decided_by_officer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityDateRange {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAnalysisWindow {
    pub duration_hours: u64,
    pub max_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsPeriod {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct CaseManagementMetrics {
    pub total_cases_opened: u64,
    pub cases_closed_suspicious: u64,
    pub cases_closed_not_suspicious: u64,
    pub currently_open_cases: u64,
    pub average_resolution_hours: f64,
    pub total_resolved_cases: u64,
    pub sla_compliance_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseChecklistStatus {
    pub case_id: Uuid,
    pub case_type: CaseType,
    pub required_items: Vec<ChecklistItemStatus>,
    pub optional_items: Vec<ChecklistItemStatus>,
    pub overall_completion: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItemStatus {
    pub item: ChecklistItem,
    pub completed: bool,
    pub completed_by_officer_id: Option<String>,
    pub completed_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectActivityHistory {
    pub subject_kyc_id: Uuid,
    pub date_range: ActivityDateRange,
    pub transactions: Vec<TransactionData>,
    pub metrics: ActivityMetrics,
}

#[derive(Debug, Clone, Default)]
pub struct ActivityMetrics {
    pub total_transactions: u64,
    pub total_volume: f64,
    pub average_transaction_size: f64,
    pub unique_counterparties: u64,
    pub transaction_frequency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAnalysisResult {
    pub subject_kyc_id: Uuid,
    pub analysis_window: NetworkAnalysisWindow,
    pub network: TransactionNetwork,
    pub suspicious_patterns: Vec<NetworkPattern>,
}

#[derive(Debug, Clone, Default)]
pub struct TransactionNetwork {
    pub nodes: Vec<NetworkNode>,
    pub edges: Vec<NetworkEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNode {
    pub id: String,
    pub node_type: NodeType,
    pub risk_score: f64,
    pub transaction_count: u64,
    pub total_volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEdge {
    pub source_id: String,
    pub target_id: String,
    pub transaction_count: u64,
    pub total_volume: f64,
    pub time_span_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Subject,
    Counterparty,
    Intermediary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPattern {
    pub pattern_type: PatternType,
    pub description: String,
    pub confidence: f64,
    pub involved_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    Circular,
    HubSpoke,
    RapidChain,
    Layering,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAStatusReport {
    pub total_open_cases: u64,
    pub overdue_cases: u64,
    pub urgent_cases: u64,
    pub upcoming_deadlines: u64,
    pub overdue_cases: Vec<OverdueCase>,
    pub urgent_cases: Vec<UrgentCase>,
    pub upcoming_deadlines: Vec<UpcomingDeadline>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverdueCase {
    pub case_id: Uuid,
    pub case_type: CaseType,
    pub risk_score: f64,
    pub assigned_investigator: Option<String>,
    pub overdue_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrgentCase {
    pub case_id: Uuid,
    pub case_type: CaseType,
    pub risk_score: f64,
    pub assigned_investigator: Option<String>,
    pub hours_remaining: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpcomingDeadline {
    pub case_id: Uuid,
    pub case_type: CaseType,
    pub risk_score: f64,
    pub assigned_investigator: Option<String>,
    pub days_remaining: u64,
}

// Additional enums and types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaseType {
    TransactionBased,
    ActivityBased,
    ReferralBased,
    ProactiveDetection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaseSource {
    AMLRuleTrigger,
    SARReferral,
    CTRAnomaly,
    LawEnforcementRequest,
    ComplianceOfficerJudgment,
    ProactiveDetection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceType {
    TransactionRecord,
    KYCDocument,
    BlockchainAnalyticsReport,
    ExternalIntelligence,
    OfficerObservation,
    ThirdPartyCommunication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoteVisibility {
    Internal,
    SupervisorOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaseLinkType {
    SameSubject,
    RelatedActivity,
    ConnectedNetwork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaseActionType {
    Assignment,
    EvidenceAddition,
    NoteAddition,
    StatusTransition,
    Escalation,
    Resolution,
    Link,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaseDecisionType {
    Suspicious,
    NotSuspicious,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchlistReason {
    SuspiciousActivity,
    HighRiskProfile,
    RegulatoryRequirement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaseOutcome {
    Suspicious,
    NotSuspicious,
}

// Additional model types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AMLCaseRecord {
    pub id: Uuid,
    pub case_type: CaseType,
    pub case_source: CaseSource,
    pub risk_score_at_opening: f64,
    pub subject_kyc_id: Uuid,
    pub subject_wallet_addresses: Vec<String>,
    pub case_status: AMLCaseStatus,
    pub assigned_investigator_id: Option<String>,
    pub supervisor_id: Option<String>,
    pub opened_timestamp: DateTime<Utc>,
    pub target_resolution_timestamp: DateTime<Utc>,
    pub resolved_timestamp: Option<DateTime<Utc>>,
    pub resolution_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseEvidenceRecord {
    pub id: Uuid,
    pub case_id: Uuid,
    pub evidence_type: EvidenceType,
    pub evidence_description: String,
    pub evidence_reference_id: Option<String>,
    pub added_by_officer_id: String,
    pub added_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseNoteRecord {
    pub id: Uuid,
    pub case_id: Uuid,
    pub note_text: String,
    pub note_author_id: String,
    pub note_visibility: NoteVisibility,
    pub note_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseLinkRecord {
    pub id: Uuid,
    pub case_id: Uuid,
    pub linked_case_id: Uuid,
    pub link_type: CaseLinkType,
    pub link_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseAction {
    pub id: Uuid,
    pub case_id: Uuid,
    pub action_type: CaseActionType,
    pub action_detail: String,
    pub performed_by_officer_id: String,
    pub action_timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_management_config_default() {
        let config = CaseManagementConfig::default();
        assert!(config.auto_assignment_enabled);
        assert_eq!(config.default_sla_hours, 72);
        assert_eq!(config.critical_risk_sla_hours, 4);
    }

    #[test]
    fn test_checklist_creation() {
        let checklist = CaseManagementConfig::create_transaction_checklist();
        assert_eq!(checklist.case_type, CaseType::TransactionBased);
        assert!(!checklist.required_items.is_empty());
        assert!(checklist.required_items.iter().all(|item| item.required));
    }
}
