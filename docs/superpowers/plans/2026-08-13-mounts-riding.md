# Mounts and Riding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rewrite `1.21.0` / `mounts`: riding ranks at a stable master, three learnable mounts, **V** toggles the last mount, and flying is Expert + gryphon instead of free travel flight.

**Architecture:** Player-only `Riding` column (`rank`, `known`, `last_id`, `active_id`). Content tables `RIDING_RANKS` / `MOUNTS`. `PlayerIntent.fly_toggle` stays on the wire; `mount::toggle_mount` runs in `apply_intents_motion` before `step_player_motion`. `Motion.flying` is set only by a flying mount. Client draws a child silhouette from `EntitySnapshot.mounted`. No `EntityKind::Mount`.

**Tech Stack:** Rust 2021, existing crates, Bevy 0.16 client, protocol rev 8, upstream 0.31.0.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- `PROTOCOL_REV` remains **8**. New fields `#[serde(default)]`.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- New per-actor state is a `World` column. Do not add a fat `Entity` or `EntityKind::Mount`.
- Tick-phase fingerprint stays `3214741777866168171`. No new named phase.
- English-only toasts. Locked copy is in the spec §5.3 and §5.5.
- Riding is **not** a profession (`Progress.professions` untouched).
- Author for commits: `yoefun <xinglinsky@outlook.com>`.
- Branch: `cursor/mounts-riding-8d89`.
- Do not bump `VERSION.toml` / workspace version until Task 12.

Spec: `docs/superpowers/specs/2026-08-13-mounts-riding-design.md`.

## File map

- Create: `crates/woc-content/src/mounts.rs` — `RidingRankDef`, `MountDef`, tables, lookups
- Create: `crates/woc-sim/src/mount.rs` — train / learn / summon / dismount / toggle
- Modify: `crates/woc-content/src/items.rs` — `ItemKind::Mount`, three items
- Modify: `crates/woc-content/src/npcs.rs` — `NpcService::RidingTrainer`, Ross
- Modify: `crates/woc-content/src/zone1.rs` — Ross spot
- Modify: `crates/woc-content/src/lib.rs` — re-exports + integrity tests
- Modify: `crates/woc-protocol/src/lib.rs` — `TrainRiding`, snapshot fields
- Modify: `crates/woc-sim/src/ecs/{components,world,spawn}.rs` — `Riding` column
- Modify: `crates/woc-sim/src/{lib,sim,player_motion,interaction,combat,death,host}.rs`
- Modify: `crates/woc-sim/src/instances/mod.rs`, `crates/woc-sim/src/delves/mod.rs`
- Modify: `crates/woc-sim/src/persist_state.rs`, `crates/woc-persist/src/models.rs`, `crates/woc-server/src/{bridge,game_ws}.rs`
- Modify: `crates/woc-sim/src/visual_catalog.rs`, `crates/woc-client/src/{hud,input,visuals}.rs`
- Modify: docs + `VERSION.toml` in Task 12

---

### Task 1: Content — riding ranks, mount table, mount items

**Files:**
- Create: `crates/woc-content/src/mounts.rs`
- Modify: `crates/woc-content/src/items.rs`
- Modify: `crates/woc-content/src/lib.rs`

**Produces:** `RidingRankDef`, `RIDING_RANKS`, `riding_rank`, `riding_rank_by_n`, `MountKind`, `MountDef`, `MOUNTS`, `mount`, `mount_by_item`, `ItemKind::Mount`, items `brown_pony` / `swift_bay_steed` / `tawny_gryphon`

- [ ] **Step 1: Write the failing tests** in `crates/woc-content/src/lib.rs`:

```rust
#[test]
fn riding_ranks_locked() {
    assert_eq!(RIDING_RANKS.len(), 3);
    let a = riding_rank("apprentice").expect("apprentice");
    assert_eq!(a.rank, 1);
    assert_eq!(a.level_req, 2);
    assert_eq!(a.copper, 10);
    assert!((a.ground_speed_mult - 1.6).abs() < 1e-6);
    assert_eq!(riding_rank_by_n(3).unwrap().id, "expert");
    assert!(riding_rank_by_n(0).is_none());
}

#[test]
fn mount_table_matches_items() {
    assert_eq!(MOUNTS.len(), 3);
    for def in MOUNTS.iter() {
        let it = item(def.item_id).unwrap_or_else(|| panic!("missing {}", def.item_id));
        assert_eq!(it.kind, ItemKind::Mount, "{}", def.id);
        assert_eq!(it.stack_size, 1);
        assert_eq!(it.max_durability, 0);
        assert!(it.equip_slot.is_none());
        assert_eq!(mount_by_item(def.item_id).map(|m| m.id), Some(def.id));
    }
    let pony = mount("brown_pony").unwrap();
    assert_eq!(pony.riding_rank, 1);
    assert!(matches!(pony.kind, MountKind::Ground));
    assert!((pony.speed_mult - 1.6).abs() < 1e-6);
    let gryphon = mount("tawny_gryphon").unwrap();
    assert!(matches!(gryphon.kind, MountKind::Flying));
    assert_eq!(gryphon.riding_rank, 3);
    assert_eq!(item("brown_pony").unwrap().vendor_buy, 25);
    assert_eq!(item("swift_bay_steed").unwrap().vendor_buy, 150);
    assert_eq!(item("tawny_gryphon").unwrap().vendor_buy, 300);
}
```

- [ ] **Step 2: Run** `cargo test -p woc-content riding_ranks_locked -- --nocapture`

Expected: FAIL (unresolved `RIDING_RANKS` / `riding_rank`)

- [ ] **Step 3: Implement**

Add `pub mod mounts;` and re-export from `lib.rs`:

```rust
pub use mounts::{
    mount, mount_by_item, riding_rank, riding_rank_by_n, MountDef, MountKind, RidingRankDef,
    MOUNTS, RIDING_RANKS,
};
```

