//! AES-256-GCM encryption helpers for PII contact fields.
//!
//! The encryption key is loaded from the `ENCRYPTION_KEY` environment variable
//! (32-byte hex string, same key used elsewhere in the platform).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use base64::{engine::general_purpose::STANDARD as B64, Engine};

const NONCE_LEN: usize = 12;

fn load_key() -> Result<Key<Aes256Gcm>, String> {
    let hex = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| "ENCRYPTION_KEY not set".to_string())?;
    let bytes = hex::decode(&hex)
        .map_err(|e| format!("ENCRYPTION_KEY is not valid hex: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!(
            "ENCRYPTION_KEY must be 32 bytes, got {}",
            bytes.len()
        ));
    }
    Ok(*Key::<Aes256Gcm>::from_slice(&bytes))
}

/// Encrypt plaintext PII. Returns `"<nonce_b64>.<ciphertext_b64>"`.
pub fn encrypt_pii(plaintext: &str) -> Result<String, String> {
    let key = load_key()?;
    let cipher = Aes256Gcm::new(&key);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    Ok(format!("{}.{}", B64.encode(nonce_bytes), B64.encode(ciphertext)))
}

/// Decrypt a value produced by `encrypt_pii`.
pub fn decrypt_pii(encoded: &str) -> Result<String, String> {
    let key = load_key()?;
    let cipher = Aes256Gcm::new(&key);

    let mut parts = encoded.splitn(2, '.');
    let nonce_b64 = parts.next().ok_or("Invalid ciphertext format")?;
    let ct_b64 = parts.next().ok_or("Invalid ciphertext format")?;

    let nonce_bytes = B64
        .decode(nonce_b64)
        .map_err(|e| format!("Nonce decode error: {}", e))?;
    let ciphertext = B64
        .decode(ct_b64)
        .map_err(|e| format!("Ciphertext decode error: {}", e))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| format!("Decryption failed: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {}", e))
}

/// Mask a wallet address for anonymised export: `GABCD...XYZ1` → `GABCD...Z1`.
pub fn mask_wallet_address(addr: &str) -> String {
    if addr.len() <= 10 {
        return "*".repeat(addr.len());
    }
    format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_wallet_address() {
        let addr = "GABCDEFGHIJKLMNOPQRSTUVWXYZ1234";
        let masked = mask_wallet_address(addr);
        assert!(masked.starts_with("GABCDE"));
        assert!(masked.ends_with("1234"));
        assert!(masked.contains("..."));
    }
}
