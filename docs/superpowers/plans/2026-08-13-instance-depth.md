# Instance Depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make 5-man dungeons playable from the Bevy client (`1.22.0` / `dungeon-depth`), then isolate delves onto unique instance keys (`1.23.0` / `delve-depth`).

**Architecture:** Keep `InstanceAt` as the only per-actor instance column. Reuse existing `InteractAction` verbs. Client **E** at a content entrance sends enter/leave; sim enforces the 5-yard gate, party-shared dungeon keys, and parent-zone eject. Delves become `{id}#{seq}` like dungeons and stop wiping the overworld.

**Tech Stack:** Rust 2021 workspace crates (`woc-protocol`, `woc-content`, `woc-sim`, `woc-client`). No new crates. No Bevy inside sim. Server routing already exists.

**Design spec:** [`docs/superpowers/specs/2026-08-13-instance-depth-design.md`](../specs/2026-08-13-instance-depth-design.md)

## Global Constraints

- `woc-sim` and `woc-content` MUST NOT depend on Bevy, `bevy_ecs`, wgpu, axum, or tokio.
- Client never decides enter/leave/advance/loot. Distance is enforced in sim.
- All timers are sim ticks. No wall clock. No empty-instance TTL (last player leave already despawns).
- Tick fingerprint must remain `3214741777866168171u64`. Delve auto-advance is an unlabeled hook after kill rewards, like `expire_invites`. No new named phase.
- `PROTOCOL_REV` stays `10`. New snapshot fields are `#[serde(default)]`.
- Upstream pin stays `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- English-only player-facing strings (exact copies from the spec §6.8).
- Do not add a new ECS column. Do not reintroduce a fat `Entity`. Pets get the existing `InstanceAt` insert.
- Do not persist `instance_id` on `CharacterSave`. Load ejects to the parent-zone entrance.
- If `develop` has already used `1.22.0` for another wave, shift both tags by one. Do not reuse `1.21.0` (`mounts`).
- Every task ends with `cargo test --workspace --exclude woc-client` green, and `cargo check -p woc-client` green when client files change.
- Do not bump workspace `version` / `VERSION.toml` until the matching implementation wave is ready to tag (Task 7 and Task 11).
- Existing `enter_dungeon` tests spawn at `(2,4)` / `(0,0)`. After Task 3 they MUST teleport to `def.entrance_*` first.

---

## File map (create / own)

| Path | Responsibility |
| --- | --- |
| `crates/woc-protocol/src/lib.rs` | Additive `instance_id` / `instance_name` / `delve_room` on `TickSnapshot` |
| `crates/woc-sim/src/instances/mod.rs` | Range gate, leave-to-entrance, unified leave, pet follow hook, toasts |
| `crates/woc-sim/src/delves/mod.rs` | Unique keys, no world wipe, key-scoped despawn (1.23.0) |
| `crates/woc-sim/src/sim.rs` | Snapshot fill, `snapshot_includes_entity` fix, delve auto-advance hook |
| `crates/woc-sim/src/spirit.rs` | Parent-zone graveyard after instance eject |
| `crates/woc-sim/src/persist_state.rs` | Load eject `instance:` / `delve:` to parent entrance |
| `crates/woc-sim/src/ecs/spawn.rs` | `create_pet` inserts `InstanceAt` and copies owner |
| `crates/woc-sim/src/zones.rs` | `follow_owner_into_instance` at end of `load_overworld_zone_at` |
| `crates/woc-content/src/delves.rs` | Hollow entrance `(8, -6)` (1.23.0) |
| `crates/woc-client/src/input.rs` | **E** enter/leave/delve dispatch |
| `crates/woc-client/src/hud.rs` | Zone line uses `instance_name` |
| `crates/woc-client/src/world_setup.rs` | Help text |
| `docs/parity/{STATUS,DEMO}.md`, `docs/ROADMAP.md`, `CHANGELOG.md`, `README.md`, `VERSION.toml`, `UPSTREAM.md`, `crates/woc-version/src/lib.rs` | Version rows |

---

### Task 1: Additive snapshot fields

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`

**Interfaces:**
- Consumes: existing `TickSnapshot`
- Produces: `instance_id: String`, `instance_name: String`, `delve_room: Option<u32>` on `TickSnapshot`, all `#[serde(default)]`. `PROTOCOL_REV` remains `10`.

- [ ] **Step 1: Write the failing protocol tests**

In `tick_snapshot_old_json_defaults_new_fields` add:

```rust
        assert!(snap.instance_id.is_empty());
        assert!(snap.instance_name.is_empty());
        assert!(snap.delve_room.is_none());
```

Add a dedicated test next to it:

```rust
    #[test]
    fn instance_snapshot_fields_default_and_roundtrip() {
        let snap: TickSnapshot = serde_json::from_str(minimal_tick_json()).unwrap();
        assert!(snap.instance_id.is_empty());
        assert!(snap.instance_name.is_empty());
        assert!(snap.delve_room.is_none());
        assert_eq!(PROTOCOL_REV, 10);

        let mut filled = snap.clone();
        filled.instance_id = "eastbrook_crypt#3".into();
        filled.instance_name = "Eastbrook Crypt".into();
        filled.delve_room = Some(1);
        let s = serde_json::to_string(&filled).unwrap();
        let back: TickSnapshot = serde_json::from_str(&s).unwrap();
        assert_eq!(back.instance_id, "eastbrook_crypt#3");
        assert_eq!(back.instance_name, "Eastbrook Crypt");
        assert_eq!(back.delve_room, Some(1));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-protocol instance_snapshot_fields_default_and_roundtrip --offline`

Expected: FAIL compiling (`instance_id` not found) or the new asserts panic.

- [ ] **Step 3: Add the fields**

On `TickSnapshot` immediately after `pub zone_id: String,`:

