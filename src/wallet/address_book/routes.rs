use super::handlers::*;
use super::groups::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub fn address_book_routes(state: Arc<AddressBookAppState>) -> Router {
    Router::new()
        // Address book entries
        .route("/api/wallet/address-book", post(create_address_book_entry))
        .route("/api/wallet/address-book", get(list_address_book_entries))
        .route("/api/wallet/address-book/:entry_id", get(get_address_book_entry))
        .route("/api/wallet/address-book/:entry_id", patch(update_address_book_entry))
        .route("/api/wallet/address-book/:entry_id", delete(delete_address_book_entry))
        .route("/api/wallet/address-book/:entry_id/restore", post(restore_address_book_entry))
        .route("/api/wallet/address-book/:entry_id/verify", post(verify_address_book_entry))
        // Groups
        .route("/api/wallet/address-book/groups", post(create_address_group))
        .route("/api/wallet/address-book/groups", get(list_address_groups))
        .route("/api/wallet/address-book/groups/:group_id", patch(update_address_group))
        .route("/api/wallet/address-book/groups/:group_id", delete(delete_address_group))
        .route("/api/wallet/address-book/groups/:group_id/members", post(add_group_members))
        .route("/api/wallet/address-book/groups/:group_id/members/:entry_id", delete(remove_group_member))
        // Import/Export
        .route("/api/wallet/address-book/import", post(import_entries))
        .route("/api/wallet/address-book/export", get(export_entries))
        // Search and suggestions
        .route("/api/wallet/address-book/search", get(search_entries))
        .route("/api/wallet/address-book/suggestions", get(get_suggestions))
        .with_state(state)
}

// Group handlers

