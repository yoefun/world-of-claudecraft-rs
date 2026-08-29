use crate::{Manifest, UpdateError};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

pub fn signing_key_from_hex(seed32: &str) -> Result<SigningKey, UpdateError> {
    let bytes = hex::decode(seed32).map_err(|_| UpdateError::Signature)?;
    let seed: [u8; 32] = bytes.try_into().map_err(|_| UpdateError::Signature)?;
    Ok(SigningKey::from_bytes(&seed))
}

pub fn verifying_key_from_hex(pub32: &str) -> Result<VerifyingKey, UpdateError> {
    let bytes = hex::decode(pub32).map_err(|_| UpdateError::Signature)?;
    let pk: [u8; 32] = bytes.try_into().map_err(|_| UpdateError::Signature)?;
    VerifyingKey::from_bytes(&pk).map_err(|_| UpdateError::Signature)
}

pub fn sign_manifest(m: &mut Manifest, key: &SigningKey) {
    m.sig.clear();
    let body = serde_json::to_vec(m).expect("manifest json");
    let sig = key.sign(&body);
    m.sig = hex::encode(sig.to_bytes());
}

pub fn verify_manifest(m: &Manifest, pk: &VerifyingKey) -> Result<(), UpdateError> {
    let mut clone = m.clone();
    let sig_hex = clone.sig.clone();
    clone.sig.clear();
    let body = serde_json::to_vec(&clone)?;
    let bytes = hex::decode(&sig_hex).map_err(|_| UpdateError::Signature)?;
    let sig = Signature::from_slice(&bytes).map_err(|_| UpdateError::Signature)?;
    pk.verify(&body, &sig).map_err(|_| UpdateError::Signature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Artifact, Manifest, UpdateError};
    use std::collections::BTreeMap;

    fn minimal_manifest() -> Manifest {
        Manifest {
            rewrite_version: "1.5.0".into(),
            protocol_rev: 6,
            target: "x86_64-unknown-linux-gnu".into(),
            files: vec![],
            full: Artifact {
                name: "full.tar.zst".into(),
                sha256: "ab".into(),
                size: 1,
            },
            delta_from: BTreeMap::new(),
            sig: String::new(),
        }
    }

    #[test]
    fn sign_then_verify_ok() {
        let sk = signing_key_from_hex(&"11".repeat(32)).unwrap();
        let pk = sk.verifying_key();
        let mut m = minimal_manifest();
        sign_manifest(&mut m, &sk);
        assert!(!m.sig.is_empty());
        verify_manifest(&m, &pk).unwrap();
    }

    #[test]
    fn tampered_version_fails_verify() {
        let sk = signing_key_from_hex(&"11".repeat(32)).unwrap();
        let pk = sk.verifying_key();
        let mut m = minimal_manifest();
        sign_manifest(&mut m, &sk);
        m.rewrite_version = "9.9.9".into();
        assert!(matches!(
            verify_manifest(&m, &pk),
            Err(UpdateError::Signature)
        ));
    }
}