```rust
    /// Unique instance key (`eastbrook_crypt#12`). Empty when overworld.
    #[serde(default)]
    pub instance_id: String,
    /// Content display name (`Eastbrook Crypt`). Empty when overworld.
    #[serde(default)]
    pub instance_name: String,
    /// 0-based delve room when inside a delve.
    #[serde(default)]
    pub delve_room: Option<u32>,
```

Append to the rev-10 comment:

```rust
/// Instance snapshot fields (1.22.0) are additive on rev 10.
```

Any `TickSnapshot { ... }` struct literal in this file must set the three new fields (`String::new()`, `String::new()`, `None`) or the crate will not compile. Update every literal the compiler lists.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-protocol --offline`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-protocol/src/lib.rs
git commit -m "feat(protocol): additive instance snapshot fields on rev 10"
```

---

### Task 2: Snapshot isolation + pet InstanceAt

**Files:**
- Modify: `crates/woc-sim/src/sim.rs` (`snapshot_includes_entity`, `snapshot_for_player`)
- Modify: `crates/woc-sim/src/ecs/spawn.rs` (`create_pet`)

**Interfaces:**
- Consumes: Task 1 snapshot fields; `InstanceAt`; `Owner`
- Produces: no cross-instance `entities`; pets spawn with `InstanceAt` copied from owner; snapshot fills `instance_id` / `instance_name` / `delve_room`

- [ ] **Step 1: Write the failing tests**

In `crates/woc-sim/src/sim.rs` tests, add:

```rust
    #[test]
    fn snapshot_hides_other_instance_players_and_mobs() {
        let mut sim = Sim::new_eastbrook("A", PlayerClass::Warrior);
        crate::ecs::spawn::create_player(&mut sim.world, 2, "B", PlayerClass::Mage, 3.0, 4.0);
        let parties = crate::social::party::PartyRoster::new();
        let mut events = Vec::new();
        let crypt = woc_content::dungeon("eastbrook_crypt").unwrap();
        if let Some(t) = sim.world.get_mut::<Transform>(sim.player_id) {
            t.x = crypt.entrance_x;
            t.z = crypt.entrance_z;
        }
        assert!(crate::instances::enter_dungeon(
            &mut sim.world,
            &parties,
            sim.player_id,
            "eastbrook_crypt",
            &mut events
        ));
        let snap = sim.snapshot_for_player(2);
        assert!(snap
            .entities
            .iter()
            .all(|e| e.id != sim.player_id || e.kind != woc_protocol::EntityKind::Player
                || {
                    // B is overworld: must not see A's instanced body
                    !snap.entities.iter().any(|e| e.id == sim.player_id)
                }));
        assert!(!snap.entities.iter().any(|e| {
            e.template_id.as_deref() == Some("crypt_warden")
        }));
    }
```

If `enter_dungeon` still has no range gate, the test can enter from spawn — keep the teleport anyway so Task 3 does not break it.

In `crates/woc-sim/src/pet/mod.rs` tests add:

```rust
    #[test]
    fn summoned_pet_receives_instance_at_column() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hunt", PlayerClass::Hunter, 2.0, 4.0);
        let mut events = Vec::new();
        assert!(summon_pet(&mut world, 1, &mut events));
        let pet = find_pet(&world, 1).unwrap();
        assert!(world.get::<crate::ecs::components::InstanceAt>(pet).is_some());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim snapshot_hides_other_instance_players_and_mobs summoned_pet_receives_instance_at_column --offline`

Expected: FAIL (`InstanceAt` missing on pet, and/or overworld snapshot still lists the warden or player A).

- [ ] **Step 3: Implement isolation + pet column + snapshot fill**

Replace `snapshot_includes_entity` match with:

```rust
    match (viewer_instance, entity_instance) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
```

In `snapshot_for_player`, after `zone_id` is set, fill:

```rust
            instance_id: world
                .get::<InstanceAt>(player_id)
                .and_then(|i| i.instance_id.clone())
                .unwrap_or_default(),
            instance_name: {
                let key = world
                    .get::<InstanceAt>(player_id)
                    .and_then(|i| i.instance_id.clone())
                    .unwrap_or_default();
                let content_id = crate::instances::dungeon_id_from_instance(&key);
                woc_content::dungeon(content_id)
                    .map(|d| d.name.to_string())
                    .or_else(|| woc_content::delve(content_id).map(|d| d.name.to_string()))
                    .unwrap_or_default()
            },
            delve_room: world
                .get::<InstanceAt>(player_id)
                .and_then(|i| i.delve_room),