async fn create_address_group(
    State(state): State<Arc<AddressBookAppState>>,
    Json(request): Json<super::models::CreateAddressGroupRequest>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    // Check group limit
    let current_count = state.group_repository.count_groups_by_owner(owner_wallet_id).await?;
    if current_count >= state.max_groups_per_wallet {
        return Err(AppError::LimitExceeded(format!(
            "Maximum groups limit ({}) reached",
            state.max_groups_per_wallet
        )));
    }

    let group = state
        .group_repository
        .create_group(owner_wallet_id, request.group_name, request.group_description)
        .await?;

    let response = super::models::AddressGroupResponse {
        id: group.id,
        group_name: group.group_name,
        group_description: group.group_description,
        member_count: 0,
        created_at: group.created_at,
        updated_at: group.updated_at,
    };

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn list_address_groups(
    State(state): State<Arc<AddressBookAppState>>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let groups = state.group_repository.list_groups(owner_wallet_id).await?;

    let mut responses = Vec::new();
    for group in groups {
        let member_count = state.group_repository.get_member_count(group.id).await?;
        responses.push(super::models::AddressGroupResponse {
            id: group.id,
            group_name: group.group_name,
            group_description: group.group_description,
            member_count,
            created_at: group.created_at,
            updated_at: group.updated_at,
        });
    }

    Ok(Json(responses).into_response())
}

async fn update_address_group(
    State(state): State<Arc<AddressBookAppState>>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<super::models::UpdateAddressGroupRequest>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let group = state
        .group_repository
        .update_group(group_id, owner_wallet_id, request.group_name, request.group_description)
        .await?;

    Ok(Json(json!({ "message": "Group updated successfully", "group_id": group.id })).into_response())
}

async fn delete_address_group(
    State(state): State<Arc<AddressBookAppState>>,
    Path(group_id): Path<Uuid>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    state
        .group_repository
        .delete_group(group_id, owner_wallet_id)
        .await?;

    Ok(Json(json!({ "message": "Group deleted successfully" })).into_response())
}

async fn add_group_members(
    State(state): State<Arc<AddressBookAppState>>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<super::models::AddGroupMembersRequest>,
) -> Result<Response, AppError> {
    // Check member limit
    let current_count = state.group_repository.count_members_in_group(group_id).await?;
    if current_count + request.entry_ids.len() as i64 > state.max_entries_per_group {
        return Err(AppError::LimitExceeded(format!(
            "Maximum entries per group limit ({}) would be exceeded",
            state.max_entries_per_group
        )));
    }

    let added = state
        .group_repository
        .add_members(group_id, request.entry_ids)
        .await?;

    Ok(Json(json!({ "message": format!("{} members added to group", added) })).into_response())
}

async fn remove_group_member(
    State(state): State<Arc<AddressBookAppState>>,
    Path((group_id, entry_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    state
        .group_repository
        .remove_member(group_id, entry_id)
        .await?;

    Ok(Json(json!({ "message": "Member removed from group" })).into_response())
}

// Import/Export handlers

async fn import_entries(
    State(state): State<Arc<AddressBookAppState>>,
    Json(request): Json<super::models::ImportEntriesRequest>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let import_service = super::import_export::ImportExportService::new(state.repository.clone());
    
    let result = import_service
        .import_from_csv(owner_wallet_id, request.csv_data, state.max_entries_per_wallet)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    state.metrics.record_import_event();

    Ok(Json(result).into_response())
}

async fn export_entries(
    State(state): State<Arc<AddressBookAppState>>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let import_service = super::import_export::ImportExportService::new(state.repository.clone());
    
    let csv_data = import_service
        .export_to_csv(owner_wallet_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    state.metrics.record_export_event();

    Ok((
        StatusCode::OK,
        [("Content-Type", "text/csv"), ("Content-Disposition", "attachment; filename=address_book.csv")],
        csv_data,
    ).into_response())
}

// Search and suggestions handlers

async fn search_entries(
    State(state): State<Arc<AddressBookAppState>>,
    Query(query): Query<super::models::SearchEntriesQuery>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let limit = query.limit.unwrap_or(20).min(100);

    let entries = state
        .repository
        .list_entries(
            owner_wallet_id,
            query.entry_type,
            query.group_id,
            Some(query.query),
            limit,
            0,
        )
        .await?;

    // Build response with details (similar to list_address_book_entries)
    let mut entry_responses = Vec::new();
    for entry in entries {
        let details = match entry.entry_type {
            super::models::AddressEntryType::StellarWallet => {
                if let Some(stellar) = state.repository.get_stellar_wallet_entry(entry.id).await? {
                    super::models::EntryDetails::StellarWallet {
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
            super::models::AddressEntryType::MobileMoney => {
                if let Some(mobile) = state.repository.get_mobile_money_entry(entry.id).await? {
                    super::models::EntryDetails::MobileMoney {
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
            super::models::AddressEntryType::BankAccount => {
                if let Some(bank) = state.repository.get_bank_account_entry(entry.id).await? {
                    super::models::EntryDetails::BankAccount {
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

        entry_responses.push(super::models::AddressBookEntryResponse {
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

    Ok(Json(entry_responses).into_response())
}

async fn get_suggestions(
    State(state): State<Arc<AddressBookAppState>>,
    Query(query): Query<super::models::GetSuggestionsQuery>,
) -> Result<Response, AppError> {
    // TODO: Extract wallet_id from auth token
    let owner_wallet_id = Uuid::new_v4(); // Placeholder

    let limit = query.limit.unwrap_or(5).min(20);

    let suggestion_service = super::suggestions::SuggestionService::new(state.repository.clone());
    
    let (entries, reason) = suggestion_service
        .get_suggestions(owner_wallet_id, &query.transaction_type, limit)
        .await?;

    // Build response with details
    let mut entry_responses = Vec::new();
    for entry in entries {
        let details = match entry.entry_type {
            super::models::AddressEntryType::StellarWallet => {
                if let Some(stellar) = state.repository.get_stellar_wallet_entry(entry.id).await? {
                    super::models::EntryDetails::StellarWallet {
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
            super::models::AddressEntryType::MobileMoney => {
                if let Some(mobile) = state.repository.get_mobile_money_entry(entry.id).await? {
                    super::models::EntryDetails::MobileMoney {
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
            super::models::AddressEntryType::BankAccount => {
                if let Some(bank) = state.repository.get_bank_account_entry(entry.id).await? {
                    super::models::EntryDetails::BankAccount {
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

        entry_responses.push(super::models::AddressBookEntryResponse {
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

    let response = super::models::SuggestionResponse {
        entries: entry_responses,
        reason,
    };

    Ok(Json(response).into_response())
}
