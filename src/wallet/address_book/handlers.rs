use super::models::*;
use super::repository::AddressBookRepository;
use super::groups::GroupRepository;
use super::verification::*;
use super::metrics::AddressBookMetrics;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AddressBookAppState {
    pub repository: Arc<AddressBookRepository>,
    pub group_repository: Arc<GroupRepository>,
    pub stellar_verifier: Arc<StellarAddressVerifier>,
    pub mobile_money_verifier: Arc<MobileMoneyVerifier>,
    pub bank_account_verifier: Arc<BankAccountVerifier>,
    pub metrics: Arc<AddressBookMetrics>,
    pub max_entries_per_wallet: i64,
    pub max_groups_per_wallet: i64,
    pub max_entries_per_group: i64,
}

/// Create a new address book entry
pub async fn create_address_book_entry(
    State(state): State<Arc<AddressBookAppState>>,
    Json(request): Json<CreateAddressBookEntryRequest>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    // Check entry limit
    let current_count = state.repository.count_entries_by_owner(owner_wallet_id).await?;
    if current_count >= state.max_entries_per_wallet {
        return Err(AppError::LimitExceeded(format!(
            "Maximum address book entries limit ({}) reached",
            state.max_entries_per_wallet
        )));
    }

    match request {
        CreateAddressBookEntryRequest::StellarWallet {
            label,
            notes,
            stellar_public_key,
            network,
        } => {
            // Create entry
            let entry = state
                .repository
                .create_entry(
                    owner_wallet_id,
                    AddressEntryType::StellarWallet,
                    label.clone(),
                    notes.clone(),
                )
                .await?;

            // Create Stellar wallet details
            let stellar_entry = state
                .repository
                .create_stellar_wallet_entry(entry.id, stellar_public_key.clone(), network.clone())
                .await?;

            // Verify the address
            let verification_result = state
                .stellar_verifier
                .verify_account(&stellar_public_key)
                .await
                .unwrap_or_else(|_| VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Failed,
                    message: Some("Verification failed".to_string()),
                    verified_account_name: None,
                    warnings: vec![],
                });

            // Update verification status
            state
                .repository
                .update_verification_status(entry.id, verification_result.verification_status.clone())
                .await?;

            if verification_result.success {
                let account_exists = verification_result.verification_status == VerificationStatus::Verified
                    || verification_result.verification_status == VerificationStatus::Pending;
                let trustline_active = verification_result.verification_status == VerificationStatus::Verified;

                state
                    .repository
                    .update_stellar_verification(entry.id, account_exists, trustline_active)
                    .await?;
            }

            // Record metrics
            state.metrics.record_entry_created(AddressEntryType::StellarWallet);
            state.metrics.record_verification_event(
                AddressEntryType::StellarWallet,
                verification_result.success,
            );

            let response = AddressBookEntryResponse {
                id: entry.id,
                entry_type: entry.entry_type,
                label: entry.label,
                notes: entry.notes,
                verification_status: verification_result.verification_status,
                last_used_at: entry.last_used_at,
                use_count: entry.use_count,
                created_at: entry.created_at,
                updated_at: entry.updated_at,
                details: EntryDetails::StellarWallet {
                    stellar_public_key: stellar_entry.stellar_public_key,
                    network: stellar_entry.network,
                    account_exists_on_stellar: stellar_entry.account_exists_on_stellar,
                    cngn_trustline_active: stellar_entry.cngn_trustline_active,
                    last_verified_at: stellar_entry.last_verified_at,
                },
            };

            Ok((StatusCode::CREATED, Json(json!({
                "entry": response,
                "verification": verification_result,
            }))).into_response())
        }
        CreateAddressBookEntryRequest::MobileMoney {
            label,
            notes,
            provider_name,
            phone_number,
            country_code,
        } => {
            // Create entry
            let entry = state
                .repository
                .create_entry(
                    owner_wallet_id,
                    AddressEntryType::MobileMoney,
                    label.clone(),
                    notes.clone(),
                )
                .await?;

            // Create mobile money details
            let mobile_entry = state
                .repository
                .create_mobile_money_entry(
                    entry.id,
                    provider_name.clone(),
                    phone_number.clone(),
                    country_code.clone(),
                )
                .await?;

            // Verify the phone number
            let verification_result = state
                .mobile_money_verifier
                .verify_account(&provider_name, &phone_number, &country_code)
                .await
                .unwrap_or_else(|_| VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Failed,
                    message: Some("Verification failed".to_string()),
                    verified_account_name: None,
                    warnings: vec![],
                });

            // Update verification status
            state
                .repository
                .update_verification_status(entry.id, verification_result.verification_status.clone())
                .await?;

            if let Some(account_name) = &verification_result.verified_account_name {
                state
                    .repository
                    .update_mobile_money_account_name(entry.id, account_name.clone())
                    .await?;
            }

            // Record metrics
            state.metrics.record_entry_created(AddressEntryType::MobileMoney);
            state.metrics.record_verification_event(
                AddressEntryType::MobileMoney,
                verification_result.success,
            );

            let response = AddressBookEntryResponse {
                id: entry.id,
                entry_type: entry.entry_type,
                label: entry.label,
                notes: entry.notes,
                verification_status: verification_result.verification_status.clone(),
                last_used_at: entry.last_used_at,
                use_count: entry.use_count,
                created_at: entry.created_at,
                updated_at: entry.updated_at,
                details: EntryDetails::MobileMoney {
                    provider_name: mobile_entry.provider_name,
                    phone_number: mobile_entry.phone_number,
                    account_name: verification_result.verified_account_name.clone(),
                    country_code: mobile_entry.country_code,
                    last_verified_at: mobile_entry.last_verified_at,
                },
            };

            Ok((StatusCode::CREATED, Json(json!({
                "entry": response,
                "verification": verification_result,
            }))).into_response())
        }
        CreateAddressBookEntryRequest::BankAccount {
            label,
            notes,
            bank_name,
            account_number,
            sort_code,
            routing_number,
            country_code,
            currency,
        } => {
            // Create entry
            let entry = state
                .repository
                .create_entry(
                    owner_wallet_id,
                    AddressEntryType::BankAccount,
                    label.clone(),
                    notes.clone(),
                )
                .await?;

            // Create bank account details
            let bank_entry = state
                .repository
                .create_bank_account_entry(
                    entry.id,
                    bank_name.clone(),
                    account_number.clone(),
                    sort_code.clone(),
                    routing_number.clone(),
                    country_code.clone(),
                    currency.clone(),
                )
                .await?;

            // Verify the account
            let verification_result = state
                .bank_account_verifier
                .verify_account(&bank_name, &account_number, &country_code)
                .await
                .unwrap_or_else(|_| VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Failed,
                    message: Some("Verification failed".to_string()),
                    verified_account_name: None,
                    warnings: vec![],
                });

            // Update verification status
            state
                .repository
                .update_verification_status(entry.id, verification_result.verification_status.clone())
                .await?;

            if let Some(account_name) = &verification_result.verified_account_name {
                state
                    .repository
                    .update_bank_account_name(entry.id, account_name.clone())
                    .await?;
            }

            // Record metrics
            state.metrics.record_entry_created(AddressEntryType::BankAccount);
            state.metrics.record_verification_event(
                AddressEntryType::BankAccount,
                verification_result.success,
            );

            let response = AddressBookEntryResponse {
                id: entry.id,
                entry_type: entry.entry_type,
                label: entry.label,
                notes: entry.notes,
                verification_status: verification_result.verification_status.clone(),
                last_used_at: entry.last_used_at,
                use_count: entry.use_count,
                created_at: entry.created_at,
                updated_at: entry.updated_at,
                details: EntryDetails::BankAccount {
                    bank_name: bank_entry.bank_name,
                    account_number: bank_entry.account_number,
                    account_name: verification_result.verified_account_name.clone(),
                    sort_code: bank_entry.sort_code,
                    routing_number: bank_entry.routing_number,
                    country_code: bank_entry.country_code,
                    currency: bank_entry.currency,
                    last_verified_at: bank_entry.last_verified_at,
                },
            };

            Ok((StatusCode::CREATED, Json(json!({
                "entry": response,
                "verification": verification_result,
            }))).into_response())
        }
    }
}

