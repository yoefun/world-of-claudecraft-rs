# Client version gate design — `1.4.0` / `client-compat`

**Status:** Proposed (planning deliverable 2026-08-13).  
**Baseline:** rewrite `1.3.0` / parity `online-hard` on `develop`.  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `client-compat`.

Post-completion program (shipped through 1.3.0): [`2026-08-13-post-completion-program-design.md`](2026-08-13-post-completion-program-design.md).

## 1. Goal

Online play currently trusts that every Bevy client and `woc-server` were built from the same tree. `GET /version` exists, `Welcome` already carries `protocol_rev`, and the HUD footer prints `WoC-rs 1.3.0`, but **nothing compares those numbers before spawn**. An old client can log in, receive a newer snapshot, and silently accept `#[serde(default)]` holes.

This program adds a **fail-closed version gate** so a mismatched online client never enters the realm.

> Same-tree cargo-run stays one click. A stale binary is stopped at the title screen and again at Hello, with English copy that says whether the *client* or the *realm* is the one that must change.

## 2. Baseline (already shipped on `develop`)

| Piece | State |
| --- | --- |
| Version | `1.3.0` / parity `online-hard` / protocol rev **6** |
| `woc-version` | Pin constants + `VersionInfo` JSON for `GET /version` (serialize only; no compat policy) |
| HTTP | `GET /version` returns rewrite/upstream/parity; no `protocol_rev`, no `min_client_version` |
| WS Hello | `token` + `character_id` only; no client identity |
| WS Welcome | `player_id` + `protocol_rev`; client toasts the rev and **does not compare it** |
| Online title | Continue → Login with no realm probe |
| Packaging | None. `cargo run -p woc-client` / `woc-server`. No installers, no update feed |

### Honest remaining debt

1. **`/version` is unused by the client.** `crates/woc-client/src/api.rs` talks to `/api/*` only.
2. **Hello cannot be rejected for version.** `game_ws.rs` authenticates and spawns before any rev check.
3. **Welcome `protocol_rev` is decorative.** `world_setup.rs` logs it; a mismatch still applies snapshots.
4. **No min-client lever.** Operators cannot force a rebuild without a protocol bump (or hoping players notice the footer).

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Native auto-update** | Packaged installers + signed feed + self-replace binary (electron-updater analogue) | Needs packaging, a host, signing, prod/dev tracks. Electron/browser are explicit non-goals | Reject |
| **B. Advisory toast only** | Fetch `/version`, banner on title, still allow Login | Old clients still spawn into a breaking snapshot | Reject |
| **C. Handshake gate + HTTP preflight (recommended)** | Pure policy in `woc-version`; `/version` advertises `protocol_rev` + `min_client_version`; title Online fail-closed; Hello carries identity; server rejects before spawn; Welcome double-checks | No download UX; players rebuild/redownload out of band | **Adopt** |

Approach A is the upstream Electron product. This rewrite’s client is Bevy and unpackaged. Do not import `docs/desktop-release.md` from levy-street.

## 4. Version map

| Rewrite | Parity label | Theme | Gate |
| --- | --- | --- | --- |
| **1.3.0** (shipped) | `online-hard` | Park/resume, AOI, Postgres notes | Disconnect → Hello resumes same entity |
| **1.4.0** (this program) | `client-compat` | Online version gate | Title blocks mismatch; Hello rejects stale identity; Welcome kicks on rev skew |

Upstream pin stays **0.31.0**. Protocol rev stays **6** (Hello fields are additive `#[serde(default)]`). Server *policy* becomes fail-closed for missing identity: an old Hello is valid JSON and is refused.

## 5. Architecture

Keep one sim, multiple hosts. Version policy is **not** a sim system and **not** a Bevy component.

```text
Title (Online selected)
    │  GET http://127.0.0.1:8787/version     (existing ureq worker thread)
    ▼
woc-version::check_compat(client, realm)
    │  Compatible → Login → CharSelect → InWorld
    │  otherwise stay on Title with user_message()
    ▼
WS Hello { token, character_id, protocol_rev, rewrite_version }
    │  server check_compat BEFORE auth/spawn
    │  reject → WsServerMsg::Error { message: user_message() }
    ▼
Welcome { player_id, protocol_rev }
    │  client: protocol_rev == PROTOCOL_REV
    │  mismatch → NetStatus::Error + return to Title
```

