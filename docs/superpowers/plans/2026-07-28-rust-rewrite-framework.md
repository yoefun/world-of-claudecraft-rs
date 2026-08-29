# Rust rewrite 0.2 — basic framework implementation plan


> **Superseded for remaining work:** After 0.2.0 framework, continue with
> [`2026-07-28-rust-rewrite-completion.md`](2026-07-28-rust-rewrite-completion.md)
> and [`../specs/2026-07-28-rust-rewrite-completion-design.md`](../specs/2026-07-28-rust-rewrite-completion-design.md).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring rewrite `0.2.0` to **framework complete** — data-driven Eastbrook loop (9 classes, inventory, quests, NPC/vendor, WorldHost, WS server) on top of the existing 0.1 combat slice.

**Architecture:** Add `woc-content` for tables; grow `woc-sim` behind `SimContext` + fixed tick phases; expand `woc-protocol` with interactions + `WorldHost`; Bevy offline host and axum WS online host both drive the same sim.

**Tech Stack:** Rust 2021, Bevy 0.16, axum 0.8 + tokio, serde/serde_json, workspace crates as in design spec.

**Design spec:** `docs/superpowers/specs/2026-07-28-rust-rewrite-framework-design.md`

## Global Constraints

- `woc-sim` and `woc-content` MUST NOT depend on Bevy, wgpu, axum, or tokio.
- All randomness via mulberry32 `Rng` on `Sim` (no `thread_rng`, no wall clock in sim).
- Tick rate remains `20` Hz (`woc_protocol::TICK_RATE`).
- Upstream pin stays `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9` unless a separate pin-bump change is intentional.
- Client never decides combat, loot, quest credit, or prices.
- YAGNI: no talents trees, dungeons, Postgres, Discord, Web3, RL, i18n in this plan.
- Prefer small modules; do not grow `crates/woc-sim/src/sim.rs` past ~400 lines without extracting leaves.
- Every phase ends with: `cargo test --workspace --exclude woc-client` green + `cargo check -p woc-client` green.
- Bump to rewrite `0.2.0` only at Phase F completion (earlier phases may keep `0.1.0` or use workspace version `0.2.0-dev` only if you also update `VERSION.toml` consistently — prefer keeping `0.1.0` until F, then one version bump commit).

---

## File map (create / own)

| Path | Responsibility |
| --- | --- |
| `crates/woc-content/` | Pure content tables + integrity tests |
| `crates/woc-content/src/classes.rs` | 9 `ClassDef` + starter kits |
| `crates/woc-content/src/abilities.rs` | Ability defs |
| `crates/woc-content/src/items.rs` | Item defs |
| `crates/woc-content/src/mobs.rs` | Mob templates |
| `crates/woc-content/src/npcs.rs` | NPC defs + vendor stock refs |
| `crates/woc-content/src/quests.rs` | Quest defs |
| `crates/woc-content/src/zone1.rs` | Eastbrook spawn lists (NPC/mob spots) |
| `crates/woc-content/src/lib.rs` | Re-exports `CLASSES`, `ITEMS`, … |
| `crates/woc-sim/src/context.rs` | `SimContext` callbacks |
| `crates/woc-sim/src/inventory.rs` | Bags / equip / grant / consume |
| `crates/woc-sim/src/stats.rs` | `recalc_player_stats` |
| `crates/woc-sim/src/quests.rs` | Accept / credit / turn-in |
| `crates/woc-sim/src/interaction.rs` | Talk / vendor / loot commands |
| `crates/woc-sim/src/host.rs` | `Sim` as `WorldHost` |
| `crates/woc-protocol/src/lib.rs` | Expanded wire types + `WorldHost` trait |
| `crates/woc-server/src/game_ws.rs` | WS session + tick loop |
| `crates/woc-client/src/` | Split UI modules; online mode later |

---

## Phase A — Content crate + SimContext + WorldHost

### Task A1: Add `woc-content` crate skeleton

**Files:**
- Create: `crates/woc-content/Cargo.toml`
- Create: `crates/woc-content/src/lib.rs`
- Modify: `Cargo.toml` (workspace members + `[workspace.dependencies] woc-content`)
- Modify: `crates/woc-sim/Cargo.toml` (depend on `woc-content`)

- [ ] **Step 1: Create crate files**

`crates/woc-content/Cargo.toml`:

```toml
[package]
name = "woc-content"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
```