Create `crates/woc-content/src/mounts.rs` with the structs and locked rows from spec §5.1–5.2. Lookups:

```rust
pub fn riding_rank(id: &str) -> Option<&'static RidingRankDef> {
    RIDING_RANKS.iter().find(|r| r.id == id)
}

pub fn riding_rank_by_n(n: u8) -> Option<&'static RidingRankDef> {
    RIDING_RANKS.iter().find(|r| r.rank == n)
}

pub fn mount(id: &str) -> Option<&'static MountDef> {
    MOUNTS.iter().find(|m| m.id == id)
}

pub fn mount_by_item(item_id: &str) -> Option<&'static MountDef> {
    MOUNTS.iter().find(|m| m.item_id == item_id)
}
```

In `items.rs`, add `Mount` to `ItemKind`. Add helper and three `ZONE1_ITEMS` rows (buy/sell from spec):

```rust
const fn mount_item(
    id: &'static str,
    name: &'static str,
    vendor_buy: u32,
    vendor_sell: u32,
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Mount,
        stack_size: 1,
        max_durability: 0,
        vendor_buy,
        vendor_sell,
        attack_power: 0.0,
        armor: 0.0,
        equip_slot: None,
        level_req: 1,
        heal_hp: 0.0,
        armor_class: None,
        weapon_style: None,
        allowed_classes: &[],
        stamina: 0.0,
        spell_power: 0.0,
        quality: ItemQuality::Common,
        enchant_id: None,
    }
}
```

Append to `ZONE1_ITEMS`: `mount_item("brown_pony", "Brown Pony", 25, 5)`, `mount_item("swift_bay_steed", "Swift Bay Steed", 150, 30)`, `mount_item("tawny_gryphon", "Tawny Gryphon", 300, 60)`.

If `every_gear_item_has_rules` panics on `ItemKind::Mount`, mounts are not equippable (`equip_slot: None`) so the early-continue already covers them.

- [ ] **Step 4: Run** `cargo test -p woc-content riding_ranks_locked mount_table_matches_items -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/mounts.rs crates/woc-content/src/items.rs crates/woc-content/src/lib.rs
git commit -m "feat(content): add riding ranks and three mount items"
```

---

### Task 2: Content — Stable Master Ross

**Files:**
- Modify: `crates/woc-content/src/npcs.rs`
- Modify: `crates/woc-content/src/zone1.rs`
- Modify: `crates/woc-content/src/lib.rs`

**Consumes:** Task 1 mount item ids  
**Produces:** `NpcService::RidingTrainer`, `NpcDef::is_riding_trainer`, NPC `stable_master_ross`, Eastbrook spot `(4.0, 9.0)`

- [ ] **Step 1: Failing tests** in `lib.rs`:

```rust
#[test]
fn stable_master_ross_roster() {
    let ross = npc("stable_master_ross").expect("ross");
    assert!(ross.is_riding_trainer());
    assert!(ross.is_vendor());
    assert!(!ross.is_profession_trainer());
    assert!(ross.trains.is_empty());
    let stock: Vec<_> = ross.vendor_stock.iter().map(|o| o.item_id).collect();
    assert!(stock.contains(&"brown_pony"));
    assert!(stock.contains(&"swift_bay_steed"));
    assert!(stock.contains(&"tawny_gryphon"));
    assert!(EASTBROOK.npcs.iter().any(|s| s.npc_id == "stable_master_ross"
        && (s.x - 4.0).abs() < 1e-6
        && (s.z - 9.0).abs() < 1e-6));
}

#[test]
fn riding_trainers_stock_mounts() {
    for n in NPCS.iter() {
        if n.services.contains(&NpcService::RidingTrainer) {
            assert!(n.is_vendor(), "{} riding trainer must vendor", n.id);
            assert!(!n.vendor_stock.is_empty(), "{} empty stock", n.id);
            for offer in n.vendor_stock {
                let it = item(offer.item_id).unwrap();
                assert_eq!(it.kind, ItemKind::Mount, "{} stocks non-mount", n.id);
            }
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test -p woc-content stable_master_ross_roster -- --nocapture`

Expected: FAIL (`is_riding_trainer` / missing NPC)

- [ ] **Step 3: Implement**

Add `RidingTrainer` to `NpcService`. Add:

```rust
pub fn is_riding_trainer(&self) -> bool {
    self.services.contains(&NpcService::RidingTrainer)
}
```

Append Ross to `ZONE1_NPCS` using spec §5.3 greeting, services, and stock. Add `NpcSpot { npc_id: "stable_master_ross", x: 4.0, z: 9.0 }` to `EASTBROOK.npcs`.

Update `profession_trainers_reference_known_professions` is unchanged (`trains` stays empty). `service_name` lives in sim (Task 5) — content compiles without it.

If `npc_services_roster_locked` should mention Ross, add `assert!(npc("stable_master_ross").unwrap().is_riding_trainer());` there too.

- [ ] **Step 4: Run** `cargo test -p woc-content stable_master_ross_roster riding_trainers_stock_mounts npc_services_roster_locked -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/npcs.rs crates/woc-content/src/zone1.rs crates/woc-content/src/lib.rs
git commit -m "feat(content): add Stable Master Ross riding trainer"
```

---

### Task 3: Protocol — TrainRiding and snapshot fields

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`

**Produces:** `InteractAction::TrainRiding`; `EntitySnapshot.mounted`; `TickSnapshot.{riding_rank,known_mounts,mounted}`; `NpcSessionSnapshot.train_riding`

- [ ] **Step 1: Failing tests** in the protocol crate tests module (next to existing interact roundtrips):

```rust
#[test]
fn train_riding_roundtrip() {
    let json = serde_json::to_string(&InteractAction::TrainRiding).unwrap();
    assert!(json.contains("train_riding"));
    let back: InteractAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, InteractAction::TrainRiding);
}