Offline mode never calls `/version` and never sends Hello.

### 5.1 Policy crate (`woc-version`)

`woc-version` stays a leaf: **do not** depend on `woc-protocol` or Bevy. Callers pass `protocol_rev` in.

Types:

```text
SemVer { major, minor, patch }          // numeric triple; strip a single -prerelease suffix
ClientIdentity { rewrite_version, protocol_rev }
RealmIdentity  { rewrite_version, protocol_rev: Option<u32>, min_client_version }
Compat =
    Compatible
  | ClientTooOld { client, min_client }
  | ProtocolMismatch { client_rev, realm_rev }
  | BadClientVersion(String)
  | BadMinVersion(String)
```

Rules, in order:

1. Parse `client.rewrite_version` and `realm.min_client_version` as `SemVer`. Fail → `BadClientVersion` / `BadMinVersion`.
2. If `realm.protocol_rev` is `Some(r)` and `client.protocol_rev != r` → `ProtocolMismatch`. `None` means “legacy `/version` omitted the field; skip protocol here” (Welcome still checks).
3. If `client` triple `<` `min_client` triple → `ClientTooOld`.
4. Else `Compatible`. A *newer* client against an older realm is allowed when protocol matches (or was skipped) and `client >= min_client`.

Prerelease: `1.4.0-pre` and `1.4.0` compare equal (suffix stripped). Operators who need to block a pre tag set `WOC_MIN_CLIENT_VERSION=1.4.1`.

`Compat::user_message()` is the only player-facing string (English):

| Variant | Copy |
| --- | --- |
| `ClientTooOld` | `version: update required (client {client} < min {min_client})` |
| `ProtocolMismatch` and `client_rev < realm_rev` | `version: update required (protocol {client_rev} < {realm_rev})` |
| `ProtocolMismatch` and `client_rev >= realm_rev` | `version: realm outdated (protocol {client_rev} > {realm_rev})` |
| `BadClientVersion` / `BadMinVersion` | `version: invalid version string ({details})` |

All messages start with `version:` so the client can kick to Title without parsing the rest.

Missing Hello fields: treat `protocol_rev: None` as `0`, `rewrite_version: None` as `"(unknown)"` (fails parse → `BadClientVersion`). Fail-closed.

### 5.2 `GET /version` payload

Additive JSON. Existing keys stay. New keys:

| Field | Type | Default if omitted (old server) |
| --- | --- | --- |
| `protocol_rev` | `u32` | `0` → client treats as “unknown”, skips HTTP protocol check |
| `min_client_version` | `string` | empty → client uses `rewrite_version` as the floor |

`VersionInfo` becomes `Serialize + Deserialize` with owned `String` fields so the Bevy client can decode the same struct. `VersionInfo::current(protocol_rev: u32)` fills pins plus `min_client_version` from env:

- `WOC_MIN_CLIENT_VERSION` if set and non-empty
- else `REWRITE_VERSION`

`woc-server` passes `PROTOCOL_REV` into `current`. `woc-version` does not import `woc-protocol`.

### 5.3 Hello (protocol rev stays 6)

Additive fields on `WsClientMsg::Hello`:

```text
protocol_rev: Option<u32>        // serde default
rewrite_version: Option<String>  // serde default
```

Old JSON `{"type":"hello","name":"Ada","class_id":"mage"}` still deserializes. New clients always send `Some(PROTOCOL_REV)` and `Some(REWRITE_VERSION.to_string())`.

Server runs `check_compat` **before** token lookup / spawn / park. Incompatible Hello gets `Error` and does not bind a player.

Do **not** bump `PROTOCOL_REV`. Snapshot schema is unchanged. Force-update is a server policy on missing identity, not a wire break.

### 5.4 Title preflight (Bevy)

Reuse the existing blocking `ureq` + `mpsc` pattern in `api.rs` (`spawn_fetch_version` → worker thread → `GET {API_BASE}/version`).

