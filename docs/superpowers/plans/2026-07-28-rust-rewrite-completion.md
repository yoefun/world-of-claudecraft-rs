# Rust Rewrite Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Orchestration:** Main agent freezes contracts → dispatches parallel subagents on isolated branches → merges by dependency → runs full tests → updates STATUS.

**Goal:** Complete remaining gameplay-core Rust rewrite (online, persist, class depth, zones, group PvE, economy, professions, light PvP) to rewrite `1.0.0-pre` / parity `completion`.

**Architecture:** One deterministic `woc-sim` + data `woc-content` + wire `woc-protocol` + hosts (`woc-client` Bevy, `woc-server` axum) + new `woc-persist` (Postgres). Parallel workstreams own disjoint paths; protocol/sim-core stay serialized choke points.

**Tech Stack:** Rust edition 2021, Bevy 0.16, axum 0.8 (+ws), tokio, serde, sqlx/Postgres (from 0.4), nightly toolchain as in `rust-toolchain.toml`, upstream pin 0.31.0.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9` unless a dedicated bump PR says otherwise.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- All sim RNG via mulberry32 on `Sim` only; no `thread_rng`, no wall clock in sim.
- Tick rate `TICK_RATE = 20`; do not reorder locked tick phases once hash tests exist.
- Client never decides combat/loot/quest/vendor/talent outcomes — intents/actions only.
- Prefer additive `#[serde(default)]` protocol fields; bump `PROTOCOL_REV` on breaking wire changes.
- English-only strings; skip Web3, Electron, Three.js, RL, admin SPA, Discord OAuth polish (see design non-goals).
- Branch naming for subagents: `cursor/<workstream-id>-8e8e` (or unique suffix). One workstream per branch.
- Before claiming done: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client` (+ clippy as CI).

**Design:** [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](../specs/2026-07-28-rust-rewrite-completion-design.md)

---

## File ownership map (avoid parallel conflicts)

| Owner | Exclusive paths |
| --- | --- |
| **CORE** (serialize) | `crates/woc-sim/src/sim.rs`, `context.rs`, `host.rs`, `entity.rs` (player component migration) |
| **PROTO** (serialize short PRs) | `crates/woc-protocol/src/lib.rs` |
| **CONTENT** | `crates/woc-content/src/**` |
| **COMBAT** | `crates/woc-sim/src/combat/**` (or `combat.rs` → split) |
| **MOB** | `crates/woc-sim/src/mob/**` |
| **MOTION** | `crates/woc-sim/src/player_motion.rs`, `physics/**` |
| **SOCIAL** | `crates/woc-sim/src/social/**`, `quests.rs`, `interaction.rs` |
| **INV** | `crates/woc-sim/src/inventory.rs`, `stats.rs`, `bags` modules |
| **INSTANCE** | `crates/woc-sim/src/instances/**`, `delves/**` |
| **ECON** | `crates/woc-sim/src/{bank,mail,market}.rs` |
| **PROF** | `crates/woc-sim/src/professions/**` |
| **PVP** | `crates/woc-sim/src/pvp/**` |
| **PERSIST** | `crates/woc-persist/**`, server auth/character routes |
| **SERVER** | `crates/woc-server/src/**` (except when blocked on CORE API) |
| **CLIENT** | `crates/woc-client/src/**` |
| **DOCS** | `docs/**`, `CHANGELOG.md`, `VERSION.toml`, `UPSTREAM.md`, `README.md` |

---

## Wave 0 — Foundation (must land first; limited parallelism)

### Batch 0A — CORE only (no parallel siblings touching sim)

#### Task 0A.1: Adopt `SimContext` + document tick phases

**Files:** `crates/woc-sim/src/context.rs`, `sim.rs`, leaf call sites gradually

- [ ] Expand `SimContext` so emit / lookup player / mutate entity work for current combat+quest+interact paths
- [ ] Document actual tick phase order in `sim.rs` module docs (match code)
- [ ] Add determinism hash test that fails if phase call order changes
- [ ] Commit: `refactor(sim): adopt SimContext seam and lock tick phase order`

#### Task 0A.2: Multi-player Entity migration

**Files:** `entity.rs`, `sim.rs`, `inventory.rs`, `quests.rs`, `interaction.rs`, `combat.rs`, `host.rs`, `stats.rs`

- [ ] Move `player_xp`, `copper`, class, inventory, equipment, quest_log onto per-player state keyed by `EntityId`
- [ ] `push_intent` / `interact` / `snapshot_for` honor `player_id`
- [ ] Keep single-player Eastbrook tests green (spawn one player)
- [ ] Commit: `refactor(sim): per-player economy and intent maps`

#### Task 0A.3: Sticky realm Hello (server)

**Files:** `crates/woc-server/src/game_ws.rs`

- [ ] Hello spawns/attaches a player entity; **does not** `Sim::new_eastbrook` reset
- [ ] Disconnect removes or parks that player only
- [ ] Integration test: two fake clients Hello without wiping NPC roster
- [ ] Commit: `fix(server): sticky realm on Hello; no full sim reset`

**Merge gate 0A:** CORE+SERVER green → tag mentally `framework-polish` start.

---

### Batch 0B — parallel after 0A merges (disjoint paths)

Dispatch **up to 4 subagents** simultaneously:

| Subagent | Workstream | Branch hint | Owns |
| --- | --- | --- | --- |
| S1 | `ws-client-split` | `cursor/ws-client-split-8e8e` | `woc-client` module split only |
| S2 | `ws-ui-chrome` | `cursor/ws-ui-chrome-8e8e` | UI panels (after split preferred; or wait for S1) |
| S3 | `ws-content-integrity` | `cursor/ws-content-expand-prep-8e8e` | content tests / stub modules for talents/zones |
| S4 | `ws-proto-rev3-scaffold` | `cursor/ws-proto-rev3-8e8e` | additive protocol fields for death/auras/party (defaults only) |

If S1 and S2 would conflict on `main.rs`, **run S1 first**, then S2. Prefer: S1 ∥ S3 ∥ S4, then S2.

#### Task 0B.1: Split `woc-client` god-file

- [ ] Extract `title.rs`, `char_create.rs`, `world_setup.rs`, `input.rs`, `hud.rs` from `main.rs`
- [ ] Leave behavior identical offline
- [ ] `cargo check -p woc-client`
- [ ] Commit: `refactor(client): split main.rs into host modules`

#### Task 0B.2: Functional UI chrome

- [ ] Character sheet (`C`), vendor panel on VendorOpen, cast bar stub, action bar Primary
- [ ] Data-driven interact prompts (stop hardcoding quest IDs where possible)
- [ ] Commit: `feat(client): character/vendor UI chrome`

#### Task 0B.3: Content stub modules + integrity tests

- [ ] Add empty/stub `talents.rs`, `zone2.rs`, `graveyards.rs`, `dungeons.rs` exports
- [ ] Integrity tests remain green
- [ ] Commit: `feat(content): stub modules for completion waves`

#### Task 0B.4: Protocol rev 3 scaffold (additive)

- [ ] Fields: `auras`, `cast`, `is_dead`, `party_id` with defaults; new `SimEvent` stubs as needed
- [ ] Bump `PROTOCOL_REV` to 3 only if breaking; else keep 2 with defaults
- [ ] Roundtrip tests
- [ ] Commit: `feat(protocol): additive fields for death/auras/party`

**Merge order 0B:** PROTO → CONTENT → CLIENT-SPLIT → UI.

---

## Wave 1 — `0.3.0` online-alive

### Batch 1A — parallel (4 agents)

| Subagent | id | Depends | Owns |
| --- | --- | --- | --- |
| A | `ws-online-client` | 0A, 0B.1 | `woc-client/src/online.rs`, net host toggle |
| B | `ws-death` | 0A, 0B.4 | `woc-sim` spirit/corpse + content graveyards |
| C | `ws-combat-core` | 0A, 0B.4 | `woc-sim/combat/` GCD/cast/aura/threat stub |
| D | `ws-motion` | 0A | `player_motion` + building AABB collide |
| E | `ws-bags` | 0A | extra equip slots, UseItem, bag expand |
| F | `ws-targeting` | 0A | tab target + clear |

**Recommended first dispatch (max parallelism without `sim.rs` fights):**  
B ∥ D ∥ E ∥ F with CORE agent integrating tick hooks; **or** assign one INTEGRATOR for `sim.rs` call sites only.

**Rule:** Leaf agents implement functions; **CORE integrator** (main agent) wires `Sim::tick` once per batch.

#### Task 1A.1: Online Bevy client

- [ ] Mode picker Offline | Online (`ws://127.0.0.1:8787/ws/game`)
- [ ] Send Hello/Intent/Interact; apply Snapshot/Events
- [ ] Two-client manual script documented in README
- [ ] Commit: `feat(client): online WebSocket host mode`

#### Task 1A.2: Death / graveyard

- [ ] On HP 0: dead state, release/respawn at Eastbrook graveyard
- [ ] Cannot attack while dead; corpse loot still works
- [ ] Determinism test kill→respawn
- [ ] Commit: `feat(sim): death spirit and graveyard respawn`

#### Task 1A.3: Combat core stub

- [ ] GCD; ≥1 timed cast; ≥1 DoT aura ticking; swing timer; threat list
- [ ] Unit tests for aura expiry + event order hash
- [ ] Commit: `feat(sim): GCD cast auras threat combat core`

#### Task 1A.4: Motion / collider

- [ ] ≥1 Eastbrook building AABB; no tunneling on slopes
- [ ] Determinism position test
- [ ] Commit: `feat(sim): building colliders and motion polish`

#### Task 1A.5: Bags depth

- [ ] OffHand/Head/Legs/Feet; UseItem consumable; bag item expands slots
- [ ] Level req blocks equip
- [ ] Commit: `feat(sim): deeper inventory equipment and consumables`

#### Task 1A.6: Targeting

- [ ] Tab cycle hostiles; Esc clear; snapshot matches
- [ ] Commit: `feat(sim): tab targeting`

### Batch 1B — after combat+death

#### Task 1B.1: Mob AI respawn + social aggro

- [ ] Respawn timer; leash; linked pack aggro
- [ ] Commit: `feat(sim): mob respawn and social aggro`

**Wave 1 merge gate:** bump to rewrite `0.3.0`, parity `online-alive`, CHANGELOG + STATUS + README demo steps 1–2 of design §8 (online + death).

---

## Wave 2 — `0.4.0` online-persist

### Batch 2A — mostly serial on persist, parallel docs/client login UI

| Subagent | id | Owns |
| --- | --- | --- |
| P | `ws-persist` | new `woc-persist` crate + migrations |
| S | `ws-server-auth` | REST auth/characters routes (after P types) |
| C | `ws-client-login` | login/char select UI (mockable until S ready) |

#### Task 2A.1: `woc-persist` crate

- [ ] Workspace member; sqlx + Postgres; migrations: accounts, characters, blob or columnar save
- [ ] Save/load: position, xp, copper, inventory, equipment, quests
- [ ] CI Postgres service
- [ ] Commit: `feat(persist): Postgres character save/load`

#### Task 2A.2: Auth + character CRUD API

- [ ] Register/login (argon2/scrypt); create/list/delete chars; enter-world token/session
- [ ] Wire WS Hello to authenticated character id
- [ ] Commit: `feat(server): auth and character CRUD`

#### Task 2A.3: Client login flow

- [ ] Login → char select → enter world (online)
- [ ] Commit: `feat(client): login and character select`

**Wave 2 merge gate:** rewrite `0.4.0`, parity `online-persist`. Quit/re-enter restores state.

---

## Wave 3 — `0.5.0` class-depth

### Batch 3A — parallel after combat core

| Subagent | id | Notes |
| --- | --- | --- |
| K | `ws-ability-kits` | content + sim multi-ability; keys 1–5 |
| T | `ws-talents` | after kits land (or content tables parallel first) |
| P | `ws-pets` | hunter/warlock summon |

#### Task 3A.1: Multi-ability kits

- [ ] ≥3 abilities per class; level gates; distinct effects
- [ ] Per-class smoke determinism scripts
- [ ] Commit: `feat(content/sim): multi-ability class kits`

#### Task 3A.2: Talents

- [ ] Spend points; one talent changes damage/behavior; respec; persist loadout
- [ ] Commit: `feat(sim): talent allocation and loadouts`

#### Task 3A.3: Pets

- [ ] Summon/dismiss; pet attacks target; snapshot includes pet
- [ ] Commit: `feat(sim): hunter and warlock pets`

**Wave 3 merge gate:** rewrite `0.5.0`, parity `class-depth`.

---

## Wave 4 — `0.6.0` open-world

### Batch 4A — parallel content + shared transition API (CORE wires once)

| Subagent | id | Owns |
| --- | --- | --- |
| Z2 | `ws-zone2` | `zone2` tables, quests, mobs, NPCs |
| Z3 | `ws-zone3` | `zone3` or portal stub + 1 camp |
| G | `ws-graveyards-multi` | per-zone graveyards |

#### Task 4A.1: Zone transition API (CORE)

- [ ] Portal/zone change without destroying player component state
- [ ] Commit: `feat(sim): zone transition`

#### Task 4A.2 / 4A.3: Zone content

- [ ] ≥5 new quests, ≥3 new mobs across zone2; zone3 minimum or stub
- [ ] Integrity tests
- [ ] Commit: `feat(content): zone2/zone3 open-world tables`

**Wave 4 merge gate:** rewrite `0.6.0`, parity `open-world`.

---

## Wave 5 — `0.7.0` group-pve

### Batch 5A — sequential-ish social → loot → instances

| Order | id | Parallel notes |
| --- | --- | --- |
| 1 | `ws-party-chat` | chat UI can parallel party sim |
| 2 | `ws-loot-rules` | after party |
| 3 | `ws-dungeons` | instance shell then content ∥ encounter |
| 4 | `ws-delves` | after shell (optional same wave) |

#### Tasks

- [ ] Party invite 2–5; party chat; quest credit/XP share
- [ ] FFA vs Need/Greed via sim RNG
- [ ] One dungeon instance + boss + separate instances per party
- [ ] One delve run (rooms → reward) — may slip to 0.7.x
- [ ] Commits per workstream

**Wave 5 merge gate:** rewrite `0.7.0`, parity `group-pve`.

---

## Wave 6 — `0.8.0` economy

### Batch 6A — parallel bank ∥ mail after persist; then market

- [ ] Bank deposit/withdraw durable
- [ ] Mail send/collect (+ item)
- [ ] Auction list/buy/expire/fee durable
- [ ] Commits: `feat(sim/persist): bank`, `mail`, `market`

**Wave 6 merge gate:** rewrite `0.8.0`, parity `economy`.

---

## Wave 7 — `0.9.0` professions + PvP

### Batch 7A — parallel

| Subagent | id |
| --- | --- |
| R | `ws-professions` |
| V | `ws-pvp` |
| W | `ws-worldboss-deeds` (may finish in 1.0-pre) |

- [ ] Gather → craft one full loop
- [ ] Duel + PvP flag + honor
- [ ] One world boss + one deed (optional here)
- [ ] Commits per stream

**Wave 7 merge gate:** rewrite `0.9.0`, parity `professions-pvp`.

---

## Wave 8 — `1.0.0-pre` completion

### Batch 8A — main agent integration

- [ ] Finish world boss/deeds if deferred
- [ ] STATUS: all gameplay-core rows `done` or accepted `partial`
- [ ] README success demo matches design §8
- [ ] CHANGELOG 1.0.0-pre; parity_target `completion`
- [ ] Full CI green
- [ ] Commit: `release: 1.0.0-pre completion parity`

---

## Main-agent merge playbook (every batch)

1. **Freeze contract:** if PROTO/CORE APIs change, merge those PRs first on integration branch.
2. **Dispatch:** create worktrees/branches; give each subagent this plan section + Global Constraints + exclusive paths + DoD.
3. **Integrate:** merge smallest risk first (content → leaf sim → client → server).
4. **Resolve:** only main agent edits `sim.rs` tick wiring and protocol conflicts.
5. **Verify:**
   ```bash
   cargo test --workspace --exclude woc-client
   cargo check -p woc-client
   ```
6. **Document:** update `docs/parity/STATUS.md` + `CHANGELOG.md` for the wave.
7. **Push/PR:** one PR per wave preferred; workstream PRs OK if stacked cleanly.

---

## Subagent prompt template

```text
You are implementing workstream <ID> from
docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md
on branch cursor/<id>-8e8e off integration branch <BASE>.

Exclusive paths: <LIST>
Do NOT modify: sim.rs tick order (leave hooks to main), protocol unless listed,
or other workstreams' paths.

Constraints: Global Constraints section of the plan. Upstream pin 0.31.0.
TDD: write failing tests first where practical.
DoD: <paste task DoD>
When done: commit, push, summarize files changed + how to verify.
```

---

## First executable dispatch (next session)

After this plan is approved, main agent should:

1. Ensure `develop` is merge base (PR #3 / framework already on develop).
2. Land **Batch 0A** (CORE) as a single agent or main agent — **no parallel sim leaves**.
3. Then dispatch Batch 0B: `ws-client-split` ∥ `ws-content-expand-prep` ∥ `ws-proto-rev3-scaffold`.
4. Then Batch 1A parallel leaves + CORE integrator.

Do **not** start Wave 2 until sticky realm + online client exist.
