# Client update packages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rewrite `1.5.0` / `client-update`: a `woc-update` crate that packs a signed full `.tar.zst` plus a per-file bsdiff delta from the previous tag, and a `woc-updater` that applies the smaller artifact then execs `woc-client`.

**Architecture:** Leaf crate (no Bevy / sim / axum). Library owns manifest, sha256, pack, qbsdiff, ed25519, `plan_fetch`, staging apply. `woc-pack` is CI; `woc-updater` is the process players start. HTTP is an `ArtifactStore`; unit tests use a temp directory. Linux x86_64 first. Depends on 1.4.0 for the title Update button only — packer tests do not.

**Tech Stack:** Rust 2021, `qbsdiff` 1.4, `tar` 0.4, `zstd` 0.13, `sha2` 0.10, `hex` 0.4, `ed25519-dalek` 2, `thiserror` 2, `ureq` 2.12 (updater HTTP), `serde_json`, existing `woc-version`. GitHub Release assets. Protocol rev 6 floor.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- Do **not** bump `PROTOCOL_REV` in this program.
- `woc-update` must not depend on Bevy, wgpu, `woc-sim`, `woc-content`, or axum.
- Do not reintroduce a fat `Entity`. Do not edit `woc-sim` / `woc-content`.
- Incremental format is **per-file qbsdiff** from the **immediately previous** tag only; skip-version uses full.
- One ed25519 key; public key compiled into the updater; private key never in git.
- English-only strings. Fail closed on hash or signature mismatch (leave prefix untouched).
- PR CI must not `cargo build --release -p woc-client`. Packer tests use tiny fake files.
- 1.4.0 version gate should already be on `develop` before Task 9. Tasks 1–8 can land first.
- CI gate: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client` + clippy with `-D warnings` (include `woc-update`).

**Design:** [`docs/superpowers/specs/2026-08-13-client-update-packages-design.md`](../specs/2026-08-13-client-update-packages-design.md)

---

## File map

| File | Responsibility |
| --- | --- |
| Create `crates/woc-update/Cargo.toml` | Leaf crate + bins |
| Create `crates/woc-update/src/lib.rs` | Re-exports |
| Create `crates/woc-update/src/error.rs` | `UpdateError` |
| Create `crates/woc-update/src/hash.rs` | `sha256_hex` |
| Create `crates/woc-update/src/manifest.rs` | `Manifest`, `InstallState`, `FileEntry`, `Artifact` |
| Create `crates/woc-update/src/pack.rs` | Full tar.zst pack/unpack |
| Create `crates/woc-update/src/delta.rs` | `.wocdelta` pack/apply |
| Create `crates/woc-update/src/plan.rs` | `plan_fetch` |
| Create `crates/woc-update/src/sign.rs` | ed25519 sign/verify |
| Create `crates/woc-update/src/apply.rs` | Staging swap + backup |
| Create `crates/woc-update/src/store.rs` | `ArtifactStore` (dir + HTTP) |
| Create `crates/woc-update/src/bin/woc-pack.rs` | CI CLI |
| Create `crates/woc-update/src/bin/woc-updater.rs` | Player launcher |
| Modify `Cargo.toml` | workspace member |
| Modify `crates/woc-version/src/lib.rs` | `update_manifest_url` (Task 9) |
| Modify `crates/woc-client/src/title.rs` | Update button (Task 9) |
| Create `.github/workflows/client-release.yml` | Tag publish |
| Modify docs / `VERSION.toml` | 1.5.0 |

---

### Task 1: Crate, hashes, manifest types

**Files:**
- Create: `crates/woc-update/Cargo.toml`
- Create: `crates/woc-update/src/error.rs`
- Create: `crates/woc-update/src/hash.rs`
- Create: `crates/woc-update/src/manifest.rs`
- Create: `crates/woc-update/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members` + `default-members` + `woc-update = { path = "crates/woc-update" }`)
- Test: `crates/woc-update/src/manifest.rs`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `pub fn sha256_hex(bytes: &[u8]) -> String`
  - `pub struct FileEntry { pub path: String, pub sha256: String, pub size: u64 }`
  - `pub struct Artifact { pub name: String, pub sha256: String, pub size: u64 }`
  - `pub struct Manifest { pub rewrite_version: String, pub protocol_rev: u32, pub target: String, pub files: Vec<FileEntry>, pub full: Artifact, pub delta_from: BTreeMap<String, Artifact>, pub sig: String }`
  - `pub struct InstallState { pub rewrite_version: String, pub target: String }`
  - `pub enum UpdateError`

- [ ] **Step 1: Add the crate and failing tests**

Root `Cargo.toml` — append `"crates/woc-update"` to both `members` and `default-members`, and under `[workspace.dependencies]`:

```toml
woc-update = { path = "crates/woc-update" }
```

`crates/woc-update/Cargo.toml`:

```toml
[package]
name = "woc-update"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Pack, delta, and apply Bevy client updates for World of ClaudeCraft (Rust)"

