# NPC Services Implementation Plan

> **Shipped as `1.11.0` / `npc-services`** on `develop` after `1.10.0` quest-depth. Historical task text below still says 1.6.0 in a few places; the tag is **1.11.0**.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make hub NPCs the sim-authoritative front for buy/sell (including buyback and unsellable quest items), durability/repair, profession training, class confirmation, and hearth bind.

**Architecture:** Replace `NpcDef` booleans with a `NpcService` slice. Talk opens a player session on `Bags.open_vendor_npc`. New interact actions (`RepairAll`, `Buyback`, `TrainClass`, `BindHearth`, `UseHearthstone`) plus a gated `TrainProfession` run in `woc-sim`. Durability lives on `InvStack` / `Bags.equipment_wear`; hearth is a player `Hearth` column. The Bevy client only sends actions and paints `TickSnapshot`.

**Tech Stack:** Rust 2021 workspace crates (`woc-content`, `woc-protocol`, `woc-sim`, `woc-persist`, `woc-client`). No new dependencies. No Bevy inside sim/content.

**Design spec:** `docs/superpowers/specs/2026-08-13-npc-services-design.md`

## Global Constraints

- `woc-sim` and `woc-content` MUST NOT depend on Bevy, `bevy_ecs`, wgpu, axum, or tokio.
- Client never decides vendor prices, repair bills, training, or hearth success.
- All randomness via mulberry32 `Rng` on `Sim`. Hearth cooldown is `Sim.tick + 18_000`, never wall clock.
- Tick fingerprint must remain `15038642330132466611u64`. No new named tick phase.
- `PROTOCOL_REV` stays `6`. New snapshot fields use `#[serde(default)]`.
- Upstream pin stays `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- English-only player-facing strings (exact copies from the spec).
- New per-actor state is a `World` column (`Hearth`) or fields on the existing player `Bags` component. Do not reintroduce a fat `Entity`.
- `Equipment` stays `Option<String>` per slot. Wear is parallel `EquipmentWear` on `Bags`.
- Direct `train_profession(&mut World, …)` stays for gather/craft unit tests. Only `InteractAction::TrainProfession` requires a trainer NPC.
- Every task ends with `cargo test --workspace --exclude woc-client` green, and `cargo check -p woc-client` green when client files change.
- Do not bump workspace `version` until the implementation wave is ready to tag `1.6.0`.

---

## File map (create / own)

| Path | Responsibility |
| --- | --- |
| `crates/woc-content/src/npcs.rs` | `NpcService`, `NpcDef` helpers, zone1 NPCs + new Eastbrook trainers/inn/smith |
| `crates/woc-content/src/npcs_zone2.rs` | Add `ProfessionTrainer` to `apothecary_vex` |
| `crates/woc-content/src/npcs_zone3.rs` | Add `Repair` to `quartermaster_bren` |
| `crates/woc-content/src/zone1.rs` | New `NpcSpot`s for smith / herbalist / innkeeper |
| `crates/woc-content/src/items.rs` | `max_durability`; smith `vendor_buy` values |
| `crates/woc-content/src/items_zone2.rs` | Add `max_durability` to every `ItemDef` literal |
| `crates/woc-content/src/lib.rs` | Export `NpcService`; integrity tests |
| `crates/woc-protocol/src/lib.rs` | New actions + `NpcSessionSnapshot` + durability/hearth snapshot fields |
| `crates/woc-sim/src/interaction.rs` | Talk session, buy/sell/buyback/repair, session snapshot |
| `crates/woc-sim/src/inventory.rs` | `InvStack::new` sets durability |
| `crates/woc-sim/src/ecs/components.rs` | `InvStack.durability`, `EquipmentWear`, `Bags.buyback`, `Hearth` |
| `crates/woc-sim/src/ecs/world.rs` | `hearth` column + despawn clear |
| `crates/woc-sim/src/ecs/spawn.rs` | Insert `Hearth` + `equipment_wear` on players |
| `crates/woc-sim/src/stats.rs` | Zero-durability gear contributes 0 |
| `crates/woc-sim/src/combat.rs` | Wear on connecting hits |
| `crates/woc-sim/src/professions/mod.rs` | Gate `TrainProfession` on trainer NPC |
| `crates/woc-sim/src/host.rs` | Route new actions; pass `target_id` into professions; `UseHearthstone` uses `self.tick` |
| `crates/woc-sim/src/zones.rs` | Hearth teleport helper (bound coords, not zone spawn) |
| `crates/woc-sim/src/persist_state.rs` | Durability + hearth fields |
| `crates/woc-persist/src/models.rs` | Additive DTO fields |
| `crates/woc-client/src/{hud,input,nameplates,visuals,map,world_setup}.rs` | Session chrome, train/repair/bind, **H** key, durability text |
| `docs/parity/{STATUS,DEMO}.md`, `docs/ROADMAP.md` | 1.6.0 rows (implementation wave) |

---

### Task 1: `NpcService` content + Eastbrook roster

**Files:**
- Modify: `crates/woc-content/src/npcs.rs`
- Modify: `crates/woc-content/src/npcs_zone2.rs`
- Modify: `crates/woc-content/src/npcs_zone3.rs`
- Modify: `crates/woc-content/src/zone1.rs`
- Modify: `crates/woc-content/src/lib.rs` (export + tests)
- Modify: `crates/woc-content/src/items.rs` (`vendor_buy` on smith goods only)
- Modify: `crates/woc-client/src/nameplates.rs`, `visuals.rs`, `map.rs` (`is_vendor` / `is_quest_giver` become methods)
- Modify: `crates/woc-sim/src/interaction.rs` (method calls)

**Interfaces:**
- Consumes: existing `NpcDef` tables, `EASTBROOK.npcs`
- Produces: `NpcService`, `NpcDef { services, vendor_stock, trains }`, methods `is_vendor()`, `is_quest_giver()`, `can_repair()`, `is_profession_trainer()`, `is_class_trainer()`, `is_innkeeper()`, `trains_profession(&self, id: &str) -> bool`

- [ ] **Step 1: Write the failing content tests**

In `crates/woc-content/src/lib.rs` add:

```rust
#[test]
fn npc_services_roster_locked() {
    use crate::NpcService;
    let alden = npc("captain_alden").unwrap();
    assert!(alden.is_quest_giver());
    assert!(alden.is_class_trainer());
    assert!(!alden.is_vendor());

    let smith = npc("smith_brann").unwrap();
    assert!(smith.is_vendor());
    assert!(smith.can_repair());
    assert!(smith.trains_profession("mining"));
    assert!(smith.trains_profession("blacksmithing"));
    assert!(!smith.trains_profession("herbalism"));
    assert!(smith.vendor_stock.iter().any(|o| o.item_id == "copper_shortsword"));

    let wren = npc("herbalist_wren").unwrap();
    assert!(wren.trains_profession("herbalism"));
    assert!(wren.trains_profession("alchemy"));
    assert!(!wren.is_vendor());

    assert!(npc("innkeeper_mara").unwrap().is_innkeeper());
    assert!(npc("apothecary_vex").unwrap().trains_profession("alchemy"));
    assert!(npc("quartermaster_bren").unwrap().can_repair());
}

