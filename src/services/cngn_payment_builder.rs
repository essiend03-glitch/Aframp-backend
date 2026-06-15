//! cNGN payment transaction builder
//! Builds payment transaction drafts, calculates fees, supports memo, and signs payloads.

// REMOVED: use crate::chains::stellar::client::StellarClient;
use crate::error::{AppError, AppErrorKind, ExternalError, ValidationError};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::{
    MuxedAccount as StrkeyMuxedAccount, PrivateKey as StrkeyPrivateKey,
    PublicKey as StrkeyPublicKey,
};
use stellar_xdr::next::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, DecoratedSignature, Hash,
    Limits, Memo, MuxedAccount, MuxedAccountMed25519, Operation, OperationBody, PaymentOp,
    Preconditions, PublicKey, SequenceNumber, Signature, SignatureHint, StringM, Transaction,
    TransactionEnvelope, TransactionExt, TransactionV1Envelope, Uint256, VecM, WriteXdr,
};

/// Supported memo types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PaymentMemo {
    None,
    Text(String),
    Id(u64),
    Hash(String),
}

/// Payment operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentOperation {
    pub source: String,
    pub destination: String,
    pub amount: String,
    pub asset_code: String,
    pub asset_issuer: String,
}

/// Unsigned payment transaction draft
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentTransactionDraft {
    pub network_passphrase: String,
    pub sequence: i64,
    pub fee_stroops: u64,
    pub memo: PaymentMemo,
    pub operation: PaymentOperation,
    pub created_at: String,
    pub transaction_hash: String,
    pub unsigned_envelope_xdr: String,
}

/// Signed payment transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPaymentTransaction {
    pub draft: PaymentTransactionDraft,
    pub hash: String,
    pub signature: String,
    pub envelope_xdr: String,
}

/// cNGN payment transaction builder
pub struct CngnPaymentBuilder {
    stellar_client: StellarClient,
    base_fee_stroops: u64,
}

impl CngnPaymentBuilder {
    pub fn new(stellar_client: StellarClient) -> Self {
        Self {
            stellar_client,
            base_fee_stroops: 100, // Stellar base fee in stroops
        }
    }

    pub fn with_base_fee(mut self, base_fee_stroops: u64) -> Self {
        self.base_fee_stroops = base_fee_stroops;
        self
    }

    /// Build an unsigned payment transaction draft
    pub async fn build_payment(
        &self,
        operation: PaymentOperation,
        memo: PaymentMemo,
        fee_stroops: Option<u64>,
    ) -> Result<PaymentTransactionDraft, AppError> {
        validate_payment_operation(&operation)?;

        let account = self.stellar_client.get_account(&operation.source).await?;
        let sequence = account.sequence + 1;
        let fee_stroops = fee_stroops.unwrap_or(self.base_fee_stroops);
        let (unsigned_xdr, tx_hash) = build_unsigned_envelope_xdr(
            &operation,
            &memo,
            fee_stroops,
            sequence,
            self.stellar_client.network().network_passphrase(),
        )?;

        Ok(PaymentTransactionDraft {
            network_passphrase: self
                .stellar_client
                .network()
                .network_passphrase()
                .to_string(),
            sequence,
            fee_stroops,
            memo,
            operation,
            created_at: chrono::Utc::now().to_rfc3339(),
            transaction_hash: tx_hash,
            unsigned_envelope_xdr: unsigned_xdr,
        })
    }

    /// Sign a payment transaction draft with a Stellar secret seed
    pub fn sign_transaction(
        &self,
        draft: PaymentTransactionDraft,
        secret_seed: &str,
    ) -> Result<SignedPaymentTransaction, AppError> {
        let signing_key = decode_signing_key(secret_seed)?;
        let (envelope_xdr, tx_hash, signature) = build_signed_envelope_xdr(
            &draft,
            &signing_key,
            self.stellar_client.network().network_passphrase(),
        )?;

        Ok(SignedPaymentTransaction {
            draft,
            hash: tx_hash,
            signature: hex::encode(signature),
            envelope_xdr,
        })
    }

    /// Calculate a simple fee for a single payment op
    pub fn calculate_fee(&self) -> u64 {
        self.base_fee_stroops
    }
}