Resource `RealmCompat` on the app:

```text
Idle
Checking
Compatible { realm_rewrite, protocol_rev }
Incompatible { message }
Unreachable { message }   // connect/timeout/bad JSON
```

Behavior:

- Offline Continue: unchanged (CharCreate). Never fetches.
- Switching to Online (key `2` / click): spawn fetch if `Idle` or previous result is stale.
- Online Continue / Enter: only if `Compatible`. If `Idle`, spawn fetch and stay. If `Checking`, stay. If `Incompatible` / `Unreachable`, stay and show the status line in `ERR` color.
- Fetch failure is fail-closed for Online (do not log in “because the version host is down”).

Status line on the title panel (new `CompatLabel` under the footer): English text from the resource. No download button. No URL opener.

### 5.5 Welcome double-check

In `apply_online_messages`, if `Welcome.protocol_rev != PROTOCOL_REV`, set `NetStatus::Error(ProtocolMismatch.user_message())`, toast, and `NextState<AppState>::Title`. Same kick if `Error.message` starts with `version:`.

In-world has no logout today; this is the path back to Title when the realm refuses the client after Hello.

### 5.6 Client Hello construction

`online::spawn_online_session` fills the new fields from `woc_protocol::PROTOCOL_REV` and `woc_version::REWRITE_VERSION`.

## 6. Definition of done (`1.4.0` / `client-compat`)

1. `check_compat` unit tests lock: equal → ok; client `<` min → `ClientTooOld`; protocol skew both directions; missing Hello identity fails; `1.4.0-pre` vs `1.4.0` equal; legacy `/version` JSON without new keys deserializes and skips HTTP protocol check.
2. `GET /version` JSON includes `protocol_rev` and `min_client_version`.
3. Hello roundtrip includes the new fields; `old_ws_hello_json_still_deserializes` still passes.
4. Server rejects Hello with missing/wrong identity before spawn (unit-test the extracted gate, not a live WS).
5. Title Online Continue is blocked until `Compatible`; Offline is unchanged.
6. Welcome rev mismatch returns to Title.
7. `VERSION.toml` / crate constants / README badges / STATUS / ROADMAP / CHANGELOG say `1.4.0` / `client-compat`.
8. `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client` pass.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Packaged installers, code signing, notarization | No desktop distribution yet |
| In-app binary download / self-replace | Needs a feed host and Approach A |
| `electron-updater` / Steam / Epic tracks | Upstream product-shell; Electron is a rewrite non-goal |
| Opening a browser download URL | No official binary URL |
| Protocol rev 7 / snapshot schema change | Additive Hello only |
| i18n of `user_message()` | English-only invariant |
| Offline version checks | Offline embeds the same crate tree |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |
| Reintroducing a fat `Entity` | Unrelated; `AGENTS.md` |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Rolling deploy: new client vs old server `/version` | Missing `protocol_rev` → skip HTTP protocol check; Welcome still compares (old servers already send it) |
| Rolling deploy: old client vs new server | Missing Hello identity → reject; title of *old* clients still enters Login — they die at Hello with `version:` Error. Acceptable: old title has no preflight. New title blocks earlier |
| `WOC_MIN_CLIENT_VERSION` typo takes the realm down | `BadMinVersion` on Hello rejects everyone; ops unset the env. Document in README |
| Hello match sites miss a field | Compiler forces every `WsClientMsg::Hello { ... }` update; plan lists them |
| Client tests excluded from CI | Policy lives in `woc-version` / `woc-protocol` / `woc-server`; `cargo check -p woc-client` is the UI compile gate |

## 9. Success demo (human)

1. Same-tree: `cargo run -p woc-server` and `cargo run -p woc-client` → Online → status `compatible` → Login as today.
2. Stop the server → Online Continue stays on Title with `Unreachable`.
3. (Dev) temporarily send `protocol_rev: Some(5)` in Hello → server Error `version: update required (protocol 5 < 6)`; client returns to Title if already InWorld.
4. `WOC_MIN_CLIENT_VERSION=9.0.0 cargo run -p woc-server` → current client blocked at Title and at Hello.

When 1–4 are true, tag `1.4.0`.
