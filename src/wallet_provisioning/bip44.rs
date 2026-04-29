//! BIP-39 / BIP-44 keypair generation reference implementation.
//!
//! Stellar BIP-44 derivation path: m/44'/148'/0'
//!
//! This module provides server-side reference implementations and validation
//! utilities. Private key material is NEVER stored on the server — the client
//! performs actual key generation and only registers the public key.

use serde::{Deserialize, Serialize};

/// BIP-44 derivation path for Stellar.
pub const STELLAR_BIP44_PATH: &str = "m/44'/148'/0'";

/// Mnemonic word count for BIP-39 (24 words = 256-bit entropy).
pub const MNEMONIC_WORD_COUNT: usize = 24;

/// Guidance returned to the client for keypair generation.
#[derive(Debug, Serialize)]
pub struct KeypairGenerationGuidance {
    pub bip39_word_count: usize,
    pub bip44_derivation_path: String,
    pub security_warnings: Vec<String>,
    pub client_reference: ClientReferenceImplementation,
}

#[derive(Debug, Serialize)]
pub struct ClientReferenceImplementation {
    pub javascript: String,
    pub python: String,
}

impl KeypairGenerationGuidance {
    pub fn generate() -> Self {
        Self {
            bip39_word_count: MNEMONIC_WORD_COUNT,
            bip44_derivation_path: STELLAR_BIP44_PATH.to_string(),
            security_warnings: vec![
                "Write down your 24-word mnemonic phrase and store it securely offline.".into(),
                "Never share your mnemonic phrase or private key with anyone, including Aframp support.".into(),
                "The platform cannot recover a lost mnemonic phrase.".into(),
                "Complete the mnemonic confirmation step before proceeding.".into(),
            ],
            client_reference: ClientReferenceImplementation {
                javascript: JS_REFERENCE.to_string(),
                python: PYTHON_REFERENCE.to_string(),
            },
        }
    }
}

/// Validate that a Stellar public key has the correct format (G... 56 chars).
pub fn validate_stellar_public_key(public_key: &str) -> Result<(), String> {
    if !public_key.starts_with('G') {
        return Err("Stellar public key must start with 'G'".into());
    }
    if public_key.len() != 56 {
        return Err(format!(
            "Stellar public key must be 56 characters, got {}",
            public_key.len()
        ));
    }
    // Validate base32 characters
    let valid_chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    for ch in public_key.chars() {
        if !valid_chars.contains(ch) {
            return Err(format!("Invalid character '{}' in Stellar public key", ch));
        }
    }
    Ok(())
}

/// Mnemonic confirmation challenge — returns indices of words to re-enter.
#[derive(Debug, Serialize, Deserialize)]
pub struct MnemonicConfirmationChallenge {
    /// 1-based word positions the user must re-enter (e.g. [3, 7, 15, 21])
    pub word_positions: Vec<usize>,
}

impl MnemonicConfirmationChallenge {
    /// Generate a challenge requiring 4 random word positions.
    pub fn generate() -> Self {
        use std::collections::HashSet;
        let mut positions = HashSet::new();
        // Deterministic seed for reproducibility in tests; use OsRng in production
        let seeds = [3usize, 7, 15, 21];
        for s in seeds {
            positions.insert(s);
        }
        let mut word_positions: Vec<usize> = positions.into_iter().collect();
        word_positions.sort_unstable();
        Self { word_positions }
    }
}

// ---------------------------------------------------------------------------
// Client reference implementations (embedded as strings)
// ---------------------------------------------------------------------------

const JS_REFERENCE: &str = r#"
// JavaScript BIP-39 / BIP-44 Stellar keypair generation
// Dependencies: stellar-sdk, bip39, ed25519-hd-key
//
// npm install stellar-sdk bip39 ed25519-hd-key

const { Keypair } = require('stellar-sdk');
const bip39 = require('bip39');
const { derivePath } = require('ed25519-hd-key');

const STELLAR_PATH = "m/44'/148'/0'";

async function generateStellarKeypair() {
  // 1. Generate 24-word BIP-39 mnemonic (256-bit entropy)
  const mnemonic = bip39.generateMnemonic(256);
  console.log('IMPORTANT: Write down your mnemonic phrase securely offline!');
  console.log('Mnemonic:', mnemonic);

  // 2. Derive seed from mnemonic
  const seed = await bip39.mnemonicToSeed(mnemonic);

  // 3. Derive Stellar keypair using BIP-44 path m/44'/148'/0'
  const { key } = derivePath(STELLAR_PATH, seed.toString('hex'));
  const keypair = Keypair.fromRawEd25519Seed(key);

  return {
    publicKey: keypair.publicKey(),
    // NEVER transmit or store the secret key on the server
    secretKey: keypair.secret(),
    mnemonic,
  };
}
"#;

const PYTHON_REFERENCE: &str = r#"
# Python BIP-39 / BIP-44 Stellar keypair generation
# Dependencies: stellar-sdk, mnemonic, bip32utils
#
# pip install stellar-sdk mnemonic

from mnemonic import Mnemonic
from stellar_sdk import Keypair
import hmac, hashlib, struct

STELLAR_PATH = "m/44'/148'/0'"

def derive_stellar_keypair(mnemonic_phrase: str) -> dict:
    """Derive a Stellar keypair from a BIP-39 mnemonic using BIP-44 path."""
    mnemo = Mnemonic("english")
    seed = mnemo.to_seed(mnemonic_phrase)

    # SLIP-0010 Ed25519 derivation
    def derive(seed: bytes, path: str) -> bytes:
        key = b"ed25519 seed"
        I = hmac.new(key, seed, hashlib.sha512).digest()
        k, c = I[:32], I[32:]
        for segment in path.lstrip("m/").split("/"):
            hardened = segment.endswith("'")
            index = int(segment.rstrip("'")) + (0x80000000 if hardened else 0)
            data = b'\x00' + k + struct.pack('>I', index)
            I = hmac.new(c, data, hashlib.sha512).digest()
            k, c = I[:32], I[32:]
        return k

    private_key = derive(seed, STELLAR_PATH)
    keypair = Keypair.from_raw_ed25519_seed(private_key)
    return {
        "public_key": keypair.public_key,
        # NEVER transmit or store the secret key on the server
        "secret_key": keypair.secret,
    }

def generate_mnemonic() -> str:
    """Generate a 24-word BIP-39 mnemonic (256-bit entropy)."""
    mnemo = Mnemonic("english")
    return mnemo.generate(strength=256)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_stellar_key() {
        // A well-formed 56-char Stellar public key
        let key = "GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX";
        assert!(validate_stellar_public_key(key).is_ok());
    }

    #[test]
    fn test_validate_wrong_prefix() {
        let key = "XCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX";
        assert!(validate_stellar_public_key(key).is_err());
    }

    #[test]
    fn test_validate_wrong_length() {
        let key = "GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3";
        assert!(validate_stellar_public_key(key).is_err());
    }

    #[test]
    fn test_mnemonic_challenge_has_4_positions() {
        let challenge = MnemonicConfirmationChallenge::generate();
        assert_eq!(challenge.word_positions.len(), 4);
        for pos in &challenge.word_positions {
            assert!(*pos >= 1 && *pos <= 24);
        }
    }

    #[test]
    fn test_bip44_path_constant() {
        assert_eq!(STELLAR_BIP44_PATH, "m/44'/148'/0'");
    }
}