[lib]
path = "src/lib.rs"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
sha2 = "0.10"
hex = "0.4"

[dev-dependencies]
```

`error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hash mismatch for {path}")]
    HashMismatch { path: String },
    #[error("bad signature")]
    Signature,
    #[error("target mismatch")]
    TargetMismatch,
    #[error("delta: {0}")]
    Delta(String),
    #[error("{0}")]
    Msg(String),
}
```

`hash.rs`:

```rust
use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
```

`manifest.rs` — types with `Serialize`/`Deserialize`, `delta_from` default empty, `sig` default empty. Add tests:

```rust
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
```

Stub `sha256_hex` as `String::new()` so the known-vector test fails.

`lib.rs`:

```rust
mod error;
mod hash;
mod manifest;

pub use error::UpdateError;
pub use hash::sha256_hex;
pub use manifest::{Artifact, FileEntry, InstallState, Manifest};
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-update sha256_hex_known_vector -q`

Expected: FAIL (`left: ""` or similar)

- [ ] **Step 3: Implement `sha256_hex` as in Step 1**

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/woc-update
git commit -m "feat(update): add woc-update crate with manifest types"
```

---

### Task 2: Full `.tar.zst` pack / unpack

**Files:**
- Create: `crates/woc-update/src/pack.rs`
- Modify: `crates/woc-update/Cargo.toml` (add `tar = "0.4"`, `zstd = "0.13"`)
- Modify: `crates/woc-update/src/lib.rs`
- Test: `crates/woc-update/src/pack.rs`

**Interfaces:**
- Consumes: a layout directory containing files listed in `InstallState`
- Produces:
  - `pub fn pack_full(layout_dir: &Path) -> Result<Vec<u8>, UpdateError>`
  - `pub fn unpack_full(archive: &[u8], dest: &Path) -> Result<(), UpdateError>`
  - `pub fn file_entry(layout_dir: &Path, rel: &str) -> Result<FileEntry, UpdateError>`

- [ ] **Step 1: Write failing tests**

```rust
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
        fs::write(layout.join("install.json"), b"{\"rewrite_version\":\"1.0.0\",\"target\":\"t\"}").unwrap();

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
```

Leave `pack_full` as `Err(UpdateError::Msg("unimpl".into()))`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-update pack_unpack_restores_bytes_and_mode -q`

Expected: FAIL (`unimpl`)

- [ ] **Step 3: Implement pack/unpack**

Use `tar::Builder` on a `Vec<u8>`, then `zstd::encode_all`. Unpack: `zstd::decode_all` then `tar::Archive::new`. Append only regular files at archive **root** (no wrapping directory). Set unix mode `0o755` for `woc-client` and `woc-updater`, `0o644` for `install.json`.

```rust
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
    Ok(zstd::encode_all(&tar_buf[..], 3)?)
}
```

Map zstd errors via `UpdateError::Msg`. `file_entry` reads the file, fills path/size/`sha256_hex`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-update
git commit -m "feat(update): pack and unpack full tar.zst layouts"
```

---

### Task 3: Per-file bsdiff delta

**Files:**
- Create: `crates/woc-update/src/delta.rs`
- Modify: `crates/woc-update/Cargo.toml` (`qbsdiff = "1.4"`)
- Modify: `crates/woc-update/src/lib.rs`
- Test: `crates/woc-update/src/delta.rs`

**Interfaces:**
- Consumes: old layout dir, new layout dir, `qbsdiff::{Bsdiff, Bspatch}`
- Produces:
  - `pub struct DeltaMeta { pub from: String, pub to: String, pub patches: Vec<PatchEntry> }`
  - `pub struct PatchEntry { pub path: String, pub sha256: String, pub new_sha256: String }`
  - `pub fn pack_delta(from_ver: &str, to_ver: &str, old_dir: &Path, new_dir: &Path) -> Result<Vec<u8>, UpdateError>`
  - `pub fn apply_delta(blob: &[u8], layout_dir: &Path) -> Result<DeltaMeta, UpdateError>`