/// List address book entries
pub async fn list_address_book_entries(
    State(state): State<Arc<AddressBookAppState>>,
    Query(query): Query<ListAddressBookEntriesQuery>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let entries = state
        .repository
        .list_entries(
            owner_wallet_id,
            query.entry_type,
            query.group_id,
            query.search,
            limit,
            offset,
        )
        .await?;

    let total = state
        .repository
        .count_entries(owner_wallet_id, query.entry_type, query.group_id, None)
        .await?;

    // Build response with details
    let mut entry_responses = Vec::new();
    for entry in entries {
        let details = match entry.entry_type {
            AddressEntryType::StellarWallet => {
                if let Some(stellar) = state.repository.get_stellar_wallet_entry(entry.id).await? {
                    EntryDetails::StellarWallet {
                        stellar_public_key: stellar.stellar_public_key,
                        network: stellar.network,
                        account_exists_on_stellar: stellar.account_exists_on_stellar,
                        cngn_trustline_active: stellar.cngn_trustline_active,
                        last_verified_at: stellar.last_verified_at,
                    }
                } else {
                    continue;
                }
            }
            AddressEntryType::MobileMoney => {
                if let Some(mobile) = state.repository.get_mobile_money_entry(entry.id).await? {
                    EntryDetails::MobileMoney {
                        provider_name: mobile.provider_name,
                        phone_number: mobile.phone_number,
                        account_name: mobile.account_name,
                        country_code: mobile.country_code,
                        last_verified_at: mobile.last_verified_at,
                    }
                } else {
                    continue;
                }
            }
            AddressEntryType::BankAccount => {
                if let Some(bank) = state.repository.get_bank_account_entry(entry.id).await? {
                    EntryDetails::BankAccount {
                        bank_name: bank.bank_name,
                        account_number: bank.account_number,
                        account_name: bank.account_name,
                        sort_code: bank.sort_code,
                        routing_number: bank.routing_number,
                        country_code: bank.country_code,
                        currency: bank.currency,
                        last_verified_at: bank.last_verified_at,
                    }
                } else {
                    continue;
                }
            }
        };

        entry_responses.push(AddressBookEntryResponse {
            id: entry.id,
            entry_type: entry.entry_type,
            label: entry.label,
            notes: entry.notes,
            verification_status: entry.verification_status,
            last_used_at: entry.last_used_at,
            use_count: entry.use_count,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
            details,
        });
    }

    let response = AddressBookEntriesListResponse {
        entries: entry_responses,
        total,
        limit,
        offset,
    };

    Ok(Json(response).into_response())
}

