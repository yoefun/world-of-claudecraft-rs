use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use woc_update::{pack_release, signing_key_from_hex, InstallState, PackOpts};

fn unique_tmp(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "woc-updater-bin-{tag}-{nanos}-{}",
        std::process::id()
    ));
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

fn copy_layout(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for name in ["woc-client", "woc-updater", "install.json"] {
        fs::copy(src.join(name), dst.join(name)).unwrap();
    }
}

#[test]
fn updater_bin_upgrades_prefix() {
    let root = unique_tmp("upgrade");
    let v100 = root.join("v100");
    let v101 = root.join("v101");
    write_layout(&v100, "1.0.0", b"OLD", b"UP-OLD");
    write_layout(&v101, "1.0.1", b"NEW", b"UP-NEW");

    let store_dir = root.join("store");
    let seed = "11".repeat(32);
    let sk = signing_key_from_hex(&seed).unwrap();
    let pk = sk.verifying_key();

    let manifest = pack_release(PackOpts {
        layout: &v101,
        prev_layout: Some(&v100),
        prev_version: Some("1.0.0"),
        out: &store_dir,
        version: "1.0.1",
        target: "x86_64-unknown-linux-gnu",
        protocol_rev: 6,
        signing_seed_hex: &seed,
    })
    .expect("pack_release");

    let manifest_path = store_dir.join(format!(
        "woc-rs-{}-{}.manifest.json",
        manifest.rewrite_version, manifest.target
    ));

    let prefix = root.join("prefix");
    copy_layout(&v100, &prefix);

    let exe = env!("CARGO_BIN_EXE_woc-updater");
    let st = Command::new(exe)
        .args([
            "--prefix",
            prefix.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--store",
            store_dir.to_str().unwrap(),
            "--once",
            "--no-exec",
            "--pubkey",
            &hex::encode(pk.to_bytes()),
            "--already-copied",
        ])
        .status()
        .unwrap();
    assert!(st.success(), "woc-updater failed with {:?}", st.code());
    assert_eq!(fs::read(prefix.join("woc-client")).unwrap(), b"NEW");
}
