//! Shared validation helpers.

use crate::error::{PersistError, PersistResult};

/// Validate username rules shared by backends.
pub fn validate_username(username: &str) -> PersistResult<()> {
    let trimmed = username.trim();
    if trimmed.len() < 3 || trimmed.len() > 32 {
        return Err(PersistError::InvalidInput(
            "username must be 3–32 characters".into(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(PersistError::InvalidInput(
            "username must be alphanumeric/underscore/hyphen".into(),
        ));
    }
    Ok(())
}

/// Validate character name.
pub fn validate_character_name(name: &str) -> PersistResult<()> {
    let trimmed = name.trim();
    if trimmed.len() < 2 || trimmed.len() > 24 {
        return Err(PersistError::InvalidInput(
            "character name must be 2–24 characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_rules() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username("abc").is_ok());
        assert!(validate_username("bad name").is_err());
        assert!(matches!(
            validate_character_name("A"),
            Err(PersistError::InvalidInput(_))
        ));
    }
}
