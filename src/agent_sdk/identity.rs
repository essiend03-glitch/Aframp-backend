use crate::agent_sdk::error::{AgentError, AgentResult};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use stellar_strkey::ed25519::{PrivateKey as StrkeySecret, PublicKey as StrkeyPublic};
use zeroize::Zeroize;

/// Cryptographic identity for an AI agent.
///
/// Holds the agent's ed25519 keypair and derives the Stellar account address.
/// The secret key is zeroized on drop.
#[derive(Debug)]
pub struct AgentIdentity {
    /// Human-readable name for this agent personality.
    pub name: String,
    /// Stellar public key (G-address).
    pub public_key: String,
    /// Raw signing key — zeroized on drop.
    signing_key: SigningKey,
}

impl Drop for AgentIdentity {
    fn drop(&mut self) {
        self.signing_key.to_bytes().zeroize();
    }
}

/// Serialisable snapshot used for secure storage / export.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentIdentityExport {
    pub name: String,
    pub public_key: String,
    /// Base64-encoded secret seed (store encrypted at rest).
    pub secret_seed_b64: String,
}

impl AgentIdentity {
    /// Generate a brand-new keypair for an agent.
    pub fn generate(name: impl Into<String>) -> AgentResult<Self> {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key: VerifyingKey = signing_key.verifying_key();

        let public_key = StrkeyPublic(verifying_key.to_bytes())
            .to_string();

        Ok(Self {
            name: name.into(),
            public_key,
            signing_key,
        })
    }

    /// Restore an identity from a Stellar secret seed (S-address).
    pub fn from_secret_seed(name: impl Into<String>, secret_seed: &str) -> AgentResult<Self> {
        let strkey = StrkeySecret::from_string(secret_seed)
            .map_err(|e| AgentError::Identity(format!("Invalid secret seed: {e}")))?;

        let signing_key = SigningKey::from_bytes(&strkey.0);
        let verifying_key: VerifyingKey = signing_key.verifying_key();
        let public_key = StrkeyPublic(verifying_key.to_bytes()).to_string();

        Ok(Self {
            name: name.into(),
            public_key,
            signing_key,
        })
    }

    /// Export identity for secure storage. Caller is responsible for encrypting
    /// `secret_seed_b64` before persisting.
    pub fn export(&self) -> AgentIdentityExport {
        let seed_bytes = self.signing_key.to_bytes();
        let secret_seed = StrkeySecret(seed_bytes).to_string();
        AgentIdentityExport {
            name: self.name.clone(),
            public_key: self.public_key.clone(),
            secret_seed_b64: base64::encode(secret_seed.as_bytes()),
        }
    }

    /// Sign arbitrary bytes — used internally for transaction signing.
    pub(crate) fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        self.signing_key.sign(message).to_bytes()
    }

    /// Return the raw secret seed string (S-address).
    pub(crate) fn secret_seed(&self) -> String {
        StrkeySecret(self.signing_key.to_bytes()).to_string()
    }
}
