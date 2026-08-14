# Class Depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rewrite `1.25.0` / `class-depth`: distinct energy/mana/rage regen, five live kit slots with restored signatures, paladin **F** aura cycle, hunter/warlock pet abilities, and HUD stance/form names.

**Architecture:** No new ECS columns and no protocol bump. Resource regen stays inside `update_player_combat`. Paladin reuses `InteractAction::CycleStance`. Pet extra hits use existing `Combat.ability_cd` plus `PetDef.ability_id`. Bevy only paints `TickSnapshot.stance_id` and sends **F** for paladin.

**Tech Stack:** Rust 2021 workspace crates (`woc-content`, `woc-sim`, `woc-client`). No new crates. No Bevy inside sim.

**Design spec:** [`docs/superpowers/specs/2026-08-14-class-depth-design.md`](../specs/2026-08-14-class-depth-design.md)

## Global Constraints

- `woc-sim` and `woc-content` MUST NOT depend on Bevy, `bevy_ecs`, wgpu, axum, or tokio.
- Client never decides regen, stance, pet hits, or kit unlocks.
- All sim RNG via mulberry32. Resource regen and pet extra hits do **not** roll the hit table.
- Tick fingerprint must remain `3214741777866168171u64`. No new named phase.
- `PROTOCOL_REV` stays `10`. Reuse `CycleStance`. Do not add snapshot fields.
- Upstream pin stays `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- English-only player-facing strings (exact copies from the spec §6.8).
- Do not add a new ECS column. Do not reintroduce a fat `Entity`. Action bar stays slots 1–5.
- `TrainClass` stays a confirmation toast. Do not gate `min_level` behind Alden.
- If `develop` has already used `1.25.0` for another wave, shift the tag by one. Do not reuse `1.24.0` (`delve-depth`).
- Every task ends with `cargo test --workspace --exclude woc-client` green, and `cargo check -p woc-client` green when client files change.
- Do not bump workspace `version` / `VERSION.toml` until Task 7.

---

## File map (create / own)

| Path | Responsibility |
| --- | --- |
| `crates/woc-sim/src/types.rs` | `ENERGY_REGEN_PER_SEC` / mana in/out / `RAGE_DECAY_OOC_PER_SEC` |
| `crates/woc-sim/src/combat.rs` | Regen in `update_player_combat`; paladin branch of `cycle_stance` / `apply_spawn_identity` |
| `crates/woc-content/src/classes.rs` | Five-slot kits; hunter/priest/mage slot 3; rogue slot 5 |
| `crates/woc-content/src/abilities.rs` | `sprint`, `imp_firebolt`; drop “stubs” comment |
| `crates/woc-content/src/ability_effects.rs` | `sprint`, `retribution_aura` |
| `crates/woc-content/src/pets.rs` | `PetDef.ability_id` |
| `crates/woc-content/src/lib.rs` | Kit-length / slot-3 identity tests |
| `crates/woc-sim/src/pet/mod.rs` | Extra hit + imp walk-to-range |
| `crates/woc-client/src/input.rs` | Paladin **F** → `CycleStance` |
| `crates/woc-client/src/hud.rs` | Paint `stance_id`; dynamic `[F]` hint |
| `docs/parity/{STATUS,DEMO}.md`, `docs/ROADMAP.md`, `CHANGELOG.md`, `README.md`, `VERSION.toml`, `UPSTREAM.md`, `crates/woc-version/src/lib.rs` | Version rows (Task 7) |

---

### Task 1: Distinct resource regen

**Files:**
- Modify: `crates/woc-sim/src/types.rs`
- Modify: `crates/woc-sim/src/combat.rs` (`update_player_combat` regen branch; tests at the class-identity module)
- Test: `crates/woc-sim/src/combat.rs`

**Interfaces:**
- Consumes: `ClassKit.resource_type`, `Combat.auto_attack`, `Combat.target`, `DT`
- Produces: `ENERGY_REGEN_PER_SEC = 10.0`, `MANA_REGEN_OOC_PER_SEC = 8.0`, `MANA_REGEN_COMBAT_PER_SEC = 2.0`, `RAGE_DECAY_OOC_PER_SEC = 3.0`

- [ ] **Step 1: Write the failing tests**

Add next to `hunter_spends_mana_not_energy` in `combat.rs`. Reuse `class_and_mob`. In combat = `auto_attack || living hostile target`. For OOC, clear both.

```rust
    #[test]
    fn energy_regens_ten_per_second() {
        let mut world = class_and_mob(PlayerClass::Rogue, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
            c.auto_attack = false;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 0.0;
        }
        let mut events = Vec::new();
        for _ in 0..20 {
            update_player_combat(1, &mut world, None, &mut hit_rng(), &mut events);
        }
        let energy = world.get::<ClassKit>(1).unwrap().resource;
        assert!(
            (energy - 10.0).abs() < 0.15,
            "rogue energy should gain 10/s OOC, got {energy}"
        );
    }

    #[test]
    fn mana_regens_slower_in_combat() {
        let mut world = class_and_mob(PlayerClass::Mage, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
            c.auto_attack = false;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 0.0;
        }
        let mut events = Vec::new();
        for _ in 0..20 {
            update_player_combat(1, &mut world, None, &mut hit_rng(), &mut events);
        }
        let ooc = world.get::<ClassKit>(1).unwrap().resource;
        assert!(
            (ooc - 8.0).abs() < 0.15,
            "mage mana should gain 8/s OOC, got {ooc}"
        );

        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 0.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
            c.auto_attack = true;
            c.swing_timer = 99.0;
        }
        events.clear();
        for _ in 0..20 {
            update_player_combat(1, &mut world, None, &mut hit_rng(), &mut events);
        }
        let ic = world.get::<ClassKit>(1).unwrap().resource;
        assert!(
            (ic - 2.0).abs() < 0.15,
            "mage mana should gain 2/s in combat, got {ic}"
        );
    }

    #[test]
    fn rage_decays_out_of_combat() {
        let mut world = class_and_mob(PlayerClass::Warrior, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
            c.auto_attack = false;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 30.0;
        }
        let mut events = Vec::new();
        for _ in 0..20 {
            update_player_combat(1, &mut world, None, &mut hit_rng(), &mut events);
        }
        let ooc = world.get::<ClassKit>(1).unwrap().resource;
        assert!(
            (ooc - 27.0).abs() < 0.15,
            "warrior rage should decay 3/s OOC, got {ooc}"
        );

        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 30.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
            c.auto_attack = true;
            c.swing_timer = 99.0;
        }
        events.clear();
        for _ in 0..20 {
            update_player_combat(1, &mut world, None, &mut hit_rng(), &mut events);
        }
        let ic = world.get::<ClassKit>(1).unwrap().resource;
        assert!(
            ic >= 29.9,
            "warrior rage must not decay in combat, got {ic}"
        );
    }
