# Post-completion program implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Orchestration:** Main agent freezes protocol/sim contracts per wave, dispatches parallel subagents on isolated branches with exclusive path ownership, then merges by dependency and runs workspace tests.

**Goal:** Close rewrite `1.0.0` (`stable`), then land `1.1.0` combat-depth, `1.2.0` content-depth, and `1.3.0` online-hard without breaking the completion demo.

**Architecture:** Keep one deterministic `woc-sim` on typed sparse-column `World`. 1.0.0 tells the truth about tick order. 1.1.0 replaces ability-id match arms with `AbilityEffect` systems that query columns. 1.2.0 adds tables on that seam. 1.3.0 parks players and shrinks snapshots. Do not reintroduce a fat `Entity`.

**Tech Stack:** Rust edition 2021, Bevy 0.16, axum 0.8 (+ws), tokio, serde, sqlx/Postgres optional, nightly as in `rust-toolchain.toml`, upstream pin 0.31.0, protocol rev 6 as the 1.0.0 floor.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9` unless a dedicated bump PR says otherwise.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- All sim RNG via mulberry32 on `Sim` only; no `thread_rng`, no wall clock in sim.
- Do not reorder locked tick phases once the 1.0.0 fingerprint lands.
- Client never decides combat/loot/quest/vendor/talent outcomes — intents/actions only.
- Prefer additive `#[serde(default)]` protocol fields; bump `PROTOCOL_REV` on breaking wire changes.
- English-only strings; skip Web3, Electron, Three.js, RL, admin SPA, Discord OAuth, map editor, minigames.
- New per-actor state is a `World` component column (`AGENTS.md` / `docs/architecture/ecs.md`). Do **not** reintroduce a fat `Entity` or `Vec` of blob actors. Combat functions take `&mut World` + `EntityId`.
- Branch naming: `cursor/<workstream-id>-9630` (or unique suffix). One workstream per branch.
- Before claiming done: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client` (+ clippy as CI).

**Design:** [`docs/superpowers/specs/2026-08-13-post-completion-program-design.md`](../specs/2026-08-13-post-completion-program-design.md)

**Dispatch schedule:** [`2026-08-13-parallel-post-completion.md`](2026-08-13-parallel-post-completion.md)

---

## File ownership map (avoid parallel conflicts)

| Owner | Exclusive paths |
| --- | --- |
| **CORE** (serialize) | `crates/woc-sim/src/sim.rs`, `context.rs` |
| **PROTO** (serialize short PRs) | `crates/woc-protocol/src/lib.rs` |
| **CONTENT** | `crates/woc-content/src/**` |
| **COMBAT** | `crates/woc-sim/src/combat.rs` (split to `combat/` only if the file is already being edited in that workstream) |
| **MOB** | `crates/woc-sim/src/mob.rs` |
| **SOCIAL** | `crates/woc-sim/src/social/**` |
| **INSTANCE** | `crates/woc-sim/src/instances/**`, `delves/**` |
| **PROF** | `crates/woc-sim/src/professions/**` |
| **PVP** | `crates/woc-sim/src/pvp/**` |
| **TALENT** | `crates/woc-sim/src/talents.rs` |
| **PERSIST** | `crates/woc-persist/**`, server auth/character routes |
| **SERVER** | `crates/woc-server/src/**` (except when blocked on CORE API) |
| **CLIENT** | `crates/woc-client/src/**` |
| **DOCS** | `docs/**`, `CHANGELOG.md`, `VERSION.toml`, `UPSTREAM.md`, `README.md`, `Cargo.toml` description, `.github/workflows/ci.yml` |

`entity.rs` is **deleted**. New actor fields go in `crates/woc-sim/src/ecs/components.rs` + `World` columns. CORE approves new columns in the same batch; never two agents adding columns in one batch.

---

## Wave 0 — `1.0.0` / `stable` (contract-close; limited parallelism)

No gameplay behavior change. Name the tick that already runs. Fix docs and CI.

### Task 0.1: Lock real tick phases

**Files:**
- Modify: `crates/woc-sim/src/context.rs`
- Modify: `crates/woc-sim/src/sim.rs` (module docs + `tick_phase_order_fingerprint_locked`)
- Test: `crates/woc-sim/src/sim.rs` (`tick_phase_order_fingerprint_locked`)

**Interfaces:**
- Consumes: current `Sim::tick_all` call order (motion → player combat → pets → mobs → auras/respawn → kill rewards/death → pvp/market → loot → snapshot)
- Produces: `TICK_PHASES` length 9; new fingerprint constant

- [ ] **Step 1: Write the failing fingerprint expectation**

In `tick_phase_order_fingerprint_locked`, change the locked hash so the current six-phase list fails once `TICK_PHASES` is updated, then update `TICK_PHASES` first:

```rust
pub const TICK_PHASES: &[&str] = &[
    "apply_intents_motion",
    "player_combat",
    "pet_ai",
    "mob_ai_combat",
    "aura_decay",
    "kill_rewards",
    "pvp_and_market",
    "loot_pickup",
    "build_snapshot",
];
```

- [ ] **Step 2: Run the fingerprint test and record the new hash**

Run: `cargo test -p woc-sim tick_phase_order_fingerprint_locked -- --nocapture`

Expected: FAIL with `assert_eq!(tick_phase_fingerprint(), 1724209595281213949u64)` left/right mismatch. Copy the left-side actual `u64`.

- [ ] **Step 3: Lock the new hash and docs**

Replace the test:

```rust
fn tick_phase_order_fingerprint_locked() {
    assert_eq!(TICK_PHASES.len(), 9);
    assert_eq!(TICK_PHASES[0], "apply_intents_motion");
    assert_eq!(TICK_PHASES[2], "pet_ai");
    assert_eq!(TICK_PHASES[6], "pvp_and_market");
    assert_eq!(TICK_PHASES[8], "build_snapshot");
    assert_eq!(tick_phase_fingerprint(), /* value from step 2 */);
}
```

Update `sim.rs` module docs to the nine-phase list. In `tick_all`, keep call order identical; only rewrite comments so “Phase N” matches `TICK_PHASES` (pets/auras/pvp are named phases, not “keeps fingerprint stable” asides).

- [ ] **Step 4: Re-run**

Run: `cargo test -p woc-sim tick_phase_order_fingerprint_locked -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/context.rs crates/woc-sim/src/sim.rs
git commit -m "fix(sim): lock real tick phases including pets, auras, and PvP"
```

### Task 0.2: CI includes `develop`

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add develop to push and pull_request**

```yaml
on:
  push:
    branches: [main, develop, "cursor/**"]
  pull_request:
    branches: [main, develop]