#[test]
fn profession_trainers_reference_known_professions() {
    for n in NPCS.iter() {
        if n.services.contains(&NpcService::ProfessionTrainer) {
            assert!(!n.trains.is_empty(), "{} trains nothing", n.id);
        }
        for id in n.trains {
            assert!(profession(id).is_some(), "{} trains unknown {id}", n.id);
        }
    }
}

#[test]
fn vendors_have_stock_and_buyable_prices() {
    for n in NPCS.iter() {
        if !n.services.contains(&NpcService::Vendor) {
            continue;
        }
        assert!(!n.vendor_stock.is_empty(), "vendor {} has empty stock", n.id);
        for o in n.vendor_stock {
            let def = item(o.item_id).expect(o.item_id);
            assert!(def.vendor_buy > 0, "{} sells {} at vendor_buy 0", n.id, o.item_id);
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content npc_services_roster_locked --offline`
Expected: FAIL compiling (`NpcService` not found) or `npc("smith_brann")` is `None`.

- [ ] **Step 3: Implement content**

Replace `NpcDef` in `npcs.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcService {
    QuestGiver,
    Vendor,
    Repair,
    ProfessionTrainer,
    ClassTrainer,
    Innkeeper,
}

#[derive(Debug, Clone)]
pub struct NpcDef {
    pub id: &'static str,
    pub name: &'static str,
    pub greeting: &'static str,
    pub services: &'static [NpcService],
    pub vendor_stock: &'static [VendorOffer],
    pub trains: &'static [&'static str],
}

impl NpcDef {
    pub fn is_quest_giver(&self) -> bool {
        self.services.contains(&NpcService::QuestGiver)
    }
    pub fn is_vendor(&self) -> bool {
        self.services.contains(&NpcService::Vendor)
    }
    pub fn can_repair(&self) -> bool {
        self.services.contains(&NpcService::Repair)
    }
    pub fn is_profession_trainer(&self) -> bool {
        self.services.contains(&NpcService::ProfessionTrainer)
    }
    pub fn is_class_trainer(&self) -> bool {
        self.services.contains(&NpcService::ClassTrainer)
    }
    pub fn is_innkeeper(&self) -> bool {
        self.services.contains(&NpcService::Innkeeper)
    }
    pub fn trains_profession(&self, id: &str) -> bool {
        self.trains.iter().any(|p| *p == id)
    }
}
```

Migrate every existing NPC: drop `is_quest_giver` / `is_vendor` fields; set `services` and `trains: &[]`.

Add Eastbrook NPCs (greetings English, locked):

- `smith_brann` / "Smith Brann" / `"Steel and ore. I can mend what the road breaks."` — `[Vendor, Repair, ProfessionTrainer]`, stock `worn_sword×1`, `wooden_buckler×1`, `copper_shortsword×1`, `recruit_tunic×1`, trains `&["mining", "blacksmithing"]`
- `herbalist_wren` / "Herbalist Wren" / `"The vale still grows, if you know where to kneel."` — `[ProfessionTrainer]`, empty stock, trains `&["herbalism", "alchemy"]`
- `innkeeper_mara` / "Innkeeper Mara" / `"Rest the night. I'll keep the hearth."` — `[Innkeeper]`

`captain_alden` services: `&[QuestGiver, ClassTrainer]`.
`apothecary_vex`: add `ProfessionTrainer` and `trains: &["herbalism", "alchemy"]`.
`quartermaster_bren`: `&[Vendor, Repair]`.

`EASTBROOK.npcs` append:

```rust
NpcSpot { npc_id: "smith_brann", x: 8.0, z: 4.0 },
NpcSpot { npc_id: "herbalist_wren", x: -6.0, z: 6.0 },
NpcSpot { npc_id: "innkeeper_mara", x: 2.0, z: 8.0 },
```

In `items.rs` set `vendor_buy` on smith goods (sell unchanged): `worn_sword` 20, `wooden_buckler` 16, `copper_shortsword` 48, `recruit_tunic` 16. Easiest path: add a `buy` parameter to `weapon` / `armor` helpers and pass `0` for unsold gear.

Export `NpcService` from `lib.rs`.

Replace every `def.is_vendor` / `n.is_quest_giver` field read with `is_vendor()` / `is_quest_giver()` in client + `interaction.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-content --offline` and `cargo test --workspace --exclude woc-client --offline`
Expected: PASS. `cargo check -p woc-client` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content crates/woc-client/src/nameplates.rs crates/woc-client/src/visuals.rs crates/woc-client/src/map.rs crates/woc-sim/src/interaction.rs
git commit -m "feat: NPC service flags and Eastbrook trainer roster"
```

---

### Task 2: Protocol actions and session snapshot types

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`

**Interfaces:**
- Consumes: existing `InteractAction`, `TickSnapshot`, `VendorSnapshot`
- Produces: `InteractAction::{RepairAll, Buyback { slot: u8 }, TrainClass, BindHearth, UseHearthstone}`; `BuybackSnapshot`; `NpcSessionSnapshot`; `TickSnapshot.open_npc`; `InvSlotSnapshot.durability`; `EquipmentSnapshot.*_durability`; `TickSnapshot.hearth_ready_tick`, `hearth_zone_id`

- [ ] **Step 1: Write the failing roundtrip test**

Append to `interact_action_roundtrip` (or a new test) the new variants. Add:

```rust
#[test]
fn npc_session_snapshot_defaults_when_omitted() {
    let json = serde_json::json!({
        "tick": 1,
        "player_id": 1,
        "entities": [],
        "progress": {
            "xp": 0, "xp_to_level": 100, "level": 1, "copper": 0
        }
    });
    let snap: TickSnapshot = serde_json::from_value(json).unwrap();
    assert!(snap.open_npc.is_none());
    assert_eq!(snap.hearth_ready_tick, 0);
    assert_eq!(snap.hearth_zone_id, "");
}

#[test]
fn repair_and_hearth_actions_roundtrip() {
    for a in [
        InteractAction::RepairAll,
        InteractAction::Buyback { slot: 0 },
        InteractAction::TrainClass,
        InteractAction::BindHearth,
        InteractAction::UseHearthstone,
    ] {
        let v = serde_json::to_value(&a).unwrap();
        let back: InteractAction = serde_json::from_value(v).unwrap();
        assert_eq!(format!("{back:?}"), format!("{a:?}"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-protocol repair_and_hearth_actions_roundtrip --offline`
Expected: FAIL (`no variant named RepairAll`).

- [ ] **Step 3: Add types**

On `InteractAction`, after `CloseVendor`:

```rust
    RepairAll,
    Buyback { slot: u8 },
    TrainClass,
    BindHearth,
    UseHearthstone,
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuybackSnapshot {
    pub slot: u8,
    pub item_id: String,
    pub count: u32,
    pub price: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcSessionSnapshot {
    pub npc_id: EntityId,
    pub npc_name: String,
    #[serde(default)]
    pub greeting: String,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub stock: Vec<VendorOfferSnapshot>,
    #[serde(default)]
    pub train_professions: Vec<String>,
    #[serde(default)]
    pub can_repair: bool,
    #[serde(default)]
    pub repair_cost: u32,
    #[serde(default)]
    pub can_bind: bool,
    #[serde(default)]
    pub buyback: Vec<BuybackSnapshot>,
}
```

On `InvSlotSnapshot`: `#[serde(default)] pub durability: Option<u32>`.
On `EquipmentSnapshot`: `#[serde(default)] pub main_hand_durability: Option<u32>` (and off_hand/head/chest/legs/feet).
On `TickSnapshot`: `#[serde(default)] pub open_npc: Option<NpcSessionSnapshot>`, `#[serde(default)] pub hearth_ready_tick: u64`, `#[serde(default)] pub hearth_zone_id: String`.

Every `TickSnapshot { ... }` literal in this file and `woc-sim` tests must compile: add `..` only if the struct already uses Default; otherwise add the new fields to the existing `Default` impl / test fixtures (`open_npc: None`, `hearth_ready_tick: 0`, `hearth_zone_id: String::new()`, durability `None`).

`PROTOCOL_REV` stays `6`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-protocol --offline` and `cargo test --workspace --exclude woc-client --offline`
Expected: PASS (fix snapshot literals until they do).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-protocol/src/lib.rs crates/woc-sim crates/woc-client crates/woc-server
git commit -m "feat: additive NPC session and hearth protocol fields"
```

---

### Task 3: Talk opens a session for every service NPC

**Files:**
- Modify: `crates/woc-sim/src/interaction.rs`
- Modify: `crates/woc-sim/src/sim.rs` (fill `open_npc` on snapshot)

**Interfaces:**
- Consumes: `NpcDef` methods from Task 1, `NpcSessionSnapshot` from Task 2
- Produces: `npc_session_snapshot(world, player_id) -> Option<NpcSessionSnapshot>`; Talk sets `open_vendor_npc` for vendor **or** repair **or** trainer **or** innkeeper; `vendor_snapshot` still returns `Some` iff `is_vendor()`

- [ ] **Step 1: Write the failing test**

In `crates/woc-sim/src/sim.rs` tests (next to `vendor_buy_spend_copper`):

```rust
#[test]
fn talk_to_trainer_opens_npc_session_without_vendor() {
    let mut sim = Sim::new_eastbrook("T", PlayerClass::Warrior);
    let wren = find_template(&sim, "herbalist_wren").expect("herbalist_wren");
    let (x, z) = {
        let t = sim.world.get::<Transform>(wren).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, x, z);
    sim.interact(wren, InteractAction::Talk);
    let snap = sim.snapshot_for_player(sim.player_id);
    assert!(snap.open_vendor.is_none());
    let session = snap.open_npc.expect("session");
    assert_eq!(session.npc_id, wren);
    assert!(session.train_professions.contains(&"herbalism".to_string()));
    assert!(!session.can_repair);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim talk_to_trainer_opens_npc_session_without_vendor --offline`
Expected: FAIL (`herbalist_wren` missing or `open_npc` always `None`).

- [ ] **Step 3: Implement**

In `talk`, set `bags.open_vendor_npc = Some(target_id)` when the def has any of Vendor / Repair / ProfessionTrainer / ClassTrainer / Innkeeper (not only `is_vendor()`). Still emit `VendorOpen` only for vendors.

Add `npc_session_snapshot` that reads the open NPC, maps services to strings (`"vendor"`, `"repair"`, `"profession_trainer"`, `"class_trainer"`, `"innkeeper"`, `"quest_giver"`), copies stock like `vendor_snapshot`, fills `train_professions` from `def.trains`, `can_repair` / `can_bind`, `repair_cost: 0` (Task 7 fills the real cost), `buyback: vec![]` (Task 4).

In `snapshot_for_player`: `open_npc: npc_session_snapshot(...)`, keep `open_vendor: vendor_snapshot(...)`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim talk_to_trainer_opens_npc_session_without_vendor vendor_buy_spend_copper --offline`
Expected: PASS. Existing vendor test still green.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/interaction.rs crates/woc-sim/src/sim.rs
git commit -m "feat: NPC talk session snapshot for trainers and inns"
```

---

### Task 4: Quest items unsellable + buyback

**Files:**
- Modify: `crates/woc-sim/src/ecs/components.rs` (`BuybackEntry`, `Bags.buyback`)
- Modify: `crates/woc-sim/src/ecs/spawn.rs` (`buyback: Vec::new()`)
- Modify: `crates/woc-sim/src/interaction.rs` (`sell`, `buyback`, session snapshot)
- Modify: `crates/woc-sim/src/host.rs` if `Buyback` needs routing (prefer handling in `handle_interact`)
- Modify: `crates/woc-sim/src/zones.rs` / `persist_state.rs` / `death` / `instances` — clear `buyback` wherever `open_vendor_npc` is already cleared

**Interfaces:**
- Consumes: `InteractAction::Buyback { slot }`, `ItemKind::Quest`
- Produces: `Bags.buyback: Vec<BuybackEntry>` cap 6; `BuybackEntry { item_id: String, count: u32, durability: Option<u32>, copper: u32 }`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn refuse_to_sell_quest_item() {
    let mut sim = Sim::new_eastbrook("Q", PlayerClass::Warrior);
    let vendor = find_template(&sim, "trader_wilkes").unwrap();
    let (x, z) = {
        let t = sim.world.get::<Transform>(vendor).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, x, z);
    if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
        assert!(crate::inventory::grant_into(&mut bags.inventory, "boar_tusk", 1));
    }
    let copper_before = sim.copper();
    sim.interact(vendor, InteractAction::Talk);
    let slot = sim.world.get::<Bags>(sim.player_id).unwrap()
        .inventory.iter().position(|s| s.as_ref().is_some_and(|st| st.item_id == "boar_tusk"))
        .unwrap() as u8;
    sim.interact(vendor, InteractAction::Sell { bag_slot: slot, count: 1 });
    assert_eq!(sim.copper(), copper_before);
    assert_eq!(
        crate::inventory::count_item(&sim.world.get::<Bags>(sim.player_id).unwrap().inventory, "boar_tusk"),
        1
    );
}

#[test]
fn sell_junk_then_buyback() {
    let mut sim = Sim::new_eastbrook("B", PlayerClass::Warrior);
    let vendor = find_template(&sim, "trader_wilkes").unwrap();
    let (x, z) = {
        let t = sim.world.get::<Transform>(vendor).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, x, z);
    if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
        assert!(crate::inventory::grant_into(&mut bags.inventory, "wolf_fang", 2));
    }
    sim.interact(vendor, InteractAction::Talk);
    let slot = sim.world.get::<Bags>(sim.player_id).unwrap()
        .inventory.iter().position(|s| s.as_ref().is_some_and(|st| st.item_id == "wolf_fang"))
        .unwrap() as u8;
    let copper_before = sim.copper();
    sim.interact(vendor, InteractAction::Sell { bag_slot: slot, count: 2 });
    let sold_for = woc_content::item("wolf_fang").unwrap().vendor_sell * 2;
    assert_eq!(sim.copper(), copper_before + sold_for);
    let session = sim.snapshot_for_player(sim.player_id).open_npc.unwrap();
    assert_eq!(session.buyback[0].item_id, "wolf_fang");
    assert_eq!(session.buyback[0].price, sold_for);
    sim.interact(vendor, InteractAction::Buyback { slot: 0 });
    assert_eq!(sim.copper(), copper_before);
    assert_eq!(
        crate::inventory::count_item(&sim.world.get::<Bags>(sim.player_id).unwrap().inventory, "wolf_fang"),
        2
    );
}
```

Put these in `sim.rs` tests (they need `find_template` / `place_player_at`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim refuse_to_sell_quest_item sell_junk_then_buyback --offline`
Expected: FAIL (quest item sells today; no `Buyback` handling).

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone)]
pub struct BuybackEntry {
    pub item_id: String,
    pub count: u32,
    pub durability: Option<u32>,
    pub copper: u32,
}
```

On `Bags`: `pub buyback: Vec<BuybackEntry>` (max 6). Initialize empty in spawn and `ecs/mod.rs` test fixtures.

`sell`: if `idef.kind == ItemKind::Quest`, toast `"This item is needed for a quest."` and return. On success, `push` a buyback entry (capture durability from the bag stack **before** `take_item`); if `buyback.len() > 6`, `remove(0)`.

`Buyback { slot }`: require open session NPC is `target_id`, vendor, in range. Remove that buyback row, charge `entry.copper`, grant the stack back (restore `durability` on the new `InvStack`). Toasts: `"Not enough copper."`, `"Inventory full."` (restore the buyback row if grant fails **after** taking copper — take copper only after grant succeeds, same order as `buy`).

Fill `npc_session_snapshot.buyback` from `Bags.buyback`.

Clear `buyback` wherever `open_vendor_npc = None` is already assigned.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim refuse_to_sell_quest_item sell_junk_then_buyback vendor_buy_spend_copper --offline`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: block quest-item vendor sales and add buyback"
```

---

### Task 5: Durability on items, stacks, wear, stats, persist

**Files:**
- Modify: `crates/woc-content/src/items.rs`, `items_zone2.rs`
- Modify: `crates/woc-sim/src/ecs/components.rs` (`InvStack.durability`, `EquipmentWear`, `Bags.equipment_wear`)
- Modify: `crates/woc-sim/src/inventory.rs` (`InvStack::new` / `grant_into`)
- Modify: `crates/woc-sim/src/stats.rs`
- Modify: `crates/woc-sim/src/interaction.rs` (equip copies wear ↔ stack)
- Modify: `crates/woc-sim/src/persist_state.rs`
- Modify: `crates/woc-persist/src/models.rs` (`InvStackDto.durability`)
- Modify: `crates/woc-sim/src/sim.rs` snapshot builder (durability fields)
- Modify: every `InvStack { item_id, count }` literal in the workspace

**Interfaces:**
- Consumes: `ItemDef.max_durability` (weapon 40, armor 30, else 0)
- Produces: `InvStack::new(item_id, count)` sets `durability`; `EquipmentWear`; `recalc_player_stats` ignores 0-wear slots

- [ ] **Step 1: Write the failing tests**

In `crates/woc-content/src/lib.rs`:

```rust
#[test]
fn gear_has_max_durability() {
    assert_eq!(item("worn_sword").unwrap().max_durability, 40);
    assert_eq!(item("recruit_tunic").unwrap().max_durability, 30);
    assert_eq!(item("baked_bread").unwrap().max_durability, 0);
    assert_eq!(item("boar_tusk").unwrap().max_durability, 0);
}
```

In `crates/woc-sim/src/stats.rs` tests (new module tests if none exist — add `#[cfg(test)]` at the bottom of `stats.rs`):

```rust
#[test]
fn broken_weapon_adds_no_attack_power() {
    let mut world = crate::ecs::World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Worn", woc_content::PlayerClass::Warrior, 0.0, 0.0);
    recalc_player_stats(&mut world, 1);
    let healthy = world.get::<crate::ecs::components::Combat>(1).unwrap().attack_damage;
    if let Some(bags) = world.get_mut::<crate::ecs::components::Bags>(1) {
        bags.equipment_wear.main_hand = Some(0);
    }
    recalc_player_stats(&mut world, 1);
    let broken = world.get::<crate::ecs::components::Combat>(1).unwrap().attack_damage;
    assert!(broken < healthy);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content gear_has_max_durability --offline`
Expected: FAIL (`no field max_durability`).

- [ ] **Step 3: Implement**

Add `pub max_durability: u32` to `ItemDef`. Helpers: weapons `40`, armor `30`, consumable/misc `0`. Update every zone2 `ItemDef { ... }` literal.

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquipmentWear {
    pub main_hand: Option<u32>,
    pub off_hand: Option<u32>,
    pub head: Option<u32>,
    pub chest: Option<u32>,
    pub legs: Option<u32>,
    pub feet: Option<u32>,
}
```

`InvStack` gains `pub durability: Option<u32>`. Constructor:

```rust
impl InvStack {
    pub fn new(item_id: impl Into<String>, count: u32) -> Self {
        let item_id = item_id.into();
        let durability = woc_content::item(&item_id).and_then(|d| {
            (d.max_durability > 0).then_some(d.max_durability)
        });
        Self { item_id, count, durability }
    }
}
```

`grant_into` uses `InvStack::new`. `create_player` sets `equipment_wear.main_hand` / `.chest` to the start items' max durability.

`recalc_player_stats`: before `add_gear_stats`, if the matching wear is `Some(0)` skip that slot. If wear is `None` but the item is equipped, treat as full (old saves).

Equip: copy `stack.durability` onto `equipment_wear` (default max). Unequip: copy wear back onto the bag `InvStack`.

`InvStackDto`: `#[serde(default)] pub durability: Option<u32>`. Persist export/import copies the field. Missing JSON → `None` → full on next recalc.

Snapshot: copy wear onto `InvSlotSnapshot.durability` and `EquipmentSnapshot.*_durability`.

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace --exclude woc-client --offline`
Expected: PASS after fixing every `InvStack` / `ItemDef` / `Bags` literal.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content crates/woc-sim crates/woc-persist crates/woc-protocol crates/woc-server crates/woc-client
git commit -m "feat: gear durability on stacks, wear, and persist"
```

---

### Task 6: Combat wear

**Files:**
- Modify: `crates/woc-sim/src/combat.rs`
- Modify: `crates/woc-sim/src/types.rs` (optional: no new combat constants required)

**Interfaces:**
- Consumes: `Bags.equipment_wear`, `deal_damage`, player auto-attack connecting branch
- Produces: `wear_player_weapon(world, player_id, events)` and `wear_player_armor(world, player_id, events)`; toast `"Your {name} is broken."` when a slot first reaches 0

- [ ] **Step 1: Write the failing test**

In `combat.rs` tests (or `sim.rs` if easier to swing):

```rust
#[test]
fn player_swing_wears_main_hand() {
    let mut world = crate::ecs::World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Swinger", woc_content::PlayerClass::Warrior, 0.0, 0.0);
    let before = world.get::<Bags>(1).unwrap().equipment_wear.main_hand.unwrap();
    let mut events = Vec::new();
    crate::combat::wear_player_weapon(&mut world, 1, &mut events);
    let after = world.get::<Bags>(1).unwrap().equipment_wear.main_hand.unwrap();
    assert_eq!(after, before - 1);
}

#[test]
fn mob_hit_wears_armor_not_weapon() {
    let mut world = crate::ecs::World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Tank", woc_content::PlayerClass::Warrior, 0.0, 0.0);
    let weapon_before = world.get::<Bags>(1).unwrap().equipment_wear.main_hand.unwrap();
    let chest_before = world.get::<Bags>(1).unwrap().equipment_wear.chest.unwrap();
    let mut events = Vec::new();
    crate::combat::wear_player_armor(&mut world, 1, &mut events);
    let bags = world.get::<Bags>(1).unwrap();
    assert_eq!(bags.equipment_wear.main_hand.unwrap(), weapon_before);
    assert_eq!(bags.equipment_wear.chest.unwrap(), chest_before - 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim player_swing_wears_main_hand --offline`
Expected: FAIL (`wear_player_weapon` not found).

- [ ] **Step 3: Implement**

```rust
pub fn wear_player_weapon(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    decrement_wear(world, player_id, EquipSlot::MainHand, events);
}

pub fn wear_player_armor(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    for slot in [EquipSlot::Head, EquipSlot::Chest, EquipSlot::Legs, EquipSlot::Feet, EquipSlot::OffHand] {
        decrement_wear(world, player_id, slot, events);
    }
}
```

`decrement_wear`: if the equipment slot is empty, return. Subtract 1 from the parallel wear (floor 0), using max if wear was `None`. If the value **becomes** 0, toast `"Your {item_name} is broken."` using `item(id).name`, then `recalc_player_stats`.

Call `wear_player_weapon` from the player auto-attack **connecting** branch (the `Some(amount)` arm after `scale_hit`, not on miss). Call `wear_player_armor` at the start of `deal_damage` when the **target** has `Bags` and the **source** is not the same id (incoming hit). Do not wear on ability damage if you cannot distinguish — spec says abilities do not spend extra durability, so gate armor wear on `ability_name.is_none()` **or** add a `from_melee_swing: bool` parameter to `deal_damage`. Prefer a boolean `melee_swing: bool` defaulted `false` at existing call sites, `true` only from player/mob auto-attack swing functions.

Do not change `TICK_PHASES`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim player_swing_wears_main_hand mob_hit_wears_armor_not_weapon --offline` and `cargo test -p woc-sim tick_phase_order_fingerprint_locked --offline`
Expected: PASS; fingerprint still `15038642330132466611`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/combat.rs
git commit -m "feat: combat durability wear on connecting melee hits"
```

---

### Task 7: `RepairAll`

**Files:**
- Modify: `crates/woc-sim/src/interaction.rs`
- Modify: `crates/woc-sim/src/host.rs` (if the action does not fall through to `handle_interact`)

**Interfaces:**
- Consumes: `InteractAction::RepairAll`, `NpcDef::can_repair`, `ItemDef.max_durability`
- Produces: `repair_cost(world, player_id) -> u32`; session `repair_cost` field

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn repair_all_at_smith_restores_gear() {
    let mut sim = Sim::new_eastbrook("R", PlayerClass::Warrior);
    let smith = find_template(&sim, "smith_brann").unwrap();
    let (x, z) = {
        let t = sim.world.get::<Transform>(smith).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, x, z);
    if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
        bags.equipment_wear.main_hand = Some(1);
        bags.equipment_wear.chest = Some(1);
    }
    if let Some(p) = sim.world.get_mut::<Progress>(sim.player_id) {
        p.copper = 10_000;
    }
    let cost = {
        let sword = 40 - 1;
        let tunic = 30 - 1;
        sword + tunic
    };
    sim.interact(smith, InteractAction::Talk);
    let session = sim.snapshot_for_player(sim.player_id).open_npc.unwrap();
    assert!(session.can_repair);
    assert_eq!(session.repair_cost, cost);
    let copper_before = sim.copper();
    sim.interact(smith, InteractAction::RepairAll);
    assert_eq!(sim.copper(), copper_before - cost);
    let wear = &sim.world.get::<Bags>(sim.player_id).unwrap().equipment_wear;
    assert_eq!(wear.main_hand, Some(40));
    assert_eq!(wear.chest, Some(30));
}

#[test]
fn repair_refuses_without_copper() {
    let mut sim = Sim::new_eastbrook("R", PlayerClass::Warrior);
    let smith = find_template(&sim, "smith_brann").unwrap();
    let (x, z) = {
        let t = sim.world.get::<Transform>(smith).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, x, z);
    if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
        bags.equipment_wear.main_hand = Some(0);
    }
    if let Some(p) = sim.world.get_mut::<Progress>(sim.player_id) {
        p.copper = 0;
    }
    sim.interact(smith, InteractAction::Talk);
    sim.interact(smith, InteractAction::RepairAll);
    assert_eq!(
        sim.world.get::<Bags>(sim.player_id).unwrap().equipment_wear.main_hand,
        Some(0)
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim repair_all_at_smith_restores_gear --offline`
Expected: FAIL (RepairAll is a no-op).

- [ ] **Step 3: Implement**

`repair_cost`: sum `(max.saturating_sub(current))` for every equipped slot with an item and every bag stack whose `item.max_durability > 0`.

`RepairAll`: NPC must `can_repair()`, in range, and `open_vendor_npc == Some(target_id)`. If copper < cost, toast `"Not enough copper."`. Else set all those durabilities to max, subtract copper, `recalc_player_stats`, toast `"Repaired for {cost} copper."`

Fill `npc_session_snapshot.repair_cost`.

Wilkes (`!can_repair`) must no-op RepairAll.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim repair_all_at_smith_restores_gear repair_refuses_without_copper vendor_buy_spend_copper --offline`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: NPC RepairAll at 1 copper per missing durability"
```

---

### Task 8: Profession trainer gate

**Files:**
- Modify: `crates/woc-sim/src/professions/mod.rs`
- Modify: `crates/woc-sim/src/host.rs` (pass `target_id` into profession interact)

**Interfaces:**
- Consumes: `InteractAction::TrainProfession { id }`, `NpcDef::trains_profession`
- Produces: `try_train_at_npc(world, player_id, npc_id, profession_id, events)`; existing `train_profession` unchanged

- [ ] **Step 1: Write the failing tests**

Keep the existing `train_profession(&mut world, …)` unit tests green (they must **not** require an NPC).

Add in `sim.rs`:

```rust
#[test]
fn train_mining_requires_smith() {
    let mut sim = Sim::new_eastbrook("P", PlayerClass::Warrior);
    sim.interact(
        sim.player_id,
        InteractAction::TrainProfession { id: "mining".into() },
    );
    assert!(sim
        .world
        .get::<Progress>(sim.player_id)
        .unwrap()
        .professions
        .get("mining")
        .is_none());

    let smith = find_template(&sim, "smith_brann").unwrap();
    let (x, z) = {
        let t = sim.world.get::<Transform>(smith).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, x, z);
    sim.interact(smith, InteractAction::Talk);
    sim.interact(
        smith,
        InteractAction::TrainProfession { id: "mining".into() },
    );
    assert_eq!(
        sim.world
            .get::<Progress>(sim.player_id)
            .unwrap()
            .professions
            .get("mining")
            .copied(),
        Some(1)
    );

    sim.interact(
        smith,
        InteractAction::TrainProfession {
            id: "herbalism".into(),
        },
    );
    assert!(sim
        .world
        .get::<Progress>(sim.player_id)
        .unwrap()
        .professions
        .get("herbalism")
        .is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim train_mining_requires_smith --offline`
Expected: FAIL (today `TrainProfession` succeeds with no NPC).

- [ ] **Step 3: Implement**

Change `professions::handle_interact` to take `target_id: EntityId`. For `TrainProfession`:

1. Target `Identity.kind == Npc`, distance ≤ `INTERACT_RANGE`.
2. `npc(template).trains_profession(id)`.
3. Else toast `"This trainer cannot teach that."` or `"Too far away."`
4. Else `train_profession(...)`.

`host.rs`: pass `target_id` instead of dropping it.

Do not add an NPC check inside `train_profession` itself.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim train_mining_requires_smith --offline` and `cargo test -p woc-sim professions --offline`
Expected: PASS (gather/craft tests still call `train_profession` directly).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/professions/mod.rs crates/woc-sim/src/host.rs crates/woc-sim/src/sim.rs
git commit -m "feat: profession training requires a matching trainer NPC"
```

---

### Task 9: Class trainer `TrainClass`

**Files:**
- Modify: `crates/woc-sim/src/interaction.rs` or a small `train_class` fn in `interaction.rs`
- Modify: `crates/woc-sim/src/host.rs` if needed

**Interfaces:**
- Consumes: `InteractAction::TrainClass`, `known_abilities_at_level`
- Produces: toast `"You are trained through level {n}."`; kit refresh

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn train_class_at_alden_refreshes_kit() {
    let mut sim = Sim::new_eastbrook("C", PlayerClass::Warrior);
    let alden = find_template(&sim, "captain_alden").unwrap();
    let (x, z) = {
        let t = sim.world.get::<Transform>(alden).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, x, z);
    sim.interact(alden, InteractAction::Talk);
    sim.interact(alden, InteractAction::TrainClass);
    assert!(sim.events.iter().any(|e| matches!(
        e,
        SimEvent::Toast { message } if message.starts_with("You are trained through level")
    )));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim train_class_at_alden_refreshes_kit --offline`
Expected: FAIL (no TrainClass handling).

- [ ] **Step 3: Implement**

`TrainClass`: NPC `is_class_trainer()`, in range. Read class + level, set `ClassKit.known_abilities` from `known_abilities_at_level`, toast `"You are trained through level {level}."`

Wilkes must no-op. Do **not** change `LearnTalent` / `RespecTalents`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim train_class_at_alden_refreshes_kit --offline`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: class trainer confirms kit through current level"
```

---

### Task 10: Hearth component + innkeeper

**Files:**
- Modify: `crates/woc-sim/src/ecs/components.rs`, `world.rs`, `spawn.rs`, `mod.rs` (despawn test fixtures if they list columns)
- Modify: `crates/woc-sim/src/types.rs` (`HEARTH_COOLDOWN_TICKS: u64 = 18_000`)
- Create or modify: hearth helpers — put `bind_hearth` / `use_hearthstone` in `crates/woc-sim/src/zones.rs` (already owns teleport) **or** `crates/woc-sim/src/interaction.rs`. Prefer `zones.rs` for teleport, `host.rs` for `UseHearthstone` (needs `self.tick`).
- Modify: `crates/woc-sim/src/persist_state.rs`, `crates/woc-persist/src/models.rs`
- Modify: `crates/woc-sim/src/sim.rs` snapshot `hearth_*`

**Interfaces:**
- Consumes: `InteractAction::BindHearth`, `UseHearthstone`
- Produces: `Hearth { zone_id, x, z, ready_tick }` player column; `load_overworld_zone_at(world, player_id, zone_id, x, z)` that populates the zone then teleports to **bound** coords

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn bind_and_hearthstone_from_wolf_run() {
    let mut sim = Sim::new_eastbrook("H", PlayerClass::Warrior);
    let mara = find_template(&sim, "innkeeper_mara").unwrap();
    let (mx, mz) = {
        let t = sim.world.get::<Transform>(mara).unwrap();
        (t.x, t.z)
    };
    place_player_at(&mut sim, mx, mz);
    sim.interact(mara, InteractAction::Talk);
    sim.interact(mara, InteractAction::BindHearth);
    let hearth = sim.world.get::<crate::ecs::components::Hearth>(sim.player_id).unwrap();
    assert!((hearth.x - mx).abs() < 0.01);

    place_player_at(&mut sim, -15.0, 55.0);
    sim.tick = 20;
    WorldHost::interact(&mut sim, sim.player_id, 0, InteractAction::UseHearthstone);
    let t = sim.world.get::<Transform>(sim.player_id).unwrap();
    assert!((t.x - mx).abs() < 0.5);
    assert!((t.z - mz).abs() < 0.5);
    assert_eq!(
        sim.world.get::<crate::ecs::components::Hearth>(sim.player_id).unwrap().ready_tick,
        20 + 18_000
    );

    place_player_at(&mut sim, -15.0, 55.0);
    WorldHost::interact(&mut sim, sim.player_id, 0, InteractAction::UseHearthstone);
    let t = sim.world.get::<Transform>(sim.player_id).unwrap();
    assert!((t.x + 15.0).abs() < 0.5, "cooldown must block the second hearth");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim bind_and_hearthstone_from_wolf_run --offline`
Expected: FAIL (`Hearth` not found).

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone)]
pub struct Hearth {
    pub zone_id: String,
    pub x: f32,
    pub z: f32,
    pub ready_tick: u64,
}
```

`impl_component!(Hearth, hearth);` Add `pub hearth: SparseSet<Hearth>` to `World` and `clear_all_columns`.

`create_player`: insert `Hearth { zone_id: "eastbrook".into(), x: EASTBROOK.player_spawn_x, z: EASTBROOK.player_spawn_z, ready_tick: 0 }`.

Extract `load_overworld_zone_at(world, player_id, zone_id, x, z)` from `load_overworld_zone` (same population / instance-clear / combat-clear / vendor-close, but `Transform` uses `x,z` + `ground_at`). `load_overworld_zone` calls it with layout spawn.

`BindHearth` in `handle_interact`: innkeeper, in range; copy player transform + zone onto `Hearth` (do not change `ready_tick`); toast `"Hearthbound."`

`UseHearthstone` in `host.rs`:

```rust
InteractAction::UseHearthstone => {
    crate::zones::use_hearthstone(&mut self.world, player_id, self.tick, &mut self.events);
}
```

`use_hearthstone`: if `tick < hearth.ready_tick` toast `"Hearthstone is not ready."` Else teleport via `load_overworld_zone_at`, set `ready_tick = tick + HEARTH_COOLDOWN_TICKS`.

Persist: `PlayerPersistentState` + `CharacterSave` fields `hearth_zone_id`, `hearth_x`, `hearth_z`, `hearth_ready_tick` with `#[serde(default)]`. Old rows → Eastbrook spawn, `ready_tick = 0`.

Snapshot: `hearth_ready_tick`, `hearth_zone_id`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim bind_and_hearthstone_from_wolf_run --offline` and `cargo test --workspace --exclude woc-client --offline`
Expected: PASS. Fingerprint unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim crates/woc-persist
git commit -m "feat: innkeeper hearth bind and tick-based hearthstone"
```

---

### Task 11: Client chrome

**Files:**
- Modify: `crates/woc-client/src/hud.rs` (repair / train / bind / buyback buttons; durability lines)
- Modify: `crates/woc-client/src/world_setup.rs` (reuse vendor panel children or add trainer/inn buttons under the same panel)
- Modify: `crates/woc-client/src/input.rs` (`KeyCode::KeyH` → `UseHearthstone` when not typing)
- Modify: `crates/woc-client/src/nameplates.rs`, `visuals.rs`, `map.rs` (markers `[#]` `[T]` `[H]`)

**Interfaces:**
- Consumes: `TickSnapshot.open_npc`, `open_vendor`, durability fields, `hearth_ready_tick`
- Produces: presentation only — every button sends an `InteractAction` targeting `open_npc.npc_id`

- [ ] **Step 1: Write a HUD unit test (string contains)**

If `hud.rs` already tests help text, extend:

```rust
#[test]
fn vendor_help_mentions_repair_when_session_can_repair() {
    // Build a TickSnapshot with open_npc.can_repair = true and assert the
    // bags/vendor help line includes "[R] Repair" after the helper that
    // formats it. If the helper is private, test via a pub(crate) formatter
    // such as `npc_session_help(snap) -> String`.
}
```

Add `pub(crate) fn npc_session_help(snap: &TickSnapshot) -> String` that returns `"[R] Repair"` / `"[H] Hearthstone"` / train hints from `open_npc` so it is testable without Bevy.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-client npc_session_help --offline`
Expected: FAIL (`npc_session_help` missing). Client tests may need GPU-less unit cfg — `hud.rs` already has `#[cfg(test)]` cases; follow that pattern.

- [ ] **Step 3: Implement**

- Nameplates: append `[#]` if `can_repair()`, `[T]` if profession or class trainer, `[H]` if innkeeper (in addition to `[!]` / `[$]`).
- Extend `sync_vendor_panel` (or a sibling `sync_npc_session_panel`) to show when `open_npc` is `Some`, even if `open_vendor` is `None`. Buttons:
  - existing Buy rows from `stock` / `open_vendor`
  - Repair → `InteractAction::RepairAll` if `can_repair` (label `Repair — {repair_cost}c`)
  - each `train_professions` → `TrainProfession { id }`
  - if services contains `class_trainer` → `TrainClass`
  - if `can_bind` → `BindHearth`
  - buyback rows → `Buyback { slot }`
- Bags/character text: for gear, `format!("{} {}/{}", name, dur, max)` when `max_durability > 0`; red-ish `TextColor` when dur == 0.
- `input.rs`: `KeyCode::KeyH` → `host.interact(player_id, InteractAction::UseHearthstone)` (target ignored). `KeyCode::KeyR` while `open_npc.can_repair` → `RepairAll` at `open_npc.npc_id`.
- Do not compute prices on the client.

- [ ] **Step 4: Run checks**

Run: `cargo check -p woc-client --offline` and `cargo test -p woc-client --offline` (unit tests only; skip GPU demo)
Expected: check PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client
git commit -m "feat: client NPC session chrome for repair, train, and hearth"
```

---

### Task 12: Docs and 1.6.0 status (implementation wave only)

**Files:**
- Modify: `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`
- Modify: `Cargo.toml` workspace `version = "1.6.0"` and `VERSION.toml` if present — **only when tagging**
- Modify: `docs/architecture/ecs.md` catalog row for `Hearth`

**Interfaces:**
- Consumes: spec §6 / §9
- Produces: STATUS rows `done` after tests are green

- [ ] **Step 1: Add STATUS rows as `done` only after Tasks 1–11 pass**

```markdown
## NPC services (`npc-services`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| `NpcService` roster | done | Smith, herbalist, innkeeper; vex trains; Bren repairs |
| Quest-item sell block + buyback | done | Cap 6; session-only |
| Durability + RepairAll | done | 40/30; 1c per point at smith/Bren |
| Profession trainer gate | done | Client Train buttons; `train_profession()` helper unchanged |
| Class trainer | done | Kit refresh toast; talents stay on N-panel |
| Hearth | done | Bind at Mara; 18_000 tick cooldown |
```

DEMO.md append:

```
8. Buy a ration from Wilkes; sell a fang and buy it back; boar tusk will not sell.
9. Wear down the starter sword on wolves; repair at Smith Brann.
10. Train Mining at Brann, Herbalism at Wren.
11. Bind at Innkeeper Mara, run to Wolf Run, press H.
```

- [ ] **Step 2: Run the full workspace tests**

Run: `cargo test --workspace --exclude woc-client --offline && cargo check -p woc-client --offline && cargo test -p woc-sim tick_phase_order_fingerprint_locked --offline`
Expected: all PASS; fingerprint `15038642330132466611`.

- [ ] **Step 3: Commit**

```bash
git add docs Cargo.toml VERSION.toml
git commit -m "docs: mark 1.6.0 npc-services complete"
```

---

## Self-review

**Spec coverage:** §5.1–5.2 roster → Task 1. Protocol → Task 2. Session → Task 3. Buy/sell/buyback → Task 4. Durability data → Task 5. Combat wear → Task 6. Repair → Task 7. Profession trainers → Task 8. Class trainer → Task 9. Hearth → Task 10. Client → Task 11. Docs/DoD demo → Task 12. Non-goals (banker, flight, limited stock, talent gating) have no tasks.

**Placeholders:** none. Commands, types, and toast strings are copied from the spec.

**Type consistency:** `NpcService` / `NpcSessionSnapshot` / `BuybackEntry` / `EquipmentWear` / `Hearth` / `HEARTH_COOLDOWN_TICKS = 18_000` / `InteractAction::{RepairAll, Buyback, TrainClass, BindHearth, UseHearthstone}` used under the same names in later tasks.