```

Do not import `LootTable` in the test module; the tests only clear `Combat.target` / `auto_attack`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib energy_regens_ten_per_second mana_regens_slower_in_combat rage_decays_out_of_combat -- --nocapture`

Expected: FAIL (`rogue energy should gain 10/s` because current regen is `1.5/s` → ~1.5 after 20 ticks; mage OOC ~1.5 not 8; warrior rage stays 30).

- [ ] **Step 3: Write minimal implementation**

In `crates/woc-sim/src/types.rs` after `RAGE_FROM_TAKEN`:

```rust
/// Energy gained per second (in or out of combat).
pub const ENERGY_REGEN_PER_SEC: f32 = 10.0;
/// Mana gained per second while out of combat.
pub const MANA_REGEN_OOC_PER_SEC: f32 = 8.0;
/// Mana gained per second while in combat.
pub const MANA_REGEN_COMBAT_PER_SEC: f32 = 2.0;
/// Rage lost per second while out of combat.
pub const RAGE_DECAY_OOC_PER_SEC: f32 = 3.0;
```

In `combat.rs` imports from `types`, add the four constants.

Replace the regen block in `update_player_combat` (the `if let Some(ResourceType::Mana | ResourceType::Energy)` arm) with:

```rust
    let in_combat = combat.auto_attack
        || combat.target.is_some_and(|tid| {
            tid != player_id
                && world.get::<Health>(tid).is_some_and(|h| h.alive)
                && (world.get::<LootTable>(tid).is_some() || world.get::<ClassKit>(tid).is_some())
        });
    match kit.resource_type {
        Some(ResourceType::Energy) => {
            gain_resource(&mut kit, ENERGY_REGEN_PER_SEC * DT);
        }
        Some(ResourceType::Mana) => {
            let rate = if in_combat {
                MANA_REGEN_COMBAT_PER_SEC
            } else {
                MANA_REGEN_OOC_PER_SEC
            };
            gain_resource(&mut kit, rate * DT);
        }
        Some(ResourceType::Rage) if !in_combat => {
            kit.resource = (kit.resource - RAGE_DECAY_OOC_PER_SEC * DT).max(0.0);
        }
        _ => {}
    }
```

`LootTable` is already imported at the top of `combat.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --lib energy_regens_ten_per_second mana_regens_slower_in_combat rage_decays_out_of_combat hunter_spends_mana_not_energy tick_phase_order_fingerprint_locked -q`

