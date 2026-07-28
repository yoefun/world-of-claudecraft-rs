//! Rewrite / upstream version pin for World of ClaudeCraft (Rust).
//!
//! Values are kept in sync with the repo-root `VERSION.toml`.

use serde::Serialize;

/// Rewrite crate / product version (semver).
pub const REWRITE_VERSION: &str = "1.0.0-pre";

/// Upstream TypeScript World of ClaudeCraft version this rewrite tracks.
pub const UPSTREAM_VERSION: &str = "0.31.0";

/// Upstream git commit SHA pinned for tracking.
pub const UPSTREAM_COMMIT: &str = "a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9";

/// Upstream repository URL.
pub const UPSTREAM_REPO: &str = "https://github.com/levy-street/world-of-claudecraft";

/// Current parity milestone name.
pub const PARITY_TARGET: &str = "completion";

/// Short footer string for HUD / window titles.
pub fn footer() -> String {
    format!("WoC-rs {REWRITE_VERSION} · upstream {UPSTREAM_VERSION}")
}

/// JSON-serializable version payload for HTTP `/version`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionInfo {
    pub rewrite_version: &'static str,
    pub upstream_version: &'static str,
    pub upstream_commit: &'static str,
    pub upstream_repo: &'static str,
    pub parity_target: &'static str,
}

impl VersionInfo {
    pub fn current() -> Self {
        Self {
            rewrite_version: REWRITE_VERSION,
            upstream_version: UPSTREAM_VERSION,
            upstream_commit: UPSTREAM_COMMIT,
            upstream_repo: UPSTREAM_REPO,
            parity_target: PARITY_TARGET,
        }
    }
}

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
}
