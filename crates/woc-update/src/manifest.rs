use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub rewrite_version: String,
    pub protocol_rev: u32,
    pub target: String,
    pub files: Vec<FileEntry>,
    pub full: Artifact,
    #[serde(default)]
    pub delta_from: BTreeMap<String, Artifact>,
    #[serde(default)]
    pub sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallState {
    pub rewrite_version: String,
    pub target: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256_hex;

    #[test]
    fn sha256_hex_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn install_state_roundtrip() {
        let s = InstallState {
            rewrite_version: "1.5.0".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: InstallState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.rewrite_version, "1.5.0");
    }

    #[test]
    fn manifest_deserializes_without_delta_or_sig() {
        let json = r#"{
            "rewrite_version": "1.5.0",
            "protocol_rev": 6,
            "target": "x86_64-unknown-linux-gnu",
            "files": [],
            "full": {"name": "full.tar.zst", "sha256": "ab", "size": 1}
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        assert!(m.delta_from.is_empty());
        assert!(m.sig.is_empty());
        assert_eq!(m.full.name, "full.tar.zst");
    }
}