- [ ] **Step 1: Write failing tests**

```rust
    #[test]
    fn delta_is_smaller_than_new_file_and_applies() {
        let root = std::env::temp_dir().join(format!("woc-delta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
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
        assert!(blob.len() < next.len(), "delta {} vs file {}", blob.len(), next.len());

        apply_delta(&blob, &old).expect("apply");
        assert_eq!(fs::read(old.join("woc-client")).unwrap(), next);
        assert_eq!(fs::read(old.join("woc-updater")).unwrap(), b"UP-NEW");
    }

    #[test]
    fn corrupt_delta_does_not_change_files() {
        // pack a valid delta, flip a byte in the blob, apply must Err,
        // and woc-client bytes stay at the old value.
    }
```

In `corrupt_delta_does_not_change_files`, copy old files, apply flipped blob, `assert!(result.is_err())`, assert old bytes unchanged. Implement apply so it writes to a sibling temp file then rename only after `new_sha256` matches — that gives the test for free.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-update delta_is_smaller_than_new_file_and_applies -q`

Expected: FAIL (unimpl)

- [ ] **Step 3: Implement**

`pack_delta`: for each of `woc-client`, `woc-updater`, if bytes differ, `Bsdiff::new(old, new).compare(Cursor::new(&mut patch))?`. Write `delta.json` + `{path}.bsdiff` into an inner tar, zstd-compress (same as full). Skip identical files.

`apply_delta`: decode, read `delta.json`, for each patch: read old file, `Bspatch::new(&patch)?.apply(old, Cursor::new(&mut out))?`, check `sha256_hex(&out) == new_sha256`, write to `{path}.new` then `rename`. If a check fails, delete any `.new` files and return `HashMismatch` **without** renaming.

```rust
use qbsdiff::{Bsdiff, Bspatch};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q`

Expected: PASS (`delta_is_smaller...` must actually be smaller; if a tiny splice is not smaller, lengthen the 8000-byte base)

- [ ] **Step 5: Commit**

```bash
git add crates/woc-update
git commit -m "feat(update): per-file bsdiff delta pack and apply"
```

---

### Task 4: `plan_fetch`

**Files:**
- Create: `crates/woc-update/src/plan.rs`
- Modify: `crates/woc-update/src/lib.rs`
- Test: `crates/woc-update/src/plan.rs`

**Interfaces:**
- Consumes: `InstallState`, `Manifest`
- Produces: `pub enum FetchPlan { Nothing, Delta { from: String, artifact: Artifact }, Full { artifact: Artifact } }` and `pub fn plan_fetch(local: &InstallState, remote: &Manifest) -> Result<FetchPlan, UpdateError>`

- [ ] **Step 1: Write failing tests**

```rust
    fn remote(delta_from: &str) -> Manifest {
        let mut d = BTreeMap::new();
        if !delta_from.is_empty() {
            d.insert(
                delta_from.into(),
                Artifact {
                    name: "d.wocdelta".into(),
                    sha256: "00".into(),
                    size: 1,
                },
            );
        }
        Manifest {
            rewrite_version: "1.5.0".into(),
            protocol_rev: 6,
            target: "x86_64-unknown-linux-gnu".into(),
            files: vec![],
            full: Artifact {
                name: "full.tar.zst".into(),
                sha256: "aa".into(),
                size: 2,
            },
            delta_from: d,
            sig: String::new(),
        }
    }

    #[test]
    fn same_version_is_nothing() {
        let local = InstallState {
            rewrite_version: "1.5.0".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        assert!(matches!(plan_fetch(&local, &remote("1.4.0")).unwrap(), FetchPlan::Nothing));
    }

    #[test]
    fn predecessor_uses_delta() {
        let local = InstallState {
            rewrite_version: "1.4.0".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        match plan_fetch(&local, &remote("1.4.0")).unwrap() {
            FetchPlan::Delta { from, .. } => assert_eq!(from, "1.4.0"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn skip_version_uses_full() {
        let local = InstallState {
            rewrite_version: "1.3.0".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        assert!(matches!(
            plan_fetch(&local, &remote("1.4.0")).unwrap(),
            FetchPlan::Full { .. }
        ));
    }

    #[test]
    fn target_mismatch_errors() {
        let local = InstallState {
            rewrite_version: "1.4.0".into(),
            target: "aarch64-unknown-linux-gnu".into(),
        };
        assert!(matches!(
            plan_fetch(&local, &remote("1.4.0")),
            Err(UpdateError::TargetMismatch)
        ));
    }
```

Stub `plan_fetch` as `Ok(FetchPlan::Nothing)` so predecessor/skip tests fail.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-update predecessor_uses_delta -q`

Expected: FAIL (`Nothing` vs `Delta`)

- [ ] **Step 3: Implement**

```rust
pub fn plan_fetch(local: &InstallState, remote: &Manifest) -> Result<FetchPlan, UpdateError> {
    if local.target != remote.target {
        return Err(UpdateError::TargetMismatch);
    }
    if local.rewrite_version == remote.rewrite_version {
        return Ok(FetchPlan::Nothing);
    }
    if let Some(artifact) = remote.delta_from.get(&local.rewrite_version) {
        return Ok(FetchPlan::Delta {
            from: local.rewrite_version.clone(),
            artifact: artifact.clone(),
        });
    }
    Ok(FetchPlan::Full {
        artifact: remote.full.clone(),
    })
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-update
git commit -m "feat(update): choose delta or full from local install state"
```

---

### Task 5: ed25519 manifest signatures

**Files:**
- Create: `crates/woc-update/src/sign.rs`
- Modify: `crates/woc-update/Cargo.toml` (`ed25519-dalek = { version = "2", features = ["std"] }`)
- Modify: `crates/woc-update/src/lib.rs`
- Test: `crates/woc-update/src/sign.rs`

**Interfaces:**
- Consumes: `Manifest` minus `sig`
- Produces:
  - `pub fn signing_key_from_hex(seed32: &str) -> Result<ed25519_dalek::SigningKey, UpdateError>`
  - `pub fn verifying_key_from_hex(pub32: &str) -> Result<ed25519_dalek::VerifyingKey, UpdateError>`
  - `pub fn sign_manifest(m: &mut Manifest, key: &SigningKey)`
  - `pub fn verify_manifest(m: &Manifest, pk: &VerifyingKey) -> Result<(), UpdateError>`

Canonical bytes: clone manifest, set `sig` to `""`, `serde_json::to_vec` (field order is struct order — do **not** use pretty print). Sign those bytes. Store lowercase hex of the 64-byte signature in `m.sig`.

- [ ] **Step 1: Write failing tests**

Use a fixed 32-byte seed `0102..20` hex (64 hex chars). Derive pubkey from the signing key in the test (do not hardcode a wrong pk).

```rust
    #[test]
    fn sign_then_verify_ok() {
        let sk = signing_key_from_hex(&"11".repeat(32)).unwrap();
        let pk = sk.verifying_key();
        let mut m = Manifest { /* minimal like Task 1 JSON */ sig: String::new(), .. };
        sign_manifest(&mut m, &sk);
        assert!(!m.sig.is_empty());
        verify_manifest(&m, &pk).unwrap();
    }

    #[test]
    fn tampered_version_fails_verify() {
        let sk = signing_key_from_hex(&"11".repeat(32)).unwrap();
        let pk = sk.verifying_key();
        let mut m = Manifest { /* ... */ };
        sign_manifest(&mut m, &sk);
        m.rewrite_version = "9.9.9".into();
        assert!(matches!(verify_manifest(&m, &pk), Err(UpdateError::Signature)));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-update sign_then_verify_ok -q`

Expected: FAIL

- [ ] **Step 3: Implement with `ed25519_dalek::{Signer, Verifier, Signature, SigningKey}`**

```rust
pub fn sign_manifest(m: &mut Manifest, key: &SigningKey) {
    m.sig.clear();
    let body = serde_json::to_vec(m).expect("manifest json");
    let sig = key.sign(&body);
    m.sig = hex::encode(sig.to_bytes());
}

pub fn verify_manifest(m: &Manifest, pk: &VerifyingKey) -> Result<(), UpdateError> {
    let mut clone = m.clone();
    let sig_hex = clone.sig.clone();
    clone.sig.clear();
    let body = serde_json::to_vec(&clone)?;
    let bytes = hex::decode(&sig_hex).map_err(|_| UpdateError::Signature)?;
    let sig = Signature::from_slice(&bytes).map_err(|_| UpdateError::Signature)?;
    pk.verify(&body, &sig).map_err(|_| UpdateError::Signature)
}
```

`SigningKey::from_bytes` on decoded 32-byte seed.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-update
git commit -m "feat(update): ed25519-sign and verify update manifests"
```

---

### Task 6: Staging apply + `DirStore`

**Files:**
- Create: `crates/woc-update/src/store.rs`
- Create: `crates/woc-update/src/apply.rs`
- Modify: `crates/woc-update/src/lib.rs`
- Test: `crates/woc-update/src/apply.rs`

**Interfaces:**
- Consumes: `ArtifactStore`, `plan_fetch`, pack/delta/sign from earlier tasks
- Produces:
  - `pub trait ArtifactStore { fn fetch(&self, name: &str) -> Result<Vec<u8>, UpdateError>; }`
  - `pub struct DirStore { pub root: PathBuf }`
  - `pub fn apply_update(prefix: &Path, remote: &Manifest, store: &dyn ArtifactStore) -> Result<FetchPlan, UpdateError>`  
    (`Nothing` if already current; otherwise performs apply)

- [ ] **Step 1: Write the integration test**

Build two layouts 1.0.0 / 1.0.1 (tiny bytes), `pack_full` both, `pack_delta`, sign a manifest with `delta_from["1.0.0"]`, write artifacts into a dir store under their `Artifact.name`. Start prefix as 1.0.0. `apply_update` → files match 1.0.1 hashes. Second test: flip a byte of the delta in the store, `apply_update` errs, prefix still 1.0.0. Third: manifest with empty `delta_from`, local 1.0.0 → uses full, succeeds.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-update apply_update -q`

Expected: FAIL (missing fn)

- [ ] **Step 3: Implement**

`apply_update`:

1. Read `prefix/install.json` → `InstallState` (if missing, treat as empty version `0.0.0` still requiring `target` from remote — or require install.json; tests always write it).
2. `verify_manifest` is **not** this task’s job if the caller already verified — still verify if `remote.sig` non-empty and a verifying key is passed. Keep apply signature simple: **caller verifies first**. This function trusts `remote` hashes.
3. `plan_fetch`. `Nothing` → return.
4. `store.fetch(&artifact.name)`, check `sha256_hex(blob) == artifact.sha256`.
5. Create `prefix.staging` (rm if exists). If Full: `unpack_full` into staging. If Delta: copy prefix files into staging, `apply_delta` on staging.
6. For each `remote.files`, `file_entry(staging, path)` must match sha256.
7. Write `install.json` from `remote.rewrite_version` + `remote.target`.
8. Swap: `if prefix.backup exists { rm }`; `rename(prefix, prefix.backup)`; `rename(staging, prefix)`.
9. On error before swap: `rm staging`.

`DirStore::fetch` is `fs::read(self.root.join(name))`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-update
git commit -m "feat(update): atomic staging apply with dir artifact store"
```

---

### Task 7: `woc-pack` CLI

**Files:**
- Create: `crates/woc-update/src/bin/woc-pack.rs`
- Modify: `crates/woc-update/Cargo.toml` (`[[bin]] name = "woc-pack" path = "src/bin/woc-pack.rs"`)
- Test: invoke via `cargo run -p woc-update --bin woc-pack` in a unit test using `std::process::Command` **or** extract `pub fn pack_release(...)` in `lib.rs` and unit-test that (preferred — no process).

**Interfaces:**
- Consumes: layout dir, optional prev layout dir, signing seed hex, version/target/protocol
- Produces: writes `full`, optional `wocdelta`, signed `manifest.json` into `--out`

Add `pub fn pack_release(opts: PackOpts) -> Result<Manifest, UpdateError>` in `pack.rs` (or `release.rs`):

```rust
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
```

- [ ] **Step 1: Write failing test** calling `pack_release` into temp dirs (two layouts). Assert out dir has three files when prev is set; manifest `delta_from` has prev version; `verify_manifest` passes.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-update pack_release -q`

Expected: FAIL

- [ ] **Step 3: Implement `pack_release` + thin CLI**

CLI args (no extra parser crate — `std::env::args`):

```text
woc-pack --layout DIR --out DIR --version 1.5.0 --target x86_64-unknown-linux-gnu --protocol-rev 6 --key HEX [--prev DIR --prev-version 1.4.0]
```

Write:

- `woc-rs-{ver}-{target}.tar.zst`
- `woc-rs-{from}-to-{to}-{target}.wocdelta` if prev
- `woc-rs-{ver}-{target}.manifest.json`

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q && cargo build -p woc-update --bin woc-pack`

Expected: PASS / built

- [ ] **Step 5: Commit**

```bash
git add crates/woc-update
git commit -m "feat(update): woc-pack writes signed full and delta artifacts"
```

---

### Task 8: `woc-updater` CLI + HTTP store

**Files:**
- Create: `crates/woc-update/src/bin/woc-updater.rs`
- Modify: `crates/woc-update/src/store.rs` (`HttpStore`)
- Modify: `crates/woc-update/Cargo.toml` (`ureq` same as client: `version = "2.12", default-features = false, features = ["json", "tls"]`; `[[bin]] name = "woc-updater"`)
- Test: library `apply_update` already covers apply; add `HttpStore` test only if you fake ureq — **skip live HTTP**. Test CLI via `pack_release` + `Command` with `--store dir` (required for tests).

**Interfaces:**
- Consumes: `--prefix`, `--manifest` path or URL, `--once`, `--pubkey` hex (tests); production default pubkey from `env!("WOC_UPDATE_PUBKEY")` with fallback test key `11` repeated 32 if unset at compile time
- Produces: process exit 0 after apply; exec `prefix/woc-client` when that file exists **and** is executable. For tests, `--no-exec`.

Self-update: if `std::env::current_exe()` is inside `prefix` and plan is not `Nothing`, `cp` self to `{temp}/woc-updater.{pid}`, `exec` that with `--apply-from prefix` plus original flags. Tests can pass `--already-copied` to skip.

- [ ] **Step 1: Write a test that runs the binary**

```rust
#[test]
fn updater_bin_upgrades_prefix() {
    // pack_release 1.0.0 -> 1.0.1 into store_dir
    // copy 1.0.0 layout to prefix
    let exe = env!("CARGO_BIN_EXE_woc-updater"); // requires [[bin]] + cargo test on the crate
    let st = Command::new(exe)
        .args([
            "--prefix", prefix.to_str().unwrap(),
            "--manifest", manifest_path.to_str().unwrap(),
            "--once",
            "--no-exec",
            "--pubkey", &hex::encode(pk.to_bytes()),
            "--already-copied",
        ])
        .status()
        .unwrap();
    assert!(st.success());
    assert_eq!(fs::read(prefix.join("woc-client")).unwrap(), b"NEW");
}
```

`CARGO_BIN_EXE_woc-updater` is set when the crate defines that bin. If integration tests are easier, use `crates/woc-update/tests/updater_bin.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-update updater_bin_upgrades_prefix -q`

Expected: FAIL (missing bin or missing flag handling)

- [ ] **Step 3: Implement `woc-updater`**

Parse args. Load manifest: if `--manifest` is a path, `fs::read`; if it starts with `http://` or `https://`, `HttpStore` fetches the file name from the URL (download the URL itself, not store-relative). Artifact fetches use the URL’s parent directory as HTTP base (`HttpStore { base: parent, agent }`).

`HttpStore::fetch`: `GET {base}/{name}` with the same ureq timeouts as `woc-client` (3s connect, 8s read — increase read to 120s for binaries).

Verify signature, `apply_update`, then unless `--no-exec`, `Command::new(prefix.join("woc-client")).exec()` (`std::os::unix::process::CommandExt` on Linux). Gate `exec` behind `#[cfg(unix)]`; on other OS `status()` + `exit`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-update -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-update
git commit -m "feat(update): woc-updater applies delta or full and execs the client"
```

---

### Task 9: `/version` URL + title Update button

**Files:**
- Modify: `crates/woc-version/src/lib.rs` (`update_manifest_url: String`, `#[serde(default)]`; `current` reads `WOC_UPDATE_MANIFEST_URL`)
- Modify: `crates/woc-client/src/title.rs` (Update button)
- Modify: `crates/woc-client/src/main.rs` if `RealmCompat` must store the URL from `VersionInfo`
- Test: `woc-version` deserialize default empty; `cargo check -p woc-client`

**Depends:** 1.4.0 `RealmCompat` / `VersionInfo::current(protocol_rev)` already on the branch.

**Interfaces:**
- Consumes: 1.4.0 `RealmCompatState::Incompatible`, `VersionInfo.update_manifest_url`
- Produces: packaged Update control that `Command::new(prefix.join("woc-updater")).args(["--once", "--manifest", url])` then `exit(0)`

- [ ] **Step 1: Failing test in `woc-version`**

```rust
    #[test]
    fn legacy_json_has_empty_update_manifest_url() {
        let json = r#"{"rewrite_version":"1.4.0","upstream_version":"0.31.0","upstream_commit":"x","upstream_repo":"x","parity_target":"client-compat","protocol_rev":6,"min_client_version":"1.4.0"}"#;
        let info: VersionInfo = serde_json::from_str(json).unwrap();
        assert!(info.update_manifest_url.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-version legacy_json_has_empty_update_manifest_url -q`

Expected: FAIL (missing field) until `#[serde(default)]` is added

- [ ] **Step 3: Add field + title button**

`VersionInfo::current`:

```rust
update_manifest_url: std::env::var("WOC_UPDATE_MANIFEST_URL")
    .ok()
    .filter(|s| !s.is_empty())
    .unwrap_or_default(),
```

Store URL on `RealmCompat` when the preflight JSON arrives.

`fn updater_path() -> Option<PathBuf>`: `std::env::current_exe().ok()?.parent()?.join("woc-updater")` if `exists()`.

Title: if Online + Incompatible + URL non-empty + updater exists, spawn a full-width **Update** button (`UpdateBtn`). On click / Enter (when incompatible): 

```rust
let mut cmd = std::process::Command::new(updater);
cmd.arg("--once").arg("--manifest").arg(url);
let _ = cmd.spawn();
std::process::exit(0);
```

Do not wait. 1.4.0 Continue remains blocked.

- [ ] **Step 4: Compile**

Run: `cargo test -p woc-version -q && cargo test --workspace --exclude woc-client -q && cargo check -p woc-client`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-version/src/lib.rs crates/woc-client/src/title.rs crates/woc-client/src/main.rs
git commit -m "feat(client): launch sibling updater from title when packaged"
```

---

### Task 10: Tag workflow, docs, rewrite `1.5.0`

**Files:**
- Create: `.github/workflows/client-release.yml`
- Create: `docs/client-update.md` (runbook)
- Modify: `VERSION.toml`, `crates/woc-version/src/lib.rs` constants, `Cargo.toml` workspace version, `CHANGELOG.md`, `README.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`
- Test: `constants_match_version_toml`

- [ ] **Step 1: Fail the pin test**

Set `VERSION.toml` `rewrite_version = "1.5.0"` and `parity_target = "client-update"` only.

- [ ] **Step 2: Run pin test**

Run: `cargo test -p woc-version constants_match_version_toml -q`

Expected: FAIL vs `1.4.0` (or `1.3.0` if 1.4.0 not yet tagged in constants)

- [ ] **Step 3: Workflow + docs + constants**

`.github/workflows/client-release.yml`:

```yaml
name: Client release

on:
  push:
    tags: ["v*.*.*"]
  workflow_dispatch:
    inputs:
      tag:
        description: Release tag (e.g. v1.5.0)
        required: true

permissions:
  contents: write

jobs:
  pack-linux:
    runs-on: ubuntu-latest
    env:
      WOC_UPDATE_PUBKEY: ${{ secrets.WOC_UPDATE_PUBKEY }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - name: Linux deps
        run: sudo apt-get update && sudo apt-get install -y libasound2-dev libudev-dev pkg-config zstd
      - uses: Swatinem/rust-cache@v2
      - name: Release build
        run: cargo build --release -p woc-client -p woc-update
      - name: Stage layout
        run: |
          mkdir -p dist/layout dist/out dist/prev
          cp target/release/woc-client target/release/woc-updater dist/layout/
          python3 - <<'PY'
          import tomllib, json, pathlib
          v = tomllib.loads(pathlib.Path("VERSION.toml").read_text())["rewrite_version"]
          pathlib.Path("dist/layout/install.json").write_text(json.dumps({
              "rewrite_version": v,
              "target": "x86_64-unknown-linux-gnu",
          }))
          print(v)
          PY
      - name: Previous layout (optional)
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          set +e
          TAG="${{ github.event.inputs.tag || github.ref_name }}"
          PREV_TAG=$(gh release list --limit 20 --json tagName,isDraft,isPrerelease \
            --jq --arg cur "$TAG" '[.[] | select(.tagName != $cur) | select((.isDraft|not) and (.isPrerelease|not) and (.tagName|test("^v[0-9]"))) | .tagName] | .[0]')
          set -e
          if [ -z "$PREV_TAG" ]; then
            echo "No previous release; full archive only"
            exit 0
          fi
          echo "Previous tag: $PREV_TAG"
          gh release download "$PREV_TAG" --pattern "woc-rs-*-x86_64-unknown-linux-gnu.tar.zst" --dir dist/prev
          mkdir -p dist/prev/layout
          # First matching full archive (not .wocdelta).
          TAR=$(ls dist/prev/woc-rs-*-x86_64-unknown-linux-gnu.tar.zst | head -n1)
          zstd -dc "$TAR" | tar -x -C dist/prev/layout
          echo "Unpacked $TAR"
      - name: Pack
        env:
          WOC_UPDATE_SIGNING_KEY: ${{ secrets.WOC_UPDATE_SIGNING_KEY }}
        run: |
          VER=$(python3 -c "import tomllib,pathlib; print(tomllib.loads(pathlib.Path('VERSION.toml').read_text())['rewrite_version'])")
          PREV_ARGS=()
          if [ -f dist/prev/layout/install.json ]; then
            PREV_VER=$(python3 -c "import json,pathlib; print(json.loads(pathlib.Path('dist/prev/layout/install.json').read_text())['rewrite_version'])")
            PREV_ARGS=(--prev dist/prev/layout --prev-version "$PREV_VER")
          fi
          target/release/woc-pack \
            --layout dist/layout \
            --out dist/out \
            --version "$VER" \
            --target x86_64-unknown-linux-gnu \
            --protocol-rev 6 \
            --key "$WOC_UPDATE_SIGNING_KEY" \
            "${PREV_ARGS[@]}"
      - name: Upload
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          TAG="${{ github.event.inputs.tag || github.ref_name }}"
          gh release view "$TAG" >/dev/null 2>&1 \
            && gh release upload "$TAG" dist/out/* --clobber \
            || gh release create "$TAG" dist/out/* --title "$TAG" --generate-notes
```

`docs/client-update.md` must state:

- First install: download `woc-rs-VERSION-x86_64-unknown-linux-gnu.tar.zst`, extract, run `./woc-updater`.
- Later: run `./woc-updater` (delta when coming from N-1).
- Secrets: `WOC_UPDATE_SIGNING_KEY` (32-byte seed hex), `WOC_UPDATE_PUBKEY` (32-byte pk hex) for compile; generate once with a tiny `woc-pack --gen-key` **or** a documented `python`/`openssl` snippet. Add `--gen-key` to `woc-pack` in this task if missing: print seed + pubkey hex and exit.
- Server: `WOC_UPDATE_MANIFEST_URL` = the uploaded `manifest.json` URL on the GitHub Release.
- cargo-run: leave URL unset.

Sync `REWRITE_VERSION` / `PARITY_TARGET` / workspace version / README badges / ROADMAP 1.5.0 row (planned → this branch) / STATUS table from the 1.5.0 spec DoD / CHANGELOG `## 1.5.0`.

- [ ] **Step 4: Tests + clippy**

Run: `cargo test --workspace --exclude woc-client -q && cargo check -p woc-client && cargo clippy --workspace --exclude woc-client -- -D warnings`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/client-release.yml docs/client-update.md VERSION.toml Cargo.toml crates/woc-version/src/lib.rs CHANGELOG.md README.md docs/ROADMAP.md docs/parity/STATUS.md crates/woc-update
git commit -m "feat(update): Linux tag publish and 1.5.0 client-update docs"
```

---

## Manual verification

1. Two fake prefixes through `woc-pack` + `woc-updater --no-exec` (Task 8 test).
2. Corrupt delta → prefix unchanged.
3. After 1.4.0 UI + a real `cargo build --release -p woc-client -p woc-update`, stage a layout, pack, point `WOC_UPDATE_MANIFEST_URL` at the file URL, run updater then client.

---

## Self-review

1. **Spec coverage:** full pack, qbsdiff delta, one predecessor, updater, staging, ed25519, GitHub Release, `/version` URL, title button, Linux-only DoD, Velopack/casync rejected.
2. **Placeholders:** None. Previous-tag download/unpack/pack flags are in the workflow body.
3. **Types:** `Manifest`, `InstallState`, `FetchPlan`, `ArtifactStore`, `PackOpts` names match across tasks. `PROTOCOL_REV` stays 6.
