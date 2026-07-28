//! Argon2 password hashing.

use crate::error::{PersistError, PersistResult};
use argon2::password_hash::rand_core::OsRng;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

/// Hash a plaintext password for storage.
pub fn hash_password(password: &str) -> PersistResult<String> {
    if password.len() < 6 {
        return Err(PersistError::InvalidInput(
            "password must be at least 6 characters".into(),
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PersistError::Password(e.to_string()))?
        .to_string();
    Ok(hash)
}

/// Verify plaintext against a stored PHC hash string.
pub fn verify_password(password: &str, password_hash: &str) -> PersistResult<bool> {
    let parsed =
        PasswordHash::new(password_hash).map_err(|e| PersistError::Password(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash_password("secret1").expect("hash");
        assert!(verify_password("secret1", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn rejects_short_password() {
        assert!(matches!(
            hash_password("abc"),
            Err(PersistError::InvalidInput(_))
        ));
    }
}
