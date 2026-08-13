use crate::{sha256_hex, UpdateError};
use qbsdiff::{Bsdiff, Bspatch};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeltaMeta {
    pub from: String,
    pub to: String,
    pub patches: Vec<PatchEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchEntry {
    pub path: String,
    pub sha256: String,
    pub new_sha256: String,
}

const DELTA_FILES: [&str; 2] = ["woc-client", "woc-updater"];

pub fn pack_delta(
    from_ver: &str,
    to_ver: &str,
    old_dir: &Path,
    new_dir: &Path,
) -> Result<Vec<u8>, UpdateError> {
    let mut patches = Vec::new();
    let mut patch_blobs: HashMap<String, Vec<u8>> = HashMap::new();

    for name in DELTA_FILES {
        let old_bytes = fs::read(old_dir.join(name))?;
        let new_bytes = fs::read(new_dir.join(name))?;
        if old_bytes == new_bytes {
            continue;
        }

        let mut patch = Vec::new();
        Bsdiff::new(&old_bytes, &new_bytes)
            .compare(Cursor::new(&mut patch))
            .map_err(|e| UpdateError::Delta(e.to_string()))?;

        patches.push(PatchEntry {
            path: name.to_string(),
            sha256: sha256_hex(&old_bytes),
            new_sha256: sha256_hex(&new_bytes),
        });
        patch_blobs.insert(name.to_string(), patch);
    }

    let meta = DeltaMeta {
        from: from_ver.to_string(),
        to: to_ver.to_string(),
        patches,
    };

    let mut tar_buf = Vec::new();
    {
        let mut ar = tar::Builder::new(&mut tar_buf);
        let json = serde_json::to_vec(&meta)?;
        let mut header = tar::Header::new_gnu();
        header.set_size(json.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        ar.append_data(&mut header, "delta.json", json.as_slice())?;

        for entry in &meta.patches {
            let patch = patch_blobs
                .get(&entry.path)
                .expect("patch blob for entry");
            let archive_name = format!("{}.bsdiff", entry.path);
            let mut header = tar::Header::new_gnu();
            header.set_size(patch.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            ar.append_data(&mut header, &archive_name, patch.as_slice())?;
        }
        ar.finish()?;
    }

    zstd::encode_all(&tar_buf[..], 3).map_err(|e| UpdateError::Msg(e.to_string()))
}

pub fn apply_delta(blob: &[u8], layout_dir: &Path) -> Result<DeltaMeta, UpdateError> {
    let tar_buf = zstd::decode_all(blob).map_err(|e| UpdateError::Msg(e.to_string()))?;

    let mut meta_json = Vec::new();
    let mut patch_blobs: HashMap<String, Vec<u8>> = HashMap::new();

    {
        let mut ar = tar::Archive::new(tar_buf.as_slice());
        for entry in ar.entries()? {
            let mut entry = entry?;
            let path = entry
                .path()?
                .to_string_lossy()
                .into_owned();
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            if path == "delta.json" {
                meta_json = data;
            } else if path.ends_with(".bsdiff") {
                let file_path = path.trim_end_matches(".bsdiff").to_string();
                patch_blobs.insert(file_path, data);
            }
        }
    }

    if meta_json.is_empty() {
        return Err(UpdateError::Delta("missing delta.json".into()));
    }

    let meta: DeltaMeta = serde_json::from_slice(&meta_json)?;
    let mut staged: Vec<PathBuf> = Vec::new();

    let apply_result = (|| {
        for patch_entry in &meta.patches {
            let patch = patch_blobs
                .get(&patch_entry.path)
                .ok_or_else(|| UpdateError::Delta(format!("missing patch for {}", patch_entry.path)))?;

            let old_path = layout_dir.join(&patch_entry.path);
            let old_bytes = fs::read(&old_path)?;
            if sha256_hex(&old_bytes) != patch_entry.sha256 {
                return Err(UpdateError::HashMismatch {
                    path: patch_entry.path.clone(),
                });
            }

            let mut out = Vec::new();
            Bspatch::new(patch)
                .map_err(|e| UpdateError::Delta(e.to_string()))?
                .apply(&old_bytes, Cursor::new(&mut out))
                .map_err(|e| UpdateError::Delta(e.to_string()))?;

            if sha256_hex(&out) != patch_entry.new_sha256 {
                return Err(UpdateError::HashMismatch {
                    path: patch_entry.path.clone(),
                });
            }

            let new_path = layout_dir.join(format!("{}.new", patch_entry.path));
            fs::write(&new_path, &out)?;
            staged.push(new_path);
        }
        Ok(())
    })();

    if let Err(e) = apply_result {
        for path in staged {
            let _ = fs::remove_file(path);
        }
        return Err(e);
    }

    for patch_entry in &meta.patches {
        let old_path = layout_dir.join(&patch_entry.path);
        let new_path = layout_dir.join(format!("{}.new", patch_entry.path));
        fs::rename(new_path, old_path)?;
    }

    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn delta_is_smaller_than_new_file_and_applies() {
        let root = unique_tmp("woc-delta");
        let old = root.join("old");
        let new = root.join("new");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        let base: Vec<u8> = (0..8000u32).map(|i| (i % 251) as u8).collect();
        let mut next = base.clone();
        next.splice(100..120, [7u8; 20]);
        fs::write(old.join("woc-client"), &base).unwrap();
        fs::write(old.join("woc-updater"), b"UP-OLD").unwrap();
        fs::write(new.join("woc-client"), &next).unwrap();
        fs::write(new.join("woc-updater"), b"UP-NEW").unwrap();

        let blob = pack_delta("1.0.0", "1.0.1", &old, &new).expect("delta");
        assert!(
            blob.len() < next.len(),
            "delta {} vs file {}",
            blob.len(),
            next.len()
        );

        apply_delta(&blob, &old).expect("apply");
        assert_eq!(fs::read(old.join("woc-client")).unwrap(), next);
        assert_eq!(fs::read(old.join("woc-updater")).unwrap(), b"UP-NEW");
    }

    #[test]
    fn corrupt_delta_does_not_change_files() {
        let root = unique_tmp("woc-delta-corrupt");
        let old = root.join("old");
        let new = root.join("new");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        let base: Vec<u8> = (0..8000u32).map(|i| (i % 251) as u8).collect();
        let mut next = base.clone();
        next.splice(100..120, [7u8; 20]);
        fs::write(old.join("woc-client"), &base).unwrap();
        fs::write(old.join("woc-updater"), b"UP-OLD").unwrap();
        fs::write(new.join("woc-client"), &next).unwrap();
        fs::write(new.join("woc-updater"), b"UP-NEW").unwrap();

        let mut blob = pack_delta("1.0.0", "1.0.1", &old, &new).expect("delta");
        assert!(!blob.is_empty());
        let flip = blob.len() / 2;
        blob[flip] ^= 0xff;

        let result = apply_delta(&blob, &old);
        assert!(result.is_err());
        assert_eq!(fs::read(old.join("woc-client")).unwrap(), base);
        assert_eq!(fs::read(old.join("woc-updater")).unwrap(), b"UP-OLD");
        assert!(!old.join("woc-client.new").exists());
        assert!(!old.join("woc-updater.new").exists());
    }
}