fn validate_payment_operation(operation: &PaymentOperation) -> Result<(), AppError> {
    if operation.amount.trim().is_empty() {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::MissingField {
                field: "amount".to_string(),
            },
        )));
    }

    if parse_muxed_account(&operation.source).is_err() {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidWalletAddress {
                address: operation.source.clone(),
                reason: "invalid source address".to_string(),
            },
        )));
    }

    if parse_muxed_account(&operation.destination).is_err() {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidWalletAddress {
                address: operation.destination.clone(),
                reason: "invalid destination address".to_string(),
            },
        )));
    }

    if operation.asset_code.trim().is_empty() {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::MissingField {
                field: "asset_code".to_string(),
            },
        )));
    }

    let asset_code = operation.asset_code.trim().to_uppercase();
    if asset_code != "XLM" && asset_code != "NATIVE" && operation.asset_issuer.trim().is_empty() {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::MissingField {
                field: "asset_issuer".to_string(),
            },
        )));
    }

    Ok(())
}

fn decode_signing_key(secret_seed: &str) -> Result<SigningKey, AppError> {
    let private_key = StrkeyPrivateKey::from_string(secret_seed).map_err(|_| {
        AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidWalletAddress {
                address: "secret_seed".to_string(),
                reason: "invalid Stellar secret seed".to_string(),
            },
        ))
    })?;
    Ok(SigningKey::from_bytes(&private_key.0))
}

fn build_unsigned_envelope_xdr(
    operation: &PaymentOperation,
    memo: &PaymentMemo,
    fee_stroops: u64,
    sequence: i64,
    network_passphrase: &str,
) -> Result<(String, String), AppError> {
    let (tx, envelope) = build_transaction(operation, memo, fee_stroops, sequence)?;
    let tx_hash = transaction_hash(&tx, network_passphrase)?;
    let xdr = envelope_to_xdr(&envelope)?;
    Ok((xdr, tx_hash))
}

fn build_signed_envelope_xdr(
    draft: &PaymentTransactionDraft,
    signing_key: &SigningKey,
    network_passphrase: &str,
) -> Result<(String, String, Vec<u8>), AppError> {
    let (tx, _) = build_transaction(
        &draft.operation,
        &draft.memo,
        draft.fee_stroops,
        draft.sequence,
    )?;

    let tx_hash_bytes = tx_hash_bytes(&tx, network_passphrase)?;
    let signature = signing_key.try_sign(&tx_hash_bytes).map_err(|_| {
        AppError::new(AppErrorKind::External(ExternalError::Blockchain {
            message: "Failed to sign transaction hash".to_string(),
            is_retryable: false,
        }))
    })?;

    ensure_signing_key_matches_source(signing_key, &draft.operation.source)?;

    let hint = signature_hint_from_signing_key(signing_key)?;
    let decorated = DecoratedSignature {
        hint,
        signature: Signature::try_from(signature.to_vec()).map_err(|_| {
            AppError::new(AppErrorKind::External(ExternalError::Blockchain {
                message: "Failed to build decorated signature".to_string(),
                is_retryable: false,
            }))
        })?,
    };

    let signatures = VecM::try_from(vec![decorated]).map_err(|_| {
        AppError::new(AppErrorKind::External(ExternalError::Blockchain {
            message: "Failed to build signature list".to_string(),
            is_retryable: false,
        }))
    })?;

    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope { tx, signatures });
    let envelope_xdr = envelope_to_xdr(&envelope)?;
    Ok((
        envelope_xdr,
        hex::encode(tx_hash_bytes),
        signature.to_bytes().to_vec(),
    ))
}

fn build_transaction(
    operation: &PaymentOperation,
    memo: &PaymentMemo,
    fee_stroops: u64,
    sequence: i64,
) -> Result<(Transaction, TransactionEnvelope), AppError> {
    let source = parse_muxed_account(&operation.source)?;
    let destination = parse_muxed_account(&operation.destination)?;
    let asset = build_asset(&operation.asset_code, &operation.asset_issuer)?;
    let amount = amount_to_stroops(&operation.amount)?;
    let memo = memo_to_xdr(memo)?;

    let payment_op = PaymentOp {
        destination,
        asset,
        amount,
    };

    let op = Operation {
        source_account: None,
        body: OperationBody::Payment(payment_op),
    };

    let operations = VecM::try_from(vec![op]).map_err(|_| {
        AppError::new(AppErrorKind::External(ExternalError::Blockchain {
            message: "Failed to build operations list".to_string(),
            is_retryable: false,
        }))
    })?;

    let tx = Transaction {
        source_account: source,
        fee: fee_stroops as u32,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo,
        operations,
        ext: TransactionExt::V0,
    };

    let signatures = VecM::try_from(Vec::<DecoratedSignature>::new()).map_err(|_| {
        AppError::new(AppErrorKind::External(ExternalError::Blockchain {
            message: "Failed to build signature list".to_string(),
            is_retryable: false,
        }))
    })?;

    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures,
    });

    Ok((tx, envelope))
}