#[test]
fn mounted_fields_default_when_omitted() {
    let snap: EntitySnapshot = serde_json::from_str(
        r#"{"id":1,"kind":"player","x":0.0,"y":0.0,"z":0.0,"yaw":0.0,"hp":1.0,"hp_max":1.0,"level":1,"name":"A","resource":0.0,"resource_max":0.0,"alive":true}"#,
    )
    .unwrap();
    assert!(snap.mounted.is_none());
    let tick: TickSnapshot = serde_json::from_str(
        r#"{"tick":0,"player_id":1,"entities":[],"progress":{"xp":0,"xp_to_level":0,"level":1,"copper":0}}"#,
    )
    .unwrap();
    assert_eq!(tick.riding_rank, 0);
    assert!(tick.known_mounts.is_empty());
    assert!(tick.mounted.is_none());
}
```

`InteractAction` must derive `PartialEq` if it does not already (it does).

- [ ] **Step 2: Run** `cargo test -p woc-protocol train_riding_roundtrip mounted_fields_default_when_omitted -- --nocapture`

Expected: FAIL (unknown variant / missing fields)

- [ ] **Step 3: Implement**

Add to `InteractAction` (after `ToggleForm` is fine):

```rust
/// Train the next riding rank at a riding trainer.
TrainRiding,
```

Add to `EntitySnapshot`:

```rust
/// Active mount id when the player is mounted.
#[serde(default)]
pub mounted: Option<String>,
```

Add to `TickSnapshot` (and `Default`):

```rust
#[serde(default)]
pub riding_rank: u8,
#[serde(default)]
pub known_mounts: Vec<String>,
#[serde(default)]
pub mounted: Option<String>,
```

Add to `NpcSessionSnapshot`:

```rust
#[serde(default)]
pub train_riding: bool,
```

Update every `EntitySnapshot { ... }` literal in this file’s tests to include `mounted: None` **or** rely on struct update if they use `..` — they are explicit; add `mounted: None`.

Keep `PROTOCOL_REV = 8`. Comment: `/// Rev 8 additive: riding snapshot + TrainRiding.`

- [ ] **Step 4: Run** `cargo test -p woc-protocol -q`

Expected: PASS (fix any snapshot literals the compiler lists)

- [ ] **Step 5: Commit**

```bash
git add crates/woc-protocol/src/lib.rs
git commit -m "feat(protocol): additive riding snapshot and TrainRiding"
```

---

### Task 4: ECS — `Riding` column

**Files:**
- Modify: `crates/woc-sim/src/ecs/components.rs`
- Modify: `crates/woc-sim/src/ecs/world.rs`
- Modify: `crates/woc-sim/src/ecs/spawn.rs`
- Modify: `docs/architecture/ecs.md` (catalog sentence)

**Produces:** `Riding { rank, known, last_id, active_id }` on every player

- [ ] **Step 1: Failing test** in `crates/woc-sim/src/ecs/mod.rs` tests (or a new `#[cfg(test)]` in components):

```rust
#[test]
fn create_player_inserts_riding_column() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", woc_content::PlayerClass::Warrior, 0.0, 0.0);
    let r = world.get::<crate::ecs::components::Riding>(1).expect("riding");
    assert_eq!(r.rank, 0);
    assert!(r.known.is_empty());
    assert!(r.last_id.is_none());
    assert!(r.active_id.is_none());
}
```

- [ ] **Step 2: Run** `cargo test -p woc-sim create_player_inserts_riding_column -- --nocapture`

Expected: FAIL (no `Riding` / get returns None)

- [ ] **Step 3: Implement**

In `components.rs` module docs table, add `` `Riding` | player ``.

```rust
#[derive(Debug, Clone, Default)]
pub struct Riding {
    pub rank: u8,
    pub known: BTreeSet<String>,
    pub last_id: Option<String>,
    pub active_id: Option<String>,
}
```

`impl_component!(Riding, riding);`

`World`: `pub riding: SparseSet<Riding>`, and `clear_all_columns` must `self.riding.remove(id)`.

`create_player`: `world.insert(id, Riding::default());` next to `Hearth`.

`docs/architecture/ecs.md`: player columns include `Riding`.

- [ ] **Step 4: Run** `cargo test -p woc-sim create_player_inserts_riding_column -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/ecs/components.rs crates/woc-sim/src/ecs/world.rs crates/woc-sim/src/ecs/spawn.rs docs/architecture/ecs.md
git commit -m "feat(sim): add player Riding ECS column"
```

---

### Task 5: Sim — `mount.rs` train / learn / summon / toggle

**Files:**
- Create: `crates/woc-sim/src/mount.rs`
- Modify: `crates/woc-sim/src/lib.rs`
- Modify: `crates/woc-sim/src/sim.rs` (call toggle; stop free-flight toasts)
- Modify: `crates/woc-sim/src/player_motion.rs` (stop reading `fly_toggle`)
- Modify: `crates/woc-sim/src/interaction.rs` (`TrainRiding`, `service_name`, `opens_npc_session`, `train_riding` on session)
- Modify: `crates/woc-sim/src/host.rs` (no extra match if `handle_interact` covers it)

**Consumes:** Tasks 1–4  
**Produces:** `train_riding`, `learn_mount`, `summon_mount`, `dismount`, `toggle_mount`, `is_mounted`

