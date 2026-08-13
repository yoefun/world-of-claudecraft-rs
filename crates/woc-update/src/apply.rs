use crate::{
    apply_delta, file_entry, install_json_bytes, plan_fetch, sha256_hex, unpack_full,
    ArtifactStore, FetchPlan, InstallState, Manifest, UpdateError,
};
use std::fs;
use std::path::{Path, PathBuf};

const LAYOUT_FILES: [&str; 3] = ["woc-client", "woc-updater", "install.json"];

fn with_suffix(prefix: &Path, suffix: &str) -> PathBuf {
    let mut os = prefix.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

fn read_install_state(prefix: &Path) -> Result<InstallState, UpdateError> {
    let path = prefix.join("install.json");
    if !path.exists() {
        return Ok(InstallState {
            rewrite_version: "0.0.0".into(),
            target: String::new(),
        });
    }
    let json = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&json)?)
}

fn remove_staging(staging: &Path) {
    let _ = fs::remove_dir_all(staging);
}

pub fn apply_update(
    prefix: &Path,
    remote: &Manifest,
    store: &dyn ArtifactStore,
) -> Result<FetchPlan, UpdateError> {
    let mut local = read_install_state(prefix)?;
    if local.target.is_empty() {
        local.target = remote.target.clone();
    }
    let plan = plan_fetch(&local, remote)?;
    if matches!(plan, FetchPlan::Nothing) {
        return Ok(plan);
    }

    let blob = match &plan {
        FetchPlan::Delta { artifact, .. } | FetchPlan::Full { artifact } => {
            let blob = store.fetch(&artifact.name)?;
            if sha256_hex(&blob) != artifact.sha256 {
                return Err(UpdateError::HashMismatch {
                    path: artifact.name.clone(),
                });
            }
            blob
        }
        FetchPlan::Nothing => unreachable!(),
    };

    let staging = with_suffix(prefix, ".staging");
    let backup = with_suffix(prefix, ".backup");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }

    let apply_result = (|| {
        fs::create_dir_all(&staging)?;

        match &plan {
            FetchPlan::Full { .. } => unpack_full(&blob, &staging)?,
            FetchPlan::Delta { .. } => {
                for name in LAYOUT_FILES {
                    let src = prefix.join(name);
                    if src.exists() {
                        fs::copy(&src, staging.join(name))?;
                    }
                }
                apply_delta(&blob, &staging)?;
            }
            FetchPlan::Nothing => unreachable!(),
        }

        fs::write(
            staging.join("install.json"),
            install_json_bytes(&remote.rewrite_version, &remote.target)?,
        )?;

        for fe in &remote.files {
            let entry = file_entry(&staging, &fe.path)?;
            if entry.sha256 != fe.sha256 {
                return Err(UpdateError::HashMismatch {
                    path: fe.path.clone(),
                });
            }
        }

        Ok::<(), UpdateError>(())
    })();

    if let Err(e) = apply_result {
        remove_staging(&staging);
        return Err(e);
    }

    if backup.exists() {
        fs::remove_dir_all(&backup)?;
    }
    fs::rename(prefix, &backup)?;
    if let Err(e) = fs::rename(&staging, prefix) {
        let _ = fs::rename(&backup, prefix);
        return Err(e.into());
    }

    Ok(plan)
}