fn transaction_hash(tx: &Transaction, network_passphrase: &str) -> Result<String, AppError> {
    Ok(hex::encode(tx_hash_bytes(tx, network_passphrase)?))
}

fn tx_hash_bytes(tx: &Transaction, network_passphrase: &str) -> Result<[u8; 32], AppError> {
    let network_id = network_id_from_passphrase(network_passphrase);
    tx.hash(network_id).map_err(|e| {
        AppError::new(AppErrorKind::External(ExternalError::Blockchain {
            message: format!("Failed to hash transaction: {}", e),
            is_retryable: false,
        }))
    })
}

fn envelope_to_xdr(envelope: &TransactionEnvelope) -> Result<String, AppError> {
    envelope.to_xdr_base64(Limits::none()).map_err(|e| {
        AppError::new(AppErrorKind::External(ExternalError::Blockchain {
            message: format!("Failed to encode transaction XDR: {}", e),
            is_retryable: false,
        }))
    })
}

fn network_id_from_passphrase(passphrase: &str) -> [u8; 32] {
    Sha256::digest(passphrase.as_bytes()).into()
}

fn memo_to_xdr(memo: &PaymentMemo) -> Result<Memo, AppError> {
    match memo {
        PaymentMemo::None => Ok(Memo::None),
        PaymentMemo::Text(text) => {
            if text.as_bytes().len() > 28 {
                return Err(AppError::new(AppErrorKind::Validation(
                    ValidationError::OutOfRange {
                        field: "memo".to_string(),
                        min: None,
                        max: Some("28 bytes".to_string()),
                    },
                )));
            }
            let value: StringM<28> = text.parse().map_err(|_| {
                AppError::new(AppErrorKind::Validation(ValidationError::InvalidAmount {
                    amount: text.clone(),
                    reason: "memo contains invalid characters".to_string(),
                }))
            })?;
            Ok(Memo::Text(value))
        }
        PaymentMemo::Id(id) => Ok(Memo::Id(*id)),
        PaymentMemo::Hash(value) => {
            let hash: Hash = value.parse().map_err(|_| {
                AppError::new(AppErrorKind::Validation(ValidationError::InvalidAmount {
                    amount: value.clone(),
                    reason: "memo hash must be 32-byte hex".to_string(),
                }))
            })?;
            Ok(Memo::Hash(hash))
        }
    }
}

fn amount_to_stroops(amount: &str) -> Result<i64, AppError> {
    let trimmed = amount.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidAmount {
                amount: amount.to_string(),
                reason: "amount is required".to_string(),
            },
        )));
    }

    if trimmed.starts_with('-') {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidAmount {
                amount: amount.to_string(),
                reason: "amount must be positive".to_string(),
            },
        )));
    }

    let mut parts = trimmed.split('.');
    let whole = parts.next().unwrap_or("0");
    let frac = parts.next().unwrap_or("");

    if parts.next().is_some() {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidAmount {
                amount: amount.to_string(),
                reason: "amount has too many decimal points".to_string(),
            },
        )));
    }

    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidAmount {
                amount: amount.to_string(),
                reason: "amount must be numeric".to_string(),
            },
        )));
    }

    if frac.len() > 7 {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidAmount {
                amount: amount.to_string(),
                reason: "amount supports up to 7 decimal places".to_string(),
            },
        )));
    }

    let mut frac_padded = frac.to_string();
    while frac_padded.len() < 7 {
        frac_padded.push('0');
    }

    let whole_value: i64 = whole.parse().map_err(|_| {
        AppError::new(AppErrorKind::Validation(ValidationError::InvalidAmount {
            amount: amount.to_string(),
            reason: "invalid whole number".to_string(),
        }))
    })?;

    let frac_value: i64 = if frac_padded.is_empty() {
        0
    } else {
        frac_padded.parse().map_err(|_| {
            AppError::new(AppErrorKind::Validation(ValidationError::InvalidAmount {
                amount: amount.to_string(),
                reason: "invalid fractional part".to_string(),
            }))
        })?
    };

    Ok(whole_value
        .checked_mul(10_000_000)
        .and_then(|v| v.checked_add(frac_value))
        .ok_or_else(|| {
            AppError::new(AppErrorKind::Validation(ValidationError::InvalidAmount {
                amount: amount.to_string(),
                reason: "amount is too large".to_string(),
            }))
        })?)
}