```

Do not change job steps.

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run workspace checks on develop"
```

### Task 0.3: Protocol and crate copy hygiene

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs` (comments on `ReleaseSpirit`, `TrainProfession`, `Gather`)
- Modify: `Cargo.toml` (`workspace.package.description`)
- Modify: `crates/woc-client/src/main.rs` crate doc
- Modify: `crates/woc-sim/src/lib.rs` crate doc

- [ ] **Step 1: Replace stub language**

Protocol:

```rust
    /// Release spirit while dead and begin the corpse run.
    ReleaseSpirit,
    /// Train a profession by content id.
    TrainProfession {
        id: String,
    },
    /// Gather from a world node.
    Gather {
        node_id: EntityId,
    },
```

`Cargo.toml`:

```toml
description = "Rust rewrite of World of ClaudeCraft (1.0.0-pre completion; targeting 1.0.0 stable)"
```

After the version bump in Task 0.5 this becomes `"Rust rewrite of World of ClaudeCraft"`.

Crate docs: drop “framework slice”; say “deterministic sim / Bevy host”.

- [ ] **Step 2: Commit**

```bash
git add crates/woc-protocol/src/lib.rs Cargo.toml crates/woc-client/src/main.rs crates/woc-sim/src/lib.rs
git commit -m "docs: drop stub/framework-slice wording for shipped systems"
```

### Task 0.4: Acceptance demo doc

**Files:**
- Create: `docs/parity/DEMO.md`
- Modify: `README.md` (Roadmap paragraph links the demo)

- [ ] **Step 1: Write the 1.0.0 script** (completion design §8, present tense)

```markdown
# 1.0.0 acceptance demo

Manual. Requires a GPU client. CI does not run this.