/// Get a single address book entry
pub async fn get_address_book_entry(
    State(state): State<Arc<AddressBookAppState>>,
    Path(entry_id): Path<Uuid>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let entry = state
        .repository
        .get_entry(entry_id, owner_wallet_id)
        .await?
        .ok_or(AppError::NotFound("Address book entry not found".to_string()))?;

    let details = match entry.entry_type {
        AddressEntryType::StellarWallet => {
            let stellar = state
                .repository
                .get_stellar_wallet_entry(entry.id)
                .await?
                .ok_or(AppError::NotFound("Stellar wallet details not found".to_string()))?;

            EntryDetails::StellarWallet {
                stellar_public_key: stellar.stellar_public_key,
                network: stellar.network,
                account_exists_on_stellar: stellar.account_exists_on_stellar,
                cngn_trustline_active: stellar.cngn_trustline_active,
                last_verified_at: stellar.last_verified_at,
            }
        }
        AddressEntryType::MobileMoney => {
            let mobile = state
                .repository
                .get_mobile_money_entry(entry.id)
                .await?
                .ok_or(AppError::NotFound("Mobile money details not found".to_string()))?;

            EntryDetails::MobileMoney {
                provider_name: mobile.provider_name,
                phone_number: mobile.phone_number,
                account_name: mobile.account_name,
                country_code: mobile.country_code,
                last_verified_at: mobile.last_verified_at,
            }
        }
        AddressEntryType::BankAccount => {
            let bank = state
                .repository
                .get_bank_account_entry(entry.id)
                .await?
                .ok_or(AppError::NotFound("Bank account details not found".to_string()))?;

            EntryDetails::BankAccount {
                bank_name: bank.bank_name,
                account_number: bank.account_number,
                account_name: bank.account_name,
                sort_code: bank.sort_code,
                routing_number: bank.routing_number,
                country_code: bank.country_code,
                currency: bank.currency,
                last_verified_at: bank.last_verified_at,
            }
        }
    };

    let response = AddressBookEntryResponse {
        id: entry.id,
        entry_type: entry.entry_type,
        label: entry.label,
        notes: entry.notes,
        verification_status: entry.verification_status,
        last_used_at: entry.last_used_at,
        use_count: entry.use_count,
        created_at: entry.created_at,
        updated_at: entry.updated_at,
        details,
    };

    Ok(Json(response).into_response())
}