`crates/woc-content/src/lib.rs`:

```rust
//! Authoritative game content tables for the Rust rewrite.
//! Pure data: no Bevy, no networking, no wall clock.

pub mod abilities;
pub mod classes;
pub mod items;
pub mod mobs;
pub mod npcs;
pub mod quests;
pub mod zone1;

pub use abilities::{AbilityDef, ABILITIES};
pub use classes::{ClassDef, PlayerClass, CLASSES};
pub use items::{ItemDef, ItemKind, ITEMS};
pub use mobs::{LootEntry, MobTemplate, MOBS};
pub use npcs::{NpcDef, VendorOffer, NPCS};
pub use quests::{QuestDef, QuestObjective, QuestReward, QUESTS};
pub use zone1::{MobSpot, NpcSpot, EASTBROOK};
```

- [ ] **Step 2: Wire workspace**

Add `"crates/woc-content"` to `[workspace].members` and:

```toml
woc-content = { path = "crates/woc-content" }
```

Add to `crates/woc-sim/Cargo.toml`:

```toml
woc-content = { workspace = true }
```

- [ ] **Step 3: Add stub modules so the crate compiles**

Create empty modules with minimal placeholder types (filled in A2–A3). For example `classes.rs` starts with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerClass {
    Warrior,
    Paladin,
    Hunter,
    Rogue,
    Priest,
    Shaman,
    Mage,
    Warlock,
    Druid,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: PlayerClass,
    pub name: &'static str,
    pub resource_type: ResourceType,
    pub base_hp: f32,
    pub primary_ability: &'static str,
    pub start_weapon: &'static str,
    pub start_chest: &'static str,
    pub start_items: &'static [(&'static str, u32)],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Rage,
    Mana,
    Energy,
}

pub static CLASSES: &[ClassDef] = &[];
```

Mirror minimal stubs for `abilities`, `items`, `mobs`, `npcs`, `quests`, `zone1` so `lib.rs` exports compile.

- [ ] **Step 4: Verify**

Run: `cargo test -p woc-content`  
Expected: PASS (0 tests OK) or compile-only success.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/woc-content crates/woc-sim/Cargo.toml Cargo.lock
git commit -m "feat(content): add woc-content crate skeleton"
```

### Task A2: Author minimal Eastbrook content tables

**Files:**
- Modify: all `crates/woc-content/src/*.rs`
- Test: `crates/woc-content/src/lib.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces: non-empty `CLASSES` (9), `ABILITIES`, `ITEMS`, `MOBS`, `NPCS`, `QUESTS`, `EASTBROOK`

- [ ] **Step 1: Write integrity failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_start_gear_exists() {
        assert_eq!(CLASSES.len(), 9);
        for class in CLASSES {
            assert!(ITEMS.iter().any(|i| i.id == class.start_weapon), "{}", class.start_weapon);
            assert!(ITEMS.iter().any(|i| i.id == class.start_chest), "{}", class.start_chest);
            assert!(ABILITIES.iter().any(|a| a.id == class.primary_ability), "{}", class.primary_ability);
            for (item_id, _) in class.start_items {
                assert!(ITEMS.iter().any(|i| i.id == *item_id), "{item_id}");
            }
        }
    }

    #[test]
    fn every_quest_npc_exists() {
        for q in QUESTS {
            assert!(NPCS.iter().any(|n| n.id == q.giver_npc), "{}", q.giver_npc);
            if let Some(turner) = q.turn_in_npc {
                assert!(NPCS.iter().any(|n| n.id == turner), "{turner}");
            }
        }
    }

    #[test]
    fn eastbrook_spots_resolve() {
        for spot in EASTBROOK.npcs {
            assert!(NPCS.iter().any(|n| n.id == spot.npc_id), "{}", spot.npc_id);
        }
        for spot in EASTBROOK.mobs {
            assert!(MOBS.iter().any(|m| m.id == spot.mob_id), "{}", spot.mob_id);
        }
    }
}
```

- [ ] **Step 2: Run test — expect FAIL**

Run: `cargo test -p woc-content every_class_start_gear_exists -- --nocapture`  
Expected: FAIL (empty tables / missing items).

- [ ] **Step 3: Fill tables**

Populate:

- 9 classes with distinct `primary_ability` and `resource_type` (Warrior/Paladin rage-or-mana as upstream-inspired; Hunter/Rogue energy; casters mana).
- Items: `worn_sword`, `recruit_tunic`, per-class starters as needed, `baked_bread`, `spring_water`, `wolf_fang`, `boar_tusk`, `copper_coin` not needed (currency is copper field), vendor `travelers_ration`.
- Mobs: `young_wolf`, `scarred_wolf`, `young_boar` with xp + loot entries.
- NPCs: `captain_alden` (quest giver), `trader_wilkes` (vendor), `town_crier` (flavor).
- Quests (≥3): e.g. `wolves_at_the_gate` (kill 3 young_wolf), `boar_tusks` (collect 2 boar_tusk), `report_to_alden` (talk/turn-in).
- `EASTBROOK`: player spawn `(2, 4)`, NPC spots near origin, wolf camp north, boar meadow east.

Keep numbers small; names may be rewrite-original if upstream strings are awkward — note deltas in `docs/parity/STATUS.md` when Phase D lands.

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p woc-content`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content
git commit -m "feat(content): author Eastbrook framework tables"
```

### Task A3: Expand protocol — interactions, inventory snapshot, WorldHost

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`
- Modify: `crates/woc-protocol/Cargo.toml` if needed (still serde only)
- Test: add `crates/woc-protocol/src/lib.rs` serde roundtrip tests

**Interfaces:**
- Produces: `InteractAction`, extended `PlayerIntent`/`TickSnapshot`/`SimEvent`, `WorldHost`, `WsClientMsg`/`WsServerMsg`, `protocol_rev: u32 = 2`

- [ ] **Step 1: Write serde roundtrip test for new enums**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interact_action_roundtrip() {
        let actions = [
            InteractAction::Talk,
            InteractAction::AcceptQuest { quest_id: "wolves_at_the_gate".into() },
            InteractAction::TurnInQuest { quest_id: "wolves_at_the_gate".into() },
            InteractAction::Buy { item_id: "travelers_ration".into(), count: 1 },
            InteractAction::Sell { bag_slot: 0, count: 1 },
            InteractAction::Equip { bag_slot: 0 },
            InteractAction::Unequip { equip_slot: EquipSlot::MainHand },
            InteractAction::LootCorpse { target_id: 3 },
            InteractAction::CloseVendor,
        ];
        for a in actions {
            let v = serde_json::to_value(&a).unwrap();
            let back: InteractAction = serde_json::from_value(v).unwrap();
            assert_eq!(format!("{back:?}"), format!("{a:?}"));
        }
    }
}
```

- [ ] **Step 2: Run — expect FAIL (types missing)**

Run: `cargo test -p woc-protocol`  
Expected: compile failure.

- [ ] **Step 3: Implement types**

Add at minimum:

```rust
pub const PROTOCOL_REV: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot { MainHand, OffHand, Chest }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractAction {
    Talk,
    AcceptQuest { quest_id: String },
    TurnInQuest { quest_id: String },
    Buy { item_id: String, count: u32 },
    Sell { bag_slot: u8, count: u32 },
    Equip { bag_slot: u8 },
    Unequip { equip_slot: EquipSlot },
    LootCorpse { target_id: EntityId },
    CloseVendor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvSlotSnapshot {
    pub item_id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestLogEntry {
    pub quest_id: String,
    pub state: String, // "active" | "ready" | "completed"
    pub counts: Vec<u32>,
}

// Extend TickSnapshot with:
// protocol_rev, inventory, equipment, quest_log, open_vendor: Option<VendorSnapshot>,
// resource_type, class_id

// Extend SimEvent with QuestAccepted, QuestProgress, QuestCompleted, ItemGained,
// ItemLost, Equipped, VendorOpen, NpcDialog

// Extend PlayerIntent with interact: Option<(EntityId, InteractAction)> OR keep
// interact as a separate WorldHost method (preferred — see trait below).

pub trait WorldHost {
    fn push_intent(&mut self, player_id: EntityId, intent: PlayerIntent);
    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction);
    fn tick_once(&mut self) -> (TickSnapshot, Vec<SimEvent>);
    fn snapshot_for(&self, player_id: EntityId) -> TickSnapshot;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsClientMsg {
    Hello { name: String, class_id: String },
    Intent(PlayerIntent),
    Interact { target_id: EntityId, action: InteractAction },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMsg {
    Welcome { player_id: EntityId, protocol_rev: u32 },
    Snapshot(TickSnapshot),
    Events { events: Vec<SimEvent> },
    Error { message: String },
}
```