- [ ] **Step 1: Failing tests** in `crates/woc-sim/src/mount.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::spawn::create_player;
    use crate::ecs::components::{Health, Motion, Progress, Riding};
    use woc_content::PlayerClass;
    use woc_protocol::SimEvent;

    fn warrior() -> (World, EntityId) {
        let mut world = World::new();
        create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        (world, 1)
    }

    fn toast_text(events: &[SimEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                SimEvent::Toast { message } => Some(message.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn untrained_toggle_does_not_fly() {
        let (mut world, id) = warrior();
        let mut events = Vec::new();
        toggle_mount(&mut world, id, &mut events);
        assert!(!world.get::<Motion>(id).unwrap().flying);
        assert!(world.get::<Riding>(id).unwrap().active_id.is_none());
        assert!(toast_text(&events).iter().any(|m| m == "You do not know a mount."));
    }

    #[test]
    fn apprentice_then_pony_summons() {
        let (mut world, id) = warrior();
        world.get_mut::<Health>(id).unwrap().level = 2;
        world.get_mut::<Progress>(id).unwrap().copper = 40;
        let mut events = Vec::new();
        assert!(learn_mount(&mut world, id, "brown_pony", &mut events));
        // rank still 0 → summon must fail
        assert!(!summon_mount(&mut world, id, "brown_pony", &mut events));
        world.get_mut::<Riding>(id).unwrap().rank = 1;
        events.clear();
        assert!(summon_mount(&mut world, id, "brown_pony", &mut events));
        assert_eq!(world.get::<Riding>(id).unwrap().active_id.as_deref(), Some("brown_pony"));
        assert!(!world.get::<Motion>(id).unwrap().flying);
        assert!(toast_text(&events).iter().any(|m| m == "You mount up."));
    }
}
```

Also rewrite `player_motion::tests::fly_toggle_enables_vertical_ascend` in this task **after** toggle is wired: the old test must fail once `step_player_motion` ignores `fly_toggle`. Replace it in Step 3 with a gryphon test in `mount.rs` (Task 6 covers flight kinematics). For this task, change that test to assert **V** alone does **not** set flying:

```rust
#[test]
fn fly_toggle_ignored_by_motion_kernel() {
    let mut world = player_at(0.0, 0.0);
    let _ = step_player_motion(
        &mut world,
        1,
        &PlayerIntent { fly_toggle: true, ..Default::default() },
    );
    assert!(!world.get::<Motion>(1).unwrap().flying);
}
```

- [ ] **Step 2: Run** `cargo test -p woc-sim untrained_toggle_does_not_fly -- --nocapture`

Expected: FAIL (`mount` module missing)

- [ ] **Step 3: Implement** `crates/woc-sim/src/mount.rs`

Public API (exact names later tasks call):

```rust
pub fn is_mounted(world: &World, player_id: EntityId) -> bool {
    world
        .get::<Riding>(player_id)
        .and_then(|r| r.active_id.as_ref())
        .is_some()
}

pub fn dismount(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) -> bool { /* ... */ }

pub fn summon_mount(
    world: &mut World,
    player_id: EntityId,
    mount_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool { /* spec §5.5 */ }

pub fn toggle_mount(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) { /* ... */ }

pub fn learn_mount(
    world: &mut World,
    player_id: EntityId,
    mount_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool { /* insert known + last_id; toast learn copy; do not summon */ }

pub fn train_riding(
    world: &mut World,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool { /* next rank; level+copper; spend copper; set rank */ }
```

`learn_mount` here only inserts into `known` + `last_id` and toasts `"You learn to ride the {name}."`. UseItem (Task 8) calls learn then summon. The test `apprentice_then_pony_summons` uses `learn_mount` then sets rank then `summon_mount`.

`train_riding` (no NPC check in the helper — NPC check is in `handle_interact`):

```rust
let next = riding.rank + 1;
let Some(def) = woc_content::riding_rank_by_n(next) else {
    events.push(SimEvent::Toast { message: "You already know that rank.".into() });
    return false;
};
let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
if level < def.level_req { /* "You are too low level." */ }
let copper = world.get::<Progress>(player_id).map(|p| p.copper).unwrap_or(0);
if copper < def.copper { /* "Not enough copper." */ }
// spend, set rank, toast "Learned {def.name}."
```

`dismount`: if `active_id` is None return false. Clear `active_id`. If `Motion.flying`, set `flying = false` (keep `y`; existing vertical pass handles fall). Toast `"You dismount."`.

`summon_mount` checks from spec §5.5 in that order. Instance: `world.get::<InstanceAt>(player_id).and_then(|i| i.instance_id.clone()).is_some()`. Swimming: `player_motion::is_swimming_at`. Stealth: `combat::is_stealthed`. Must already `known.contains(mount_id)`. On success clear travel/ghost wolf via `combat` helpers already used by `toggle_form` (`remove_named_auras` is private — either `pub(crate)` it or duplicate a small retain in `mount.rs`). Prefer `pub(crate) fn strip_travel_forms(world, id)` in `combat.rs` that removes `ghost_wolf` / `travel_form` auras and clears those `stance_id` values.

`toggle_mount`: dismount if active; else summon `last_id` or the single known id; else `"You do not know a mount."`

Wire `sim.rs` `apply_intents_motion`: **before** `step_player_motion`, if `intent.fly_toggle && alive` call `crate::mount::toggle_mount`. **Delete** the `"Travel flight engaged..."` toast block.

`player_motion.rs`: delete the `if intent.fly_toggle && health.alive { m.flying = !m.flying; ... }` block. Keep flight kinematics when `m.flying` is already true. Update the module doc: flying is mount-driven.

`interaction.rs`:

- `opens_npc_session`: `|| def.is_riding_trainer()`
- `service_name`: `NpcService::RidingTrainer => "riding_trainer"`
- `npc_session_snapshot`: `train_riding: def.is_riding_trainer()`
- `handle_interact` match `TrainRiding`: require `Bags.open_vendor_npc` whose template `is_riding_trainer()` (look up NPC `Identity.template_id` → `npc()`), else toast `"Talk to a riding trainer."`; else `mount::train_riding`.

`lib.rs`: `pub mod mount;`

- [ ] **Step 4: Run** `cargo test -p woc-sim untrained_toggle_does_not_fly apprentice_then_pony_summons fly_toggle_ignored_by_motion_kernel -q`

Expected: PASS

