# Client update packages design — `1.5.0` / `client-update`

**Status:** Proposed (planning deliverable 2026-08-13).  
**Baseline:** rewrite `1.3.0` / `online-hard` on `develop`; depends on **1.4.0** version gate.  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `client-update`.

Version gate (1.4.0): [`2026-08-13-client-version-update-design.md`](2026-08-13-client-version-update-design.md).

## 1. Goal

1.4.0 **stops** a stale Bevy client. It does not **replace** it. cargo-run developers still rebuild; packaged players need a small launcher that downloads **only what changed**.

This program adds:

1. A **layout** (`woc-updater` + `woc-client` + `install.json`) produced by `woc-pack`.
2. A **full** `.tar.zst` for first install / skipped versions.
3. A **per-file bsdiff delta** from the previous tagged release (incremental).
4. An **updater** that verifies an ed25519-signed manifest, applies full or delta into a staging directory, then execs `woc-client`.
5. A **tag workflow** that builds Linux x86_64 and uploads GitHub Release assets.

> Players start the updater, not the game. The updater is the only process allowed to replace `woc-client` (Windows cannot patch a running Bevy exe).

## 2. Baseline

| Piece | State |
| --- | --- |
| Client | `cargo run -p woc-client` — no install prefix, no sibling updater |
| CI | Ubuntu test + `cargo check -p woc-client`; no release artifacts |
| Host | None. `/version` has no package URL |
| Upstream | Electron + electron-updater + R2 feeds — **do not copy** |

Bevy payload is mostly **one native executable** (world art is procedural). File-level “download only changed files” barely helps. Chunk hashes (casync/zsync) also fare poorly: a small Rust change moves many ELF bytes. **Binary delta (bsdiff)** is the increment that actually shrinks a native client.

## 3. Approaches considered

| Approach | Increment | Skip-version | Ops | Verdict |
| --- | --- | --- | --- | --- |
| **A. Velopack** | File bsdiff/zstd deltas, mature | Full fallback | `vpk` CLI, their install layout, Windows-weighted | Reject — extra product; fights our title gate and Linux-first CI |
| **B. Content-addressed chunks** | Unchanged 64KiB blocks | Excellent | Simple object store | Reject for this payload — ELF edits scramble chunks |
| **C. Full archive only** | None | Full every time | Tiny | Reject as the ship goal (ok as fallback) |
| **D. Own packer: zstd full + per-file qbsdiff from previous tag (recommended)** | Typical code tweak is a small patch | No delta → download full | GitHub Releases; one signing key | **Adopt** |

Keep last **one** predecessor delta (`N-1 → N`). Skipping `1.3.0 → 1.5.0` when only `1.4.0→1.5.0` exists uses the full archive. Generating a mesh of every historical pair is YAGNI.

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.4.0** | `client-compat` | Gate only (no download) |
| **1.5.0** | `client-update` | Pack, delta, updater, Linux tag publish |

`PROTOCOL_REV` stays **6** unless a gameplay wire break lands elsewhere. Upstream pin stays **0.31.0**.

## 5. Architecture

New leaf crate **`woc-update`** (no Bevy, no `woc-sim`, no axum). Two binaries in that crate:

| Binary | Role |
| --- | --- |
| `woc-pack` | CI: stage dir → full tar.zst + optional delta + signed manifest |
| `woc-updater` | Player: fetch plan → download → verify → atomic replace → `exec woc-client` |

```text
GitHub Release v1.5.0
  woc-rs-1.5.0-x86_64-unknown-linux-gnu.tar.zst
  woc-rs-1.4.0-to-1.5.0-x86_64-unknown-linux-gnu.wocdelta
  woc-rs-1.5.0-x86_64-unknown-linux-gnu.manifest.json

Title (packaged, 1.4.0 gate says ClientTooOld)
    │  GET /version  →  update_manifest_url
    │  Update button →  exec woc-updater --once && exit
    ▼
woc-updater
    │  read install.json (local version)
    │  GET manifest (verify ed25519)
    │  plan_fetch → Delta if delta_from[local] else Full
    │  download artifact, sha256, apply in staging/
    │  swap staging ↔ current (keep backup/)
    ▼
exec ./woc-client
```

`cargo run -p woc-client` has no sibling updater: 1.4.0 copy only (“update required”), no button.

### 5.1 Install layout (Linux)

Directory the player unpacks (or the updater maintains):

```text
<prefix>/
  woc-updater          # launcher; argv0 the user runs
  woc-client           # Bevy game
  install.json         # { rewrite_version, target }
  backup/              # previous prefix after a successful apply (optional)
```

`install.json`:

```json
{
  "rewrite_version": "1.5.0",
  "target": "x86_64-unknown-linux-gnu"
}
```

