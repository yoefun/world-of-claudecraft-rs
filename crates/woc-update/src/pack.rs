use crate::{sha256_hex, FileEntry, UpdateError};
use std::fs::{self, File};
use std::path::Path;

/// Return all regular files in a client layout as normalized archive paths.
pub fn layout_files(layout_dir: &Path) -> Result<Vec<String>, UpdateError> {
    let mut files = Vec::new();
    collect_layout_files(layout_dir, layout_dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_layout_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> Result<(), UpdateError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_layout_files(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| UpdateError::Msg("layout path escaped root".into()))?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(relative);
        } else {
            return Err(UpdateError::Msg(format!(
                "unsupported non-regular layout entry: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Check whether all layout files except the supplied paths have identical bytes.
pub fn layout_files_match_except(
    old_dir: &Path,
    new_dir: &Path,
    excluded: &[&str],
) -> Result<bool, UpdateError> {
    let mut old_files = layout_files(old_dir)?;
    let mut new_files = layout_files(new_dir)?;
    old_files.retain(|path| !excluded.contains(&path.as_str()));
    new_files.retain(|path| !excluded.contains(&path.as_str()));
    if old_files != new_files {
        return Ok(false);
    }
    for relative in old_files {
        if fs::read(old_dir.join(&relative))? != fs::read(new_dir.join(&relative))? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn pack_full(layout_dir: &Path) -> Result<Vec<u8>, UpdateError> {
    let mut tar_buf = Vec::new();
    {
        let mut ar = tar::Builder::new(&mut tar_buf);
        for relative in layout_files(layout_dir)? {
            let path = layout_dir.join(&relative);
            let mut file = File::open(&path)?;
            let mut header = tar::Header::new_gnu();
            let meta = file.metadata()?;
            header.set_size(meta.len());
            header.set_mode(
                if matches!(relative.as_str(), "woc-client" | "woc-updater") {
                    0o755
                } else {
                    0o644
                },
            );
            header.set_cksum();
            ar.append_data(&mut header, &relative, &mut file)?;
        }
        ar.finish()?;
    }
    zstd::encode_all(&tar_buf[..], 3).map_err(|e| UpdateError::Msg(e.to_string()))
}

pub fn unpack_full(archive: &[u8], dest: &Path) -> Result<(), UpdateError> {
    let tar_buf = zstd::decode_all(archive).map_err(|e| UpdateError::Msg(e.to_string()))?;
    let mut ar = tar::Archive::new(tar_buf.as_slice());
    ar.unpack(dest)?;
    Ok(())
}

pub fn file_entry(layout_dir: &Path, rel: &str) -> Result<FileEntry, UpdateError> {
    let path = layout_dir.join(rel);
    let bytes = std::fs::read(&path)?;
    Ok(FileEntry {
        path: rel.to_string(),
        sha256: sha256_hex(&bytes),
        size: bytes.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256_hex;
    use std::fs;
    use std::path::PathBuf;

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!("woc-pack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn pack_unpack_restores_bytes_and_mode() {
        let layout = tmp().join("layout");
        fs::create_dir_all(&layout).unwrap();
        fs::write(layout.join("woc-client"), b"GAME").unwrap();
        fs::write(layout.join("woc-updater"), b"UP").unwrap();
        fs::create_dir_all(layout.join("assets/models")).unwrap();
        fs::write(layout.join("assets/models/test.glb"), b"GLB").unwrap();
        fs::write(
            layout.join("install.json"),
            b"{\"rewrite_version\":\"1.0.0\",\"target\":\"t\"}",
        )
        .unwrap();

        let blob = pack_full(&layout).expect("pack");
        assert!(!blob.is_empty());
        let dest = tmp().join("out");
        unpack_full(&blob, &dest).expect("unpack");
        assert_eq!(fs::read(dest.join("woc-client")).unwrap(), b"GAME");
        assert_eq!(fs::read(dest.join("woc-updater")).unwrap(), b"UP");
        assert_eq!(
            fs::read(dest.join("assets/models/test.glb")).unwrap(),
            b"GLB"
        );
        let e = file_entry(&dest, "woc-client").unwrap();
        assert_eq!(e.sha256, sha256_hex(b"GAME"));
        assert_eq!(e.size, 4);
    }
}