Also run `cargo test -p woc-sim fly_toggle` to confirm the old name is gone.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/mount.rs crates/woc-sim/src/lib.rs crates/woc-sim/src/sim.rs crates/woc-sim/src/player_motion.rs crates/woc-sim/src/interaction.rs crates/woc-sim/src/combat.rs crates/woc-sim/src/host.rs
git commit -m "feat(sim): riding train/learn/summon and V mount toggle"
```

---

### Task 6: Motion — mount speed and flying gryphon

**Files:**
- Modify: `crates/woc-sim/src/player_motion.rs`
- Modify: `crates/woc-sim/src/mount.rs` (gryphon test)

**Produces:** mounted horizontal speed from `MountDef.speed_mult`; flying mount sets `Motion.flying` in `summon_mount` (already Task 5) and Space still climbs

- [ ] **Step 1: Failing tests** in `mount.rs`:

```rust
#[test]
fn pony_is_faster_than_foot() {
    let (mut world, id) = warrior();
    world.get_mut::<Riding>(id).unwrap().rank = 1;
    world.get_mut::<Riding>(id).unwrap().known.insert("brown_pony".into());
    let mut events = Vec::new();
    assert!(summon_mount(&mut world, id, "brown_pony", &mut events));
    let z0 = world.get::<Transform>(id).unwrap().z;
    let intent = PlayerIntent { move_z: 1.0, facing: 0.0, ..Default::default() };
    let _ = crate::player_motion::step_player_motion(&mut world, id, &intent);
    let mounted_dz = world.get::<Transform>(id).unwrap().z - z0;

    let (mut foot, fid) = warrior();
    let z1 = foot.get::<Transform>(fid).unwrap().z;
    let _ = crate::player_motion::step_player_motion(&mut foot, fid, &intent);
    let foot_dz = foot.get::<Transform>(fid).unwrap().z - z1;
    assert!(mounted_dz > foot_dz * 1.4);
}

#[test]
fn gryphon_toggle_allows_ascend() {
    let (mut world, id) = warrior();
    world.get_mut::<Health>(id).unwrap().level = 8;
    world.get_mut::<Riding>(id).unwrap().rank = 3;
    world.get_mut::<Riding>(id).unwrap().known.insert("tawny_gryphon".into());
    world.get_mut::<Riding>(id).unwrap().last_id = Some("tawny_gryphon".into());
    let mut events = Vec::new();
    toggle_mount(&mut world, id, &mut events);
    assert!(world.get::<Motion>(id).unwrap().flying);
    let start_y = world.get::<Transform>(id).unwrap().y;
    let up = PlayerIntent { jump: true, ..Default::default() };
    for _ in 0..10 {
        let _ = crate::player_motion::step_player_motion(&mut world, id, &up);
    }
    assert!(world.get::<Transform>(id).unwrap().y > start_y + 2.0);
}
```

- [ ] **Step 2: Run** `cargo test -p woc-sim pony_is_faster_than_foot -- --nocapture`

Expected: FAIL (speeds equal)

- [ ] **Step 3: Implement** in `step_player_motion` after the existing

```rust
} * crate::combat::move_speed_mult(world, player_id);
```

multiply again:

```rust
let mount_mult = world
    .get::<Riding>(player_id)
    .and_then(|r| r.active_id.as_deref())
    .and_then(woc_content::mount)
    .map(|m| m.speed_mult)
    .unwrap_or(1.0);
let speed = speed * mount_mult;
```

Do **not** also apply rank `ground_speed_mult`. Mount row is authoritative.

`summon_mount` for `MountKind::Flying` must set `Motion.flying = true`, `on_ground = false`, lift `y` by 1.5 (copy the old fly-toggle engage). Ground mounts force `flying = false`.

If pony test still fails because one tick of acceleration is capped, compare after 8 identical wish ticks instead of 1.

- [ ] **Step 4: Run** `cargo test -p woc-sim pony_is_faster_than_foot gryphon_toggle_allows_ascend fly_toggle_ignored_by_motion_kernel -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/player_motion.rs crates/woc-sim/src/mount.rs
git commit -m "feat(sim): mount speed multiplier and gryphon flight"
```

---

### Task 7: Forced dismount hooks

**Files:**
- Modify: `crates/woc-sim/src/mount.rs` (tests)
- Modify: `crates/woc-sim/src/sim.rs` (attack start)
- Modify: `crates/woc-sim/src/combat.rs` (`deal_damage`, ability fire, `toggle_stealth`, `toggle_form`, `cycle_stance`)
- Modify: `crates/woc-sim/src/death.rs`
- Modify: `crates/woc-sim/src/instances/mod.rs`
- Modify: `crates/woc-sim/src/delves/mod.rs`
- Modify: `crates/woc-sim/src/player_motion.rs` (swim)

**Produces:** combat / death / instance / stealth / form / swim dismount

- [ ] **Step 1: Failing tests** in `mount.rs`:

```rust
fn mount_pony(world: &mut World, id: EntityId) {
    world.get_mut::<Riding>(id).unwrap().rank = 1;
    world.get_mut::<Riding>(id).unwrap().known.insert("brown_pony".into());
    let mut events = Vec::new();
    assert!(summon_mount(world, id, "brown_pony", &mut events));
}

#[test]
fn damage_dismounts() {
    let (mut world, id) = warrior();
    mount_pony(&mut world, id);
    crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 3.0, 0.0);
    let mut events = Vec::new();
    crate::combat::deal_damage(&mut world, 2, id, 5.0, None, true, &mut events);
    assert!(world.get::<Riding>(id).unwrap().active_id.is_none());
}