1. Two clients online on one `woc-server`; both see each other move.
2. Create chars, quit, re-login — gear/quests/talents restored (memory or Postgres).
3. Spend talents, cast 3+ abilities, summon a hunter or warlock pet (T).
4. Travel Eastbrook → Eastfen, die, release, respawn at a graveyard.
5. Party a dungeon boss (Eastbrook Crypt) → Need/Greed loot (1/2/3).
6. Bank an item and copper; mail copper; list then buy/cancel on the AH; gather + craft one salve.
7. Duel a player; honor increments.

Footer must read `WoC-rs 1.0.0 · upstream 0.31.0` after the version bump.
```

- [ ] **Step 2: Commit**

```bash
git add docs/parity/DEMO.md README.md
git commit -m "docs: add 1.0.0 acceptance demo script"
```

### Task 0.5: Version bump to `1.0.0`

**Files:** `VERSION.toml`, every crate `Cargo.toml` that inherits workspace version (workspace.package.version), `UPSTREAM.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `CHANGELOG.md`, `README.md` badges

Do this **last** in Wave 0, after 0.1–0.4 merge.

- [ ] Bump `rewrite_version` and `workspace.package.version` to `1.0.0`
- [ ] `parity_target = "stable"`
- [ ] CHANGELOG: new `## 1.0.0` section — contract-close only; move Unreleased polish that already shipped on pre into 1.0.0-pre or 1.0.0 as historically accurate
- [ ] STATUS: add a `1.0.0 stable` heading; keep completion table as prior
- [ ] Commit: `release: 1.0.0 stable parity`

**Wave 0 merge gate:** `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client` + `cargo fmt --all -- --check`.

---

## Wave 1 — `1.1.0` / `combat-depth`

Depends on Wave 0 fingerprint. PROTO freeze first if snapshot needs miss/crit flags.

### File map (Wave 1)

| Create | Responsibility |
| --- | --- |
| `crates/woc-content/src/ability_effects.rs` | `AbilityEffect` + `DamageSchool` enums; helpers |
| Modify `crates/woc-content/src/abilities.rs` | Each `AbilityDef` carries `effect: AbilityEffect` |
| Modify `crates/woc-sim/src/combat.rs` | `apply_ability_effect`; miss/crit; targeting by effect |
| Modify `crates/woc-sim/src/types.rs` | `MISS_CHANCE`, `CRIT_CHANCE`, `CRIT_MULT` constants |
| Optional PROTO | Snapshot `last_hit_result: none/hit/miss/crit` with `#[serde(default)]` — only if the HUD needs it |

### Task 1.1: Content `AbilityEffect`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DamageSchool {
    Physical,
    Fire,
    Nature,
    Shadow,
    Holy,
    Arcane,
    Frost,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbilityEffect {
    WeaponDamage { coefficient: f32 },
    SpellDamage { school: DamageSchool },
    Heal { coefficient: f32 },
    AoeDamage { radius: f32, max_targets: u32 },
    ApplyAura,
    Interrupt,
    Taunt { threat: f32 },
}
```

Put aura numbers in `crates/woc-content/src/ability_effects.rs` as `pub struct AuraDef { pub id, duration, tick_interval, tick_damage, tick_heal }` plus `pub fn aura_for_ability(ability_id: &str) -> Option<&'static AuraDef>`. `ApplyAura` looks up that table by `abil.id`. **Do not** keep match arms on `"heroic_strike"` inside `combat.rs`.

- [ ] Add `effect` field to `AbilityDef`
- [ ] Map existing kits: melee strikes → `WeaponDamage`; fireball → `SpellDamage { Fire }`; holy_shock → `SpellDamage { Holy }` **and** add `lesser_heal` (priest) or retarget holy_shock as heal when the target is friendly (see 1.3)
- [ ] Cleave → `AoeDamage { radius: 4.0, max_targets: 3 }`
- [ ] Integrity test: every `ABILITIES` row has an effect; every class kit slot resolves
- [ ] Commit: `feat(content): data-driven ability effects`

### Task 1.2: Miss / crit

**Files:** `crates/woc-sim/src/combat.rs`, `types.rs`  
**Test:** `crates/woc-sim/src/combat.rs` or `crates/woc-sim/tests/combat_hit_table.rs`

