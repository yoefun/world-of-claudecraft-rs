use crate::{
    file_entry, pack_delta, pack_full, sha256_hex, sign_manifest, signing_key_from_hex, Artifact,
    Manifest, UpdateError,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub struct PackOpts<'a> {
    pub layout: &'a Path,
    pub prev_layout: Option<&'a Path>,
    pub prev_version: Option<&'a str>,
    pub out: &'a Path,
    pub version: &'a str,
    pub target: &'a str,
    pub protocol_rev: u32,
    pub signing_seed_hex: &'a str,
}

pub fn pack_release(opts: PackOpts<'_>) -> Result<Manifest, UpdateError> {
    fs::create_dir_all(opts.out)?;

    let full_name = format!("woc-rs-{}-{}.tar.zst", opts.version, opts.target);
    let full_blob = pack_full(opts.layout)?;
    fs::write(opts.out.join(&full_name), &full_blob)?;

    let mut delta_from = BTreeMap::new();
    if let (Some(prev_layout), Some(prev_version)) = (opts.prev_layout, opts.prev_version) {
        let delta_name = format!(
            "woc-rs-{}-to-{}-{}.wocdelta",
            prev_version, opts.version, opts.target
        );
        let delta_blob = pack_delta(prev_version, opts.version, prev_layout, opts.layout)?;
        fs::write(opts.out.join(&delta_name), &delta_blob)?;
        delta_from.insert(
            prev_version.to_string(),
            artifact_from_blob(delta_name, &delta_blob),
        );
    }

    let files = ["woc-client", "woc-updater", "install.json"]
        .into_iter()
        .map(|p| file_entry(opts.layout, p))
        .collect::<Result<Vec<_>, _>>()?;

    let mut manifest = Manifest {
        rewrite_version: opts.version.to_string(),
        protocol_rev: opts.protocol_rev,
        target: opts.target.to_string(),
        files,
        full: artifact_from_blob(full_name.clone(), &full_blob),
        delta_from,
        sig: String::new(),
    };

    let sk = signing_key_from_hex(opts.signing_seed_hex)?;
    sign_manifest(&mut manifest, &sk);

    let manifest_name = format!("woc-rs-{}-{}.manifest.json", opts.version, opts.target);
    fs::write(
        opts.out.join(&manifest_name),
        serde_json::to_vec(&manifest)?,
    )?;

    Ok(manifest)
}

fn artifact_from_blob(name: String, blob: &[u8]) -> Artifact {
    Artifact {
        name,
        sha256: sha256_hex(blob),
        size: blob.len() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{verify_manifest, InstallState};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("woc-release-{tag}-{nanos}-{}", std::process::id()));
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

    #[test]
    fn pack_release_writes_signed_full_delta_and_manifest() {
        let root = unique_tmp("pack");
        let prev = root.join("prev");
        let layout = root.join("layout");
        let out = root.join("out");
        write_layout(&prev, "1.4.0", b"OLD-CLIENT", b"OLD-UP");
        write_layout(&layout, "1.5.0", b"NEW-CLIENT", b"NEW-UP");

        let seed = "11".repeat(32);
        let sk = signing_key_from_hex(&seed).unwrap();
        let pk = sk.verifying_key();

        let manifest = pack_release(PackOpts {
            layout: &layout,
            prev_layout: Some(&prev),
            prev_version: Some("1.4.0"),
            out: &out,
            version: "1.5.0",
            target: "x86_64-unknown-linux-gnu",
            protocol_rev: 6,
            signing_seed_hex: &seed,
        })
        .expect("pack_release");

        let entries: Vec<_> = fs::read_dir(&out).unwrap().collect();
        assert_eq!(entries.len(), 3, "expected full, delta, manifest");

        assert_eq!(manifest.rewrite_version, "1.5.0");
        assert_eq!(manifest.protocol_rev, 6);
        assert_eq!(manifest.target, "x86_64-unknown-linux-gnu");
        assert_eq!(manifest.files.len(), 3);
        assert!(manifest.delta_from.contains_key("1.4.0"));
        assert_eq!(
            manifest.full.name,
            "woc-rs-1.5.0-x86_64-unknown-linux-gnu.tar.zst"
        );
        let delta = manifest.delta_from.get("1.4.0").unwrap();
        assert_eq!(
            delta.name,
            "woc-rs-1.4.0-to-1.5.0-x86_64-unknown-linux-gnu.wocdelta"
        );

        assert!(out.join(&manifest.full.name).exists());
        assert!(out.join(&delta.name).exists());
        assert!(out
            .join(format!(
                "woc-rs-{}-{}.manifest.json",
                manifest.rewrite_version, manifest.target
            ))
            .exists());

        verify_manifest(&manifest, &pk).unwrap();

        let written: Manifest = serde_json::from_slice(
            &fs::read(out.join(format!(
                "woc-rs-{}-{}.manifest.json",
                manifest.rewrite_version, manifest.target
            )))
            .unwrap(),
        )
        .unwrap();
        verify_manifest(&written, &pk).unwrap();
    }
}
