# Client version gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop a mismatched Bevy online client from entering the realm: title-screen `/version` preflight, Hello identity, Welcome double-check, then tag rewrite `1.4.0` / `client-compat`.

**Architecture:** Pure compat policy lives in leaf crate `woc-version` (no Bevy, no `woc-protocol` dep). `GET /version` advertises `protocol_rev` + `min_client_version`. Hello gains additive identity fields (protocol rev stays **6**). Server rejects before spawn. Title Online Continue is fail-closed. Offline is unchanged. No installers, no binary download.

**Tech Stack:** Rust edition 2021, Bevy 0.16, axum 0.8, ureq 2.12 (existing client HTTP), serde, existing `woc-version` / `woc-protocol` / `woc-server` / `woc-client`. Upstream pin 0.31.0. Protocol rev 6 floor.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- Do **not** bump `PROTOCOL_REV` (Hello fields are additive `#[serde(default)]`).
- `woc-version` must not depend on `woc-protocol`, Bevy, axum, or tokio. Callers pass `protocol_rev` in.
- `woc-sim` / `woc-content` are untouched. No new ECS columns. Do not reintroduce a fat `Entity`.
- English-only `Compat::user_message()` strings; every player-facing error starts with `version:`.
- Client never decides combat/loot/quest outcomes.
- Prefer additive serde defaults. Missing Hello identity is valid JSON and is **refused** (fail-closed policy).
- No packaged installers, electron-updater, download URL, or self-replace binary.
- CI gate: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client` (+ clippy as CI). Policy tests must live in crates CI actually runs (`woc-version`, `woc-protocol`, `woc-server`).
- Branch naming for implementation: `cursor/<workstream-id>-680e` (or unique suffix).

**Design:** [`docs/superpowers/specs/2026-08-13-client-version-update-design.md`](../specs/2026-08-13-client-version-update-design.md)

---

## File map

| File | Responsibility |
| --- | --- |
| Create `crates/woc-version/src/compat.rs` | `SemVer`, `parse_semver`, `ClientIdentity`, `RealmIdentity`, `Compat`, `check_compat`, `min_client_version_from_env` |
| Modify `crates/woc-version/src/lib.rs` | `mod compat`; `VersionInfo` owned strings + `protocol_rev` + `min_client_version`; `current(protocol_rev)` |
| Modify `crates/woc-protocol/src/lib.rs` | Additive Hello `protocol_rev` / `rewrite_version`; tests |
| Modify `crates/woc-server/src/main.rs` | `VersionInfo::current(PROTOCOL_REV)` |
| Modify `crates/woc-server/src/game_ws.rs` | Hello gate before auth/spawn |
| Modify `crates/woc-client/src/api.rs` | `spawn_fetch_version` |
| Modify `crates/woc-client/src/main.rs` | `RealmCompat` resource |
| Modify `crates/woc-client/src/title.rs` | Online preflight + block Continue |
| Modify `crates/woc-client/src/menu_ui.rs` | `status_color` recognizes `version:` |
| Modify `crates/woc-client/src/online.rs` | Hello identity |
| Modify `crates/woc-client/src/world_setup.rs` | Welcome rev check + kick to Title |
| Modify `VERSION.toml`, `CHANGELOG.md`, `README.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md` | `1.4.0` / `client-compat` |

Do not edit `crates/woc-sim/**` or `crates/woc-content/**`.

---

### Task 1: Compat policy in `woc-version`

**Files:**
- Create: `crates/woc-version/src/compat.rs`
- Modify: `crates/woc-version/src/lib.rs` (add `mod compat; pub use compat::{...};`)
- Test: `crates/woc-version/src/compat.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: nothing (leaf)
- Produces:
  - `pub fn parse_semver(s: &str) -> Option<SemVer>`
  - `pub struct SemVer { pub major: u64, pub minor: u64, pub patch: u64 }` with `Ord`
  - `pub struct ClientIdentity { pub rewrite_version: String, pub protocol_rev: u32 }`
  - `pub struct RealmIdentity { pub rewrite_version: String, pub protocol_rev: Option<u32>, pub min_client_version: String }`
  - `pub enum Compat { Compatible, ClientTooOld { client: String, min_client: String }, ProtocolMismatch { client_rev: u32, realm_rev: u32 }, BadClientVersion(String), BadMinVersion(String) }`
  - `impl Compat { pub fn is_ok(&self) -> bool; pub fn user_message(&self) -> String }`
  - `pub fn check_compat(client: &ClientIdentity, realm: &RealmIdentity) -> Compat`
  - `impl ClientIdentity { pub fn from_hello(protocol_rev: Option<u32>, rewrite_version: Option<&str>) -> Self }`

- [ ] **Step 1: Write the failing tests**

Create `crates/woc-version/src/compat.rs` with tests first (the types can be stubbed so the file compiles, or put tests in `lib.rs` and add the module). Prefer the whole module in one file:

```rust
//! Client / realm rewrite + protocol compatibility.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

pub fn parse_semver(_s: &str) -> Option<SemVer> {
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    pub rewrite_version: String,
    pub protocol_rev: u32,
}

impl ClientIdentity {
    pub fn from_hello(protocol_rev: Option<u32>, rewrite_version: Option<&str>) -> Self {
        Self {
            protocol_rev: protocol_rev.unwrap_or(0),
            rewrite_version: rewrite_version.unwrap_or("(unknown)").to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealmIdentity {
    pub rewrite_version: String,
    pub protocol_rev: Option<u32>,
    pub min_client_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compat {
    Compatible,
    ClientTooOld {
        client: String,
        min_client: String,
    },
    ProtocolMismatch {
        client_rev: u32,
        realm_rev: u32,
    },
    BadClientVersion(String),
    BadMinVersion(String),
}

impl Compat {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    pub fn user_message(&self) -> String {
        String::new()
    }
}

pub fn check_compat(_client: &ClientIdentity, _realm: &RealmIdentity) -> Compat {
    Compat::Compatible
}

#[cfg(test)]
mod tests {
    use super::*;

    fn realm(min: &str, proto: Option<u32>) -> RealmIdentity {
        RealmIdentity {
            rewrite_version: "1.4.0".into(),
            protocol_rev: proto,
            min_client_version: min.into(),
        }
    }

    fn client(ver: &str, proto: u32) -> ClientIdentity {
        ClientIdentity {
            rewrite_version: ver.into(),
            protocol_rev: proto,
        }
    }

    #[test]
    fn parse_strips_prerelease_and_compares_triples() {
        assert_eq!(
            parse_semver("1.4.0"),
            Some(SemVer {
                major: 1,
                minor: 4,
                patch: 0
            })
        );
        assert_eq!(parse_semver("1.4.0-pre"), parse_semver("1.4.0"));
        assert!(parse_semver("1.3.0") < parse_semver("1.4.0"));
        assert!(parse_semver("(unknown)").is_none());
        assert!(parse_semver("").is_none());
        assert!(parse_semver("nope").is_none());
    }

    #[test]
    fn equal_client_and_min_with_matching_protocol_is_compatible() {
        let c = check_compat(&client("1.4.0", 6), &realm("1.4.0", Some(6)));
        assert!(c.is_ok());
    }

    #[test]
    fn newer_client_against_older_min_is_compatible() {
        let c = check_compat(&client("1.5.0", 6), &realm("1.4.0", Some(6)));
        assert!(c.is_ok());
    }

    #[test]
    fn prerelease_equals_release_triple() {
        let c = check_compat(&client("1.4.0-pre", 6), &realm("1.4.0", Some(6)));
        assert!(c.is_ok());
    }

    #[test]
    fn client_below_min_is_too_old() {
        let c = check_compat(&client("1.3.0", 6), &realm("1.4.0", Some(6)));
        assert_eq!(
            c,
            Compat::ClientTooOld {
                client: "1.3.0".into(),
                min_client: "1.4.0".into(),
            }
        );
        let msg = c.user_message();
        assert!(msg.starts_with("version:"));
        assert!(msg.contains("update required"));
        assert!(msg.contains("1.3.0"));
        assert!(msg.contains("1.4.0"));
    }

    #[test]
    fn protocol_client_behind_realm() {
        let c = check_compat(&client("1.4.0", 5), &realm("1.4.0", Some(6)));
        assert_eq!(
            c,
            Compat::ProtocolMismatch {
                client_rev: 5,
                realm_rev: 6,
            }
        );
        let msg = c.user_message();
        assert!(msg.starts_with("version:"));
        assert!(msg.contains("update required"));
        assert!(msg.contains("protocol 5 < 6"));
    }

    #[test]
    fn protocol_client_ahead_of_realm() {
        let c = check_compat(&client("1.4.0", 7), &realm("1.4.0", Some(6)));
        assert_eq!(
            c,
            Compat::ProtocolMismatch {
                client_rev: 7,
                realm_rev: 6,
            }
        );
        let msg = c.user_message();
        assert!(msg.starts_with("version:"));
        assert!(msg.contains("realm outdated"));
        assert!(msg.contains("protocol 7 > 6"));
    }

    #[test]
    fn missing_realm_protocol_skips_protocol_check() {
        let c = check_compat(&client("1.4.0", 6), &realm("1.4.0", None));
        assert!(c.is_ok());
    }

    #[test]
    fn missing_hello_identity_fails_closed() {
        let c = check_compat(
            &ClientIdentity::from_hello(None, None),
            &realm("1.4.0", Some(6)),
        );
        assert!(!c.is_ok());
        assert!(c.user_message().starts_with("version:"));
    }

    #[test]
    fn bad_min_version() {
        let c = check_compat(&client("1.4.0", 6), &realm("not-a-version", Some(6)));
        assert_eq!(c, Compat::BadMinVersion("not-a-version".into()));
        assert!(c.user_message().starts_with("version:"));
    }
}
```

Add at the bottom of `crates/woc-version/src/lib.rs` (before the existing `#[cfg(test)]` module is fine; keep existing tests):

```rust
mod compat;
pub use compat::{
    check_compat, ClientIdentity, Compat, RealmIdentity, SemVer, parse_semver,
};
```

(`min_client_version_from_env` is Task 2.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-version parse_strips_prerelease -- --nocapture`

Expected: FAIL (`parse_semver` returns `None`, assertion `Some(SemVer { ... })` fails)

- [ ] **Step 3: Implement the minimal policy**

Replace the stubs in `compat.rs`:

```rust
pub fn parse_semver(s: &str) -> Option<SemVer> {
    let core = s.split('-').next().unwrap_or(s).trim();
    if core.is_empty() {
        return None;
    }
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(SemVer {
        major,
        minor,
        patch,
    })
}

impl Compat {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Compatible => "compatible".into(),
            Self::ClientTooOld { client, min_client } => {
                format!("version: update required (client {client} < min {min_client})")
            }
            Self::ProtocolMismatch {
                client_rev,
                realm_rev,
            } if client_rev < realm_rev => {
                format!("version: update required (protocol {client_rev} < {realm_rev})")
            }
            Self::ProtocolMismatch {
                client_rev,
                realm_rev,
            } => {
                format!("version: realm outdated (protocol {client_rev} > {realm_rev})")
            }
            Self::BadClientVersion(s) | Self::BadMinVersion(s) => {
                format!("version: invalid version string ({s})")
            }
        }
    }
}

pub fn check_compat(client: &ClientIdentity, realm: &RealmIdentity) -> Compat {
    let Some(_) = parse_semver(&client.rewrite_version) else {
        return Compat::BadClientVersion(client.rewrite_version.clone());
    };
    let Some(min) = parse_semver(&realm.min_client_version) else {
        return Compat::BadMinVersion(realm.min_client_version.clone());
    };
    if let Some(realm_rev) = realm.protocol_rev {
        if client.protocol_rev != realm_rev {
            return Compat::ProtocolMismatch {
                client_rev: client.protocol_rev,
                realm_rev,
            };
        }
    }
    let client_sem = parse_semver(&client.rewrite_version).expect("checked");
    if client_sem < min {
        return Compat::ClientTooOld {
            client: client.rewrite_version.clone(),
            min_client: realm.min_client_version.clone(),
        };
    }
    Compat::Compatible
}
```

Keep `from_hello` as in Step 1.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-version -q`

Expected: PASS (existing `constants_match_version_toml` / `footer_contains_both_versions` plus the new compat tests)

- [ ] **Step 5: Commit**

```bash
git add crates/woc-version/src/compat.rs crates/woc-version/src/lib.rs
git commit -m "feat(version): add client/realm compat policy"
```

---

### Task 2: `/version` payload fields

**Files:**
- Modify: `crates/woc-version/src/compat.rs` (add `min_client_version_from_env`)
- Modify: `crates/woc-version/src/lib.rs` (`VersionInfo`)
- Modify: `crates/woc-server/src/main.rs` (`VersionInfo::current(PROTOCOL_REV)`)
- Test: `crates/woc-version/src/lib.rs`

**Interfaces:**
- Consumes: `REWRITE_VERSION`; `protocol_rev: u32` from the server
- Produces:
  - `VersionInfo { rewrite_version: String, upstream_version: String, upstream_commit: String, upstream_repo: String, parity_target: String, protocol_rev: u32, min_client_version: String }` with `Serialize + Deserialize`
  - `VersionInfo::current(protocol_rev: u32) -> Self`
  - `impl VersionInfo { pub fn realm_identity(&self) -> RealmIdentity }`
  - `pub fn min_client_version_from_env() -> String`

- [ ] **Step 1: Write the failing tests**

In `crates/woc-version/src/lib.rs` tests module, add:

```rust
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
```

`VersionInfo::current` currently takes no arguments and has no `protocol_rev` field — this test must fail to compile (or fail at runtime once the signature exists but returns defaults).

Also add `serde_json = "1"` under `[dev-dependencies]` in `crates/woc-version/Cargo.toml` (workspace already has `serde_json`). Prefer:

```toml
[dev-dependencies]
toml = "0.8"
serde_json = { workspace = true }
```

Workspace already defines `serde_json`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-version current_includes_protocol_and_min_client -q`

Expected: compile FAIL (`this function takes 0 arguments but 1 argument was supplied`) or missing fields.

- [ ] **Step 3: Implement payload + env helper**

In `compat.rs`:

```rust
pub fn min_client_version_from_env() -> String {
    std::env::var("WOC_MIN_CLIENT_VERSION")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::REWRITE_VERSION.to_string())
}
```

Export it from `lib.rs` (`pub use compat::min_client_version_from_env`).

Replace `VersionInfo` in `lib.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::compat::{min_client_version_from_env, RealmIdentity};

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
```

Remove the old `use serde::Serialize;` at the top of `lib.rs` (replaced above). Keep `pub use` of compat items including `min_client_version_from_env`.

In `crates/woc-server/src/main.rs`, change the version handler and add the protocol import:

```rust
use woc_protocol::PROTOCOL_REV;
use woc_version::{footer, VersionInfo};
```

```rust
async fn version() -> Json<VersionInfo> {
    Json(VersionInfo::current(PROTOCOL_REV))
}
```

`woc-server` already depends on `woc-protocol`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-version -q && cargo check -p woc-server`

Expected: PASS / finished `check`

- [ ] **Step 5: Commit**

```bash
git add crates/woc-version/src/lib.rs crates/woc-version/src/compat.rs crates/woc-version/Cargo.toml crates/woc-server/src/main.rs
git commit -m "feat(version): advertise protocol_rev and min_client_version on /version"
```

---

### Task 3: Hello identity fields (protocol rev stays 6)

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs:8-12` (comment), `:692-706` (`Hello`), `:785-810` (`ws_hello_roundtrip`), `:1117-1135` (`old_ws_hello_json_still_deserializes`)
- Test: same file

**Interfaces:**
- Consumes: existing `WsClientMsg::Hello { name, class_id, token, character_id }`
- Produces: same variant plus `protocol_rev: Option<u32>` and `rewrite_version: Option<String>`, both `#[serde(default)]`

- [ ] **Step 1: Write the failing test**

In `old_ws_hello_json_still_deserializes`, after the existing asserts (once the fields exist), we will assert the new fields are `None`. First add a new test that will fail to compile until fields exist — put it next to `ws_hello_roundtrip`:

```rust
    #[test]
    fn ws_hello_identity_roundtrip() {
        let msg = WsClientMsg::Hello {
            name: "Ada".into(),
            class_id: "mage".into(),
            token: Some("tok".into()),
            character_id: Some("11111111-1111-1111-1111-111111111111".into()),
            protocol_rev: Some(6),
            rewrite_version: Some("1.4.0".into()),
        };
        let s = serde_json::to_string(&msg).unwrap();
        assert!(s.contains("protocol_rev"));
        assert!(s.contains("rewrite_version"));
        let back: WsClientMsg = serde_json::from_str(&s).unwrap();
        match back {
            WsClientMsg::Hello {
                protocol_rev,
                rewrite_version,
                ..
            } => {
                assert_eq!(protocol_rev, Some(6));
                assert_eq!(rewrite_version.as_deref(), Some("1.4.0"));
            }
            _ => panic!("expected Hello"),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-protocol ws_hello_identity_roundtrip -q`

Expected: compile FAIL (`missing fields protocol_rev, rewrite_version` or `no such field`)

- [ ] **Step 3: Add fields and fix every Hello construction/match**

Update the protocol comment:

```rust
/// Protocol revision for snapshot / WS envelopes (0.1 was implicit rev 1).
/// Rev 3: authenticated Hello (`token` + `character_id`) and inventory slot indices.
/// Rev 4: jump / swim / flight intent + motion snapshot flags.
/// Rev 5: clear_target intent + ability_bar kit slots for combat HUD.
/// Rev 6: bank copper + pending loot (this rev). Hello may also carry additive
/// `protocol_rev` / `rewrite_version` identity; omitting them is valid JSON and
/// the server refuses those Hellos (policy, not a wire bump).
pub const PROTOCOL_REV: u32 = 6;
```

Hello variant:

```rust
    Hello {
        #[serde(default)]
        name: String,
        #[serde(default)]
        class_id: String,
        /// Bearer session token from REST login/register.
        #[serde(default)]
        token: Option<String>,
        /// Durable character UUID (string form).
        #[serde(default)]
        character_id: Option<String>,
        /// Client protocol revision. Missing (old clients) deserializes as `None`.
        #[serde(default)]
        protocol_rev: Option<u32>,
        /// Client rewrite semver. Missing deserializes as `None`.
        #[serde(default)]
        rewrite_version: Option<String>,
    },
```

Fix **all** Hello construction/match sites in this crate (compiler lists them):

1. `ws_hello_roundtrip` constructor: add `protocol_rev: None, rewrite_version: None` (or `Some` — either is fine; prefer `None` so the test still covers defaults on the way back if you serialize them as null — `None` with default skip? Serde includes `"protocol_rev":null` unless `skip_serializing_if`. Do **not** add skip; null is fine. Match arm: add `..` or bind the new fields and `assert!(protocol_rev.is_none())`.
2. `old_ws_hello_json_still_deserializes` match: add `protocol_rev, rewrite_version` (or `..`) and `assert!(protocol_rev.is_none()); assert!(rewrite_version.is_none());`

Do **not** edit `woc-client` / `woc-server` in this task if you want a green protocol crate first — the workspace `cargo test --workspace --exclude woc-client` **will fail to compile `woc-server`** until Task 4 fills the Hello match. Implement Task 3 and Task 4 in the same working tree before claiming workspace green; still commit Task 3 as its own commit if `woc-protocol` tests pass:

Run: `cargo test -p woc-protocol -q`

Leave `woc-server` / `woc-client` compile errors for Task 4 / Task 6. If CI on the branch runs workspace clippy, **land Task 4 in the same push** as Task 3.

- [ ] **Step 4: Run protocol tests**

Run: `cargo test -p woc-protocol -q`

Expected: PASS

- [ ] **Step 5: Commit** (same push as Task 4 if CI would otherwise fail)

```bash
git add crates/woc-protocol/src/lib.rs
git commit -m "feat(protocol): additive Hello protocol_rev and rewrite_version"
```

---

### Task 4: Server Hello gate before spawn

**Files:**
- Modify: `crates/woc-server/src/game_ws.rs:13-14` (imports), `:132-145` (Hello match)
- Test: `crates/woc-server/src/game_ws.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `ClientIdentity::from_hello`, `check_compat`, `RealmIdentity`, `min_client_version_from_env`, `REWRITE_VERSION`, `PROTOCOL_REV`, Hello fields from Task 3
- Produces: `WsServerMsg::Error { message: compat.user_message() }` and `continue` without spawn when `!compat.is_ok()`

- [ ] **Step 1: Write the failing test**

At the top of `game_ws.rs` tests module, add:

```rust
    use woc_version::{check_compat, min_client_version_from_env, ClientIdentity, Compat, RealmIdentity, REWRITE_VERSION};

    fn test_realm() -> RealmIdentity {
        RealmIdentity {
            rewrite_version: REWRITE_VERSION.to_string(),
            protocol_rev: Some(PROTOCOL_REV),
            min_client_version: min_client_version_from_env(),
        }
    }

    #[test]
    fn hello_without_identity_rejected() {
        let c = check_compat(&ClientIdentity::from_hello(None, None), &test_realm());
        assert!(!c.is_ok());
        assert!(c.user_message().starts_with("version:"));
    }

    #[test]
    fn hello_with_current_identity_accepted() {
        let c = check_compat(
            &ClientIdentity::from_hello(Some(PROTOCOL_REV), Some(REWRITE_VERSION)),
            &test_realm(),
        );
        assert_eq!(c, Compat::Compatible);
    }

    #[test]
    fn hello_wrong_protocol_rejected() {
        let c = check_compat(
            &ClientIdentity::from_hello(Some(PROTOCOL_REV.saturating_sub(1)), Some(REWRITE_VERSION)),
            &test_realm(),
        );
        assert!(matches!(c, Compat::ProtocolMismatch { .. }));
    }
```

These pass as soon as Task 1 exists; they lock the **server’s intended call**. The compile failure for this task is the Hello match still missing fields.

- [ ] **Step 2: Confirm Hello match does not compile**

Run: `cargo test -p woc-server hello_without_identity_rejected -q`

Expected: compile FAIL on `WsClientMsg::Hello { name: _, class_id: _, token, character_id }` (`missing fields protocol_rev, rewrite_version`) after Task 3.

- [ ] **Step 3: Gate Hello before auth**

Imports:

```rust
use woc_protocol::{EntityId, WorldHost, WsClientMsg, WsServerMsg, PROTOCOL_REV, TICK_RATE};
use woc_version::{
    check_compat, min_client_version_from_env, ClientIdentity, RealmIdentity, REWRITE_VERSION,
};
```

Replace the Hello match head and insert the gate **immediately after** destructuring, **before** token checks:

```rust
            WsClientMsg::Hello {
                name: _,
                class_id: _,
                token,
                character_id,
                protocol_rev,
                rewrite_version,
            } => {
                let realm = RealmIdentity {
                    rewrite_version: REWRITE_VERSION.to_string(),
                    protocol_rev: Some(PROTOCOL_REV),
                    min_client_version: min_client_version_from_env(),
                };
                let client = ClientIdentity::from_hello(
                    protocol_rev,
                    rewrite_version.as_deref(),
                );
                match check_compat(&client, &realm) {
                    woc_version::Compat::Compatible => {}
                    other => {
                        let _ = out_tx.send(err_json(&other.user_message()));
                        continue;
                    }
                }
                let Some(token) = token.filter(|t| !t.is_empty()) else {
                    let _ = out_tx.send(err_json("Hello requires token + character_id"));
                    continue;
                };
                // ... existing character_id / persist / spawn unchanged ...
```

Use `woc_version::Compat::Compatible` or import `Compat`. Do not spawn, park, or look up the token on mismatch.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-server -q && cargo test -p woc-protocol -q && cargo test -p woc-version -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-server/src/game_ws.rs crates/woc-protocol/src/lib.rs
git commit -m "feat(server): reject Hello when client identity is incompatible"
```

If Task 3 was not committed separately, include `crates/woc-protocol/src/lib.rs` here as shown.

---

### Task 5: Title Online `/version` preflight

**Files:**
- Modify: `crates/woc-client/src/api.rs` (fetch helper + deserialize test)
- Modify: `crates/woc-client/src/main.rs` (`RealmCompat` resource + `init_resource`)
- Modify: `crates/woc-client/src/title.rs` (probe, status line, block Continue)
- Modify: `crates/woc-client/src/menu_ui.rs:232-250` (`status_color`)
- Test: `crates/woc-client/src/api.rs` (deserialize only; CI excludes `woc-client` — policy already covered in `woc-version`)

**Interfaces:**
- Consumes: `GET {API_BASE}/version` → `woc_version::VersionInfo`; `check_compat`; `ClientIdentity { rewrite_version: REWRITE_VERSION, protocol_rev: PROTOCOL_REV }`; `VersionInfo::realm_identity`
- Produces: `RealmCompat` resource; title Continue to Login only when `Compatible`

- [ ] **Step 1: Write the failing client JSON test**

In `crates/woc-client/src/api.rs` tests:

```rust
    #[test]
    fn version_info_from_server_json() {
        let json = r#"{
            "rewrite_version": "1.3.0",
            "upstream_version": "0.31.0",
            "upstream_commit": "abc",
            "upstream_repo": "https://example.invalid",
            "parity_target": "online-hard",
            "protocol_rev": 6,
            "min_client_version": "1.3.0"
        }"#;
        let info: woc_version::VersionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.protocol_rev, 6);
        assert_eq!(info.min_client_version, "1.3.0");
    }
```

This passes once Task 2 landed; it locks the client’s decode path. The rest of this task is compile-gated by `cargo check -p woc-client`.

- [ ] **Step 2: Run the JSON test locally**

Run: `cargo test -p woc-client version_info_from_server_json -q`

Expected: PASS if GPU-less unit tests in this crate run on the agent (they do not need a window). If the crate fails to link Bevy in this environment, skip and rely on `cargo check -p woc-client` after Step 3.

- [ ] **Step 3: Fetch helper, resource, title probe**

`api.rs` — add after the existing spawn helpers (reuse `agent()`):

```rust
use woc_version::VersionInfo;

fn fetch_version_blocking() -> Result<VersionInfo, String> {
    let url = format!("{API_BASE}/version");
    match agent().get(&url).call() {
        Ok(resp) => resp
            .into_json::<VersionInfo>()
            .map_err(|e| format!("bad version response: {e}")),
        Err(ureq::Error::Status(_, resp)) => Err(read_error(resp)),
        Err(e) => Err(format!("request failed: {e}")),
    }
}

/// Spawn a thread that GETs `/version`.
pub fn spawn_fetch_version() -> Receiver<Result<VersionInfo, String>> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("woc-api-version".into())
        .spawn(move || {
            let _ = tx.send(fetch_version_blocking());
        })
        .expect("spawn api version");
    rx
}
```

`main.rs` — add resource and init it:

```rust
use std::sync::mpsc::Receiver;
use woc_version::VersionInfo;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum RealmCompatState {
    #[default]
    Idle,
    Checking,
    Compatible {
        realm_rewrite: String,
        protocol_rev: u32,
    },
    Incompatible {
        message: String,
    },
    Unreachable {
        message: String,
    },
}

#[derive(Resource, Default)]
pub(crate) struct RealmCompat {
    pub(crate) state: RealmCompatState,
    pub(crate) pending: Option<Receiver<Result<VersionInfo, String>>>,
}

impl RealmCompat {
    pub(crate) fn status_line(&self) -> String {
        match &self.state {
            RealmCompatState::Idle => "Online: not checked".into(),
            RealmCompatState::Checking => "Online: checking realm version…".into(),
            RealmCompatState::Compatible {
                realm_rewrite,
                protocol_rev,
            } => format!("Online: compatible · realm {realm_rewrite} · proto {protocol_rev}"),
            RealmCompatState::Incompatible { message } | RealmCompatState::Unreachable { message } => {
                message.clone()
            }
        }
    }

    pub(crate) fn begin_probe(&mut self) {
        if matches!(self.state, RealmCompatState::Checking) {
            return;
        }
        self.state = RealmCompatState::Checking;
        self.pending = Some(crate::api::spawn_fetch_version());
    }

    pub(crate) fn poll(&mut self) {
        let Some(rx) = self.pending.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(info)) => {
                self.pending = None;
                let client = woc_version::ClientIdentity {
                    rewrite_version: woc_version::REWRITE_VERSION.to_string(),
                    protocol_rev: woc_protocol::PROTOCOL_REV,
                };
                match woc_version::check_compat(&client, &info.realm_identity()) {
                    woc_version::Compat::Compatible => {
                        self.state = RealmCompatState::Compatible {
                            realm_rewrite: info.rewrite_version,
                            protocol_rev: info.protocol_rev,
                        };
                    }
                    other => {
                        self.state = RealmCompatState::Incompatible {
                            message: other.user_message(),
                        };
                    }
                }
            }
            Ok(Err(message)) => {
                self.pending = None;
                self.state = RealmCompatState::Unreachable {
                    message: format!("version: unreachable ({message})"),
                };
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                self.state = RealmCompatState::Unreachable {
                    message: "version: unreachable (version thread disconnected)".into(),
                };
            }
        }
    }
}
```

In `fn main()`, after `.init_resource::<AuthSession>()`:

```rust
        .init_resource::<RealmCompat>()
```

`menu_ui.rs` `status_color` — add:

```rust
    || lower.starts_with("version:")
    || lower.contains("outdated")
    || lower.contains("unreachable")
```

next to the existing `lower.contains("mismatch")` checks.

`title.rs`:

- Import `RealmCompat`, `RealmCompatState`, `menu_ui::{status_color, ERR}` as needed, `BODY`.
- Add `#[derive(Component)] struct CompatLabel;`
- In `setup_title`, after the footer text spawn, add:

```rust
                p.spawn((
                    CompatLabel,
                    Text::new("Offline: version check skipped"),
                    TextFont::from_font_size(13.0),
                    TextColor(MUTED),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                ));
```

- Change `continue_from_title` to take `compat: &mut RealmCompat`:

```rust
fn continue_from_title(mode: PlayMode, next: &mut NextState<AppState>, compat: &mut RealmCompat) {
    match mode {
        PlayMode::Offline => next.set(AppState::CharCreate),
        PlayMode::Online => {
            if matches!(compat.state, RealmCompatState::Compatible { .. }) {
                next.set(AppState::Login);
            } else {
                compat.begin_probe();
            }
        }
    }
}
```

- `title_clicks` / `title_input`: add `mut compat: ResMut<RealmCompat>`. When setting `PlayMode::Online`, call `compat.begin_probe()`. Pass `&mut compat` into `continue_from_title`.
- Add systems to the title `Update` chain (after `title_input`): `poll_realm_compat`, `refresh_compat_label`.

```rust
fn poll_realm_compat(mut compat: ResMut<RealmCompat>) {
    compat.poll();
}

fn refresh_compat_label(
    mode: Res<PlayMode>,
    compat: Res<RealmCompat>,
    mut label: Query<(&mut Text, &mut TextColor), With<CompatLabel>>,
) {
    let Ok((mut text, mut color)) = label.single_mut() else {
        return;
    };
    match *mode {
        PlayMode::Offline => {
            **text = "Offline: version check skipped".into();
            *color = TextColor(MUTED);
        }
        PlayMode::Online => {
            let line = compat.status_line();
            let busy = matches!(compat.state, RealmCompatState::Checking);
            **text = line.clone();
            *color = TextColor(status_color(busy, &line));
        }
    }
}
```

`title_clicks` must not call `begin_probe` on Offline clicks. Online button / Digit2 / toggle-to-Online → `begin_probe`.

- [ ] **Step 4: Compile the client**

Run: `cargo check -p woc-client`

Expected: finished `dev` profile check. Then:

Run: `cargo test --workspace --exclude woc-client -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/api.rs crates/woc-client/src/main.rs crates/woc-client/src/title.rs crates/woc-client/src/menu_ui.rs
git commit -m "feat(client): fail-closed online version preflight on title"
```

---

### Task 6: Hello identity + Welcome kick

**Files:**
- Modify: `crates/woc-client/src/online.rs:39` (imports), `:59-65` (Hello)
- Modify: `crates/woc-client/src/world_setup.rs` (`apply_online_messages` + InWorld kick system)
- Test: none in CI; `cargo check -p woc-client` is the gate. Policy for Welcome mismatch is the same `Compat::ProtocolMismatch` messages as Task 1.

**Interfaces:**
- Consumes: `PROTOCOL_REV`, `REWRITE_VERSION`, `Compat::user_message` / `check_compat` not required on Welcome — compare `protocol_rev` integers and format via `Compat::ProtocolMismatch`
- Produces: Hello always sends `Some(PROTOCOL_REV)` and `Some(REWRITE_VERSION.to_string())`; Welcome skew → `NetStatus::Error` starting with `version:` → `AppState::Title`

- [ ] **Step 1: Write a tiny protocol-level lock (optional but keep CI green)**

Already covered by Task 3 `ws_hello_identity_roundtrip`. No new failing test required. Proceed to implementation.

- [ ] **Step 2: Confirm current Hello does not compile** (if Task 3 landed)

Run: `cargo check -p woc-client`

Expected: FAIL `missing fields protocol_rev, rewrite_version` in `online.rs` until Step 3.

- [ ] **Step 3: Fill Hello; check Welcome; kick to Title**

`online.rs`:

```rust
use woc_protocol::{WsClientMsg, WsServerMsg, PROTOCOL_REV};
use woc_version::REWRITE_VERSION;
```

```rust
    let hello = WsClientMsg::Hello {
        name: String::new(),
        class_id: String::new(),
        token: Some(token),
        character_id: Some(character_id.to_string()),
        protocol_rev: Some(PROTOCOL_REV),
        rewrite_version: Some(REWRITE_VERSION.to_string()),
    };
```

`world_setup.rs` — change `apply_online_messages` to return whether to leave the world:

```rust
fn apply_online_messages(host: &mut GameHost) -> bool {
    // ... drain pending as today ...
    let mut kick = false;
    for msg in pending {
        match msg {
            WsServerMsg::Welcome {
                player_id,
                protocol_rev,
            } => {
                if protocol_rev != woc_protocol::PROTOCOL_REV {
                    let message = woc_version::Compat::ProtocolMismatch {
                        client_rev: woc_protocol::PROTOCOL_REV,
                        realm_rev: protocol_rev,
                    }
                    .user_message();
                    host.net_status = NetStatus::Error(message.clone());
                    host.recent_toasts.push((message, 5.0));
                    kick = true;
                    continue;
                }
                host.net_status = NetStatus::Connected { player_id };
                host.snapshot.player_id = player_id;
                host.recent_toasts.push((
                    format!("Welcome · player #{player_id} · proto {protocol_rev}"),
                    3.0,
                ));
            }
            WsServerMsg::Error { message } => {
                host.net_status = NetStatus::Error(message.clone());
                host.recent_toasts.push((message.clone(), 5.0));
                if message.starts_with("version:") {
                    kick = true;
                }
            }
            // Snapshot / Events / Chat / PartyUpdate unchanged
            ...
        }
    }
    kick
}
```

`sim_fixed_step` currently takes no `NextState`. Add a sibling system registered from `main.rs` in the InWorld chain **or** from `world_setup::plugin`:

In `world_setup::plugin`:

```rust
        .add_systems(
            Update,
            kick_incompatible_session.run_if(in_state(AppState::InWorld)),
        );
```

```rust
fn kick_incompatible_session(mut host: ResMut<GameHost>, mut next: ResMut<NextState<AppState>>) {
    if host.is_online() && apply_online_messages(&mut host) {
        next.set(AppState::Title);
    }
}
```

**Do not** call `apply_online_messages` twice. Today `sim_fixed_step` calls it when `host.is_online()`. Split:

- Remove the `apply_online_messages` call from `sim_fixed_step`.
- `kick_incompatible_session` always drains the WS queue for online hosts (even when not kicking).
- `sim_fixed_step` still applies snapshots already stored on `host` after the kick system runs.

Order: register `kick_incompatible_session` **before** `sim_fixed_step` in `main.rs`’s InWorld chain (it already lists `world_setup::sim_fixed_step`). Adding the system on `world_setup::plugin` with default order may run after `sim_fixed_step` depending on plugin order. **Safest:** keep draining inside `sim_fixed_step` but pass `Option<&mut NextState<AppState>>`.

Preferred (one drain, explicit order) — change `sim_fixed_step` signature:

```rust
pub(crate) fn sim_fixed_step(
    time: Res<Time>,
    mut host: ResMut<GameHost>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: Query<(Entity, &SimVisual, &mut VisualMotion)>,
) {
    if host.is_online() {
        if apply_online_messages(&mut host) {
            next.set(AppState::Title);
            return;
        }
        // ... existing accumulator / intent send ...
```

`OnExit(InWorld)` already runs `cleanup_world`. Returning early skips one visual sync; that is fine.

- [ ] **Step 4: Compile**

Run: `cargo check -p woc-client && cargo test --workspace --exclude woc-client -q`

Expected: check finished, tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/online.rs crates/woc-client/src/world_setup.rs
git commit -m "feat(client): send Hello identity and kick on protocol mismatch"
```

---

### Task 7: Docs, demo notes, rewrite `1.4.0`

**Files:**
- Modify: `VERSION.toml` (`rewrite_version`, `parity_target`)
- Modify: `crates/woc-version/src/lib.rs` (`REWRITE_VERSION`, `PARITY_TARGET`)
- Modify: `Cargo.toml` (`workspace.package.version`)
- Modify: `CHANGELOG.md` (Unreleased → 1.4.0 notes)
- Modify: `README.md` (badges, footer, `/version` fields, `WOC_MIN_CLIENT_VERSION`)
- Modify: `docs/ROADMAP.md`
- Modify: `docs/parity/STATUS.md`
- Modify: `docs/parity/DEMO.md` (one online bullet: title must show compatible)
- Test: `crates/woc-version` `constants_match_version_toml`

**Interfaces:**
- Consumes: shipped Tasks 1–6
- Produces: rewrite `1.4.0` / parity `client-compat`

- [ ] **Step 1: Write the failing pin test**

Change **only** `VERSION.toml` first:

```toml
rewrite_version = "1.4.0"
upstream_repo = "https://github.com/levy-street/world-of-claudecraft"
upstream_version = "0.31.0"
upstream_commit = "a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9"
parity_target = "client-compat"
```

- [ ] **Step 2: Run pin test**

Run: `cargo test -p woc-version constants_match_version_toml -q`

Expected: FAIL (`assert_eq!` rewrite_version `"1.3.0"` vs `"1.4.0"`)

- [ ] **Step 3: Sync constants and docs**

`crates/woc-version/src/lib.rs`:

```rust
pub const REWRITE_VERSION: &str = "1.4.0";
pub const PARITY_TARGET: &str = "client-compat";
```

Root `Cargo.toml`:

```toml
version = "1.4.0"
```

`CHANGELOG.md` — under `## Unreleased` add an `### Added` bullet, and a new `## 1.4.0` section (keep Unreleased for later):

```markdown
## Unreleased

### Added

- Online client version gate (`1.4.0` / `client-compat`): title `/version` preflight, Hello identity, Welcome kick.

## 1.4.0 — 2026-08-13

### Added

- `woc-version::check_compat` fail-closed policy (semver floor + exact `protocol_rev`).
- `GET /version` fields `protocol_rev` and `min_client_version` (`WOC_MIN_CLIENT_VERSION` override).
- Hello additive `protocol_rev` / `rewrite_version`; server rejects missing or stale identity before spawn.
- Title Online Continue blocked until compatible; Welcome protocol skew returns to Title.
```

(Keep the existing Unreleased combat/online bullets **above** or move them under `## 1.3.0` if they already shipped — do **not** delete 1.3.0 history. If current Unreleased is the 1.3.0 landing notes, cut that block into `## 1.3.0` dated from `CHANGELOG` / git, and let `## Unreleased` start the 1.4.0 bullets. Inspect `CHANGELOG.md` at implementation time and preserve history.)

`README.md`:

- Badge `rewrite-1.4.0`
- Opening sentence **Rewrite `1.4.0`** / parity **`client-compat`**
- Footer example `WoC-rs 1.4.0 · upstream 0.31.0`
- After the `curl http://127.0.0.1:8787/version` comment, document:

```text
GET /version includes protocol_rev and min_client_version.
Online title probes this before Login (fail-closed).
WOC_MIN_CLIENT_VERSION=1.4.0 overrides the floor (default: rewrite version).
```

`docs/ROADMAP.md` — the planning PR already added a **1.4.0 (planned)** row and a “Client version gate (planned)” section. Flip those to shipped:

```markdown
| **1.4.0** (this branch) | `client-compat` | Online version gate (title preflight + Hello identity) |
```

Rename the section heading from “planned” to current, keep the spec/plan links.

`docs/parity/STATUS.md`:

- Current rewrite line → `1.4.0` / `client-compat`
- New table section:

```markdown
## Client version gate (`client-compat`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| `check_compat` policy | done | `woc-version`; prerelease suffix stripped |
| `/version` protocol + min client | done | `WOC_MIN_CLIENT_VERSION` |
| Hello identity | done | Additive; missing → reject |
| Title Online preflight | done | Fail-closed |
| Welcome kick | done | `version:` → Title |
| Packaged auto-update | deferred | No installers |
```

`docs/parity/DEMO.md` — add one line to the online path: title must show `Online: compatible` before Continue.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-version -q && cargo test --workspace --exclude woc-client -q && cargo check -p woc-client`

Expected: PASS / check finished. Clippy: `cargo clippy --workspace --exclude woc-client -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add VERSION.toml Cargo.toml crates/woc-version/src/lib.rs CHANGELOG.md README.md docs/ROADMAP.md docs/parity/STATUS.md docs/parity/DEMO.md
git commit -m "chore: tag rewrite 1.4.0 client-compat version gate"
```

---

## Manual verification (after Task 7)

Not CI. Agent or human:

1. `cargo run -p woc-server` and `cargo run -p woc-client` → **2 Online** → status compatible → Continue → Login as today.
2. Stop the server → **2 Online** → Continue stays on Title, status starts with `version: unreachable`.
3. `WOC_MIN_CLIENT_VERSION=9.0.0 cargo run -p woc-server` → current client `version: update required`.

---

## Self-review

1. **Spec coverage:** Goal (fail-closed gate), `/version` fields, Hello identity, Welcome kick, min-client env, non-goals (no auto-update), 1.4.0 bump — Tasks 1–7.
2. **Placeholders:** None. Hello match sites listed. `PROTOCOL_REV` stays 6.
3. **Types:** `ClientIdentity` / `RealmIdentity` / `Compat` / `VersionInfo::current(protocol_rev)` / `from_hello` names are consistent across tasks.