Expected: PASS. Fingerprint still `3214741777866168171u64`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/types.rs crates/woc-sim/src/combat.rs
git commit -m "feat(sim): distinct energy, mana, and rage regeneration"
```

---

### Task 2: Five-slot kits and restored signatures

**Files:**
- Modify: `crates/woc-content/src/classes.rs` (`ROGUE_KIT`, `HUNTER_KIT`, `PRIEST_KIT`, `MAGE_KIT`)
- Modify: `crates/woc-content/src/lib.rs` (`every_class_has_multi_ability_kit` plus slot-id tests)
- Modify: `crates/woc-content/src/abilities.rs` (delete the “Class-engine stubs” comment; keep those defs)
- Test: `crates/woc-content/src/lib.rs`
- Test: existing `crates/woc-sim/src/combat.rs` kit-slot tests that name Aimed Shot / Mind Blast / Arcane Missiles on slot 3 — update them in this task if they fail

**Interfaces:**
- Consumes: existing `multi_shot`, `shadow_word_pain`, `counterspell` ability ids
- Produces: every `ClassDef.kit` length 5 with slots `{1,2,3,4,5}`; hunter 3 = `multi_shot`; priest 3 = `shadow_word_pain`; mage 3 = `counterspell`; rogue 5 still missing until Task 3 (add a placeholder slot 5 pointing at `sprint` only after Task 3, **or** add the sprint `AbilityDef` stub in this task’s Step 3 so the kit compiles)

Do Task 3 immediately after if this task adds `sprint` to the kit before the ability exists — prefer adding the `sprint` `AbilityDef` + aura in Task 3 first. **Order override:** implement Task 3 *before* finishing this task’s kit table if the compiler requires `ability("sprint")`. The checklist below assumes Task 3’s ability exists; if you run this task first, keep rogue at 4 slots until Task 3 then bump the length assertion in the same commit as Sprint.

Recommended order: **Task 3, then the kit table in this task.** If you already started here, add rogue slot 5 in the Task 3 commit instead of here.

- [ ] **Step 1: Write the failing content tests**

In `crates/woc-content/src/lib.rs`, change `every_class_has_multi_ability_kit`:

```rust
            assert_eq!(
                class.kit.len(),
                5,
                "{} kit needs 5 abilities, got {}",
                class.name,
                class.kit.len()
            );
```

Keep the existing slot `1..=5` / unique / primary checks. After the loop over `CLASSES`, add:

```rust
    #[test]
    fn restored_class_depth_kit_slots() {
        assert_eq!(
            class_ability_for_slot(PlayerClass::Hunter, 3)
                .expect("hunter 3")
                .id,
            "multi_shot"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Priest, 3)
                .expect("priest 3")
                .id,
            "shadow_word_pain"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Mage, 3)
                .expect("mage 3")
                .id,
            "counterspell"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Rogue, 5)
                .expect("rogue 5")
                .id,
            "sprint"
        );
    }
```

If `class_kit_slot5_is_charge` / similar sim tests assert hunter 3 is Aimed Shot, priest 3 Mind Blast, or mage 3 Arcane Missiles, change those expected ids in the **same** commit as the kit swap (search `aimed_shot`, `mind_blast`, `arcane_missiles` in `crates/woc-sim` and `crates/woc-content`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content --lib every_class_has_multi_ability_kit restored_class_depth_kit_slots -q`

Expected: FAIL (`Rogue kit needs 5 abilities, got 4` and/or slot 3 id mismatches).

- [ ] **Step 3: Write minimal implementation**

`HUNTER_KIT` slot 3 `ability_id: "multi_shot"`.  
`PRIEST_KIT` slot 3 `ability_id: "shadow_word_pain"`.  
`MAGE_KIT` slot 3 `ability_id: "counterspell"`.  
`ROGUE_KIT` append:

```rust
    ClassKitEntry {
        slot: 5,
        ability_id: "sprint",
    },
```

(`sprint` must exist — Task 3.)

In `abilities.rs` replace `// —— Class-engine stubs (off-kit; Charge/Blink/Shield/Life Tap/Aspect on 1.7 bars) ——` with `// —— Class-engine identity (Charge / Blink / Shield / Life Tap / Aspect) ——`.

Leave `aimed_shot`, `mind_blast`, `arcane_missiles` in `ABILITIES`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-content --lib -q && cargo test -p woc-sim --lib -- hunter_aspect mage_slot priest_slot rogue_ -- --test-threads=1 -q`

Expected: PASS. Then `cargo test --workspace --exclude woc-client -q`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/classes.rs crates/woc-content/src/lib.rs crates/woc-content/src/abilities.rs crates/woc-sim/src/combat.rs
git commit -m "feat(content): restore five-slot class kits and off-bar signatures"
```

---

### Task 3: Rogue Sprint

**Files:**
- Modify: `crates/woc-content/src/ability_effects.rs` (`AURAS` + uniqueness test already loops)
- Modify: `crates/woc-content/src/abilities.rs` (new `sprint` `AbilityDef`)
- Modify: `crates/woc-sim/src/combat.rs` (sim test)
- Test: `crates/woc-content/src/ability_effects.rs`, `crates/woc-sim/src/combat.rs`

**Interfaces:**
- Consumes: `AbilityEffect::ApplyAura`, `AuraDef.is_self_buff` (`move_mult >= 1.0`)
- Produces: ability id `sprint`; aura id `sprint` duration 8, `move_mult` 1.5, `breaks_on_damage` false; cost 40, CD 20, min_level 1

Do this **before** Task 2’s rogue slot 5 if `ability("sprint")` would otherwise panic.

- [ ] **Step 1: Write the failing tests**

