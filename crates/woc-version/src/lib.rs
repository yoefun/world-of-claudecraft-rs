//! Rewrite / upstream version pin for World of ClaudeCraft (Rust).
//!
//! Values are kept in sync with the repo-root `VERSION.toml`.

use serde::{Deserialize, Serialize};

/// Rewrite crate / product version (semver).
pub const REWRITE_VERSION: &str = "1.25.0";

/// Upstream TypeScript World of ClaudeCraft version this rewrite tracks.
pub const UPSTREAM_VERSION: &str = "0.31.0";

/// Upstream git commit SHA pinned for tracking.
pub const UPSTREAM_COMMIT: &str = "a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9";

/// Upstream repository URL.
pub const UPSTREAM_REPO: &str = "https://github.com/levy-street/world-of-claudecraft";

/// Current parity milestone name.
pub const PARITY_TARGET: &str = "class-depth";

/// Short footer string for HUD / window titles.
pub fn footer() -> String {
    format!("WoC-rs {REWRITE_VERSION} · upstream {UPSTREAM_VERSION}")
}

/// JSON-serializable version payload for HTTP `/version`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionInfo {
    pub rewrite_version: String,
    pub upstream_version: String,
    pub upstream_commit: String,
    pub upstream_repo: String,
    pub parity_target: String,
    #[serde(default)]
    pub protocol_rev: u32,
    #[serde(default)]
    pub min_client_version: String,
    #[serde(default)]
    pub update_manifest_url: String,
}

impl VersionInfo {
    pub fn current(protocol_rev: u32) -> Self {
        Self {
            rewrite_version: REWRITE_VERSION.to_string(),
            upstream_version: UPSTREAM_VERSION.to_string(),
            upstream_commit: UPSTREAM_COMMIT.to_string(),
            upstream_repo: UPSTREAM_REPO.to_string(),
            parity_target: PARITY_TARGET.to_string(),
            protocol_rev,
            min_client_version: min_client_version_from_env(),
            update_manifest_url: std::env::var("WOC_UPDATE_MANIFEST_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_default(),
        }
    }

    pub fn realm_identity(&self) -> RealmIdentity {
        let min = if self.min_client_version.is_empty() {
            self.rewrite_version.clone()
        } else {
            self.min_client_version.clone()
        };
        RealmIdentity {
            rewrite_version: self.rewrite_version.clone(),
            protocol_rev: if self.protocol_rev == 0 {
                None
            } else {
                Some(self.protocol_rev)
            },
            min_client_version: min,
        }
    }
}

mod compat;
pub use compat::{
    check_compat, min_client_version_from_env, parse_semver, ClientIdentity, Compat, RealmIdentity,
    SemVer,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn constants_match_version_toml() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("VERSION.toml");
        let text = fs::read_to_string(&root).expect("VERSION.toml");
        let value: toml::Value = text.parse().expect("parse VERSION.toml");
        assert_eq!(value["rewrite_version"].as_str().unwrap(), REWRITE_VERSION);
        assert_eq!(
            value["upstream_version"].as_str().unwrap(),
            UPSTREAM_VERSION
        );
        assert_eq!(value["upstream_commit"].as_str().unwrap(), UPSTREAM_COMMIT);
        assert_eq!(value["parity_target"].as_str().unwrap(), PARITY_TARGET);
    }

    #[test]
    fn footer_contains_both_versions() {
        let f = footer();
        assert!(f.contains(REWRITE_VERSION));
        assert!(f.contains(UPSTREAM_VERSION));
    }

    #[test]
    fn current_includes_protocol_and_min_client() {
        let info = VersionInfo::current(6);
        assert_eq!(info.rewrite_version, REWRITE_VERSION);
        assert_eq!(info.protocol_rev, 6);
        assert_eq!(info.min_client_version, REWRITE_VERSION);
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"protocol_rev\":6"));
        assert!(json.contains("min_client_version"));
    }

    #[test]
    fn legacy_version_json_deserializes_with_defaults() {
        let json = r#"{
            "rewrite_version": "1.3.0",
            "upstream_version": "0.31.0",
            "upstream_commit": "abc",
            "upstream_repo": "https://example.invalid",
            "parity_target": "online-hard"
        }"#;
        let info: VersionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.rewrite_version, "1.3.0");
        assert_eq!(info.protocol_rev, 0);
        assert!(info.min_client_version.is_empty());
        let realm = info.realm_identity();
        assert_eq!(realm.protocol_rev, None);
        assert_eq!(realm.min_client_version, "1.3.0");
        let c = check_compat(
            &ClientIdentity {
                rewrite_version: "1.3.0".into(),
                protocol_rev: 6,
            },
            &realm,
        );
        assert!(c.is_ok());
    }

    #[test]
    fn min_client_env_defaults_to_rewrite_version() {
        assert_eq!(min_client_version_from_env(), REWRITE_VERSION);
    }

    #[test]
    fn legacy_json_has_empty_update_manifest_url() {
        let json = r#"{"rewrite_version":"1.4.0","upstream_version":"0.31.0","upstream_commit":"x","upstream_repo":"x","parity_target":"client-compat","protocol_rev":6,"min_client_version":"1.4.0"}"#;
        let info: VersionInfo = serde_json::from_str(json).unwrap();
        assert!(info.update_manifest_url.is_empty());
    }
}