Keep 0.1 fields working; add new fields with `#[serde(default)]` where sensible.

- [ ] **Step 4: Fix compile of dependents**

Update `woc-sim` / `woc-client` snapshot construction for new fields (defaults empty OK until Phase B/D).

- [ ] **Step 5: Tests PASS + commit**

```bash
cargo test -p woc-protocol
cargo test --workspace --exclude woc-client
git add crates/woc-protocol crates/woc-sim crates/woc-client
git commit -m "feat(protocol): rev2 interactions, snapshots, WorldHost"
```

### Task A4: SimContext + phased tick + content-driven spawn

**Files:**
- Create: `crates/woc-sim/src/context.rs`
- Modify: `crates/woc-sim/src/sim.rs`, `lib.rs`, `entity.rs`
- Test: extend determinism test in `sim.rs`

**Interfaces:**
- Consumes: `woc_content::EASTBROOK`, mob/npc templates
- Produces: `Sim::new_eastbrook(name, class)`, phased `tick`, `impl WorldHost for Sim`

- [ ] **Step 1: Failing test — Eastbrook spawn from tables**

```rust
#[test]
fn eastbrook_spawns_npcs_and_mobs_from_content() {
    let sim = Sim::new_eastbrook("Tester", woc_content::PlayerClass::Warrior);
    let npc_count = sim.entities.iter().filter(|e| e.kind == EntityKind::Npc).count();
    let mob_count = sim.entities.iter().filter(|e| e.kind == EntityKind::Mob).count();
    assert!(npc_count >= 3, "expected town NPCs, got {npc_count}");
    assert!(mob_count >= 4, "expected camps, got {mob_count}");
}
```

Add `EntityKind::Npc` to protocol if missing.

- [ ] **Step 2: Run — FAIL**

- [ ] **Step 3: Implement**

- Add `SimContext` with `emit`, entity getters as needed (start thin).
- Replace `new_combat_slice` with `new_eastbrook` (keep `new_combat_slice` as deprecated wrapper calling Warrior Eastbrook for one release if client still uses it).
- Spawn player from `CLASSES` starter stats; spawn NPCs/mobs from `EASTBROOK`.
- Structure `tick` into named private methods matching design phase order (even if some phases no-op).
- Implement `WorldHost` in `host.rs`.

- [ ] **Step 4: Update client bootstrap to `new_eastbrook`**

- [ ] **Step 5: Tests + commit**

```bash
cargo test -p woc-sim
cargo check -p woc-client
git add crates/woc-sim crates/woc-client crates/woc-protocol
git commit -m "feat(sim): SimContext, phased tick, content-driven Eastbrook"
```

**Phase A exit criteria:** content integrity tests green; Eastbrook spawn test green; protocol rev2 compiles; client still playable (Warrior combat may still be the only deep combat path).

---

## Phase B — Inventory & equipment

### Task B1: Inventory model + grant/loot into bags

**Files:**
- Create: `crates/woc-sim/src/inventory.rs`
- Modify: `entity.rs` / player meta on `Sim`, `combat.rs` loot path, snapshot builder
- Test: `inventory` unit tests + sim integration

**Constants:** `BACKPACK_SLOTS: u8 = 16`

- [ ] **Step 1: Failing test**

```rust
#[test]
fn loot_goes_into_backpack_not_bag_item_string() {
    let mut sim = Sim::new_eastbrook("Looter", PlayerClass::Warrior);
    // Grant directly:
    assert!(sim.grant_item(sim.player_id, "wolf_fang", 1).is_ok());
    let snap = sim.snapshot_for(sim.player_id);
    assert!(snap.inventory.iter().any(|s| s.item_id == "wolf_fang" && s.count == 1));
}
```

- [ ] **Step 2: Implement `PlayerInventory { slots: Vec<Option<InvStack>> }`, `grant_item`, `remove_item`, stacking rules (`stack_size` from `ItemDef`)**

- [ ] **Step 3: Change kill loot to call `grant_item` / ground loot pickup into bags; remove `bag_item: Option<String>` from progress (or keep deprecated field empty)**

- [ ] **Step 4: Snapshot `inventory` field populated

- [ ] **Step 5: Commit `feat(sim): backpack inventory and item grants`

### Task B2: Equipment + recalc_player_stats

**Files:**
- Create: `crates/woc-sim/src/stats.rs`
- Modify: `inventory.rs`, `interaction.rs` (equip actions), combat damage to use recalc'd attack