In `ability_effects.rs` `aura_table_resolves_named_defs`, add:

```rust
        let sprint = aura("sprint").expect("sprint");
        assert!((sprint.move_mult - 1.5).abs() < f32::EPSILON);
        assert!(!sprint.breaks_on_damage);
        assert_eq!(sprint.duration, 8.0);
```

In `combat.rs` next to `toggle_form_speeds_druid`:

```rust
    #[test]
    fn rogue_sprint_buffs_move_speed() {
        let mut world = class_and_mob(PlayerClass::Rogue, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
        }
        fire_slot(&mut world, AbilitySlot::Slot5);
        assert!(
            world
                .get::<Auras>(1)
                .unwrap()
                .auras
                .iter()
                .any(|a| a.id == "sprint"),
            "rogue slot 5 should apply Sprint"
        );
        assert!(
            (move_speed_mult(&world, 1) - 1.5).abs() < 1e-3,
            "sprint should raise move speed"
        );
    }
```

`AbilitySlot` is already used in this module. If Task 2 has not yet bound slot 5, this test belongs in the Task 2 commit after the kit row.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content --lib aura_table_resolves_named_defs -q`

Expected: FAIL (`sprint` missing).

- [ ] **Step 3: Write minimal implementation**

In `ability_effects.rs`, after the `form` helper, add:

```rust
const fn haste(id: &'static str, duration: f32, move_mult: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult,
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
    }
}
```

In `AURAS`, after `form("travel_form", 120.0, 1.4),`:

```rust
    haste("sprint", 8.0, 1.5),
```

In `abilities.rs`, after the rogue `kick` def (before the priest block), add:

```rust
    AbilityDef {
        id: "sprint",
        name: "Sprint",
        damage: 0.0,
        cost: 40.0,
        cooldown: 20.0,
        range: 0.0,
        cast_time: 0.0,
        min_level: 1,
        aura: Some("sprint"),
        effect: AbilityEffect::ApplyAura,
        flags: AbilityFlags::DEFAULT,
    },
```

Sprint **does** break stealth (`breaks_stealth` default true). Do not set `stealth_opener`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-content --lib aura_table_resolves_named_defs aura_ids_are_unique -q && cargo test -p woc-sim --lib rogue_sprint_buffs_move_speed -q`

Expected: PASS (`rogue_sprint` waits on Task 2 kit). If kit is not yet 5, run only the content tests here and move the sim test to Task 2 Step 4.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/ability_effects.rs crates/woc-content/src/abilities.rs crates/woc-sim/src/combat.rs
git commit -m "feat(content): add rogue Sprint self-buff"
```

---

### Task 4: Paladin aura cycle

**Files:**
- Modify: `crates/woc-content/src/ability_effects.rs` (`retribution_aura`)
- Modify: `crates/woc-sim/src/combat.rs` (`apply_spawn_identity`, `cycle_stance`, tests)
- Test: `crates/woc-sim/src/combat.rs`

**Interfaces:**
- Consumes: `InteractAction::CycleStance` (unchanged wire)
- Produces: paladin spawn `stance_id = "devotion"`; cycle `devotion` ↔ `retribution`; toasts `Devotion Aura.` / `Retribution Aura.`; aura `retribution_aura` `damage_mult` 1.1 duration 3600

- [ ] **Step 1: Write the failing tests**

In `ability_effects.rs` `aura_table_resolves_named_defs`:

```rust
        let retribution = aura("retribution_aura").expect("retribution_aura");
        assert!((retribution.damage_mult - 1.1).abs() < f32::EPSILON);
        assert_eq!(retribution.armor_flat, 0.0);
```

In `combat.rs` next to `cycle_stance_applies_defensive_then_battle`:

```rust
    #[test]
    fn paladin_cycle_stance_swaps_devotion_and_retribution() {
        let mut world = class_and_mob(PlayerClass::Paladin, 1);
        assert_eq!(
            world.get::<ClassKit>(1).unwrap().stance_id.as_deref(),
            Some("devotion")
        );
        assert!(world
            .get::<Auras>(1)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == "devotion_aura"));
        cycle_stance(&mut world, 1, &mut Vec::new());
        assert_eq!(
            world.get::<ClassKit>(1).unwrap().stance_id.as_deref(),
            Some("retribution")
        );
        assert!(world
            .get::<Auras>(1)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == "retribution_aura"));
        assert!(world
            .get::<Auras>(1)
            .unwrap()
            .auras
            .iter()
            .all(|a| a.id != "devotion_aura"));
        cycle_stance(&mut world, 1, &mut Vec::new());
        assert_eq!(
            world.get::<ClassKit>(1).unwrap().stance_id.as_deref(),
            Some("devotion")
        );
    }

    #[test]
    fn paladin_retribution_buffs_outgoing_damage() {
        let mut world = class_and_mob(PlayerClass::Paladin, 1);
        cycle_stance(&mut world, 1, &mut Vec::new());
        let hp = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Primary);
        let with_ret = hp - world.get::<Health>(2).unwrap().hp;

        let mut world = class_and_mob(PlayerClass::Paladin, 1);
        let hp = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Primary);
        let with_dev = hp - world.get::<Health>(2).unwrap().hp;
        assert!(
            with_ret > with_dev + 0.5,
            "retribution aura should raise crusader strike damage ({with_ret} vs {with_dev})"
        );
    }