/// Update address book entry
pub async fn update_address_book_entry(
    State(state): State<Arc<AddressBookAppState>>,
    Path(entry_id): Path<Uuid>,
    Json(request): Json<UpdateAddressBookEntryRequest>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let entry = state
        .repository
        .update_entry(entry_id, owner_wallet_id, request.label, request.notes)
        .await?;

    Ok(Json(json!({ "message": "Entry updated successfully", "entry_id": entry.id })).into_response())
}

/// Delete address book entry (soft delete)
pub async fn delete_address_book_entry(
    State(state): State<Arc<AddressBookAppState>>,
    Path(entry_id): Path<Uuid>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    state
        .repository
        .soft_delete_entry(entry_id, owner_wallet_id)
        .await?;

    Ok(Json(json!({ "message": "Entry deleted successfully" })).into_response())
}

/// Restore soft-deleted entry
pub async fn restore_address_book_entry(
    State(state): State<Arc<AddressBookAppState>>,
    Path(entry_id): Path<Uuid>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let entry = state
        .repository
        .restore_entry(entry_id, owner_wallet_id)
        .await?;

    Ok(Json(json!({ "message": "Entry restored successfully", "entry_id": entry.id })).into_response())
}

/// Verify an address book entry
pub async fn verify_address_book_entry(
    State(state): State<Arc<AddressBookAppState>>,
    Path(entry_id): Path<Uuid>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let entry = state
        .repository
        .get_entry(entry_id, owner_wallet_id)
        .await?
        .ok_or(AppError::NotFound("Address book entry not found".to_string()))?;

    let verification_result = match entry.entry_type {
        AddressEntryType::StellarWallet => {
            let stellar = state
                .repository
                .get_stellar_wallet_entry(entry.id)
                .await?
                .ok_or(AppError::NotFound("Stellar wallet details not found".to_string()))?;

            let result = state
                .stellar_verifier
                .verify_account(&stellar.stellar_public_key)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            if result.success {
                let account_exists = result.verification_status == VerificationStatus::Verified
                    || result.verification_status == VerificationStatus::Pending;
                let trustline_active = result.verification_status == VerificationStatus::Verified;

                state
                    .repository
                    .update_stellar_verification(entry.id, account_exists, trustline_active)
                    .await?;
            }

            state
                .repository
                .update_verification_status(entry.id, result.verification_status.clone())
                .await?;

            state.metrics.record_verification_event(
                AddressEntryType::StellarWallet,
                result.success,
            );

            result
        }
        AddressEntryType::MobileMoney => {
            let mobile = state
                .repository
                .get_mobile_money_entry(entry.id)
                .await?
                .ok_or(AppError::NotFound("Mobile money details not found".to_string()))?;

            let result = state
                .mobile_money_verifier
                .verify_account(&mobile.provider_name, &mobile.phone_number, &mobile.country_code)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            if let Some(account_name) = &result.verified_account_name {
                state
                    .repository
                    .update_mobile_money_account_name(entry.id, account_name.clone())
                    .await?;
            }

            state
                .repository
                .update_verification_status(entry.id, result.verification_status.clone())
                .await?;

            state.metrics.record_verification_event(
                AddressEntryType::MobileMoney,
                result.success,
            );

            result
        }
        AddressEntryType::BankAccount => {
            let bank = state
                .repository
                .get_bank_account_entry(entry.id)
                .await?
                .ok_or(AppError::NotFound("Bank account details not found".to_string()))?;

            let result = state
                .bank_account_verifier
                .verify_account(&bank.bank_name, &bank.account_number, &bank.country_code)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

            if let Some(account_name) = &result.verified_account_name {
                state
                    .repository
                    .update_bank_account_name(entry.id, account_name.clone())
                    .await?;
            }

            state
                .repository
                .update_verification_status(entry.id, result.verification_status.clone())
                .await?;

            state.metrics.record_verification_event(
                AddressEntryType::BankAccount,
                result.success,
            );

            result
        }
    };

    Ok(Json(verification_result).into_response())
}

// Error handling
#[derive(Debug)]
pub enum AppError {
    Database(sqlx::Error),
    NotFound(String),
    LimitExceeded(String),
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::LimitExceeded(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