- [ ] **Step 1: Test equip weapon increases attack power / weapon damage field**

```rust
#[test]
fn equipping_weapon_updates_stats() {
    let mut sim = Sim::new_eastbrook("Gear", PlayerClass::Warrior);
    // starter weapon should already be equipped OR in bag — assert attack_power > 0
    let before = sim.player().unwrap().attack_power;
    assert!(before > 0.0);
    // grant + equip a stronger test weapon if authored; else assert chest armor raises armor stat
}
```

- [ ] **Step 2: Implement `EquipSlot` map on player, `recalc_player_stats(base class + gear)`, wire `InteractAction::Equip/Unequip`

- [ ] **Step 3: Commit `feat(sim): equipment slots and stat recalc`

**Phase B exit criteria:** loot appears in snapshot inventory; equip works; combat uses equipped weapon stats.

---

## Phase C — Nine classes + resources

### Task C1: Class create path

**Files:**
- Modify: `woc-sim` spawn, `woc-client` character create UI
- Test: spawn each class

```rust
#[test]
fn all_nine_classes_spawn() {
    for class in [
        PlayerClass::Warrior, PlayerClass::Paladin, PlayerClass::Hunter,
        PlayerClass::Rogue, PlayerClass::Priest, PlayerClass::Shaman,
        PlayerClass::Mage, PlayerClass::Warlock, PlayerClass::Druid,
    ] {
        let sim = Sim::new_eastbrook("C", class);
        let p = sim.player().unwrap();
        assert!(p.alive);
        assert!(p.hp_max > 0.0);
    }
}
```

- [ ] Implement resource pools (rage/mana/energy) from `ClassDef.resource_type`
- [ ] Map ability slot 1 → class `primary_ability` definition (damage/cost from content)
- [ ] Client: 9-class select grid; pass class into `new_eastbrook`
- [ ] Commit `feat: nine-class create and starter resources`

**Phase C note:** Deep class kits / dual wield / pets are out of scope; one primary damage ability each is enough.

---

## Phase D — Quests + NPC talk

### Task D1: Quest log + accept/turn-in

**Files:**
- Create: `crates/woc-sim/src/quests.rs`
- Create: `crates/woc-sim/src/interaction.rs`
- Modify: combat kill hook → quest credit
- Test: full quest happy path

```rust
#[test]
fn wolf_quest_accept_kill_turnin() {
    let mut sim = Sim::new_eastbrook("Q", PlayerClass::Warrior);
    let giver = sim.entities.iter().find(|e| e.template_id.as_deref() == Some("captain_alden")).unwrap().id;
    sim.interact(sim.player_id, giver, InteractAction::AcceptQuest { quest_id: "wolves_at_the_gate".into() });
    // kill 3 young wolves via helper or combat loop
    // ...
    sim.interact(sim.player_id, giver, InteractAction::TurnInQuest { quest_id: "wolves_at_the_gate".into() });
    let log = sim.snapshot_for(sim.player_id).quest_log;
    assert!(log.iter().any(|q| q.quest_id == "wolves_at_the_gate" && q.state == "completed"));
}
```

- [ ] Implement `QuestProgress` on player meta
- [ ] `on_mob_killed_for_quests`, collect objectives on `grant_item`
- [ ] Emit `QuestAccepted` / `QuestProgress` / `QuestCompleted`
- [ ] Talk action returns `NpcDialog` / available quest list via events or snapshot
- [ ] Commit `feat(sim): quest accept, credit, turn-in`

### Task D2: Client quest UX

**Files:**
- Split or extend `crates/woc-client/src/main.rs` → `ui_quest.rs`, `ui_hud.rs`
- Keys: `L` quest log; tracker shows first active quest

- [ ] Render quest tracker from snapshot
- [ ] Interact key `E` talks to nearest NPC / confirms accept when dialog open
- [ ] Commit `feat(client): quest log and tracker`

**Phase D exit criteria:** demo steps 2–4 of design §11 work offline for Warrior.

---

## Phase E — Vendor + second camp polish

### Task E1: Vendor buy/sell

**Files:**
- Modify: `interaction.rs`, snapshot `open_vendor`, content vendor offers on `trader_wilkes`