No other files are required. Do not ship `target/release` extras.

### 5.2 Manifest (signed)

`woc-rs-{ver}-{target}.manifest.json`:

```json
{
  "rewrite_version": "1.5.0",
  "protocol_rev": 6,
  "target": "x86_64-unknown-linux-gnu",
  "files": [
    {"path": "woc-client", "sha256": "<hex>", "size": 1},
    {"path": "woc-updater", "sha256": "<hex>", "size": 1},
    {"path": "install.json", "sha256": "<hex>", "size": 1}
  ],
  "full": {"name": "woc-rs-1.5.0-x86_64-unknown-linux-gnu.tar.zst", "sha256": "<hex>", "size": 1},
  "delta_from": {
    "1.4.0": {
      "name": "woc-rs-1.4.0-to-1.5.0-x86_64-unknown-linux-gnu.wocdelta",
      "sha256": "<hex>",
      "size": 1
    }
  },
  "sig": "<hex ed25519 signature of canonical body>"
}
```

**Canonical body** = JSON of the struct **without** `sig`, keys sorted, no insignificant whitespace (`serde_json::to_vec` of a `BTreeMap`-ordered type, or sign a separate `sig_payload` string). Tests lock: mutate `rewrite_version` → verify fails.

Public key is **compiled into `woc-updater`** (`WOC_UPDATE_PUBKEY` hex, 32-byte compressed ed25519). Private key is GitHub secret `WOC_UPDATE_SIGNING_KEY` (32-byte seed hex). Key rotation is out of scope (single key).

### 5.3 Full package

`tar` + `zstd` of the three layout files at archive root (no wrapping folder). SHA-256 of the compressed blob is `full.sha256`.

### 5.4 Delta package (incremental)

A `.wocdelta` is `tar.zst` containing:

```text
delta.json
woc-client.bsdiff
woc-updater.bsdiff
```

`delta.json`:

```json
{
  "from": "1.4.0",
  "to": "1.5.0",
  "patches": [
    {"path": "woc-client", "sha256": "<bsdiff hex>", "new_sha256": "<result file hex>"},
    {"path": "woc-updater", "sha256": "<bsdiff hex>", "new_sha256": "<result file hex>"}
  ]
}
```

`install.json` is rewritten locally from `to` (not patched). Unchanged files may be omitted from `patches` (copy through).

Algorithm: **`qbsdiff` 1.4** (`Bsdiff` / `Bspatch`), bsdiff 4.x compatible. Apply: `Bspatch::new(patch)?.apply(old_bytes, Cursor::new(&mut new))`. After each file, require `sha256(new) == new_sha256`. Any mismatch → abort, restore `backup/`, fall back to **full** once.

If local `install.json` version has no `delta_from` entry, skip delta and download `full`.

### 5.5 Fetch plan

```text
plan_fetch(local, remote) =
  if local.rewrite_version == remote.rewrite_version → Nothing
  else if remote.delta_from.get(local.rewrite_version) → Delta
  else → Full
```

Target mismatch (`local.target != remote.target`) → error, do not apply.

### 5.6 Apply (atomic)

1. Copy current prefix to `prefix.staging/` (or extract full archive there).
2. For delta: patch files in staging; write new `install.json`.
3. Verify every `remote.files` hash in staging.
4. `prefix → prefix.backup` (rm old backup first), `prefix.staging → prefix`.
5. On any failure before swap: delete staging, leave current untouched.
6. After swap failure mid-rename: restore from backup if current is missing.

**Self-update:** the running `woc-updater` cannot overwrite itself on all OSes. Required sequence:

1. `cp woc-updater /tmp/woc-updater.<pid>`
2. `exec /tmp/woc-updater.<pid> --apply-from <prefix>` (same argv, extra flag)
3. Temp copy patches `<prefix>/woc-updater` then execs `<prefix>/woc-client`

`--once` from the game: check+apply+exec client (no loop). Default (player double-click): same.

### 5.7 Artifact store

Library trait (tests use a temp directory; production uses HTTP):

```text
trait ArtifactStore {
  fn fetch(&self, name: &str) -> Result<Vec<u8>, UpdateError>;
}
```

HTTP base = directory that contains the manifest name. GitHub Releases: `WOC_UPDATE_BASE_URL=https://github.com/yoefun/world-of-claudecraft-rs/releases/download/v1.5.0` **or** a stable “latest” URL.

Prefer **one stable manifest URL** that always points at current:

- Env `WOC_UPDATE_MANIFEST_URL` (absolute JSON URL) on the **server**, echoed in `/version`
- Default empty → no packaged update (dev)