#[test]
fn instance_refuses_mount() {
    let (mut world, id) = warrior();
    world.get_mut::<crate::ecs::components::InstanceAt>(id).unwrap().instance_id =
        Some("eastbrook_crypt#1".into());
    world.get_mut::<Riding>(id).unwrap().rank = 1;
    world.get_mut::<Riding>(id).unwrap().known.insert("brown_pony".into());
    let mut events = Vec::new();
    assert!(!summon_mount(&mut world, id, "brown_pony", &mut events));
    assert!(toast_text(&events).iter().any(|m| m == "You cannot mount here."));
}
```

- [ ] **Step 2: Run** `cargo test -p woc-sim damage_dismounts -- --nocapture`

Expected: FAIL (still mounted)

- [ ] **Step 3: Implement**

Call `crate::mount::dismount(world, player_id, events);` from:

1. `deal_damage` after a hit is applied to a target that has `Riding` (call even if absorb soaked all HP — the function ran past the NPC/dead guards). Call **before** returning on death so the toast still fires.
2. `sim.rs` when `intent.attack` sets `auto_attack = true`.
3. Player ability consumption in `update_player_combat` (the branch that actually begins a swing/cast/instant — not GCD rejects). If the exact site is `try_use_ability` / similar, dismount there once.
4. `toggle_stealth` after a successful stealth-on (and at the start of `summon_mount` refuse if stealthed — already Task 5).
5. Start of `toggle_form` and `cycle_stance` (warrior stance change also dismounts).
6. `finalize_player_death` in `death.rs`.
7. `enter_dungeon` after setting `instance_id`; same for `enter_delve`.
8. `step_player_motion`: if a ground mount is active and `is_swimming_at` after the move, `dismount`. `step_player_motion` currently has no `events` — either return a `dismount: bool` on `MotionEffect` or call dismount from `sim.rs` after motion when swimming && mounted-ground. Prefer extending `MotionEffect`:

```rust
pub struct MotionEffect {
    pub fall_damage: f32,
    pub dismount: bool,
}
```

Set `dismount: true` when swim starts on a ground mount; `sim.rs` then calls `mount::dismount`. Update existing fall-damage tests that construct / match `MotionEffect`.

Keep dismount toast once per call (`dismount` already no-ops if not mounted).

- [ ] **Step 4: Run** `cargo test -p woc-sim damage_dismounts instance_refuses_mount -q` and `cargo test -p woc-sim --lib death instances player_motion combat -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/mount.rs crates/woc-sim/src/sim.rs crates/woc-sim/src/combat.rs crates/woc-sim/src/death.rs crates/woc-sim/src/instances/mod.rs crates/woc-sim/src/delves/mod.rs crates/woc-sim/src/player_motion.rs
git commit -m "feat(sim): dismount on combat, death, instances, and swim"
```

---

### Task 8: UseItem learns and summons mounts

**Files:**
- Modify: `crates/woc-sim/src/interaction.rs`
- Modify: `crates/woc-sim/src/mount.rs` (optional extra tests)

**Produces:** bag use of `ItemKind::Mount` learns (consume once) then summons

- [ ] **Step 1: Failing test** in `interaction.rs` tests (follow `use_item_via_interact_action`):

```rust
#[test]
fn use_pony_item_learns_and_mounts() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    world.get_mut::<Health>(1).unwrap().level = 2;
    world.get_mut::<Riding>(1).unwrap().rank = 1;
    let slot = {
        let bags = world.get_mut::<Bags>(1).unwrap();
        let idx = bags.inventory.iter().position(|s| s.is_none()).unwrap();
        bags.inventory[idx] = Some(InvStack::new("brown_pony", 1));
        idx as u8
    };
    let mut events = Vec::new();
    handle_interact(
        &mut world,
        1,
        0,
        &InteractAction::UseItem { bag_slot: slot },
        0,
        &mut events,
    );
    assert!(world.get::<Bags>(1).unwrap().inventory.iter().flatten().all(|s| s.item_id != "brown_pony"));
    assert!(world.get::<Riding>(1).unwrap().known.contains("brown_pony"));
    assert_eq!(world.get::<Riding>(1).unwrap().active_id.as_deref(), Some("brown_pony"));
}
```

Second assertion test: using another pony while known does not consume and stays mounted / remounts.

```rust
#[test]
fn duplicate_pony_not_consumed() {
    // after known.contains, grant a second pony, UseItem, count stays 1, toast "You already know that mount."
}
```

Spec: “A second copy … toasts `You already know that mount.` and is not consumed.” Then still `summon_mount` that id (or dismount if already on it). Implement: if known, toast already-know, **do not consume**, then if active == this id dismount else summon.

- [ ] **Step 2: Run** `cargo test -p woc-sim use_pony_item_learns_and_mounts -- --nocapture`

Expected: FAIL (`Cannot use that.`)

- [ ] **Step 3: Implement** in `use_item_from_bag` **before** the consumable heal branch:

```rust
if idef.kind == ItemKind::Mount {
    let Some(mdef) = woc_content::mount_by_item(&stack.item_id) else {
        events.push(SimEvent::Toast { message: "Cannot use that.".into() });
        return;
    };
    let known = world
        .get::<Riding>(player_id)
        .is_some_and(|r| r.known.contains(mdef.id));
    if !known {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            if !remove_item(&mut bags.inventory, &stack.item_id, 1) {
                return;
            }
        }
        let _ = crate::mount::learn_mount(world, player_id, mdef.id, events);
    } else {
        events.push(SimEvent::Toast {
            message: "You already know that mount.".into(),
        });
    }
    if world
        .get::<Riding>(player_id)
        .and_then(|r| r.active_id.as_deref())
        == Some(mdef.id)
    {
        let _ = crate::mount::dismount(world, player_id, events);
    } else {
        let _ = crate::mount::summon_mount(world, player_id, mdef.id, events);
    }
    return;
}
```

First-use should not also emit already-know. The `known` snapshot is taken **before** learn.

- [ ] **Step 4: Run** `cargo test -p woc-sim use_pony_item_learns_and_mounts duplicate_pony_not_consumed -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/interaction.rs crates/woc-sim/src/mount.rs
git commit -m "feat(sim): UseItem learns mount items"
```

---

### Task 9: Persist + server bridge

**Files:**
- Modify: `crates/woc-sim/src/persist_state.rs`
- Modify: `crates/woc-persist/src/models.rs`
- Modify: `crates/woc-persist/src/memory.rs` (constructors)
- Modify: `crates/woc-persist/src/postgres.rs` if it maps completion fields
- Modify: `crates/woc-server/src/bridge.rs`
- Modify: `crates/woc-server/src/game_ws.rs` (test `PlayerPersistentState` literal)

**Produces:** `riding_rank`, `known_mounts`, `last_mount` survive save/load; old JSON defaults

- [ ] **Step 1: Failing tests**

In `persist_state.rs`:

```rust
#[test]
fn riding_round_trips() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    {
        let r = world.get_mut::<Riding>(1).unwrap();
        r.rank = 2;
        r.known.insert("brown_pony".into());
        r.last_id = Some("brown_pony".into());
        r.active_id = Some("brown_pony".into());
    }
    let state = export_player_state(&world, 1).unwrap();
    assert_eq!(state.riding_rank, 2);
    assert!(state.known_mounts.contains(&"brown_pony".into()));
    assert_eq!(state.last_mount, "brown_pony");
    let mut world2 = World::new();
    create_player_from_state(&mut world2, 2, "Ada", PlayerClass::Warrior, &state);
    let r = world2.get::<Riding>(2).unwrap();
    assert_eq!(r.rank, 2);
    assert!(r.known.contains("brown_pony"));
    assert_eq!(r.last_id.as_deref(), Some("brown_pony"));
    assert!(r.active_id.is_none(), "load starts dismounted");
}
```

In `woc-persist` models tests, parse completion JSON **without** riding keys and assert `riding_rank == 0`.

- [ ] **Step 2: Run** `cargo test -p woc-sim riding_round_trips -- --nocapture`

Expected: FAIL (missing fields on `PlayerPersistentState`)

- [ ] **Step 3: Implement**

`PlayerPersistentState` add:

```rust
pub riding_rank: u8,
pub known_mounts: BTreeSet<String>,
pub last_mount: String,
```

`is_virgin`: also require `riding_rank == 0 && known_mounts.is_empty()`.

Export from `Riding` (active ignored). Apply onto `Riding` (set `active_id = None`).

`Character`, `CharacterSave`, `CharacterCompletionDto`:

```rust
#[serde(default)]
pub riding_rank: u8,
#[serde(default)]
pub known_mounts: Vec<String>,
#[serde(default)]
pub last_mount: String,
```

Update `Default`, `From`, `completion_from_json` legacy arm, `memory.rs` / `postgres.rs` field copies, `bridge.rs` both directions, every struct literal the compiler reports (`game_ws.rs` spawn test, persist tests).

No SQL migration.

- [ ] **Step 4: Run** `cargo test -p woc-sim riding_round_trips -q` and `cargo test -p woc-persist -q` and `cargo test -p woc-server spawn_with_state_restores_progression -q`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/persist_state.rs crates/woc-persist crates/woc-server/src/bridge.rs crates/woc-server/src/game_ws.rs
git commit -m "feat(persist): save riding rank and known mounts"
```