```

Fix every `TickSnapshot {` literal in `woc-sim` the compiler reports.

In `create_pet`, after `Owner` insert:

```rust
    world.insert(id, InstanceAt::default());
    if let Some(owner_inst) = world.get::<InstanceAt>(owner_id).cloned() {
        if let Some(slot) = world.get_mut::<InstanceAt>(id) {
            *slot = owner_inst;
        }
    }
    if let Some(zone) = world
        .get::<Identity>(owner_id)
        .map(|i| i.zone_id.clone())
    {
        if let Some(identity) = world.get_mut::<Identity>(id) {
            identity.zone_id = zone;
        }
    }
```

`create_pet` already imports `InstanceAt` via the spawn module's use list — add it if missing.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim snapshot_hides_other_instance_players_and_mobs summoned_pet_receives_instance_at_column --offline`

Expected: PASS. Then `cargo test --workspace --exclude woc-client --offline`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/sim.rs crates/woc-sim/src/ecs/spawn.rs crates/woc-sim/src/pet/mod.rs
git commit -m "fix(sim): hide cross-instance actors and tag pets with InstanceAt"
```

---

### Task 3: Dungeon range gate, leave-to-entrance, pet follow, toasts

**Files:**
- Modify: `crates/woc-sim/src/instances/mod.rs`
- Modify: `crates/woc-sim/src/zones.rs` (pet follow at end of `load_overworld_zone_at`)

**Interfaces:**
- Consumes: `INSTANCE_ENTER_RANGE = 5.0`; `DungeonDef.entrance_*`
- Produces: `follow_owner_into_instance(world, player_id)`; range-gated `enter_dungeon`; `leave_instance` lands on entrance; toasts from spec §6.8

- [ ] **Step 1: Write the failing tests**

In `crates/woc-sim/src/instances/mod.rs` tests, add a helper used by every enter:

```rust
    fn place_at_dungeon(world: &mut World, player_id: EntityId, dungeon_id: &str) {
        let def = woc_content::dungeon(dungeon_id).unwrap();
        if let Some(t) = world.get_mut::<Transform>(player_id) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        if let Some(h) = world.get_mut::<Health>(player_id) {
            h.level = def.min_level.max(h.level);
        }
    }
```

Call `place_at_dungeon` at the start of every existing `enter_dungeon(...)` test (after `create_player`, before enter). Then add:

```rust
    #[test]
    fn enter_rejects_when_too_far() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Far", PlayerClass::Warrior, 2.0, 4.0);
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(!enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "You must be closer to the entrance."
        )));
        assert!(world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
    }

    #[test]
    fn enter_rejects_low_level_at_barrow_entrance() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Low", PlayerClass::Warrior, 25.0, 430.0);
        place_at_dungeon(&mut world, 1, "mirefen_barrow");
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 1;
        }
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(!enter_dungeon(
            &mut world,
            &parties,
            1,
            "mirefen_barrow",
            &mut events
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message }
                if message == "You must be level 3 to enter Mirefen Barrow."
        )));
    }

    #[test]
    fn leave_lands_on_crypt_entrance_not_zone_spawn() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        events.clear();
        assert!(leave_instance(&mut world, 1, &mut events));
        let t = world.get::<Transform>(1).unwrap();
        let def = woc_content::dungeon("eastbrook_crypt").unwrap();
        assert!((t.x - def.entrance_x).abs() < 1e-3);
        assert!((t.z - def.entrance_z).abs() < 1e-3);
        assert_eq!(world.get::<Identity>(1).unwrap().zone_id, "eastbrook");
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Left the instance."
        )));
    }

    #[test]
    fn hunter_pet_follows_into_crypt() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hunt", PlayerClass::Hunter, 2.0, 4.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        let mut events = Vec::new();
        assert!(crate::pet::summon_pet(&mut world, 1, &mut events));
        let pet = crate::pet::find_pet(&world, 1).unwrap();
        let parties = PartyRoster::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        let key = world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        assert_eq!(
            world
                .get::<InstanceAt>(pet)
                .and_then(|i| i.instance_id.clone())
                .as_deref(),
            Some(key.as_str())
        );
    }
```

Change `leave_returns_to_overworld_spawn_and_removes_boss` assertions: `t.x/t.z` must equal Crypt entrance, **not** `EASTBROOK.player_spawn_*`. Keep boss-despawn and `InstanceLeft` asserts. Same for `enter_mirefen_barrow_and_leave_returns_to_mirefen`: land on Barrow entrance, zone still `mirefen`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim enter_rejects_when_too_far leave_lands_on_crypt_entrance_not_zone_spawn --offline`

Expected: FAIL (far enter still succeeds; leave still uses zone spawn).

- [ ] **Step 3: Implement**

In `instances/mod.rs`:

```rust
pub const INSTANCE_ENTER_RANGE: f32 = 5.0;

pub fn follow_owner_into_instance(world: &mut World, player_id: EntityId) {
    let Some(pet) = crate::pet::find_pet(world, player_id) else {
        return;
    };
    let inst = world.get::<InstanceAt>(player_id).cloned().unwrap_or_default();
    let zone = world
        .get::<Identity>(player_id)
        .map(|i| i.zone_id.clone())
        .unwrap_or_default();
    let (px, pz) = world
        .get::<Transform>(player_id)
        .map(|t| (t.x, t.z))
        .unwrap_or((0.0, 0.0));
    if world.get::<InstanceAt>(pet).is_none() {
        world.insert(pet, InstanceAt::default());
    }
    if let Some(slot) = world.get_mut::<InstanceAt>(pet) {
        *slot = inst;
    }
    if let Some(identity) = world.get_mut::<Identity>(pet) {
        identity.zone_id = zone;
    }
    if let Some(t) = world.get_mut::<Transform>(pet) {
        t.x = px + 1.5;
        t.z = pz;
        t.y = crate::ecs::spawn::ground_at(t.x, t.z);
    }
}

fn xz_dist(world: &World, player_id: EntityId, x: f32, z: f32) -> f32 {
    world
        .get::<Transform>(player_id)
        .map(|t| {
            let dx = t.x - x;
            let dz = t.z - z;
            (dx * dx + dz * dz).sqrt()
        })
        .unwrap_or(f32::MAX)
}
```

At the top of `enter_dungeon`, after the existing player-kind check, replace the silent `level < min || in_instance` with:

```rust
    if in_instance {
        events.push(SimEvent::Toast {
            message: "You are already in an instance.".into(),
        });
        return false;
    }
    if level < def.min_level {
        events.push(SimEvent::Toast {
            message: format!(
                "You must be level {} to enter {}.",
                def.min_level, def.name
            ),
        });
        return false;
    }
    if xz_dist(world, player_id, def.entrance_x, def.entrance_z) > INSTANCE_ENTER_RANGE {
        events.push(SimEvent::Toast {
            message: "You must be closer to the entrance.".into(),
        });
        return false;
    }
```

If `dungeon(dungeon_id)` is `None`:

```rust
        events.push(SimEvent::Toast {
            message: "There is no such instance.".into(),
        });
```

After a successful teleport / `InstanceAt` write, call `follow_owner_into_instance(world, player_id)` and push `Entered {def.name}.`

Change `leave_instance` overworld load from `load_overworld_zone` to:

```rust
    if !crate::zones::load_overworld_zone_at(
        world,
        player_id,
        def.zone_id,
        def.entrance_x,
        def.entrance_z,
    ) {
        return false;
    }
```

(`load_overworld_zone_at` is `pub(crate)` — already visible to instances.) After a successful leave, if `follow_owner` is not yet inside `load_overworld_zone_at`, call it here and toast `Left the instance.`

