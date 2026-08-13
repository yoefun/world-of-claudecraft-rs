use crate::{sha256_hex, FileEntry, UpdateError};
use std::fs::File;
use std::path::Path;

pub fn pack_full(layout_dir: &Path) -> Result<Vec<u8>, UpdateError> {
    let mut tar_buf = Vec::new();
    {
        let mut ar = tar::Builder::new(&mut tar_buf);
        for name in ["woc-client", "woc-updater", "install.json"] {
            let path = layout_dir.join(name);
            let mut file = File::open(&path)?;
            let mut header = tar::Header::new_gnu();
            let meta = file.metadata()?;
            header.set_size(meta.len());
            header.set_mode(if name == "install.json" { 0o644 } else { 0o755 });
            header.set_cksum();
            ar.append_data(&mut header, name, &mut file)?;
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
        let e = file_entry(&dest, "woc-client").unwrap();
        assert_eq!(e.sha256, sha256_hex(b"GAME"));
        assert_eq!(e.size, 4);
    }
}