---

### Task 10: Snapshot wiring

**Files:**
- Modify: `crates/woc-sim/src/sim.rs` (`entity_snapshot`, `snapshot_for_player`)

**Produces:** other players see `EntitySnapshot.mounted`; HUD fields on `TickSnapshot`

- [ ] **Step 1: Failing test** in `sim.rs` tests:

```rust
#[test]
fn snapshot_includes_mounted() {
    let mut sim = Sim::new_empty_eastbrook();
    let id = sim.spawn_player("Ada", woc_content::PlayerClass::Warrior);
    {
        let r = sim.world.get_mut::<crate::ecs::components::Riding>(id).unwrap();
        r.rank = 1;
        r.known.insert("brown_pony".into());
    }
    let mut events = Vec::new();
    assert!(crate::mount::summon_mount(&mut sim.world, id, "brown_pony", &mut events));
    let snap = sim.snapshot_for_player(id);
    assert_eq!(snap.riding_rank, 1);
    assert!(snap.known_mounts.iter().any(|m| m == "brown_pony"));
    assert_eq!(snap.mounted.as_deref(), Some("brown_pony"));
    let me = snap.entities.iter().find(|e| e.id == id).unwrap();
    assert_eq!(me.mounted.as_deref(), Some("brown_pony"));
}
```

Use the real `spawn_player` / `Sim` constructor that existing tests use (`new_empty_eastbrook` / `spawn_player` names — grep and copy the local helper if the names differ).

- [ ] **Step 2: Run** `cargo test -p woc-sim snapshot_includes_mounted -- --nocapture`

Expected: FAIL (fields default empty)

- [ ] **Step 3: Implement**

`entity_snapshot`:

```rust
mounted: world
    .get::<Riding>(id)
    .and_then(|r| r.active_id.clone()),
```

`snapshot_for_player` `TickSnapshot { ... }`:

```rust
riding_rank: world.get::<Riding>(player_id).map(|r| r.rank).unwrap_or(0),
known_mounts: world
    .get::<Riding>(player_id)
    .map(|r| r.known.iter().cloned().collect())
    .unwrap_or_default(),
mounted: world
    .get::<Riding>(player_id)
    .and_then(|r| r.active_id.clone()),
```

- [ ] **Step 4: Run** `cargo test -p woc-sim snapshot_includes_mounted tick_phase_order_fingerprint_locked -q`

