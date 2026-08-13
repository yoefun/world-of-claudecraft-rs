# Class-identity program implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Orchestration:** Main agent freezes protocol/sim contracts per wave, dispatches parallel subagents on isolated branches with exclusive path ownership, then merges by dependency and runs workspace tests.

**Goal:** After class-kit identity ([PR #20](https://github.com/yoefun/world-of-claudecraft-rs/pull/20)) is on `develop`, land rewrite `1.6.0` (`class-engine`), `1.7.0` (`class-identity`), and `1.8.0` (`class-forms`) so each of the nine classes has a playable signature without porting upstream spellbooks.

**Architecture:** Keep one deterministic `woc-sim` on typed sparse-column `World`. New player-only identity lives on `ClassKit`. Absorb and fear-break live on `AuraInstance`. Do not grow `AbilitySlot` past 5. Do not reintroduce a fat `Entity`.

**Tech Stack:** Rust edition 2021, Bevy 0.16, axum 0.8 (+ws), serde, sqlx/Postgres optional, nightly as in `rust-toolchain.toml`, upstream pin 0.31.0. Protocol **rev 7** at 1.6. Rewrite versions **1.6.0–1.8.0** (1.4/1.5 reserved for client-compat/update).

**Design:** [`docs/superpowers/specs/2026-08-13-class-identity-program-design.md`](../specs/2026-08-13-class-identity-program-design.md)

**Dispatch schedule:** [`2026-08-13-parallel-class-identity.md`](2026-08-13-parallel-class-identity.md)

## Global Constraints

- Merge **PR #20** to `develop` before any 1.6 code. This plan does not re-implement named auras / Execute / HealOrHarm.
- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- `woc-sim` / `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- All sim RNG via mulberry32 on `Sim` only.
- Do not reorder locked tick phases.
- Client never decides combat outcomes.
- Additive `#[serde(default)]` on snapshot fields; bump `PROTOCOL_REV` to 7 once at 1.6 (new `InteractAction` variants).
- English-only strings.
- New *per-actor* combat state → `ClassKit` / `Combat` / `Auras` fields, not a new blob struct. New *per-realm* state → `Sim`.
- Do not add `AbilitySlot::Slot6`.
- Do not edit `VERSION.toml` until the matching wave gate (1.6 / 1.7 / 1.8).
- Before claiming a wave done: `cargo test --workspace --exclude woc-client` + `cargo clippy --workspace --exclude woc-client -- -D warnings` + `cargo check -p woc-client` when the environment has Bevy deps.
- Branch naming: `cursor/<workstream-id>-67ff` (or unique suffix). One workstream per branch.

## File ownership map

| Owner | Exclusive paths |
| --- | --- |
| **PROTO** | `crates/woc-protocol/src/lib.rs` |
| **CONTENT** | `crates/woc-content/src/abilities.rs`, `ability_effects.rs`, `classes.rs`, `lib.rs` tests |
| **COMBAT** | `crates/woc-sim/src/combat.rs` |
| **KIT** | `crates/woc-sim/src/ecs/components.rs` (`ClassKit`, `Combat`, `AuraInstance` fields only), `ecs/spawn.rs` |
| **MOB** | `crates/woc-sim/src/mob.rs` |
| **MOTION** | `crates/woc-sim/src/player_motion.rs`, `entity_motion.rs` |
| **CORE** | `crates/woc-sim/src/sim.rs` (snapshot + interact dispatch only) |
| **CLIENT** | `crates/woc-client/src/input.rs`, `hud.rs` |
| **PERSIST** | `crates/woc-persist/src/models.rs` (stance_id default) |
| **DOCS** | `docs/parity/**`, `CHANGELOG.md`, `VERSION.toml`, `docs/ROADMAP.md` |

CORE approves new `ClassKit` / `Combat` / `AuraInstance` fields in the same 1.6 batch. Never two agents adding columns in one batch.

---

## Prerequisite

- [ ] **PR #20 merged to `develop`** (`feat: 完善各职业技能处理`). If not merged, cherry-pick or rebase 1.6 work onto that branch — do not fork a third copy of the aura table.

---

## Wave 1 — `1.6.0` / `class-engine`

PROTO freeze first (rev 7), then KIT+CONTENT in parallel with COMBAT depending on both.

### Task 1.6.0: Protocol rev 7

**Files:** `crates/woc-protocol/src/lib.rs`  
**Test:** existing snapshot roundtrip + new defaults test

```rust
// TickSnapshot — all #[serde(default)]
pub combo_points: u8,
pub stealthed: bool,
pub stance_id: String,
pub absorb: f32,

// InteractAction
ToggleStealth,
CycleStance,
ToggleForm,
```

- [ ] Add fields and variants; `PROTOCOL_REV = 7`
- [ ] Test: omitted JSON fields deserialize to 0 / false / ""
- [ ] Test: new interact variants roundtrip
- [ ] Commit: `feat(protocol): rev 7 combo stealth stance absorb`

### Task 1.6.1: Kit / aura / combat columns

**Files:** `crates/woc-sim/src/ecs/components.rs`, `spawn.rs`

- [ ] `ClassKit`: `combo_points: u8`, `stealthed: bool`, `stance_id: Option<String>`
- [ ] `Combat`: `cast_lockout: f32`
- [ ] `AuraInstance`: `absorb: f32`, `breaks_on_damage: bool`
- [ ] `create_player` zeros the new fields; `refresh_known_abilities` does not touch combo
- [ ] Commit: `feat(sim): class identity fields on ClassKit Combat Auras`

### Task 1.6.2: Content flags + four primary effects

**Files:** `crates/woc-content/src/ability_effects.rs`, `abilities.rs`, `classes.rs`, `lib.rs`

`AuraDef` gains `absorb: f32`, `breaks_on_damage: bool` (default 0 / false via const helpers).

`AbilityDef` gains (all have defaults in the table):

```rust
pub requires_stealth: bool,
pub breaks_stealth: bool,      // default true
pub combo_add: u8,             // 0 = none
pub combo_spend: bool,
pub combo_per_point: f32,      // eviscerate scale
pub self_aoe: bool,
pub interrupt_lockout: f32,    // 0 = none
pub rage_dump: bool,
```

New `AbilityEffect` variants: `Absorb { amount }`, `Charge { gap }`, `Blink { distance }`, `Convert { hp_cost, resource_gain }`.

Add **stub abilities** (may sit off-kit until 1.7): `power_word_shield`, `charge`, `blink`, `life_tap`, `battle_shout`, `aspect_of_the_hawk`. Integrity: every new id resolves; `ApplyAura` still requires `aura`.

Hunter `resource_type: ResourceType::Mana`.

- [ ] Content tests: hunter is mana; shield is Absorb; charge/blink/life_tap variants exist
- [ ] Commit: `feat(content): class-engine ability flags and stubs`

### Task 1.6.3: Combat dispatch

**Files:** `crates/woc-sim/src/combat.rs`  
**Constants:** `crates/woc-sim/src/types.rs` — `RAGE_FROM_TAKEN: f32 = 0.05`

Behavior:

1. `deal_damage`: apply absorb on target auras first; if `breaks_on_damage`, clear those auras; if victim has rage, `gain_resource(mitigated * RAGE_FROM_TAKEN)`; if victim `stealthed`, clear stealth.
2. `Interrupt`: set `cast_lockout = def.interrupt_lockout.max(1.5)` in addition to clearing cast.
3. Starting a cast / instant: fail if `cast_lockout > 0` (still tick lockout down with GCD).
4. `AoeDamage` + `self_aoe` (or no hostile): origin = caster.
5. `Execute` + `rage_dump`: after base cost, spend remaining rage and multiply damage by `1 + spent / resource_max`.
6. Combo: on successful harm, `combo_points = (combo + combo_add).min(5)`; if `combo_spend`, scale by `1 + combo_per_point * combo` then zero combo.
7. `requires_stealth`: `aim_ability` returns None unless stealthed.
8. `breaks_stealth` (default): clear stealth after the ability starts (including Cheap Shot).
9. `Absorb`: apply aura with `absorb = amount` on heal-target (self/friendly).
10. `Charge { gap }`: if dist in `(MELEE, gap]`, `step_toward` / snap to melee (fail + toast if sweep blocked), then weapon hit.
11. `Blink { distance }`: offset Transform along yaw; clamp + ground.
12. `Convert`: subtract HP (leave at least 1 if alive), add resource.

Tick `cast_lockout` in `update_player_combat` / `update_mob_combat` with `DT`.

Tests in `combat.rs`:

- [ ] `absorb_soaks_damage_before_hp`
- [ ] `interrupt_sets_cast_lockout`
- [ ] `self_aoe_fires_without_hostile_target` (mage frost_nova or stub)
- [ ] `rage_increases_when_warrior_is_hit`
- [ ] `execute_dumps_remaining_rage`
- [ ] `combo_builder_and_spend` (can use a temporary warrior hit if rogue kit not wired yet — prefer applying `combo_add` on sinister_strike in this same commit)
- [ ] `charge_closes_gap_then_hits`
- [ ] `blink_displaces_along_facing`
- [ ] `life_tap_converts_hp_to_mana`
- [ ] `cheap_shot_requires_stealth` (set `stealthed` in the test fixture)

- [ ] Commit: `feat(sim): absorb lockout combo charge blink convert`

### Task 1.6.4: Stealth aggro + move

**Files:** `crates/woc-sim/src/mob.rs`, `player_motion.rs`

- [ ] Mob aggro: skip stealthed players unless `d <= MELEE_RANGE`
- [ ] Stealthed move_speed_mult *= 0.7 (stack with chill via `min`)
- [ ] Test: `stealth_skips_wolf_aggro_at_range`
- [ ] Test: `stealth_breaks_when_wolf_hits_in_melee`
- [ ] Commit: `feat(sim): stealth aggro skip and move penalty`

### Task 1.6.5: Snapshot + interact + HUD

**Files:** `crates/woc-sim/src/sim.rs` (interact + snapshot), `crates/woc-client/src/input.rs`, `hud.rs`

- [ ] `ToggleStealth` sets/clears `ClassKit.stealthed` (rogue only in 1.6; other classes toast “You cannot stealth.”)
- [ ] Snapshot fills `combo_points`, `stealthed`, `stance_id`, `absorb` (sum remaining absorb auras)
- [ ] Client: **Z** → `ToggleStealth`; HUD shows combo `●●●○○` and `STEALTH` when set
- [ ] Commit: `feat(sim/client): stealth interact and identity snapshot`

### Task 1.6.6: Version gate

**Files:** `VERSION.toml`, workspace `Cargo.toml` version, `docs/parity/STATUS.md`, `CHANGELOG.md`, `docs/ROADMAP.md`, `docs/parity/DEMO.md` footer note

- [ ] `rewrite_version = "1.6.0"`, `parity_target = "class-engine"`
- [ ] STATUS: new post-1.3 table rows for engine pieces `done`
- [ ] Commit: `release: 1.6.0 class-engine`

**Wave 1 merge gate:** workspace tests + clippy `-D warnings`. Priest shield soak + stealth aggro skip must pass without GPU.

---

## Wave 2 — `1.7.0` / `class-identity`

Depends on 1.6. CONTENT kits + COMBAT wiring; CLIENT key hints.

### Kit slot swaps (keep ≤5)

| Class | Slot change |
| --- | --- |
| Rogue | Keep 4; Z stealth (not a slot). Mark `sinister_strike.combo_add = 1`, `eviscerate.combo_spend = true`, `cheap_shot.requires_stealth = true` |
| Priest | Slot 5 `shadow_word_pain` → `power_word_shield` (pain remains as an ability id, off-bar) |
| Warrior | Slot 5 `rend` → `charge` (rend stays in `ABILITIES`, off-bar). Battle Shout waits for 1.8 stance. |
| Mage | Slot 5 `counterspell` stays; Slot 4 frost_nova gets `self_aoe`; replace Slot 5 with `blink` (counterspell remains an id, earth_shock-style interrupt still on shaman) |
| Hunter | `resource_type` already Mana; Slot 5 `multi_shot` → `aspect_of_the_hawk` |

Document the dropped on-bar ids in CHANGELOG (“still in `ABILITIES`, not on the default bar”).

### Task 1.7.1: Content kit wiring

- [ ] Apply the table above; integrity: each of the five classes still has ≥2 effect kinds
- [ ] `battle_shout` ApplyAura party? **1.7 ships self-buff only** (`buff` via existing aura `move_mult` is wrong). Add `AuraDef.damage_mult: f32` (default 1.0) applied in `deal_damage` for the source. Keep it small: `1.1` for 120s.
- [ ] Aspect: same `damage_mult` 1.1
- [ ] Commit: `feat(content): 1.7 identity kit swaps`

### Task 1.7.2: Sim tests per class

- [ ] `rogue_cheap_shot_fails_without_stealth`
- [ ] `rogue_eviscerate_scales_with_combo`
- [ ] `priest_shield_on_slot5`
- [ ] `warrior_charge_from_eight_yards`
- [ ] `mage_frost_nova_without_target`
- [ ] `mage_blink_on_slot5`
- [ ] `hunter_spends_mana_not_energy`
- [ ] Commit: `feat(sim): class-identity behavior tests`

### Task 1.7.3: HUD copy

- [ ] Action bar names follow kit swaps; toast on stealth toggle
- [ ] Commit: `feat(client): identity HUD hints (Z stealth, combo dots)` if not finished in 1.6.5

### Task 1.7.4: Version gate

- [ ] `1.7.0` / `class-identity`; STATUS + CHANGELOG + DEMO step “rogue stealth opener”
- [ ] Commit: `release: 1.7.0 class-identity`

---

## Wave 3 — `1.8.0` / `class-forms`

### Task 1.8.1: Aura extras

**Files:** content `AuraDef` + combat `deal_damage`

- [ ] `damage_mult` already from 1.7; add `thorns: f32` (Lightning Shield: attacker takes `thorns` when they melee the bearer)
- [ ] `breaks_on_damage` already from 1.6 (Fear, Ghost Wolf, Travel Form)
- [ ] Commit: `feat(content/sim): thorns and form auras`

### Task 1.8.2: Kit + interact

| Class | Signature | Binding |
| --- | --- | --- |
| Paladin | Devotion Aura (`armor` via existing armor talent path or `AuraDef` armor_flat on self) auto-applied at spawn; Seal = on-hit absorb? **Seal = on-hit extra damage aura on Crusader Strike** (`crusader_strike.aura = Some("seal_righteousness")` small extra hit is enough) | F unused |
| Shaman | Slot swap: `flame_shock` stays; add `lightning_shield` replacing Slot 5; Ghost Wolf via `ToggleForm` | F |
| Warlock | Slot 4 stays Immolate; Life Tap replaces nothing — **Interact? ** Put `life_tap` on Slot 4, Immolate off-bar; Fear on Slot 5 replacing nothing if only 4 slots — warlock currently 4 slots: Slot 4 life_tap, Slot 5 fear |
| Druid | `ToggleForm` travel form | F |
| Warrior | `CycleStance` battle (default) / defensive (`damage_mult` 0.9, armor_flat +20). Entering battle stance applies a small self `damage_mult` shout aura | F |

- [ ] Persist `stance_id` on `Character` / `CharacterSave` with `#[serde(default)]`
- [ ] Ghost Wolf / Travel Form: `move_mult` 1.4, `breaks_on_damage`
- [ ] Fear: stun + `breaks_on_damage`, duration 4s
- [ ] Commit: `feat(content/sim): paladin shaman warlock druid warrior forms`

### Task 1.8.3: Tests

- [ ] `lightning_shield_thorns_hits_attacker`
- [ ] `fear_breaks_when_damaged`
- [ ] `travel_form_speeds_then_breaks_on_hit`
- [ ] `defensive_stance_reduces_damage`
- [ ] `life_tap_and_fear_on_warlock_bar`
- [ ] Commit: `test(sim): class-forms signatures`

### Task 1.8.4: Client F key

- [ ] **F** → `CycleStance` if warrior, else `ToggleForm` if shaman/druid, else no-op toast
- [ ] Commit: `feat(client): F stance/form toggle`

### Task 1.8.5: Version gate

- [ ] `1.8.0` / `class-forms`; STATUS all nine classes have a signature row; DEMO steps for travel form + fear
- [ ] Commit: `release: 1.8.0 class-forms`

**Wave 3 merge gate:** all nine classes mentioned in STATUS with a one-line signature; workspace tests green.

---

## Main-agent merge playbook

1. Freeze PROTO (rev 7) and merge before CONTENT/COMBAT that depend on new snapshot fields.
2. Merge KIT fields in the same batch as first COMBAT use (avoid two column PRs).
3. After each wave: bump version on a docs-only follow-up commit, not inside a combat PR.
4. Do not start 1.7 until 1.6 tests listed above are green on `develop`.
5. If 1.4/1.5 client PRs land in between, rebase; do not reuse 1.4/1.5 version numbers.

## Out of scope (do not sneak in)

Ability ranks, 27 talent specs, pet roster, bear/cat, Ice Block, channels, dodge/parry/block, school resist, 6th action-bar slot.