In `zones.rs` at the end of `load_overworld_zone_at`, before `true`:

```rust
    crate::instances::follow_owner_into_instance(world, player_id);
```

This covers hearth and leave. `follow_owner_into_instance` must not create a module cycle: `instances` already uses `zones::load_overworld_zone`. Putting the helper in `instances` and calling it from `zones` is fine (zones → instances → zones only via functions, not module init). If the compiler reports a cycle, move `follow_owner_into_instance` into `pet/mod.rs` instead and call it from both.

Also place-at-entrance every other `enter_dungeon` call site the workspace tests use (`sim.rs` `enter_dungeon_dismounts_mounted_player`, etc.). Search: `rg "enter_dungeon\(" crates`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --workspace --exclude woc-client --offline`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/instances/mod.rs crates/woc-sim/src/zones.rs crates/woc-sim/src/sim.rs
git commit -m "feat(sim): dungeon enter range, leave-to-entrance, pet follow"
```

---

### Task 4: Death release uses parent-zone graveyard

**Files:**
- Modify: `crates/woc-sim/src/spirit.rs`
- Modify: `crates/woc-sim/src/instances/mod.rs` (export a parent-zone helper if needed)

**Interfaces:**
- Consumes: `leave_instance` / `load_overworld_zone_at`; `graveyard_for_zone`
- Produces: `release_spirit` ejects instance then lands on parent GY

- [ ] **Step 1: Write the failing test**

In `crates/woc-sim/src/spirit.rs` tests:

```rust
    #[test]
    fn release_in_barrow_uses_mirefen_graveyard() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 25.0, 430.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 3;
        }
        let def = woc_content::dungeon("mirefen_barrow").unwrap();
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        let parties = crate::social::party::PartyRoster::new();
        let mut events = Vec::new();
        assert!(crate::instances::enter_dungeon(
            &mut world,
            &parties,
            1,
            "mirefen_barrow",
            &mut events
        ));
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 0.0;
            h.alive = false;
        }
        assert!(release_spirit(&mut world, 1, &mut events));
        assert!(world
            .get::<crate::ecs::components::InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
        let gy = woc_content::graveyard("mirefen_graveyard").unwrap();
        let t = world.get::<Transform>(1).unwrap();
        assert!((t.x - gy.x).abs() < 1e-3);
        assert!((t.z - gy.z).abs() < 1e-3);
        assert_eq!(world.get::<crate::ecs::components::Identity>(1).unwrap().zone_id, "mirefen");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim release_in_barrow_uses_mirefen_graveyard --offline`

Expected: FAIL (lands on eastbrook graveyard).

- [ ] **Step 3: Implement**

Add to `instances/mod.rs`:

```rust
pub fn parent_zone_for_instance_key(instance_id: &str) -> Option<&'static str> {
    let content_id = dungeon_id_from_instance(instance_id);
    dungeon(content_id)
        .map(|d| d.zone_id)
        .or_else(|| woc_content::delve(content_id).map(|d| d.zone_id))
}
```

`release_spirit`:

```rust
    let instance_key = world
        .get::<crate::ecs::components::InstanceAt>(player_id)
        .and_then(|i| i.instance_id.clone());
    let parent_zone = instance_key
        .as_deref()
        .and_then(crate::instances::parent_zone_for_instance_key)
        .map(|s| s.to_string())
        .or_else(|| {
            world
                .get::<crate::ecs::components::Identity>(player_id)
                .map(|i| i.zone_id.clone())
        })
        .unwrap_or_else(|| "eastbrook".into());

    if instance_key.is_some() {
        let _ = crate::instances::leave_instance(world, player_id, events);
    }

    let gy = graveyard_for_zone(&parent_zone)
        .or_else(|| graveyard(DEFAULT_GRAVEYARD_ID))
        .unwrap_or_else(|| GRAVEYARDS fallback as today);
```

Then teleport / revive as today. `leave_instance` already sets zone + entrance; overwrite Transform with GY coords afterwards. Import `Identity` / `InstanceAt` in `spirit.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim release_in_barrow_uses_mirefen_graveyard release_after_death_lands_on_eastbrook_graveyard --offline`

Expected: PASS. Full workspace exclude client PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/spirit.rs crates/woc-sim/src/instances/mod.rs
git commit -m "feat(sim): instance death release uses parent-zone graveyard"
```

---

### Task 5: Persist ejects to parent entrance

**Files:**
- Modify: `crates/woc-sim/src/persist_state.rs`

**Interfaces:**
- Consumes: `dungeon()` / `delve()` / `dungeon_id_from_instance`
- Produces: load of `instance:*` / `delve:*` → parent zone + entrance xz, `InstanceAt` cleared

- [ ] **Step 1: Write the failing test**

In `persist_state.rs` tests (or `sim.rs` if that is where apply is covered), add:

```rust
    #[test]
    fn apply_barrow_zone_ejects_to_mirefen_entrance() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        let mut state = crate::persist_state::export_player_state(&world, 1).unwrap();
        state.zone_id = "instance:mirefen_barrow".into();
        state.pos_x = 40.0;
        state.pos_z = 445.0;
        state.level = 3;
        state.xp = 10;
        crate::persist_state::apply_player_state(&mut world, 1, &state);
        assert_eq!(world.get::<Identity>(1).unwrap().zone_id, "mirefen");
        let def = woc_content::dungeon("mirefen_barrow").unwrap();
        let t = world.get::<Transform>(1).unwrap();
        assert!((t.x - def.entrance_x).abs() < 1e-3);
        assert!((t.z - def.entrance_z).abs() < 1e-3);
        assert!(world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
    }
```

If `is_virgin()` still true, dirty another field `is_virgin` checks (inventory/equipment). `level = 3` plus `xp = 10` is enough on current `is_virgin`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim apply_barrow_zone_ejects_to_mirefen_entrance --offline`

Expected: FAIL (zone becomes `eastbrook`).

- [ ] **Step 3: Implement**

