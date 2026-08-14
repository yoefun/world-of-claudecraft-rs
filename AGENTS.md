# Agent instructions

These rules override convenience. They apply to every change in this repository.

## Sim storage is ECS columns

`woc-sim` gameplay actors live in a typed sparse-column `World` (`crates/woc-sim/src/ecs/`). **World is the source of truth** for per-actor gameplay state.

**Do not:**

- Reintroduce a fat `Entity` struct or a homogeneous `Vec` of blob actors.
- Introduce a new catch-all actor struct (`struct Actor`, `struct Unit`, another `Vec<Entity>`).
- Depend on Bevy / `bevy_ecs` / wgpu / axum / tokio from `woc-sim` or `woc-content`.
- Put HP, inventory, quests, or combat into Bevy components on the client. Those stay in the sim (or on `TickSnapshot` for display).

**Do:**

- New *per-actor* state → new component in `ecs/components.rs` + `SparseSet` field on `World` + `Component` impl + `insert` only on the actor kinds that need it.
- New *per-realm* state → field on `Sim` (like `Mailbox`, `AuctionHouse`, `PartyRoster`).
- New *visual-only* state → Bevy component in `woc-client`.
- Query the columns a system needs. If you branch on `EntityKind` / `Identity.kind` to skip missing data, you wanted a component query instead.
- Keep tick phase order, mulberry32 RNG, and “client never decides combat/loot/quests”.

Design reference: `docs/superpowers/specs/2026-08-13-sim-ecs-design.md`. Human-facing summary: `docs/architecture/ecs.md`.

## Cursor Cloud specific instructions

Rust nightly (pinned by `rust-toolchain.toml`) with `rustfmt` + `clippy` is preinstalled. Standard lint/test/build commands live in `README.md` and `.github/workflows/ci.yml` — use those (`cargo fmt --all -- --check`, `cargo clippy --workspace --exclude woc-client -- -D warnings`, `cargo test --workspace --exclude woc-client`, `cargo build -p woc-server`, `cargo check -p woc-client`).

Services:

- `woc-server` (online backend): `cargo run -p woc-server` → listens on `0.0.0.0:8787`. Defaults to an in-memory store; set `DATABASE_URL=postgres://woc:woc@127.0.0.1:5432/woc` for durable Postgres. Verify with `curl http://127.0.0.1:8787/version`. REST auth/character flow: `POST /api/register`, `POST /api/login` (returns bearer `token`), `POST /api/characters` (`{"name","class_id"}`, e.g. `class_id:"warrior"`), `POST /api/characters/{id}/enter`.
- `woc-client` (Bevy GPU client): `cargo run -p woc-client`.

Non-obvious caveats:

- System libraries required beyond the Rust toolchain (baked into the environment image): `libasound2-dev`, `libudev-dev`, `pkg-config` (needed to compile/`cargo check` `woc-client`), plus `libxkbcommon-x11-0`, `libvulkan1`, `mesa-vulkan-drivers`, `libgl1-mesa-dri` (needed only to actually run the client).
- The cloud VM has no hardware GPU (`/dev/dri` absent). To run `woc-client` you must force software Vulkan (Mesa lavapipe) and target the virtual X display: `DISPLAY=:1 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json cargo run -p woc-client`. Rendering is CPU-only (llvmpipe) and very slow.
- Building the `woc-client` binary is heavy (several minutes cold; the debug binary is ~1.4 GB), so prefer `cargo check -p woc-client` unless you truly need to run it.
- Known blocker for running the client interactively: `woc-client` panics at startup with Bevy `error[B0001]` in `crates/woc-client/src/login.rs` `refresh_login_chrome` (an unfiltered `Query<&mut Text>` conflicts with the filtered `&mut Text` queries in the same system). This crashes the client before the title screen is usable, so the server-side online flow above is the runnable end-to-end path until that system is fixed.
