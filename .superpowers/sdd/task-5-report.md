### Task 5 Report: Title Online `/version` preflight

**Status:** Implemented and verified on branch `cursor/client-version-update-680e`.

**TDD evidence:**

1. Added `version_info_from_server_json` to `crates/woc-client/src/api.rs` before production Task 5 code.
2. First focused run:
   - Command: `cargo test -p woc-client version_info_from_server_json -q`
   - Result: failed before test execution because `alsa-sys` could not find `alsa.pc`.
   - Remediation: installed `libasound2-dev`.
3. Second focused run:
   - Command: `cargo test -p woc-client version_info_from_server_json -q`
   - Result: failed before test execution because `libudev-sys` could not find `libudev.pc`.
   - Remediation: installed `libudev-dev`.
4. Third focused run:
   - Command: `cargo test -p woc-client version_info_from_server_json -q`
   - Result: failed on the known compile gap in `online.rs`: missing `protocol_rev` and `rewrite_version` in `WsClientMsg::Hello`.
   - Remediation: added only the permitted Hello fields from the task instructions.
5. Post-implementation focused run:
   - Command: `cargo test -p woc-client version_info_from_server_json -q`
   - Result: failed because `RealmCompat` stored `std::sync::mpsc::Receiver` directly, which is not `Sync` for a Bevy `Resource`.
   - Remediation: wrapped the pending receiver in `Mutex`, matching the existing `GameHost` receiver pattern.
6. Final focused run:
   - Command: `cargo test -p woc-client version_info_from_server_json -q`
   - Result: passed, 1 test passed, 0 failed.

**Verification:**

- `cargo test -p woc-client version_info_from_server_json -q`
  - Passed: 1 passed, 0 failed.
- `cargo check -p woc-client`
  - Passed: finished `dev` profile.
- `cargo test --workspace --exclude woc-client -q`
  - Passed: all reported test groups passed with 0 failures.

**Commits:**

- `a7c0284 feat(client): fail-closed online version preflight on title`
- `7444904 fix(client): make realm compatibility receiver resource safe`

**Concerns / notes:**

- Task 6 behavior was intentionally not implemented: no Welcome kick handling and no Title return.
- No protocol or rewrite version bumps were made.
- No Update button was added.