Add this helper in `persist_state.rs` and use it where the current code forces `eastbrook`:

```rust
fn eject_instance_zone(zone_id: &str) -> (String, Option<(f32, f32)>) {
    let content_id = zone_id
        .split(':')
        .nth(1)
        .unwrap_or(zone_id);
    let content_id = crate::instances::dungeon_id_from_instance(content_id);
    if let Some(def) = woc_content::dungeon(content_id) {
        return (canonical_parent_zone(def.zone_id), Some((def.entrance_x, def.entrance_z)));
    }
    if let Some(def) = woc_content::delve(content_id) {
        return (canonical_parent_zone(def.zone_id), Some((def.entrance_x, def.entrance_z)));
    }
    ("eastbrook".into(), None)
}

fn canonical_parent_zone(zone_id: &str) -> String {
    match zone_id {
        "eastbrook" | "eastbrook_vale" => "eastbrook".into(),
        "mirefen" => "mirefen".into(),
        "eastfen" | "fenbridge" | "mirefen_marsh" => "eastfen".into(),
        "thornpeak" | "thornpeak_heights" | "highwatch" => "thornpeak".into(),
        other => other.to_string(),
    }
}
```

```rust
    let mut pos_x = state.pos_x;
    let mut pos_z = state.pos_z;
    if zone_id.starts_with("instance:") || zone_id.starts_with("delve:") {
        let (parent, entrance) = eject_instance_zone(&zone_id);
        zone_id = parent;
        if let Some((x, z)) = entrance {
            pos_x = x;
            pos_z = z;
        }
    }
```

Write `Identity.zone_id = zone_id` and `Transform` from `pos_x` / `pos_z`. Keep clearing `InstanceAt`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim apply_barrow_zone_ejects_to_mirefen_entrance --offline`

Expected: PASS. Workspace exclude client PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/persist_state.rs
git commit -m "feat(sim): persist ejects instance zones to parent entrance"
```

---

### Task 6: Bevy E enter/leave + HUD name

**Files:**
- Modify: `crates/woc-client/src/input.rs`
- Modify: `crates/woc-client/src/hud.rs`
- Modify: `crates/woc-client/src/world_setup.rs`