```

Keep `cycle_stance_applies_defensive_then_battle` unchanged. A mage `CycleStance` must still toast `You cannot change stance.` — existing sim.rs host test covers that.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib paladin_cycle_stance_swaps_devotion_and_retribution -q`

Expected: FAIL (`stance_id` is `None` at paladin spawn; `cycle_stance` rejects paladin).

- [ ] **Step 3: Write minimal implementation**

In `AURAS` after `armor_aura("devotion_aura", 3600.0, 20.0),`:

```rust
    buff("retribution_aura", 3600.0, 1.1),
```

`buff` already sets `damage_mult` and `armor_flat = 0`.

`apply_spawn_identity` paladin arm:

```rust
        Some(woc_content::PlayerClass::Paladin) => {
            if stance.as_deref() == Some("retribution") {
                apply_named_aura(world, player_id, player_id, "retribution_aura", &mut events);
            } else {
                if let Some(kit) = world.get_mut::<ClassKit>(id) {
                    kit.stance_id = Some("devotion".into());
                }
                apply_named_aura(world, player_id, player_id, "devotion_aura", &mut events);
            }
        }
```

Use `player_id` not `id` in that `get_mut` — the snippet’s `id` is a typo. Correct:

```rust
                if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
                    kit.stance_id = Some("devotion".into());
                }
```

Replace `cycle_stance` body after reading `class`:

```rust
    match class {
        Some(woc_content::PlayerClass::Warrior) => {
            crate::mount::dismount(world, player_id, events);
            let current = world
                .get::<ClassKit>(player_id)
                .and_then(|k| k.stance_id.clone());
            let next = if current.as_deref() == Some("defensive") {
                "battle"
            } else {
                "defensive"
            };
            if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
                kit.stance_id = Some(next.into());
            }
            remove_named_auras(world, player_id, &["battle_shout", "defensive_stance"]);
            if next == "battle" {
                apply_named_aura(world, player_id, player_id, "battle_shout", events);
                events.push(SimEvent::Toast {
                    message: "Battle Stance.".into(),
                });
            } else {
                apply_named_aura(world, player_id, player_id, "defensive_stance", events);
                events.push(SimEvent::Toast {
                    message: "Defensive Stance.".into(),
                });
            }
        }
        Some(woc_content::PlayerClass::Paladin) => {
            crate::mount::dismount(world, player_id, events);
            let current = world
                .get::<ClassKit>(player_id)
                .and_then(|k| k.stance_id.clone());
            let next = if current.as_deref() == Some("retribution") {
                "devotion"
            } else {
                "retribution"
            };
            if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
                kit.stance_id = Some(next.into());
            }
            remove_named_auras(world, player_id, &["devotion_aura", "retribution_aura"]);
            if next == "devotion" {
                apply_named_aura(world, player_id, player_id, "devotion_aura", events);
                events.push(SimEvent::Toast {
                    message: "Devotion Aura.".into(),
                });
            } else {
                apply_named_aura(world, player_id, player_id, "retribution_aura", events);
                events.push(SimEvent::Toast {
                    message: "Retribution Aura.".into(),
                });
            }
        }
        _ => {
            events.push(SimEvent::Toast {
                message: "You cannot change stance.".into(),
            });
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --lib paladin_cycle_stance paladin_retribution cycle_stance_applies_defensive -q && cargo test -p woc-sim --lib class_identity_snapshot_roundtrip -q`

Expected: PASS. Search `sim.rs` for paladin spawn `stance_id` assertions if any expect empty string — update to `"devotion"`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/ability_effects.rs crates/woc-sim/src/combat.rs crates/woc-sim/src/sim.rs
git commit -m "feat(sim): paladin F cycles Devotion and Retribution auras"
```

---

### Task 5: Pet Bite and Firebolt

**Files:**
- Modify: `crates/woc-content/src/pets.rs`
- Modify: `crates/woc-content/src/abilities.rs` (`imp_firebolt`)
- Modify: `crates/woc-sim/src/pet/mod.rs` (`tick_one_pet`, tests)
- Test: `crates/woc-sim/src/pet/mod.rs`

**Interfaces:**
- Consumes: `PetDef`, `Combat.ability_cd`, `deal_damage`, `DT`, `MELEE_RANGE`
- Produces: `PetDef.ability_id: Option<&'static str>`; hunter `Some("wolf_bite")`; warlock `Some("imp_firebolt")`; extra hit every 6 s; imp walk-to 14 yd when Firebolting

- [ ] **Step 1: Write the failing tests**

In `pets.rs` after `PETS`, the struct change will not compile until Step 3 — write tests in `pet/mod.rs` first against the new field.

In `crates/woc-sim/src/pet/mod.rs` tests:

```rust
    #[test]
    fn hunter_pet_bites_on_cooldown() {
        let mut world = hunter_world();
        let mut events = Vec::new();
        assert!(summon_pet(&mut world, 1, &mut events));
        let pet = find_pet(&world, 1).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 10.0, -5.0)
            .expect("wolf");
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
        }
        if let Some(t) = world.get_mut::<Transform>(pet) {
            t.x = 10.0;
            t.z = -5.0;
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
            h.hp_max = 500.0;
        }
        let hp = world.get::<Health>(2).unwrap().hp;
        tick_pets(&mut world, &mut events);
        let after = world.get::<Health>(2).unwrap().hp;
        assert!(after < hp, "wolf pet should Bite in melee, {after} vs {hp}");
        assert!(
            events.iter().any(|e| matches!(
                e,
                SimEvent::Damage {
                    ability: Some(name),
                    ..
                } if name == "Wolf Bite"
            )),
            "Bite should name Wolf Bite, got {events:?}"
        );
        let hp2 = after;
        events.clear();
        tick_pets(&mut world, &mut events);
        assert_eq!(
            world.get::<Health>(2).unwrap().hp,
            hp2,
            "Bite CD should skip the next tick"
        );
    }

    #[test]
    fn warlock_imp_firebolts_at_range() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Lock", PlayerClass::Warlock, 0.0, 0.0);
        let mut events = Vec::new();
        assert!(summon_pet(&mut world, 1, &mut events));
        let pet = find_pet(&world, 1).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 12.0, 0.0)
            .expect("wolf");
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
        }
        if let Some(t) = world.get_mut::<Transform>(pet) {
            t.x = 0.0;
            t.z = 0.0;
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
            h.hp_max = 500.0;
        }
        let hp = world.get::<Health>(2).unwrap().hp;
        tick_pets(&mut world, &mut events);
        let after = world.get::<Health>(2).unwrap().hp;
        assert!(
            after < hp,
            "imp Firebolt should hit from 12 yd, {after} vs {hp}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                SimEvent::Damage {
                    ability: Some(name),
                    ..
                } if name == "Firebolt"
            )),
            "imp should name Firebolt, got {events:?}"
        );
    }
```

Use `woc_content::ability("wolf_bite").unwrap().name` if you do not want to hardcode `"Wolf Bite"` — then the test stays tied to the content table.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib hunter_pet_bites_on_cooldown warlock_imp_firebolts_at_range -q`

Expected: FAIL (compile error on `ability_id` **or** no `Wolf Bite` / `Firebolt` event because only `"pet"` white swings exist).

- [ ] **Step 3: Write minimal implementation**

`PetDef` in `pets.rs`:

```rust
pub struct PetDef {
    pub id: &'static str,
    pub name: &'static str,
    pub owner_class: PlayerClass,
    pub hp: f32,
    pub attack_damage: f32,
    pub level: u32,
    pub ability_id: Option<&'static str>,
}
```

Hunter row: `ability_id: Some("wolf_bite")`. Warlock row: `ability_id: Some("imp_firebolt")`.

Add `imp_firebolt` to `abilities.rs` (near mob abilities):

```rust
    AbilityDef {
        id: "imp_firebolt",
        name: "Firebolt",
        damage: 14.0,
        cost: 0.0,
        cooldown: 6.0,
        range: 14.0,
        cast_time: 0.0,
        min_level: 1,
        aura: None,
        effect: AbilityEffect::SpellDamage {
            school: DamageSchool::Fire,
        },
        flags: AbilityFlags::DEFAULT,
    },
```

In `tick_one_pet`, after setting `attack_tid`, resolve content:

```rust
    let pet_ability = world
        .get::<Identity>(pet_id)
        .and_then(|i| i.template_id.clone())
        .and_then(|id| woc_content::pet(&id).and_then(|d| d.ability_id))
        .and_then(woc_content::ability);
```

`Identity` is already used in tests; import it at the top of `pet/mod.rs` if the production path needs it (`use crate::ecs::components::Identity`).

White-swing block stays for melee. **Additionally**, each tick:

```rust
    if let Some(c) = world.get_mut::<Combat>(pet_id) {
        if c.ability_cd > 0.0 {
            c.ability_cd = (c.ability_cd - DT).max(0.0);
        }
    }
```

When `attack_tid` is `Some(tid)` and `pet_ability` is `Some(def)`:

- `range = if def.range > 0.0 { def.range } else { MELEE_RANGE }`
- if `dist > range * 0.85`, `step_toward` the target (same as today’s melee chase, but threshold is `range`)
- else if `Combat.ability_cd <= 0.0`:

```rust
            let dmg = def.damage + world.get::<Combat>(pet_id).map(|c| c.attack_damage).unwrap_or(0.0) * 0.35;
            if let Some(c) = world.get_mut::<Combat>(pet_id) {
                c.ability_cd = 6.0;
            }
            deal_damage(world, owner_id, tid, dmg, Some(def.name), true, events);
