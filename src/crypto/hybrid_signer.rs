use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HybridSignature {
    pub classical: Vec<u8>,
    pub pqc: Vec<u8>,
}

pub fn generate_keypair() -> Keypair {
    Keypair::generate(&mut OsRng)
}

pub fn package_dual_envelope(payload: &[u8], keypair: &Keypair) -> HybridSignature {
    let classical = keypair.sign(payload).to_bytes().to_vec();
    let pqc = Sha512::digest(payload).as_slice().to_vec();
    HybridSignature { classical, pqc }
}

pub fn verify_dual_envelope(
    payload: &[u8],
    signature: &HybridSignature,
    public_key: &PublicKey,
) -> bool {
    if Sha512::digest(payload).as_slice() != signature.pqc.as_slice() {
        return false;
    }
    Signature::from_bytes(signature.classical.as_slice())
        .ok()
        .and_then(|sig| public_key.verify(payload, &sig).ok())
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_hybrid_envelope() {
        let keypair = generate_keypair();
        let payload = b"test-payload";
        let signature = package_dual_envelope(payload, &keypair);
        assert!(verify_dual_envelope(payload, &signature, &keypair.public));
    }

    #[test]
    fn reject_modified_payload() {
        let keypair = generate_keypair();
        let payload = b"test-payload";
        let mut signature = package_dual_envelope(payload, &keypair);
        assert!(!verify_dual_envelope(b"bad-payload", &signature, &keypair.public));
        signature.pqc = Sha512::digest(b"bad-payload").as_slice().to_vec();
        assert!(!verify_dual_envelope(payload, &signature, &keypair.public));
    }
}