**Interfaces:**
- Consumes: `TickSnapshot.instance_id` / `instance_name`; `woc_content::{DUNGEONS, dungeon}`; `INSTANCE_ENTER_RANGE` (re-declare `5.0` in the client or `pub use` from sim — client already depends on `woc-sim` / `woc-content`. Use `woc_sim::instances::INSTANCE_ENTER_RANGE` if you `pub use` it from `woc-sim/src/lib.rs`, else duplicate `const ENTER_RANGE: f32 = 5.0` in `input.rs` to avoid expanding sim's public surface. Prefer `pub use crate::instances::INSTANCE_ENTER_RANGE` from `woc-sim/src/lib.rs`.)
- Produces: **E** sends `EnterDungeon` / `LeaveInstance`; HUD shows `Eastbrook Crypt`

- [ ] **Step 1: Write the failing client unit tests**

In `crates/woc-client/src/input.rs` `#[cfg(test)]` next to `quest_interact_actions` tests, add a pure helper (do not require a live `GameHost`):

```rust
pub(crate) fn dungeon_interact_action(
    player_x: f32,
    player_z: f32,
    zone_id: &str,
    instance_id: &str,
) -> Option<InteractAction> {
    const RANGE: f32 = 5.0;
    let dist = |x: f32, z: f32| {
        let dx = player_x - x;
        let dz = player_z - z;
        (dx * dx + dz * dz).sqrt()
    };
    if !instance_id.is_empty() {
        let content_id = instance_id.split('#').next().unwrap_or(instance_id);
        if let Some(def) = woc_content::dungeon(content_id) {
            if dist(def.entrance_x, def.entrance_z) < RANGE {
                return Some(InteractAction::LeaveInstance);
            }
        }
        if let Some(def) = woc_content::delve(content_id) {
            if dist(def.entrance_x, def.entrance_z) < RANGE {
                return Some(InteractAction::LeaveInstance);
            }
        }
        return None;
    }
    for def in woc_content::DUNGEONS {
        if def.zone_id != zone_id && def.zone_id != zone_id.trim_start_matches("instance:") {
            // eastbrook crypt lives in eastbrook
        }
        let zone_ok = def.zone_id == zone_id
            || (def.zone_id == "eastbrook" && zone_id == "eastbrook")
            || (def.zone_id == "mirefen" && zone_id == "mirefen");
        if zone_ok && dist(def.entrance_x, def.entrance_z) < RANGE {
            return Some(InteractAction::EnterDungeon {
                dungeon_id: def.id.to_string(),
            });
        }
    }
    None
}

#[test]
fn e_at_crypt_entrance_enters() {
    let crypt = woc_content::dungeon("eastbrook_crypt").unwrap();
    let action = dungeon_interact_action(crypt.entrance_x, crypt.entrance_z, "eastbrook", "");
    assert_eq!(
        action,
        Some(InteractAction::EnterDungeon {
            dungeon_id: "eastbrook_crypt".into()
        })
    );
}

#[test]
fn e_at_spawn_does_not_enter_crypt() {
    assert!(dungeon_interact_action(2.0, 4.0, "eastbrook", "").is_none());
}

#[test]
fn e_inside_crypt_at_entrance_leaves() {
    let crypt = woc_content::dungeon("eastbrook_crypt").unwrap();
    let action = dungeon_interact_action(
        crypt.entrance_x,
        crypt.entrance_z,
        "instance:eastbrook_crypt",
        "eastbrook_crypt#9",
    );
    assert_eq!(action, Some(InteractAction::LeaveInstance));
}
```

`InteractAction` must be `PartialEq` — it already is if derived; if not, match instead of `assert_eq!`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-client e_at_crypt_entrance_enters --offline`

Expected: FAIL (`dungeon_interact_action` missing) or the test module does not compile.

- [ ] **Step 3: Implement helper + wire E + HUD**

Keep the helper `pub(crate)` in `input.rs`. In `handle_interact_keys`, after the loot/corpse block and **before** the NPC search:

```rust
    let player = /* already have player snap */;
    if let Some(action) = dungeon_interact_action(
        player.x,
        player.z,
        &host.snapshot.zone_id,
        &host.snapshot.instance_id,
    ) {
        host.interact(host.snapshot.player_id, action);
        return;
    }
```

Do **not** send `EnterDelve` in this task.

`hud.rs` `zone_name`:

```rust
fn zone_name(snap: &TickSnapshot) -> &str {
    if !snap.instance_name.is_empty() {
        &snap.instance_name
    } else if snap.zone_id.is_empty() {
        "—"
    } else {
        &snap.zone_id
    }
}
```

`world_setup.rs` help string: insert `E dungeon` after the existing `E interact/loot` token (keep one E mention: `E interact/loot/dungeon`).

Export `INSTANCE_ENTER_RANGE` from `woc-sim/src/lib.rs` only if the helper imports it; the client helper may use a local `5.0` matching the spec.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-client e_at_crypt_entrance_enters e_at_spawn_does_not_enter_crypt e_inside_crypt_at_entrance_leaves --offline`

Expected: PASS. `cargo check -p woc-client --offline`. Workspace exclude client PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/input.rs crates/woc-client/src/hud.rs crates/woc-client/src/world_setup.rs crates/woc-sim/src/lib.rs
git commit -m "feat(client): E enters and leaves dungeons at the entrance"
```

---

### Task 7: Tag `1.22.0` / `dungeon-depth`

**Files:**
- Modify: `VERSION.toml`, `Cargo.toml` workspace.package.version, `crates/woc-version/src/lib.rs`, `UPSTREAM.md`, `README.md`, `CHANGELOG.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`

**Interfaces:**
- Consumes: Tasks 1–6
- Produces: rewrite `1.22.0` / parity `dungeon-depth`. Protocol stays 10.

- [ ] **Step 1: Update version files**

`VERSION.toml`: `rewrite_version = "1.22.0"`, `parity_target = "dungeon-depth"`.

`woc-version`: `REWRITE_VERSION = "1.22.0"`, `PARITY_TARGET = "dungeon-depth"`.

Workspace `Cargo.toml` version `1.22.0`.

`CHANGELOG.md` new top section:

```markdown
## 1.22.0 — 2026-08-14

### Added

- **1.22.0 `dungeon-depth`:** Bevy **E** at a dungeon entrance enters; **E** at the same point inside leaves to that entrance.
- Sim 5-yard enter gate; leave lands on the portal, not the zone spawn.
- Snapshot hides cross-instance players/mobs; pets copy `InstanceAt` and follow.
- Death release uses the parent-zone graveyard; persist ejects `instance:` saves to the parent entrance.
- Additive `instance_id` / `instance_name` / `delve_room` on rev **10**.
```

README badge + "What works" + footer `1.22.0`. ROADMAP table row shipped. STATUS current rewrite line + a `dungeon-depth` done table (enter/leave/isolation/pets/death/persist/client). DEMO step 5: walk to Crypt, **E** in, **E** out at entrance; two clients share one key.

Search leftover `1.21.0` product strings that should move: `rg "1\\.21\\.0"` — keep historical CHANGELOG / plan files.

- [ ] **Step 2: Run version + workspace tests**

Run: `cargo test -p woc-version --offline && cargo test --workspace --exclude woc-client --offline && cargo check -p woc-client --offline`

Expected: PASS. `tick_phase_order_fingerprint_locked` still `3214741777866168171`. `PROTOCOL_REV == 10`.

- [ ] **Step 3: Commit**

```bash
git add VERSION.toml Cargo.toml Cargo.lock crates/woc-version/src/lib.rs UPSTREAM.md README.md CHANGELOG.md docs/ROADMAP.md docs/parity/STATUS.md docs/parity/DEMO.md
git commit -m "release: 1.22.0 dungeon-depth"
```

---

### Task 8: Delve unique keys, no world wipe

**Files:**
- Modify: `crates/woc-sim/src/delves/mod.rs`

**Interfaces:**
- Consumes: `INSTANCE_ENTER_RANGE`, `follow_owner_into_instance`, `dungeon_id_from_instance`
- Produces: `{delve}#{seq}` keys; enter/advance never despawn untagged overworld actors

- [ ] **Step 1: Write the failing tests**

Replace/extend `enter_clear_advance_and_complete_grants_final_reward`:

After `create_mob_from_template(..., "young_boar", ...)` and `enter_delve`, assert the boar **still lives** (id 2 still has `Health.alive`). Assert player `instance_id` starts with `eastbrook_hollow#` (not equals `"eastbrook_hollow"`). Update `defeat_current_room` / `living_delve_mobs` to match by `dungeon_id_from_instance(key) == "eastbrook_hollow"` **or** by the full key stored on the player.

Add:

```rust
    #[test]
    fn two_players_get_distinct_hollow_keys() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "A", PlayerClass::Warrior, 8.0, -6.0);
        crate::ecs::spawn::create_player(&mut world, 2, "B", PlayerClass::Mage, 8.0, -6.0);
        // 1.23 moves entrance to (8,-6); until Task 9 the table is still (0,0).
        // Place both on the *current* table entrance so this task stays green:
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        for id in [1, 2] {
            if let Some(t) = world.get_mut::<Transform>(id) {
                t.x = def.entrance_x;
                t.z = def.entrance_z;
            }
        }
        let mut events = Vec::new();
        assert!(enter_delve(&mut world, 1, "eastbrook_hollow", &mut events));
        assert!(enter_delve(&mut world, 2, "eastbrook_hollow", &mut events));
        let a = world.get::<InstanceAt>(1).unwrap().instance_id.clone().unwrap();
        let b = world.get::<InstanceAt>(2).unwrap().instance_id.clone().unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("eastbrook_hollow#"));
        assert!(b.starts_with("eastbrook_hollow#"));
    }

    #[test]
    fn enter_delve_rejects_when_too_far() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Far", PlayerClass::Warrior, 2.0, 4.0);
        let mut events = Vec::new();
        assert!(!enter_delve(&mut world, 1, "eastbrook_hollow", &mut events));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "You must be closer to the entrance."
        )));
    }
```

Update `defeat_current_room` to take the player's full key. Update `world_host_dispatches_enter_and_advance_actions` to stand on the current entrance before `InteractAction::EnterDelve`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim two_players_get_distinct_hollow_keys enter_delve_rejects_when_too_far --offline`

Expected: FAIL (shared bare id; far enter succeeds; overworld boar gone).

- [ ] **Step 3: Implement**

`enter_delve`:

- Delete the `to_remove` full-world despawn loop.
- Add the same in-instance / level / range / unknown-id toasts as dungeon.
- Allocate `format!("{}#{}", def.id, seq)` like `enter_dungeon`.
- Do **not** call `find_party_instance`.
- Tag room mobs with the full key.
- `follow_owner_into_instance`.
- Toast `Entered {def.name}.`

`try_advance_delve`: compare mob `InstanceAt` to the player's **full** key. When clearing, only despawn Mob/Loot whose `instance_id` equals that key. On complete, grant rewards then call `leave_instance` (Task 3 dungeon-only leave will fail for delve until the next step — implement delve branch in `leave_instance` **in this task**):

```rust
    if let Some(def) = dungeon(dungeon_id) {
        // existing entrance teleport
    } else if let Some(def) = woc_content::delve(dungeon_id) {
        if !crate::zones::load_overworld_zone_at(
            world, player_id, def.zone_id, def.entrance_x, def.entrance_z,
        ) {
            return false;
        }
    } else {
        return false;
    }
```

Complete path: rewards first, then `leave_instance` (which toasts `Left the instance.` — the spec also emits `DelveCompleted`. Order: rewards → `DelveCompleted` → `leave_instance` which emits `InstanceLeft` + `Left the instance.`). If `leave_instance` toasts feel noisy after a reward, still keep both; spec lists both events.

Abort `LeaveInstance` mid-delve: no copper, no greaves.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim --offline` (delve + instance modules)

Expected: PASS. Workspace exclude client PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/delves/mod.rs crates/woc-sim/src/instances/mod.rs
git commit -m "feat(sim): isolate delves with unique keys and no world wipe"
```

---

### Task 9: Move Hollow entrance + auto-advance

**Files:**
- Modify: `crates/woc-content/src/delves.rs`
- Modify: `crates/woc-sim/src/sim.rs` (`tick_all` after kill rewards)

**Interfaces:**
- Consumes: `try_advance_delve`
- Produces: entrance `(8.0, -6.0)`; `tick_all` advances a cleared room the same tick as the last kill

- [ ] **Step 1: Write the failing tests**

In `crates/woc-content/src/delves.rs` tests:

```rust
    #[test]
    fn hollow_entrance_is_away_from_eastbrook_spawn() {
        let hollow = delve("eastbrook_hollow").unwrap();
        let dx = hollow.entrance_x - 2.0;
        let dz = hollow.entrance_z - 4.0;
        assert!((dx * dx + dz * dz).sqrt() > 5.0);
        assert!((hollow.entrance_x - 8.0).abs() < 1e-3);
        assert!((hollow.entrance_z + 6.0).abs() < 1e-3);
    }
```

In `crates/woc-sim/src/delves/mod.rs` or `sim.rs`:

```rust
    #[test]
    fn tick_all_auto_advances_cleared_delve_room() {
        let mut sim = crate::Sim::new_eastbrook("Delver", PlayerClass::Warrior);
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        if let Some(t) = sim.world.get_mut::<Transform>(sim.player_id) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        let mut events = Vec::new();
        assert!(crate::delves::enter_delve(
            &mut sim.world,
            sim.player_id,
            "eastbrook_hollow",
            &mut events
        ));
        let key = sim
            .world
            .get::<InstanceAt>(sim.player_id)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        for id in sim.world.ids::<Identity>() {
            if sim.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob)
                && sim
                    .world
                    .get::<InstanceAt>(id)
                    .and_then(|i| i.instance_id.as_deref())
                    == Some(key.as_str())
            {
                if let Some(h) = sim.world.get_mut::<Health>(id) {
                    h.hp = 0.0;
                    h.alive = false;
                }
            }
        }
        let _ = sim.tick_all();
        assert_eq!(
            sim.world
                .get::<InstanceAt>(sim.player_id)
                .and_then(|i| i.delve_room),
            Some(1)
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content hollow_entrance_is_away_from_eastbrook_spawn --offline`

Expected: FAIL (entrance still `0,0`).

- [ ] **Step 3: Implement**

`DELVES[0].entrance_x = 8.0`, `entrance_z = -6.0`.

In `tick_all`, after `on_player_death_check` / kill-reward block and before `// Phase 7: pvp_and_market`:

```rust
        for &pid in &player_ids {
            if sim_player_in_delve(&self.world, pid) {
                let _ = crate::delves::try_advance_delve(&mut self.world, pid, &mut self.events);
            }
        }
```

```rust
fn sim_player_in_delve(world: &World, pid: EntityId) -> bool {
    world
        .get::<InstanceAt>(pid)
        .and_then(|i| i.delve_room)
        .is_some()
}
```

Do not touch `TICK_PHASES`.

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace --exclude woc-client --offline`

Expected: PASS. Fingerprint test PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/delves.rs crates/woc-sim/src/sim.rs crates/woc-sim/src/delves/mod.rs
git commit -m "feat(sim): move hollow entrance and auto-advance delve rooms"
```

---

### Task 10: Client E enters Hollow + HUD room

**Files:**
- Modify: `crates/woc-client/src/input.rs`
- Modify: `crates/woc-client/src/hud.rs`

**Interfaces:**
- Consumes: Task 6 helper; `woc_content::DELVES`; `delve_room`
- Produces: **E** at Hollow entrance sends `EnterDelve`; HUD `Eastbrook Hollow — Room 1`

- [ ] **Step 1: Write the failing tests**

Extend `dungeon_interact_action` (rename to `instance_interact_action` if you want; keep the old name and add delve at the end of the overworld branch):

```rust
    for def in woc_content::DELVES {
        let zone_ok = def.zone_id == zone_id || (def.zone_id == "eastbrook" && zone_id == "eastbrook");
        if zone_ok && dist(def.entrance_x, def.entrance_z) < RANGE {
            return Some(InteractAction::EnterDelve {
                delve_id: def.id.to_string(),
            });
        }
    }
```

Tests:

```rust
#[test]
fn e_at_hollow_entrance_enters_delve() {
    let hollow = woc_content::delve("eastbrook_hollow").unwrap();
    let action = dungeon_interact_action(hollow.entrance_x, hollow.entrance_z, "eastbrook", "");
    assert_eq!(
        action,
        Some(InteractAction::EnterDelve {
            delve_id: "eastbrook_hollow".into()
        })
    );
}

#[test]
fn e_at_spawn_still_does_not_enter_hollow() {
    assert!(dungeon_interact_action(2.0, 4.0, "eastbrook", "").is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-client e_at_hollow_entrance_enters_delve --offline`

Expected: FAIL (`None` because Task 6 helper has no delve branch).

- [ ] **Step 3: Implement**

Add the `DELVES` loop to the helper (after dungeons, only when `instance_id` is empty). Leave-inside-delve already works if Task 6 checks `delve(content_id)` — add that Leave branch if it is missing.

HUD `zone_name` cannot return `&str` once you format a room suffix. Change it to `String`:

```rust
fn zone_name(snap: &TickSnapshot) -> String {
    if !snap.instance_name.is_empty() {
        if let Some(room) = snap.delve_room {
            return format!("{} — Room {}", snap.instance_name, room + 1);
        }
        return snap.instance_name.clone();
    }
    if snap.zone_id.is_empty() {
        "—".into()
    } else {
        snap.zone_id.clone()
    }
}
```

Update every `zone_name(snap)` call site that expected `&str` (they already `format!` it in several panels).

Help text: `E interact/loot/dungeon/delve`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-client e_at_hollow_entrance_enters_delve e_at_spawn_still_does_not_enter_hollow --offline && cargo check -p woc-client --offline`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/input.rs crates/woc-client/src/hud.rs crates/woc-client/src/world_setup.rs
git commit -m "feat(client): E enters Eastbrook Hollow; HUD shows delve room"
```

---

### Task 11: Tag `1.23.0` / `delve-depth`

**Files:** same version/docs set as Task 7

**Interfaces:**
- Consumes: Tasks 8–10
- Produces: rewrite `1.23.0` / parity `delve-depth`

- [ ] **Step 1: Bump versions and docs**

`VERSION.toml` / `woc-version` / workspace: `1.23.0`, `delve-depth`.

CHANGELOG top:

```markdown
## 1.23.0 — 2026-08-14

### Added

- **1.23.0 `delve-depth`:** Eastbrook Hollow uses `{id}#{seq}` keys and no longer despawns the overworld.
- Two players get two Hollows. Room clears auto-advance on the kill tick.
- Hollow entrance moved to `(8, -6)`. **E** enters; Leave / Hearth / release abort with no reward.
```

STATUS `delve-depth` done table. DEMO 5 / 19: walk southeast of spawn to the Hollow portal, **E**, clear three rooms (auto-advance), land on `(8, -6)`; a second client still sees Eastbrook wolves. ROADMAP shipped rows. README footer `1.23.0`.

- [ ] **Step 2: Run verification**

Run: `cargo test --workspace --exclude woc-client --offline && cargo check -p woc-client --offline && cargo test -p woc-sim tick_phase_order_fingerprint_locked --offline`

Expected: PASS. Fingerprint `3214741777866168171`. `PROTOCOL_REV == 10`.

- [ ] **Step 3: Commit**

```bash
git add VERSION.toml Cargo.toml Cargo.lock crates/woc-version/src/lib.rs UPSTREAM.md README.md CHANGELOG.md docs/ROADMAP.md docs/parity/STATUS.md docs/parity/DEMO.md
git commit -m "release: 1.23.0 delve-depth"
```

---

## Self-review

**Spec coverage**

| Spec section | Task |
| --- | --- |
| §6.2 snapshot fields | Task 1 |
| §6.4 isolation + pet column + snapshot fill | Task 2 |
| §6.3 range / leave-to-entrance / pet follow / toasts | Task 3 |
| §6.5 death GY | Task 4 |
| §6.6 persist eject | Task 5 |
| §6.7 client E dungeon + HUD | Task 6 |
| §6.10 DoD 1.22.0 + version | Task 7 |
| §7.1 unique keys / no wipe / leave abort | Task 8 |
| §7.2 auto-advance + §7.3 entrance move | Task 9 |
| §7.4 client delve E + room HUD | Task 10 |
| §7.5 DoD 1.23.0 + version | Task 11 |
| §8 non-goals | no tasks (Finder, lockout, raid boss, portal actor, persist instance id) |

**Placeholder scan:** no TBD / “handle edge cases” / “write tests for the above”.

**Type consistency:** `INSTANCE_ENTER_RANGE = 5.0`; keys `{id}#{seq}`; toasts copied from spec §6.8; `leave_instance` serves dungeon + delve; `follow_owner_into_instance` used on enter, leave, and `load_overworld_zone_at`; `PROTOCOL_REV` stays 10; fingerprint unchanged.

## Main-agent merge playbook

1. Land Tasks 1–7 as `1.22.0`. Do not start Task 8 until Crypt **E** enter/leave and Barrow persist eject are green.
2. Land Tasks 8–11 as `1.23.0`. Do not add a third dungeon or a Dungeon Finder in this program.
3. After each wave: `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client`.