```rust
#[test]
fn vendor_buy_spend_copper() {
    let mut sim = Sim::new_eastbrook("V", PlayerClass::Warrior);
    sim.copper = 100;
    let vendor = /* trader_wilkes id */;
    sim.interact(sim.player_id, vendor, InteractAction::Talk);
    sim.interact(sim.player_id, vendor, InteractAction::Buy { item_id: "travelers_ration".into(), count: 1 });
    assert!(sim.copper < 100);
    assert!(sim.snapshot_for(sim.player_id).inventory.iter().any(|s| s.item_id == "travelers_ration"));
}
```

- [ ] Implement buy/sell with price from `VendorOffer` / `ItemDef.vendor_price`
- [ ] Client vendor panel when `open_vendor` present
- [ ] Commit `feat: vendor buy/sell`

### Task E2: Boar camp + collect quest

- [ ] Ensure boar meadow spawns and `boar_tusks` quest completes via inventory credit
- [ ] Commit `feat(content): boar camp collect quest wired`

**Phase E exit criteria:** design demo step 5 works.

---

## Phase F — Online WorldHost + 0.2.0 release

### Task F1: Server embeds sim on WebSocket

**Files:**
- Create: `crates/woc-server/src/game_ws.rs`
- Modify: `crates/woc-server/src/main.rs`, `Cargo.toml` (add `woc-sim`, `woc-protocol`, `axum` ws feature / `tokio-tungstenite` as required by axum 0.8)
- Test: `crates/woc-server/tests/ws_smoke.rs` or `#[tokio::test]` in module

```rust
#[tokio::test]
async fn ws_hello_receives_welcome_and_snapshot() {
    // bind random port, connect, send Hello, expect Welcome + Snapshot
}
```

- [ ] Shared `Arc<Mutex<Sim>>` or channel to tick task at 20 Hz
- [ ] On Hello: create player entity in sim (multiplayer: multiple players in one Eastbrook)
- [ ] Forward Intent/Interact; broadcast or unicast Snapshot/Events
- [ ] Disconnect: mark player dead/removed
- [ ] Commit `feat(server): websocket WorldHost for woc-sim`

### Task F2: Client online mode

**Files:**
- Create: `crates/woc-client/src/net.rs`
- Modify: title screen mode picker Offline | Online (`ws://127.0.0.1:8787/ws/game`)

- [ ] Online path sends intents over WS; applies snapshots to render state (no local sim tick)
- [ ] Offline path unchanged (`WorldHost` over local `Sim`)
- [ ] Commit `feat(client): online mode via websocket`

### Task F3: Version bump + parity docs + changelog

**Files:**
- `VERSION.toml`, `Cargo.toml` workspace version, `crates/woc-version`, `UPSTREAM.md`, `README.md`, `CHANGELOG.md`, `docs/parity/STATUS.md`

- [ ] Set `rewrite_version = "0.2.0"`, `parity_target = "framework"`
- [ ] Mark framework rows done/partial in STATUS
- [ ] README “What works in 0.2”
- [ ] Run full verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude woc-client -- -D warnings
cargo test --workspace --exclude woc-client
cargo check -p woc-client
cargo run -p woc-server &  # manual WS smoke if possible
```

- [ ] Commit `release: rewrite 0.2.0 framework complete`

**Phase F exit criteria:** design §3 definition of done + §11 demo script satisfied.

---

## Suggested execution order (agents)

1. Execute Phase A (Tasks A1–A4) fully before starting B.  
2. Phase B → C → D → E sequentially.  
3. Phase F last; only then bump version to `0.2.0`.  
4. After each phase, update `docs/parity/STATUS.md` notes (even before the final version bump).  
5. Prefer **subagent-driven-development**: one fresh agent per task, review gate between tasks.

## Out of scope reminders (do not implement in this plan)

Talents, dungeons/delves/PvP/market/mail/professions/parties, Postgres auth, Discord, Web3, RL, Electron, full DESIGN.md UI chrome, byte-identical terrain.

---

## Spec coverage self-check

| Design § | Tasks |
| --- | --- |
| Content data crate | A1–A2 |
| SimContext + phases | A4 |
| 9 classes | A2 + C1 |
| Inventory/equip | B1–B2 |
| Quests | D1–D2 |
| NPC/vendor | D1, E1 |
| Eastbrook scaffold | A2, A4, E2 |
| WorldHost | A3–A4, F1–F2 |
| WS online | F1–F2 |
| Docs/version | F3 |
| Non-goals | Global constraints + out-of-scope section |