Updater also accepts `--manifest <url-or-path>`.

### 5.8 `/version` additive field (after 1.4.0)

```text
update_manifest_url: string   // serde default ""
```

`VersionInfo::current` reads `WOC_UPDATE_MANIFEST_URL`. Empty means cargo-run / gate-only.

### 5.9 Title (packaged)

Requires 1.4.0 `RealmCompat`. If `Incompatible` **and** `update_manifest_url` non-empty **and** `current_exe().parent()/woc-updater` exists:

- Primary button **Update** (Enter while incompatible): spawn `woc-updater --once --manifest <url>` with cwd = prefix, then `std::process::exit(0)`.
- Do not download inside Bevy (GPU process, file locks).

Offline mode still skips `/version`.

### 5.10 Build and publish (Linux x86_64)

New workflow `.github/workflows/client-release.yml`:

**Triggers:** `push` tags `v*.*.*` and `workflow_dispatch` (version input).

**Job `pack-linux`** (`ubuntu-latest`):

1. Install Bevy Linux deps (same as CI) + `zstd`.
2. `cargo build --release -p woc-client -p woc-update`
3. Stage `dist/layout/{woc-client,woc-updater,install.json}` (`install.json` rewrite_version from `VERSION.toml`).
4. `gh release download` previous tag’s layout or full archive if present (continue on missing).
5. `woc-pack --layout dist/layout --out dist/out --prev dist/prev --key $WOC_UPDATE_SIGNING_KEY --protocol-rev 6`
6. `gh release upload $TAG dist/out/*` (manifest, full, delta if produced).

Do **not** run this on every PR. Workspace tests of `woc-update` use tiny fake files (no GPU).

Windows / macOS: not in 1.5.0 DoD (need native runners; Bevy cross-compile is not this program).

## 6. Definition of done (`1.5.0` / `client-update`)

1. `woc-update` unit tests: pack/unpack roundtrip; bsdiff apply restores bytes; patch smaller than target on a constructed similar pair; `plan_fetch` nothing/delta/full; bad sig rejected; staging apply leaves original intact on hash fail.
2. `woc-pack` produces full + manifest; with `--prev` also a `.wocdelta`.
3. `woc-updater --once` against a `DirStore` (temp “remote”) upgrades a fake prefix.
4. `/version` includes `update_manifest_url` (empty default).
5. Packaged title shows Update and execs the sibling updater (compile-checked; GPU demo manual).
6. `client-release.yml` exists; documented secrets `WOC_UPDATE_SIGNING_KEY`; pubkey baked via `WOC_UPDATE_PUBKEY` at compile time (dev tests generate ephemeral keys, not the prod secret).
7. README runbook: first install (download full tar.zst), subsequent (run updater), `WOC_UPDATE_MANIFEST_URL`.
8. `VERSION.toml` / badges / STATUS / ROADMAP / CHANGELOG = `1.5.0` / `client-update`.
9. `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client` pass. Clippy includes `woc-update`.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Velopack / Squirrel / electron-updater | Wrong client stack |
| casync / zsync chunk stores | Weak on a single ELF |
| Deltas from every historical version | One predecessor is enough |
| Windows / macOS release jobs | Follow-up when runners exist |
| Apple notarization, Authenticode, SteamPipe | Product shell |
| In-Bevy download bar as the apply engine | File locks; launcher owns apply |
| Auto-update while InWorld | Must return to Title / quit |
| Key rotation / multiple channels (`dev` vs `prod`) | Single key, single channel |
| Browser / Electron / Capacitor | Rewrite non-goals |
| Bumping upstream past 0.31.0 | Dedicated pin PR |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Delta apply corrupts binary | Per-file `new_sha256`; fail → backup + one full retry |
| Missing previous release on first 1.5.0 tag | Packer emits full only; delta optional |
| Unsigned / swapped GitHub asset | Manifest `sig`; updater refuses unsigned/wrong key |
| Bevy release build time | Isolated workflow, not PR CI |
| Updater cannot overwrite itself | Exec temp copy first (§5.6) |
| 1.4.0 not merged yet | 1.5.0 client UX task waits on gate; packer/updater lib does not |

## 9. Success demo (human)

1. `woc-pack` two tiny prefixes 1.0.0 / 1.0.1 → delta smaller than full; `woc-updater --once` on 1.0.0 dir becomes 1.0.1 hashes.
2. Remove `delta_from` → updater still succeeds via full.
3. Flip one byte of the delta → updater aborts, prefix unchanged.
4. After 1.4.0 UI: packaged tree with a too-old `install.json` → Title Update → process replaced by new `woc-client`.

When 1–3 are green in CI and 4 is a manual packaged run, tag `1.5.0`.
