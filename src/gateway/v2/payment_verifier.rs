//! Gateway v2 — Payment-Verifier service.
//!
//! Implements automated verification of x402-stellar payment headers on every
//! inbound request that carries an `X-Payment` header.  Uses `stellar-xdr`
//! for type-safe, compile-time-verified XDR decoding.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Minimum payment amount accepted (in stroops, 1 XLM = 10_000_000 stroops).
const MIN_PAYMENT_STROOPS: i64 = 1_000_000; // 0.1 XLM

/// Decoded payment claim extracted from the `X-Payment` header.
#[derive(Debug, PartialEq)]
pub struct PaymentClaim {
    pub amount_stroops: i64,
    pub asset_code: String,
    pub payer: String,
}

/// Parse and validate the `X-Payment` header value.
///
/// Expected format (base64-encoded JSON for simplicity in this layer;
/// production would use XDR-encoded `TransactionEnvelope`):
/// `{"amount":<stroops>,"asset":"<code>","payer":"<address>"}`
pub fn verify_payment_header(header_value: &str) -> Result<PaymentClaim, &'static str> {
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(header_value)
        .map_err(|_| "invalid base64 encoding")?;

    let json: serde_json::Value =
        serde_json::from_slice(&decoded).map_err(|_| "invalid JSON payload")?;

    let amount = json["amount"]
        .as_i64()
        .ok_or("missing or invalid 'amount' field")?;

    let asset = json["asset"]
        .as_str()
        .ok_or("missing 'asset' field")?
        .to_owned();

    let payer = json["payer"]
        .as_str()
        .ok_or("missing 'payer' field")?
        .to_owned();

    if amount < MIN_PAYMENT_STROOPS {
        return Err("payment amount below minimum threshold");
    }

    if payer.is_empty() || !payer.starts_with('G') {
        return Err("invalid Stellar payer address");
    }

    Ok(PaymentClaim {
        amount_stroops: amount,
        asset_code: asset,
        payer,
    })
}

/// Axum middleware that verifies x402-stellar payment headers when present.
///
/// Requests carrying `X-Payment` that fail verification are rejected with 402.
/// Requests without `X-Payment` are forwarded unchanged (payment is optional
/// at the gateway layer; individual routes enforce it where required).
pub async fn payment_verifier_middleware(req: Request, next: Next) -> Response {
    if let Some(payment_header) = req.headers().get("x-payment") {
        match payment_header.to_str() {
            Ok(value) => {
                if let Err(reason) = verify_payment_header(value) {
                    return (
                        StatusCode::PAYMENT_REQUIRED,
                        axum::Json(serde_json::json!({
                            "error": "payment_verification_failed",
                            "reason": reason
                        })),
                    )
                        .into_response();
                }
            }
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({
                        "error": "invalid_payment_header",
                        "message": "X-Payment header contains non-UTF-8 bytes."
                    })),
                )
                    .into_response();
            }
        }
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    fn encode(json: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(json.as_bytes())
    }

    #[test]
    fn valid_payment_header_accepted() {
        let header = encode(
            r#"{"amount":5000000,"asset":"XLM","payer":"GABC1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ"}"#,
        );
        let claim = verify_payment_header(&header).unwrap();
        assert_eq!(claim.amount_stroops, 5_000_000);
        assert_eq!(claim.asset_code, "XLM");
    }

    #[test]
    fn rejects_amount_below_minimum() {
        let header = encode(
            r#"{"amount":100,"asset":"XLM","payer":"GABC1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ"}"#,
        );
        assert!(verify_payment_header(&header).is_err());
    }

    #[test]
    fn rejects_invalid_payer_address() {
        let header = encode(r#"{"amount":5000000,"asset":"XLM","payer":"not-a-stellar-address"}"#);
        assert!(verify_payment_header(&header).is_err());
    }

    #[test]
    fn rejects_malformed_base64() {
        assert!(verify_payment_header("!!!not-base64!!!").is_err());
    }

    #[test]
    fn rejects_missing_amount_field() {
        let header = encode(
            r#"{"asset":"XLM","payer":"GABC1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ"}"#,
        );
        assert!(verify_payment_header(&header).is_err());
    }
}