Expected: PASS, fingerprint still `3214741777866168171`

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/sim.rs
git commit -m "feat(sim): snapshot mounted id and riding rank"
```

---

### Task 11: Client HUD, UseItem, visuals

**Files:**
- Modify: `crates/woc-client/src/hud.rs`
- Modify: `crates/woc-client/src/input.rs`
- Modify: `crates/woc-sim/src/visual_catalog.rs`
- Modify: `crates/woc-client/src/visuals.rs`

**Produces:** Train riding button; bag use on mounts; **V** unchanged; child mount mesh

- [ ] **Step 1: Failing tests**

In `visual_catalog.rs` tests (or add):

```rust
#[test]
fn mount_visual_keys() {
    assert_eq!(mount_visual_spec("brown_pony").key, "mount_pony");
    assert_eq!(mount_visual_spec("swift_bay_steed").family, VisualFamily::Boar);
    assert_eq!(mount_visual_spec("tawny_gryphon").family, VisualFamily::Harpy);
}
```

HUD unit tests that build `chrome_snapshot()` should still compile after `TickSnapshot::default()` gains fields (Task 3). Add:

```rust
#[test]
fn train_riding_button_when_session_offers_it() {
    let mut snap = chrome_snapshot();
    snap.open_npc = Some(NpcSessionSnapshot {
        npc_id: 9,
        npc_name: "Stable Master Ross".into(),
        greeting: "A horse knows the road better than most maps.".into(),
        services: vec!["riding_trainer".into(), "vendor".into()],
        train_riding: true,
        ..Default::default()
    });
    // If NpcSessionSnapshot has no Default, set every field explicitly including train_riding: true.
}
```

If HUD has no assertion helper for buttons, skip the HUD test and cover via `cargo check -p woc-client`. Prefer adding `#[derive(Default)]` on `NpcSessionSnapshot` if missing.

- [ ] **Step 2: Run** `cargo test -p woc-sim mount_visual_keys -- --nocapture`

Expected: FAIL

- [ ] **Step 3: Implement**

`visual_catalog.rs`: `pub fn mount_visual_spec(mount_id: &str) -> VisualSpec` matching spec keys. Reuse `MOB_BOAR` parts with a lighter brown for pony, darker for steed; reuse `MOB_HARPY` with tan/gold for gryphon. Export from `woc-sim` lib if not already `pub use visual_catalog::*`.

`visuals.rs` `spawn_entity_visual`: after spawning player parts, if `snap.mounted.as_deref()` is Some, spawn a child using `mount_visual_spec` parts, and add `0.55` to the player root Y (or part offsets). On snapshot apply, if `mounted` changed, rebuild the visual (follow existing template-change rebuild if any; otherwise despawn/respawn that `SimVisual`). Keep this YAGNI: rebuild the whole `SimVisual` when `mounted` differs from the last applied snapshot for that id.

`input.rs`: treat `ItemKind::Mount` like consumable for **F** / use:

```rust
} else if def.kind == ItemKind::Consumable || def.kind == ItemKind::Mount {
```

`hud.rs` session panel: if `npc.train_riding` spawn **Train riding** → `InteractAction::TrainRiding`.

Character sheet / XP chrome: if `snap.riding_rank > 0`, draw `Riding: {name}` using `riding_rank_by_n`; if `snap.mounted` Some, draw `Mounted: {mount name}`.

Do not change **V** binding.

- [ ] **Step 4: Run** `cargo test -p woc-sim mount_visual_keys -q` and `cargo check -p woc-client` and `cargo test -p woc-client --lib -q` if unit tests exist without GPU.

Expected: check green. Skip `cargo run -p woc-client` in CI.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/visual_catalog.rs crates/woc-sim/src/lib.rs crates/woc-client/src/hud.rs crates/woc-client/src/input.rs crates/woc-client/src/visuals.rs
git commit -m "feat(client): riding trainer chrome and mount silhouettes"
```

---

### Task 12: Version, docs, demo

**Files:**
- Modify: `VERSION.toml` (`1.16.0`, `parity_target = "mounts"`)
- Modify: `Cargo.toml` workspace `version`
- Modify: `CHANGELOG.md`, `README.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`
- Modify: spec status line to Shipped after tests pass

**Produces:** tagged rewrite identity for the implementation wave

- [ ] **Step 1: Failing check** — bump version first in tests if any crate asserts `1.13.0`. Grep `1.13.0` and update.

```bash
rg "1\\.13\\.0" -g '!target/**' -g '!docs/superpowers/**'
```

Leave historical changelog / old spec titles alone. Update README badge, footer, controls (**V** = mount toggle, not free flight).

- [ ] **Step 2: Run** `cargo test --workspace --exclude woc-client -q`

Expected: FAIL until versions match, then PASS after Step 3

- [ ] **Step 3: Implement docs**

`CHANGELOG.md` new `## 1.16.0` at top: riding ranks, Ross, three mounts, **V** mount toggle, Expert gryphon replaces free flight, protocol rev 8 additive.

`README.md` What works + controls: **V** toggle mount (requires training + known mount). Footer `WoC-rs 1.16.0`.

`ROADMAP.md` add row **1.16.0** `mounts`. Point at this spec/plan.

`STATUS.md` current rewrite `1.16.0` / `mounts` table (train, learn, toggle, dismount, persist, client).

`DEMO.md` add step 16: Ross train + pony + **V**; combat dismount; gryphon after Expert; crypt refuses mount. Footer version.

Spec header **Status:** Shipped.

- [ ] **Step 4: Run** `cargo test --workspace --exclude woc-client -q` and `cargo test -p woc-sim tick_phase_order_fingerprint_locked -q`

Expected: PASS, fingerprint `3214741777866168171`

- [ ] **Step 5: Commit**

```bash
git add VERSION.toml Cargo.toml CHANGELOG.md README.md docs
git commit -m "docs: ship 1.16.0 mounts and riding"
```

---

## Self-review

1. **Spec coverage:** ranks/items → T1; Ross → T2; protocol → T3; column → T4; train/toggle → T5; speed/flight → T6; dismount → T7; UseItem → T8; persist → T9; snapshot → T10; client → T11; version/docs → T12. Non-goals (taxi, journal, class mounts) have no tasks.
2. **Placeholders:** none. Toast strings copied from the spec.
3. **Types:** `Riding`, `train_riding`, `learn_mount`, `summon_mount`, `dismount`, `toggle_mount`, `TrainRiding`, `mounted` Option&lt;String&gt; used consistently.

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-13-mounts-riding.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks
2. **Inline Execution** — execute in-session with executing-plans checkpoints

Which approach?