fn build_asset(asset_code: &str, asset_issuer: &str) -> Result<Asset, AppError> {
    let code = asset_code.trim().to_uppercase();
    if code == "XLM" || code == "NATIVE" {
        return Ok(Asset::Native);
    }

    if !(1..=12).contains(&code.len()) {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidCurrency {
                currency: asset_code.to_string(),
                reason: "asset code must be 1-12 characters".to_string(),
            },
        )));
    }

    let issuer = parse_account_id(asset_issuer)?;
    let bytes = code.as_bytes();

    if code.len() <= 4 {
        let mut buffer = [0u8; 4];
        buffer[..bytes.len()].copy_from_slice(bytes);
        Ok(Asset::CreditAlphanum4(AlphaNum4 {
            asset_code: AssetCode4(buffer),
            issuer,
        }))
    } else {
        let mut buffer = [0u8; 12];
        buffer[..bytes.len()].copy_from_slice(bytes);
        Ok(Asset::CreditAlphanum12(AlphaNum12 {
            asset_code: AssetCode12(buffer),
            issuer,
        }))
    }
}

fn parse_account_id(address: &str) -> Result<AccountId, AppError> {
    let public_key = StrkeyPublicKey::from_string(address).map_err(|_| {
        AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidWalletAddress {
                address: address.to_string(),
                reason: "invalid Stellar public key".to_string(),
            },
        ))
    })?;
    Ok(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
        public_key.0,
    ))))
}

fn parse_muxed_account(address: &str) -> Result<MuxedAccount, AppError> {
    if address.starts_with('M') {
        let muxed = StrkeyMuxedAccount::from_string(address).map_err(|_| {
            AppError::new(AppErrorKind::Validation(
                ValidationError::InvalidWalletAddress {
                    address: address.to_string(),
                    reason: "invalid muxed address".to_string(),
                },
            ))
        })?;
        Ok(MuxedAccount::MuxedEd25519(MuxedAccountMed25519 {
            id: muxed.id,
            ed25519: Uint256(muxed.ed25519),
        }))
    } else {
        let public_key = StrkeyPublicKey::from_string(address).map_err(|_| {
            AppError::new(AppErrorKind::Validation(
                ValidationError::InvalidWalletAddress {
                    address: address.to_string(),
                    reason: "invalid Stellar public key".to_string(),
                },
            ))
        })?;
        Ok(MuxedAccount::Ed25519(Uint256(public_key.0)))
    }
}

fn signature_hint_from_signing_key(signing_key: &SigningKey) -> Result<SignatureHint, AppError> {
    let public_key_bytes = signing_key.verifying_key().to_bytes();
    let hint = &public_key_bytes[public_key_bytes.len() - 4..];
    SignatureHint::try_from(hint).map_err(|_| {
        AppError::new(AppErrorKind::External(ExternalError::Blockchain {
            message: "Failed to compute signature hint".to_string(),
            is_retryable: false,
        }))
    })
}

fn ensure_signing_key_matches_source(
    signing_key: &SigningKey,
    source: &str,
) -> Result<(), AppError> {
    let public_key_bytes = signing_key.verifying_key().to_bytes();
    let expected = if source.starts_with('M') {
        StrkeyMuxedAccount::from_string(source)
            .map(|muxed| muxed.ed25519)
            .map_err(|_| {
                AppError::new(AppErrorKind::Validation(
                    ValidationError::InvalidWalletAddress {
                        address: source.to_string(),
                        reason: "invalid muxed source address".to_string(),
                    },
                ))
            })?
    } else {
        StrkeyPublicKey::from_string(source)
            .map(|pk| pk.0)
            .map_err(|_| {
                AppError::new(AppErrorKind::Validation(
                    ValidationError::InvalidWalletAddress {
                        address: source.to_string(),
                        reason: "invalid source public key".to_string(),
                    },
                ))
            })?
    };

    if public_key_bytes != expected {
        return Err(AppError::new(AppErrorKind::Validation(
            ValidationError::InvalidWalletAddress {
                address: source.to_string(),
                reason: "secret seed does not match source account".to_string(),
            },
        )));
    }

    Ok(())
}
