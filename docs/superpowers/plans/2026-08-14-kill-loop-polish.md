# Kill-loop Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]` ) syntax for tracking.

**Goal:** Close remaining `1.14.0` / `kill-loop` spec and review leftovers: victim `InstanceAt` on loot, pet last-hit still drops when the owner is gone, `SimEvent::Loot.count` treats 0 as 1, portal zone RNG no longer collides on equal-length tags, and a leashed mob does not re-aggro until it reaches Home.

**Architecture:** Keep the typed sparse-column `World`. No new tick phase. Extract small helpers used by the existing `kill_rewards` / `update_mob_ai` / `load_overworld_zone_at` seams. Do not reintroduce a fat `Entity`. Do not bump `VERSION.toml` (still `1.14.0`).

**Tech Stack:** Rust 2021 workspace crates (`woc-content`, `woc-protocol`, `woc-sim`, `woc-client`). Protocol rev 8. Upstream 0.31.0. Tick fingerprint `3214741777866168171`.

**Design:** [`docs/superpowers/specs/2026-08-13-kill-loop-design.md`](../specs/2026-08-13-kill-loop-design.md) §5.3, §5.4, §5.6.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- `PROTOCOL_REV` remains **8**. New fields `#[serde(default)]`.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- All sim RNG via mulberry32 on `Sim` only; no wall clock.
- Tick-phase fingerprint stays `3214741777866168171`. Hook inside existing `kill_rewards` / `mob_ai_combat` / portal load. No new named phase.
- Client never decides combat/loot/spawn/respawn outcomes.
- English-only toasts. Locked copy: `"Loot expired."`
- New per-actor state is a field on an existing `World` column. Do **not** add a fat `Entity` or a `SpawnGroup` actor.
- Do not populate `LootTable.loot_copper` / `loot_item`.
- Do **not** bump `VERSION.toml`. This is polish on tagged `1.14.0`.
- Mob-on-mob kills still must not spawn loot.
- Before claiming done: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client`.

## File map

| Path | Responsibility |
| --- | --- |
| `crates/woc-sim/src/sim.rs` | `kill_rewards`: loot drop predicate; victim `InstanceAt` fallback |
| `crates/woc-sim/src/combat.rs` | `grant_loot_pile` emits `count.max(1)`; optional helpers if extracted here |
| `crates/woc-sim/src/zones.rs` | Portal population seed from tag bytes, not `tag.len()` |
| `crates/woc-sim/src/mob.rs` | Skip new aggro until Home (leash evade) |
| `docs/superpowers/specs/2026-08-13-kill-loop-design.md` | Status → Shipped |
| `docs/parity/STATUS.md` | Note polish leftovers closed |

---

### Task 1: Victim instance loot, pet drop, `Loot.count`

**Files:**
- Modify: `crates/woc-sim/src/sim.rs` (`kill_rewards` block ~521–586; tests near `instance_independent_loot_tags_all_piles`)
- Modify: `crates/woc-sim/src/combat.rs` (`grant_loot_pile`; tests in the same module)

**Produces:** Loot piles inherit the victim's `InstanceAt` when the credited killer has none; a pet last-hit still spawns loot if the owner entity is missing; `SimEvent::Loot.count` is never 0.

Spec quotes:

- §5.3: `kill_rewards` already copies `InstanceAt` from the killer; also copy from the **victim** when the killer has none (pet / overworld).
- §5.4: If the owner is missing or dead, skip XP (existing player check) but still spawn loot at the corpse.
- §5.6: `SimEvent::Loot` `count` of 0 means “treat as 1” for old peers.

Keep the final-review rule: hostile **mob** killers must not mint loot.

- [ ] **Step 1: Write the failing tests**

In `crates/woc-sim/src/sim.rs` tests, next to `instance_independent_loot_tags_all_piles`:

```rust
#[test]
fn loot_inherits_victim_instance_when_killer_has_none() {
    let mut sim = Sim::new_eastbrook("NoInst", PlayerClass::Warrior);
    let instance_id = "eastbrook_crypt#victim-loot".to_string();
    if let Some(i) = sim.world.get_mut::<InstanceAt>(sim.player_id) {
        i.instance_id = None;
    }
    let mob_id = sim.world.next_id();
    spawn::create_mob_from_template(&mut sim.world, mob_id, "barrow_hag", 0.0, 0.0)
        .expect("barrow_hag");
    if let Some(i) = sim.world.get_mut::<InstanceAt>(mob_id) {
        i.instance_id = Some(instance_id.clone());
    }
    if let Some(h) = sim.world.get_mut::<Health>(mob_id) {
        h.hp = 1.0;
        h.hp_max = 1.0;
    }
    place_player_at(&mut sim, 2.5, 0.0);
    if let Some(kit) = sim.world.get_mut::<ClassKit>(sim.player_id) {
        kit.resource = 100.0;
    }
    if let Some(c) = sim.world.get_mut::<Combat>(sim.player_id) {
        c.target = Some(mob_id);
        c.auto_attack = true;
    }
    let intent = PlayerIntent {
        attack: true,
        ability: Some(AbilitySlot::Primary),
        target_id: Some(mob_id),
        ..Default::default()
    };
    let mut saw_kill = false;
    for _ in 0..400 {
        let (_snap, events) = sim.tick(intent);
        if events
            .iter()
            .any(|e| matches!(e, SimEvent::Kill { victim, .. } if *victim == mob_id))
        {
            saw_kill = true;
            break;
        }
    }
    assert!(saw_kill, "barrow_hag should die in combat");
    let piles: Vec<Option<String>> = sim
        .world
        .ids::<LootPile>()
        .into_iter()
        .filter_map(|id| {
            let item = sim.world.get::<LootPile>(id)?.item.clone();
            if matches!(item.as_deref(), Some("hag_claw") | Some("hag_focus")) {
                Some(sim.world.get::<InstanceAt>(id)?.instance_id.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(!piles.is_empty(), "player kill must still drop loot");
    assert!(
        piles.iter().all(|inst| inst.as_ref() == Some(&instance_id)),
        "piles must inherit the victim's instance when the killer has none"
    );
}

#[test]
fn pet_last_hit_without_owner_still_spawns_loot() {
    let mut sim = Sim::new_eastbrook("Orphan", PlayerClass::Hunter);
    assert!(crate::pet::summon_pet(&mut sim.world, sim.player_id, &mut sim.events));
    let pet = crate::pet::find_pet(&sim.world, sim.player_id).expect("pet");
    if let Some(owner) = sim.world.get_mut::<crate::ecs::components::Owner>(pet) {
        owner.owner_id = 999_999;
    }
    let mob_id = sim.world.next_id();
    spawn::create_mob_from_template(&mut sim.world, mob_id, "young_wolf", 0.0, 0.0)
        .expect("young_wolf");
    if let Some(h) = sim.world.get_mut::<Health>(mob_id) {
        h.hp = 1.0;
        h.hp_max = 1.0;
    }
    let piles_before = sim.world.ids::<LootPile>().len();
    crate::combat::deal_damage(
        &mut sim.world,
        pet,
        mob_id,
        50.0,
        None,
        true,
        &mut sim.events,
    );
    // Drive the same kill_rewards helper tick_all uses (do not copy the if-player guard into the test).
    sim.grant_pending_kill_rewards();
    let piles_after = sim.world.ids::<LootPile>().len();
    assert!(
        piles_after > piles_before,
        "pet last-hit must still spawn corpse loot when the owner entity is missing"
    );
    assert!(
        sim.world
            .get::<crate::ecs::components::Progress>(sim.player_id)
            .is_some(),
        "living hunter is not the credited killer; XP path stays skipped"
    );
}
```

`grant_pending_kill_rewards` is a new `impl Sim` method: move the existing phase-6 reward/loot loop (from `collect_pending_mob_kills` through `maybe_start_party_roll`) into it, and call it from `tick_all`. Tests must not re-implement the drop predicate.

In `crates/woc-sim/src/combat.rs` tests:

```rust
#[test]
fn loot_event_count_treats_zero_as_one() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    let lid = crate::ecs::spawn::create_loot_ex(
        &mut world,
        50,
        0.0,
        0.0,
        3,
        Some("wolf_fang".into()),
        0,
        0,
        "eastbrook",
    );
    if let Some(p) = world.get_mut::<LootPile>(lid) {
        p.count = 0;
    }
    let mut events = Vec::new();
    assert!(grant_loot_pile(&mut world, 1, lid, &mut events));
    assert!(events.iter().any(|e| matches!(
        e,
        SimEvent::Loot { player: 1, count: 1, .. }
    )));
}
```

If `grant_loot_pile` is private, the test stays in the same module (it already hosts `young_wolf_spawn_loot_grants_count_two`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim loot_inherits_victim_instance_when_killer_has_none pet_last_hit_without_owner_still_spawns_loot loot_event_count_treats_zero_as_one -- --nocapture`

Expected: FAIL (piles have `instance_id = None`; pet kill spawns no loot; event count is 0). If a test fails to compile because `grant_pending_kill_rewards` does not exist yet, that is an acceptable RED.

- [ ] **Step 3: Write minimal implementation**

1. Extract the current `tick_all` phase-6 body into `impl Sim { pub(crate) fn grant_pending_kill_rewards(&mut self) }`. `tick_all` calls it in the same place.
2. Drop predicate: spawn loot when credited killer `Identity.kind` is `Player` **or** `Pet`. `Mob` (and anything else) still does not drop. Do not spawn loot for mob-on-mob.
3. Instance stamp:

```rust
let inst = self
    .world
    .get::<InstanceAt>(reward.killer)
    .and_then(|i| i.instance_id.clone())
    .or_else(|| {
        self.world
            .get::<InstanceAt>(reward.victim)
            .and_then(|i| i.instance_id.clone())
    });
```

Copy onto each new pile when `inst` is `Some`.
4. In `grant_loot_pile`, `let count = pile.count.max(1);` and use `count` for both `grant_item` and `SimEvent::Loot { count, .. }`.

Do not change `PROTOCOL_REV`. Do not spawn loot when the credited killer is a `Mob`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim loot_inherits_victim_instance_when_killer_has_none pet_last_hit_without_owner_still_spawns_loot loot_event_count_treats_zero_as_one mob_killer_does_not_spawn_loot instance_independent_loot_tags_all_piles pet_last_hit_credits_owner_xp tick_phase_order_fingerprint_locked -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/sim.rs crates/woc-sim/src/combat.rs
git commit -m "fix(sim): victim instance loot, pet drop, Loot.count"
```

---

### Task 2: Portal zone RNG seed

**Files:**
- Modify: `crates/woc-sim/src/zones.rs`

**Produces:** `eastfen` and `mirefen` (both length 7) no longer share a population RNG seed. `populate_all_overworld` is unchanged (it already uses the caller’s `Sim.rng`).

- [ ] **Step 1: Write the failing test** in `crates/woc-sim/src/zones.rs` tests (add a `#[cfg(test)]` module if missing; otherwise append):

```rust
#[test]
fn zone_population_seed_differs_for_equal_length_tags() {
    let a = zone_population_seed("eastfen");
    let b = zone_population_seed("mirefen");
    assert_ne!(
        a, b,
        "equal-length zone tags must not share a portal population seed"
    );
    assert_ne!(zone_population_seed("eastbrook"), a);
}
```

`zone_population_seed` is a new `pub(crate)` (or private, tests in the same module) helper.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim zone_population_seed_differs_for_equal_length_tags -- --nocapture`

Expected: FAIL to compile (helper missing) or FAIL assertion if a stub returns `WORLD_SEED.wrapping_add(tag.len() as u32)`.

- [ ] **Step 3: Write minimal implementation**

```rust
fn zone_population_seed(tag: &str) -> u32 {
    let mut h = WORLD_SEED;
    for b in tag.as_bytes() {
        h = h.wrapping_mul(16777619) ^ u32::from(*b);
    }
    if h == 0 {
        0x9e3779b9
    } else {
        h
    }
}
```

In `load_overworld_zone_at`, replace `WORLD_SEED.wrapping_add(tag.len() as u32)` with `zone_population_seed(tag)`.

Do **not** change `populate_all_overworld` seeding. While in this file, delete the redundant `Home` patch after `create_mob_from_template` in `populate_all_overworld` (the factory already sets `Home` to the spawn `x,z`). Do not retouch `ensure_zone_population` Home (it does not patch Home today).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim zone_population_seed_differs_for_equal_length_tags -- --nocapture` and `cargo test -p woc-sim --lib zones -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/zones.rs
git commit -m "fix(sim): seed portal zone population from tag bytes"
```

---

### Task 3: Leash evade until Home

**Files:**
- Modify: `crates/woc-sim/src/mob.rs`

**Produces:** After leash (or any drop-target return), the mob does not acquire a new target until it is at Home. Spec §5.2 still restores HP when `d_home > LEASH_RANGE`. This closes the leash-boundary re-pull: walking back inside 40 yd no longer re-aggros at full HP mid-path.

- [ ] **Step 1: Write the failing test** in `crates/woc-sim/src/mob.rs` tests:

```rust
#[test]
fn returning_mob_does_not_reaggro_until_home() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 2.0, 0.0);
    crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
    // Place the wolf inside leash, not at Home, with no target.
    if let Some(t) = world.get_mut::<Transform>(2) {
        t.x = 10.0;
        t.z = 0.0;
        t.y = crate::ecs::spawn::ground_at(t.x, t.z);
    }
    if let Some(c) = world.get_mut::<Combat>(2) {
        c.target = None;
    }
    update_mob_ai(&mut world, 2, 1);
    assert!(
        world.get::<Combat>(2).unwrap().target.is_none(),
        "must not acquire aggro while returning to Home"
    );
    // Snap to Home — now it may aggro.
    if let Some(t) = world.get_mut::<Transform>(2) {
        t.x = 0.0;
        t.z = 0.0;
        t.y = crate::ecs::spawn::ground_at(t.x, t.z);
    }
    update_mob_ai(&mut world, 2, 1);
    assert_eq!(world.get::<Combat>(2).unwrap().target, Some(1));
}
```

Keep existing `leash_restores_hp_and_clears_auras` / `leash_clears_target_and_returns_home_when_too_far` / `social_aggro_pulls_nearby_same_camp_ally` passing. Social aggro still requires an engager that already has a target; a wolf sitting at Home still pulls.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim returning_mob_does_not_reaggro_until_home -- --nocapture`

Expected: FAIL (`target == Some(1)` while at x=10).

- [ ] **Step 3: Write minimal implementation**

Add a private `fn at_home(world: &World, id: EntityId) -> bool` using the same 0.2 yd snap already used by `move_toward_home` / `step_toward_home`. In `update_mob_ai`, only acquire a new target when `at_home` is true (still require `d_player <= AGGRO_RANGE` and stealth visibility). Do not add a new component. Do not skip leash HP reset. Do not start a respawn timer on leash.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim returning_mob_does_not_reaggro_until_home leash_restores_hp_and_clears_auras leash_clears_target_and_returns_home_when_too_far social_aggro_pulls_nearby_same_camp_ally lost_target_returns_home_when_player_dead -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/mob.rs
git commit -m "fix(sim): leashed mobs evade until they reach Home"
```

---

### Task 4: Docs — mark leftovers closed

**Files:**
- Modify: `docs/superpowers/specs/2026-08-13-kill-loop-design.md` (Status line only)
- Modify: `docs/parity/STATUS.md` (kill-loop table notes)
- Modify: `docs/superpowers/plans/2026-08-13-kill-loop.md` is historical; do not rewrite tasks. Link this polish plan from STATUS.

**Produces:** Spec status is no longer “Proposed”. STATUS notes the polish items.

- [ ] **Step 1: Edit spec status**

Change the spec header `**Status:** Proposed (planning deliverable 2026-08-13).` to `**Status:** Shipped as rewrite 1.14.0 / kill-loop (polish 2026-08-14).`

In STATUS kill-loop table, add/adjust:

| Victim instance loot | done | Killer `InstanceAt` else victim |
| Portal zone seed | done | Tag bytes, not `tag.len()` |
| Leash evade | done | No re-aggro until Home |

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/specs/2026-08-13-kill-loop-design.md docs/parity/STATUS.md
git commit -m "docs: mark kill-loop polish leftovers shipped"
```

No `VERSION.toml` bump. No protocol rev bump.

---

## Verification (controller)

After Task 4:

```bash
cargo test --workspace --exclude woc-client
cargo check -p woc-client
```

Fingerprint test `tick_phase_order_fingerprint_locked` must still equal `3214741777866168171`.