```

Keep the existing melee white swing when `dist <= MELEE_RANGE` (imp at 12 yd Firebolts without a white swing; wolf in melee both Bites on CD and whites on the swing timer).

For the chase: today the code always chases to melee. Change the chase threshold to `pet_ability.map(|d| if d.range > MELEE_RANGE { d.range } else { MELEE_RANGE }).unwrap_or(MELEE_RANGE)` so the imp stops at 14 yd.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --lib hunter_pet_bites_on_cooldown warlock_imp_firebolts_at_range hunter_can_summon warlock_summons_imp -q && cargo test -p woc-content --lib -q`

Expected: PASS. Fix every `PetDef {` literal in the repo (search `PetDef {`) so tests compile.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/pets.rs crates/woc-content/src/abilities.rs crates/woc-sim/src/pet/mod.rs
git commit -m "feat(sim): hunter pet Bite and warlock imp Firebolt"
```

---

### Task 6: Client HUD and paladin F

**Files:**
- Modify: `crates/woc-client/src/input.rs` (**F** match)
- Modify: `crates/woc-client/src/hud.rs` (`class_interact_hint`, HP line, tests)
- Test: `crates/woc-client/src/hud.rs`

**Interfaces:**
- Consumes: `TickSnapshot.stance_id`, `progress.class_id`
- Produces: paladin **F** sends `CycleStance`; HP line suffix from spec §6.5; dynamic `[F] Battle` / `[F] Devotion` / `[F] Ghost Wolf` hints

- [ ] **Step 1: Write the failing HUD tests**

Replace `warrior_and_druid_action_bar_hint_f_key` expectations so warrior with empty `stance_id` still shows `[F]` and a named stance when filled:

```rust
    #[test]
    fn warrior_and_druid_action_bar_hint_f_key() {
        let mut snap = chrome_snapshot();
        snap.progress.class_id = "warrior".into();
        snap.ability_bar = vec![woc_protocol::AbilityBarSlot {
            slot: 1,
            ability_id: "heroic_strike".into(),
            name: "Heroic Strike".into(),
            known: true,
            ready: true,
            cooldown: 0.0,
        }];
        snap.stance_id = "battle".into();
        assert!(format_action_bar(&snap).contains("[F] Battle"));
        snap.stance_id = "defensive".into();
        assert!(format_action_bar(&snap).contains("[F] Defensive"));
        snap.progress.class_id = "paladin".into();
        snap.stance_id = "devotion".into();
        assert!(format_action_bar(&snap).contains("[F] Devotion"));
        snap.stance_id = "retribution".into();
        assert!(format_action_bar(&snap).contains("[F] Retribution"));
        snap.progress.class_id = "druid".into();
        snap.stance_id.clear();
        assert!(format_action_bar(&snap).contains("[F] Form"));
        snap.stance_id = "travel_form".into();
        assert!(format_action_bar(&snap).contains("[F] Travel Form"));
        snap.progress.class_id = "shaman".into();
        snap.stance_id = "ghost_wolf".into();
        assert!(format_action_bar(&snap).contains("[F] Ghost Wolf"));
        snap.progress.class_id = "mage".into();
        snap.stance_id.clear();
        let mage = format_action_bar(&snap);
        assert!(!mage.contains("[F]"));
    }
```

Find the HP-line formatter test if one exists (`HP {:.0}`). If none, add in the same `#[cfg(test)]` as `format_action_bar`:

```rust
    fn stance_label(stance_id: &str) -> &str {
        match stance_id {
            "battle" => "Battle",
            "defensive" => "Defensive",
            "devotion" => "Devotion",
            "retribution" => "Retribution",
            "ghost_wolf" => "Ghost Wolf",
            "travel_form" => "Travel Form",
            other => other,
        }
    }
```