/// Like [`apply_update`], but a failed delta apply retries once with the full archive.
pub fn apply_update_with_full_fallback(
    prefix: &Path,
    remote: &Manifest,
    store: &dyn ArtifactStore,
) -> Result<FetchPlan, UpdateError> {
    let mut local = read_install_state(prefix)?;
    if local.target.is_empty() {
        local.target = remote.target.clone();
    }
    let planned = plan_fetch(&local, remote)?;
    match apply_update(prefix, remote, store) {
        Ok(plan) => Ok(plan),
        Err(_) if matches!(planned, FetchPlan::Delta { .. }) => {
            let mut full_only = remote.clone();
            full_only.delta_from.clear();
            apply_update(prefix, &full_only, store)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pack_delta, pack_full, sign_manifest, signing_key_from_hex, Artifact, DirStore, FileEntry,
    };
    use ed25519_dalek::SigningKey;
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("woc-apply-{tag}-{nanos}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_layout(dir: &Path, ver: &str, client: &[u8], updater: &[u8]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("woc-client"), client).unwrap();
        fs::write(dir.join("woc-updater"), updater).unwrap();
        let install = InstallState {
            rewrite_version: ver.into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        fs::write(
            dir.join("install.json"),
            serde_json::to_string(&install).unwrap(),
        )
        .unwrap();
    }

    fn file_entries(dir: &Path) -> Vec<FileEntry> {
        ["woc-client", "woc-updater", "install.json"]
            .into_iter()
            .map(|p| file_entry(dir, p).unwrap())
            .collect()
    }

    fn build_manifest(
        v101_dir: &Path,
        full_blob: &[u8],
        delta_blob: Option<&[u8]>,
        from_ver: Option<&str>,
        sk: &SigningKey,
    ) -> Manifest {
        let full = Artifact {
            name: "full.tar.zst".into(),
            sha256: sha256_hex(full_blob),
            size: full_blob.len() as u64,
        };
        let mut delta_from = BTreeMap::new();
        if let (Some(blob), Some(from)) = (delta_blob, from_ver) {
            delta_from.insert(
                from.into(),
                Artifact {
                    name: "delta.wocdelta".into(),
                    sha256: sha256_hex(blob),
                    size: blob.len() as u64,
                },
            );
        }
        let mut m = Manifest {
            rewrite_version: "1.0.1".into(),
            protocol_rev: 6,
            target: "x86_64-unknown-linux-gnu".into(),
            files: file_entries(v101_dir),
            full,
            delta_from,
            sig: String::new(),
        };
        sign_manifest(&mut m, sk);
        m
    }

    fn write_store(
        store_dir: &Path,
        manifest: &Manifest,
        full_blob: &[u8],
        delta_blob: Option<&[u8]>,
    ) {
        fs::create_dir_all(store_dir).unwrap();
        fs::write(store_dir.join(&manifest.full.name), full_blob).unwrap();
        if let Some(blob) = delta_blob {
            if let Some(delta) = manifest.delta_from.values().next() {
                fs::write(store_dir.join(&delta.name), blob).unwrap();
            }
        }
    }

    fn copy_layout(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).unwrap();
        for name in LAYOUT_FILES {
            fs::copy(src.join(name), dst.join(name)).unwrap();
        }
    }

    struct Fixture {
        _root: PathBuf,
        _v100: PathBuf,
        v101: PathBuf,
        prefix: PathBuf,
        store_dir: PathBuf,
        full_blob: Vec<u8>,
        delta_blob: Vec<u8>,
        sk: SigningKey,
    }

    impl Fixture {
        fn new() -> Self {
            let root = unique_tmp("fixture");
            let v100 = root.join("v100");
            let v101 = root.join("v101");
            write_layout(&v100, "1.0.0", b"V100", b"UP100");
            write_layout(&v101, "1.0.1", b"V101", b"UP101");
            let full_blob = pack_full(&v101).unwrap();
            let delta_blob = pack_delta("1.0.0", "1.0.1", &v100, &v101).unwrap();
            let sk = signing_key_from_hex(&"11".repeat(32)).unwrap();
            let prefix = root.join("prefix");
            copy_layout(&v100, &prefix);
            let store_dir = root.join("store");
            Self {
                _root: root,
                _v100: v100,
                v101,
                prefix,
                store_dir,
                full_blob,
                delta_blob,
                sk,
            }
        }
    }

    #[test]
    fn apply_update_delta_success() {
        let fx = Fixture::new();
        let manifest = build_manifest(
            &fx.v101,
            &fx.full_blob,
            Some(&fx.delta_blob),
            Some("1.0.0"),
            &fx.sk,
        );
        write_store(
            &fx.store_dir,
            &manifest,
            &fx.full_blob,
            Some(&fx.delta_blob),
        );
        let store = DirStore {
            root: fx.store_dir.clone(),
        };

        let plan = apply_update(&fx.prefix, &manifest, &store).unwrap();
        assert!(matches!(plan, FetchPlan::Delta { .. }));
        assert_eq!(fs::read(fx.prefix.join("woc-client")).unwrap(), b"V101");
        assert_eq!(fs::read(fx.prefix.join("woc-updater")).unwrap(), b"UP101");
        let install: InstallState =
            serde_json::from_slice(&fs::read(fx.prefix.join("install.json")).unwrap()).unwrap();
        assert_eq!(install.rewrite_version, "1.0.1");
        assert_eq!(install.target, "x86_64-unknown-linux-gnu");
        assert!(!with_suffix(&fx.prefix, ".staging").exists());
    }

    #[test]
    fn apply_update_corrupt_delta_leaves_prefix_unchanged() {
        let fx = Fixture::new();
        let manifest = build_manifest(
            &fx.v101,
            &fx.full_blob,
            Some(&fx.delta_blob),
            Some("1.0.0"),
            &fx.sk,
        );
        write_store(
            &fx.store_dir,
            &manifest,
            &fx.full_blob,
            Some(&fx.delta_blob),
        );
        let delta_path = fx.store_dir.join("delta.wocdelta");
        let mut stored = fs::read(&delta_path).unwrap();
        let flip = stored.len() / 2;
        stored[flip] ^= 0xff;
        fs::write(&delta_path, &stored).unwrap();
        let store = DirStore {
            root: fx.store_dir.clone(),
        };

        assert!(apply_update(&fx.prefix, &manifest, &store).is_err());
        assert_eq!(fs::read(fx.prefix.join("woc-client")).unwrap(), b"V100");
        assert_eq!(fs::read(fx.prefix.join("woc-updater")).unwrap(), b"UP100");
        let install: InstallState =
            serde_json::from_slice(&fs::read(fx.prefix.join("install.json")).unwrap()).unwrap();
        assert_eq!(install.rewrite_version, "1.0.0");
        assert!(!with_suffix(&fx.prefix, ".staging").exists());
    }

    #[test]
    fn apply_update_full_fallback_when_no_delta_from() {
        let fx = Fixture::new();
        let manifest = build_manifest(&fx.v101, &fx.full_blob, None, None, &fx.sk);
        write_store(&fx.store_dir, &manifest, &fx.full_blob, None);
        let store = DirStore {
            root: fx.store_dir.clone(),
        };

        let plan = apply_update(&fx.prefix, &manifest, &store).unwrap();
        assert!(matches!(plan, FetchPlan::Full { .. }));
        assert_eq!(fs::read(fx.prefix.join("woc-client")).unwrap(), b"V101");
        assert_eq!(fs::read(fx.prefix.join("woc-updater")).unwrap(), b"UP101");
        let install: InstallState =
            serde_json::from_slice(&fs::read(fx.prefix.join("install.json")).unwrap()).unwrap();
        assert_eq!(install.rewrite_version, "1.0.1");
    }

    #[test]
    fn apply_update_full_fallback_after_corrupt_delta() {
        let fx = Fixture::new();
        let manifest = build_manifest(
            &fx.v101,
            &fx.full_blob,
            Some(&fx.delta_blob),
            Some("1.0.0"),
            &fx.sk,
        );
        write_store(
            &fx.store_dir,
            &manifest,
            &fx.full_blob,
            Some(&fx.delta_blob),
        );
        let delta_path = fx.store_dir.join("delta.wocdelta");
        let mut stored = fs::read(&delta_path).unwrap();
        let flip = stored.len() / 2;
        stored[flip] ^= 0xff;
        fs::write(&delta_path, &stored).unwrap();
        let store = DirStore {
            root: fx.store_dir.clone(),
        };

        let plan = apply_update_with_full_fallback(&fx.prefix, &manifest, &store).unwrap();
        assert!(matches!(plan, FetchPlan::Full { .. }));
        assert_eq!(fs::read(fx.prefix.join("woc-client")).unwrap(), b"V101");
    }
}