```rust
pub const MISS_CHANCE: f32 = 0.05;
pub const CRIT_CHANCE: f32 = 0.10;
pub const CRIT_MULT: f32 = 2.0;

pub enum HitResult {
    Miss,
    Hit,
    Crit,
}

pub fn roll_hit(rng: &mut crate::rng::Rng) -> HitResult {
    let r = rng.next_f32();
    if r < MISS_CHANCE {
        HitResult::Miss
    } else if r < MISS_CHANCE + CRIT_CHANCE {
        HitResult::Crit
    } else {
        HitResult::Hit
    }
}
```

- [ ] Failing test: seeded RNG sequence yields at least one Miss and one Crit in 200 auto-attacks
- [ ] `deal_damage` takes `HitResult`; Miss deals 0 and emits a toast; Crit multiplies
- [ ] Auto-attack and `apply_ability_effect` both roll (heals crit; they do not miss in 1.1 — document that)
- [ ] Commit: `feat(sim): miss and crit hit table`

### Task 1.3: Heal, AoE, interrupt, taunt

**Files:** `combat.rs` (targeting + effects), `pvp/mod.rs` only if heal should work in duels (it should)

Targeting rules:

- Damage / AoE / interrupt / taunt: current hostile target (mob, or player if `pvp` says the pair may fight)
- Heal: if target is missing, friendly, or is a mob, heal **self**; if target is a living party member or pet, heal that entity
- Player combat currently bails when `entities[ti].kind != EntityKind::Mob` — **that bail must become effect-aware**

```rust
pub fn apply_ability_effect(
    world: &mut crate::ecs::World,
    rng: &mut crate::rng::Rng,
    src: woc_protocol::EntityId,
    abil: &woc_content::AbilityDef,
    events: &mut Vec<SimEvent>,
) { /* dispatch on abil.effect; query Health / Combatant / ClassKit / Auras columns */ }
```

Tests (each in `combat.rs` `#[cfg(test)]`):

1. `cleave_damages_two_wolves_in_radius` — two young_wolf within 4 yd of warrior; slot 2 hits both.
2. `priest_heal_restores_party_member` — priest + warrior in party; warrior at 10 HP; heal increases warrior HP.
3. `interrupt_cancels_mob_cast` — give a mob a `CastState`; interrupt clears it.
4. `taunt_sets_mob_target_and_threat` — two players on threat; taunt makes mob target the taunter.

- [ ] Remove `apply_primary_dot`’s ability-id match; drive DoTs from content aura table keyed by ability id
- [ ] Commit: `feat(sim): heal, cleave AoE, interrupt, and taunt`

**Wave 1 merge gate:** rewrite `1.1.0`, `parity_target = "combat-depth"`, CHANGELOG + STATUS. Priest heal + warrior cleave must pass without GPU.

---

## Wave 2 — `1.2.0` / `content-depth`

Depends on Wave 1 (`AbilityEffect`). Pick **mining + blacksmithing** (not a second gather-only).

### File map (Wave 2)

| Create / modify | Responsibility |
| --- | --- |
| `crates/woc-content/src/professions.rs` | `mining`, `blacksmithing` defs |
| `crates/woc-content/src/gather_nodes.rs` | ≥3 copper-vein nodes (Eastbrook + Eastfen) |
| `crates/woc-content/src/recipes.rs` | ≥2 smith recipes (copper bar, copper shortsword) |
| `crates/woc-content/src/items.rs` | ore, bar, crafted weapon |
| `crates/woc-content/src/dungeons.rs` | crypt `trash` spots **or** sibling `dungeon_encounters.rs` |
| `crates/woc-sim/src/instances/mod.rs` | spawn trash on enter; despawn with instance |
| `crates/woc-content/src/talents.rs` + `woc-sim/src/talents.rs` | one proc/effect talent per class |
| Optional: `crates/woc-content/src/dungeons.rs` second dungeon **or** `delves.rs` second delve | one extra instance |

### Task 2.1: Mining → blacksmithing

- [ ] Content integrity: nodes resolve profession+item+zone; recipes consume ore; product is equippable `MainHand`
- [ ] Sim: existing `train` / `gather` / `craft` hooks work without combat changes
- [ ] Test: train mining+blacksmithing, grant ore, craft sword, equip
- [ ] Commit: `feat(content/sim): mining and blacksmithing loop`

### Task 2.2: Crypt trash

**Interfaces:**
- Extend `DungeonDef` additively:

```rust
pub struct DungeonTrashSpot {
    pub mob_id: &'static str,
    pub x: f32,
    pub z: f32,
    pub count: u32,
}
pub struct DungeonDef {
    // existing fields...
    pub trash: &'static [DungeonTrashSpot],
}
```

- [ ] `enter_dungeon` spawns trash with the instance key (same as boss)
- [ ] Leaving despawns instance-tagged mobs that are dead **and** still despawns living trash when the last player leaves (existing leave path)
- [ ] Test: enter crypt → ≥2 living trash mobs + boss; party kill credit still works
- [ ] Commit: `feat(sim): dungeon trash packs`

### Task 2.3: Second instance

Choose **one**: `mirefen_barrow` dungeon (boss `barrow_hag`) **or** `eastfen_sinkhole` 3-room delve. Do not add both in this wave.

- [ ] Content + enter/leave tests
- [ ] Commit: `feat(content): second dungeon or delve`

### Task 2.4: Ability-modifying talents

Each class: keep the three stat talents; add a 4th talent `effect: "ability_mod"` with a stable key (`cleave_targets_plus`, `heal_pct`, `crit_pct`, …).

`talents.rs` sim: expose `fn talent_bonus(player, key) -> f32` that combat already calls for crit/AoE max_targets/heal coefficient.

- [ ] Test: warrior with `cleave_targets_plus` hits `max_targets + 1`
- [ ] Commit: `feat(sim): talent ability modifiers`

**Wave 2 merge gate:** rewrite `1.2.0`, `parity_target = "content-depth"`.

---

## Wave 3 — `1.3.0` / `online-hard`

### Task 3.1: Park and resume

**Files:** `crates/woc-server/src/game_ws.rs`, `crates/woc-sim/src/sim.rs` (`despawn` vs `park`)

Policy:

- On WS close: set `player.alive` unchanged; remove from intent map; **keep** the entity; record `durable_id`
- On Hello with `character_id` matching `durable_id`: reuse that `EntityId`; do not `spawn_player`
- If the entity was removed (server restart): inject `CharacterSave` as today

- [ ] Test (server or sim): spawn A, `park_player(id)`, `resume_player(durable_id)` returns the same `EntityId` and position
- [ ] Commit: `feat(server): resume parked player on reconnect`

### Task 3.2: Snapshot AOI

**Files:** `crates/woc-sim/src/sim.rs` (`snapshot_for_player`)

```rust
pub const SNAPSHOT_AOI_RADIUS: f32 = 80.0;
```

Include always: the viewer, party members, current target, all NPCs whose `zone_id` matches the viewer, all loot the viewer may roll on.  
Include if `dist2d <= SNAPSHOT_AOI_RADIUS`: other players, mobs, pets.  
Exclude: far mobs/players.

- [ ] Test: mob at 5 yd in snapshot; mob at 200 yd same zone omitted
- [ ] Talk/quest still works because NPCs are always included
- [ ] Commit: `feat(sim): radius AOI for mob and player snapshots`

### Task 3.3: Persist production notes

**Files:** `README.md`, `crates/woc-persist/src/lib.rs` module docs, `UPSTREAM.md` unchanged pin

- [ ] Document `DATABASE_URL` as the durable realm path; memory is dev default
- [ ] Commit: `docs: Postgres production persist path`

**Wave 3 merge gate:** rewrite `1.3.0`, `parity_target = "online-hard"`.

---

## Main-agent merge playbook (every batch)

1. **Freeze contract:** protocol or `TICK_PHASES` changes merge first.
2. **Dispatch:** exclusive paths from the ownership map; no two agents in `sim.rs` or `woc-protocol`.
3. **Integrate:** content → leaf sim → client → server.
4. **Verify:**

```bash
cargo test --workspace --exclude woc-client
cargo check -p woc-client
```

5. **Document:** `docs/parity/STATUS.md` + `CHANGELOG.md` for the wave.
6. **Do not** reintroduce a fat actor struct. New columns follow `AGENTS.md`.

---

## First executable dispatch (next session)

1. Land **Task 0.1** (CORE only) on `cursor/ws-tick-phases-9630`.
2. Parallel: Task 0.2 CI ∥ Task 0.3 copy ∥ Task 0.4 demo doc.
3. Main agent Task 0.5 version bump after those merge.
4. Do not start Wave 1 until the nine-phase fingerprint is on `develop`.