Put `stance_label` next to `class_interact_hint` (production), not only in tests.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-client --lib warrior_and_druid_action_bar_hint_f_key -q`

Expected: FAIL (`[F] Stance` vs `[F] Battle`).

- [ ] **Step 3: Write minimal implementation**

`class_interact_hint`:

```rust
fn class_interact_hint(snap: &TickSnapshot) -> String {
    match snap.progress.class_id.as_str() {
        "rogue" => {
            if snap.stealthed {
                "   [Z] STEALTH".into()
            } else {
                "   [Z] Stealth".into()
            }
        }
        "warrior" | "paladin" | "shaman" | "druid" => {
            let label = match snap.stance_id.as_str() {
                "" if snap.progress.class_id == "warrior" => "Stance",
                "" if snap.progress.class_id == "paladin" => "Aura",
                "" => "Form",
                id => stance_label(id),
            };
            format!("   [F] {label}")
        }
        _ => String::new(),
    }
}
```

`format_action_bar` currently interpolates `hint = class_interact_hint(snap)` as `&str`. Change the caller to take `String` (`{hint}` still works).

HP line in the live HUD update (the `format!( "HP {:.0}/{:.0} ...` )`) append:

```rust
                stance = if snap.stance_id.is_empty() {
                    String::new()
                } else {
                    format!("   {}", stance_label(&snap.stance_id))
                },
```

Add `{stance}` to that format string after `{combo}`.

`input.rs` **F** match:

```rust
            "warrior" | "paladin" => host.interact(player_id, InteractAction::CycleStance),
            "shaman" | "druid" => host.interact(player_id, InteractAction::ToggleForm),
            _ => {}
        }
```

Update the comment above it to `Warrior/paladin stance-or-aura / shaman+druid form (F).`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-client --lib warrior_and_druid_action_bar_hint_f_key rogue_action_bar_hints_stealth_key -q && cargo check -p woc-client`

Expected: PASS / check green.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/hud.rs crates/woc-client/src/input.rs
git commit -m "feat(client): paint stance names and bind paladin F"
```

---

### Task 7: Docs, demo, version tag

**Files:**
- Modify: `VERSION.toml` (`rewrite_version = "1.25.0"`, `parity_target = "class-depth"`)
- Modify: `Cargo.toml` workspace `version`
- Modify: `crates/woc-version/src/lib.rs` (and any test locking `1.24.0`)
- Modify: `UPSTREAM.md`, `README.md`, `CHANGELOG.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`

**Interfaces:**
- Consumes: Tasks 1–6 green
- Produces: rewrite `1.25.0` / `class-depth`; STATUS class-depth table all `done` except trainer `n/a` / kept confirmation

- [x] **Step 1: Lock version strings**

`VERSION.toml`:

```toml
rewrite_version = "1.25.0"
parity_target = "class-depth"
```

Workspace `Cargo.toml` `version = "1.25.0"`. Search `1.24.0` in `crates/woc-version` and crate tests that assert the rewrite string; bump those that lock the *current* rewrite, not historical changelog examples.

- [x] **Step 2: STATUS / ROADMAP / DEMO / CHANGELOG**

ROADMAP: mark `1.25.0` shipped `class-depth` with one-line theme from the spec goal.

STATUS: new **Class depth (`class-depth`) — done** table:

| Subsystem | Status | Notes |
| --- | --- | --- |
| Energy 10/s | done | in and out of combat |
| Mana 8/s OOC, 2/s combat | done | |
| Rage decay 3/s OOC | done | taken + swing gain kept |
| Five-slot kits | done | rogue Sprint; hunter Multi-Shot; priest SW:P; mage Counterspell |
| Paladin F auras | done | Devotion / Retribution; spawn `devotion` |
| HUD stance/form | done | HP line + `[F]` label |
| Pet Bite / Firebolt | done | 6 s CD; imp 14 yd |
| Protocol | done | Rev 10 unchanged |
| Ability ranks / trainer | n/a | Confirmation toast kept |

DEMO: add step 21:

```
21. Rogue energy fills in ~10 s; **5** Sprint. Paladin **F** Devotion ↔ Retribution. Hunter pet Bites; warlock imp Firebolts from 14 yd. Mage **3** Counterspell. Footer `WoC-rs 1.25.0 · upstream 0.31.0`.
```

CHANGELOG `## 1.25.0` Added list matching STATUS.

README “What works in 1.25.0” one paragraph. Footer badge `rewrite-1.25.0`.

- [x] **Step 3: Run the full gate**

Run: `cargo test --workspace --exclude woc-client && cargo check -p woc-client && cargo test -p woc-sim --lib tick_phase_order_fingerprint_locked -q`

Expected: PASS. Fingerprint `3214741777866168171u64`. `PROTOCOL_REV == 10`.

- [x] **Step 4: Commit**

```bash
git add VERSION.toml Cargo.toml crates/woc-version UPSTREAM.md README.md CHANGELOG.md docs/ROADMAP.md docs/parity/STATUS.md docs/parity/DEMO.md
git commit -m "docs: tag 1.25.0 class-depth"
```

---

## Self-review

**Spec coverage:** §6.1 regen → Task 1. §6.2 kits → Task 2. §6.3 Sprint → Task 3. §6.4 paladin F → Task 4. §6.5 HUD → Task 6. §6.6 pets → Task 5. §6.7 trainer unchanged (no task). §6.8 copy in Task 4 toasts. §6.9 test names all appear. Version map → Task 7.

**Placeholder scan:** no TBD / “add tests later” / “similar to Task N” without code.

**Type consistency:** `ENERGY_REGEN_PER_SEC` / `MANA_REGEN_OOC_PER_SEC` / `MANA_REGEN_COMBAT_PER_SEC` / `RAGE_DECAY_OOC_PER_SEC`; `sprint` / `retribution_aura` / `imp_firebolt`; `PetDef.ability_id`; paladin `stance_id` `"devotion"` | `"retribution"`; toasts `Devotion Aura.` / `Retribution Aura.`

**Order:** Task 3 (Sprint ability) before Task 2 rogue slot 5 if compiling mid-task. Task 4 independent of kits. Task 5 independent of HUD. Task 6 after Task 4 so paladin `stance_id` exists. Task 7 last.
