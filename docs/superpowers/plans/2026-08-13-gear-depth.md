# Gear-depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rewrite `1.9.0` / `gear-depth`: class/armor equip rules, two-hand occupancy, Neck/Finger, stamina + spell power, independent loot rolls, and a character sheet that displays sim stats.

**Architecture:** Keep equipment on `Bags` (player column). `woc-content::can_equip` is the single rule function. `recalc_player_stats` writes `Combat.attack_damage` / `armor` / `spell_power`. Protocol stays rev 7 with additive serde defaults. Do not add durability, quality, or a fat `Entity`.

**Tech Stack:** Rust edition 2021, existing crates (`woc-content`, `woc-protocol`, `woc-sim`, `woc-persist`, `woc-server`, `woc-client`), Bevy 0.16 client presentation, protocol rev 7, upstream pin 0.31.0.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- `PROTOCOL_REV` remains **7**. New snapshot fields use `#[serde(default)]`.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- All sim RNG via mulberry32 on `Sim` only.
- Client never decides equip/combat/loot outcomes.
- English-only strings.
- New per-actor state is a `World` column. Equipment stays inside `Bags`. Do **not** reintroduce a fat `Entity`.
- Tick fingerprint stays `15038642330132466611`.
- Do not add `ItemDef.max_durability` (NPC-services) or item quality/enchants (manufacturing draft).
- Before claiming done: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client` (+ clippy as CI).

**Design:** [`docs/superpowers/specs/2026-08-13-gear-depth-design.md`](../specs/2026-08-13-gear-depth-design.md)

---

## File map

| File | Responsibility |
| --- | --- |
| Modify `crates/woc-content/src/items.rs` | `ArmorClass`, `WeaponStyle`, `EquipDeny`, `can_equip`, `ItemDef` fields, helpers, zone1 gear tags, new jewelry/weapons |
| Modify `crates/woc-content/src/items_zone2.rs` | Fill new `ItemDef` fields; `fen_staff` / `hag_focus` |
| Modify `crates/woc-content/src/lib.rs` | Re-exports + integrity tests |
| Modify `crates/woc-content/src/mobs.rs` | Independent-roll-friendly tables; `crypt_warden` template |
| Modify `crates/woc-content/src/mobs_zone2.rs` | Pendant/staff/focus rows |
| Modify `crates/woc-protocol/src/lib.rs` | `EquipSlot::{Neck,Finger}`, snapshot fields, `TickSnapshot` AP/armor/SP |
| Modify `crates/woc-sim/src/ecs/components.rs` | `Equipment.neck/finger`, `Combat.spell_power` |
| Modify `crates/woc-sim/src/ecs/spawn.rs` | Starter cap/pants/boots; `spell_power: 0.0` |
| Modify `crates/woc-sim/src/interaction.rs` | `can_equip`, two-hand occupancy, jewelry slots |
| Modify `crates/woc-sim/src/stats.rs` | Stamina + spell power + eight slots |
| Modify `crates/woc-sim/src/combat.rs` | Spell/heal use `spell_power`; independent loot piles |
| Modify `crates/woc-sim/src/persist_state.rs` | Virgin check includes jewelry |
| Modify `crates/woc-persist/src/models.rs` | `EquipmentDto` neck/finger |
| Modify `crates/woc-server/src/bridge.rs` | DTO mapping |
| Modify `crates/woc-client/src/hud.rs` / `input.rs` | Sheet + numbered equip/use/unequip |
| Modify `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`, `CHANGELOG.md`, `VERSION.toml` | Tag `1.9.0` in the **implementation** wave, not this planning PR |

---

### Task 1: `can_equip` + `ItemDef` fields

**Files:**
- Modify: `crates/woc-content/src/items.rs`
- Modify: `crates/woc-content/src/items_zone2.rs` (every `ItemDef { ... }` literal)
- Modify: `crates/woc-content/src/lib.rs` (re-exports)
- Test: `crates/woc-content/src/items.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: existing `ItemDef`, `ItemEquipSlot`, `ItemKind`, `PlayerClass`
- Produces: `ArmorClass`, `WeaponStyle`, `EquipDeny`, `can_equip`, `class_armor_cap`, `armor_outranks`; `ItemDef` fields listed in the spec

- [ ] **Step 1: Write the failing tests**

Append to `items.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlayerClass;

    #[test]
    fn mage_cannot_wear_leather_or_sword() {
        let sword = item("worn_sword").unwrap();
        assert_eq!(
            can_equip(sword, PlayerClass::Mage, 10),
            Err(EquipDeny::WrongClass)
        );
        let tunic = item("recruit_tunic").unwrap();
        assert_eq!(
            can_equip(tunic, PlayerClass::Mage, 10),
            Err(EquipDeny::WrongArmor)
        );
    }

    #[test]
    fn warrior_can_wear_cloth_and_leather() {
        assert!(can_equip(item("recruit_robe").unwrap(), PlayerClass::Warrior, 1).is_ok());
        assert!(can_equip(item("recruit_tunic").unwrap(), PlayerClass::Warrior, 1).is_ok());
    }

    #[test]
    fn level_req_still_blocks() {
        assert_eq!(
            can_equip(item("veteran_helm").unwrap(), PlayerClass::Warrior, 1),
            Err(EquipDeny::LevelReq(5))
        );
    }

    #[test]
    fn junk_is_not_gear() {
        assert_eq!(
            can_equip(item("wolf_fang").unwrap(), PlayerClass::Warrior, 1),
            Err(EquipDeny::NotGear)
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content mage_cannot_wear_leather_or_sword -- --nocapture`  
Expected: FAIL compiling (`can_equip` / `EquipDeny` not found) or FAIL assertion once stubs exist.

- [ ] **Step 3: Minimal types + `can_equip`**

In `items.rs`, add enums and extend `ItemDef`. Empty `allowed_classes` means all classes. Keep helpers compiling by filling new fields:

```rust
use crate::PlayerClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorClass {
    Cloth,
    Leather,
    Mail,
    Plate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponStyle {
    OneHand,
    TwoHand,
    Ranged,
    Shield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipDeny {
    NotGear,
    LevelReq(u32),
    WrongClass,
    WrongArmor,
}

pub fn class_armor_cap(class: PlayerClass) -> ArmorClass {
    match class {
        PlayerClass::Warrior | PlayerClass::Paladin => ArmorClass::Plate,
        PlayerClass::Hunter | PlayerClass::Shaman => ArmorClass::Mail,
        PlayerClass::Rogue | PlayerClass::Druid => ArmorClass::Leather,
        PlayerClass::Priest | PlayerClass::Mage | PlayerClass::Warlock => ArmorClass::Cloth,
    }
}

fn armor_rank(class: ArmorClass) -> u8 {
    match class {
        ArmorClass::Cloth => 0,
        ArmorClass::Leather => 1,
        ArmorClass::Mail => 2,
        ArmorClass::Plate => 3,
    }
}

pub fn can_equip(def: &ItemDef, class: PlayerClass, level: u32) -> Result<(), EquipDeny> {
    if def.equip_slot.is_none() {
        return Err(EquipDeny::NotGear);
    }
    if level < def.level_req {
        return Err(EquipDeny::LevelReq(def.level_req));
    }
    if !def.allowed_classes.is_empty() && !def.allowed_classes.contains(&class) {
        return Err(EquipDeny::WrongClass);
    }
    if let Some(ac) = def.armor_class {
        if armor_rank(ac) > armor_rank(class_armor_cap(class)) {
            return Err(EquipDeny::WrongArmor);
        }
    }
    Ok(())
}
```

`ItemDef` gains `armor_class: Option<ArmorClass>`, `weapon_style: Option<WeaponStyle>`, `allowed_classes: &'static [PlayerClass]`, `stamina: f32`, `spell_power: f32`.

Update `weapon` / `armor` / `consumable` / `misc` to set those fields. Temporary defaults so the crate compiles:

- `weapon`: `weapon_style: Some(WeaponStyle::OneHand)`, `allowed_classes: &[]`, zeros for sta/sp, `armor_class: None`
- `armor`: `armor_class: Some(ArmorClass::Cloth)`, `weapon_style: None` (shield helper in Task 2)
- zone2 literals: copy the same defaults onto every `ItemDef { ... }`

Re-export from `lib.rs`:

```rust
pub use items::{
    can_equip, class_armor_cap, item, ArmorClass, EquipDeny, ItemDef, ItemEquipSlot, ItemKind,
    WeaponStyle, ITEMS,
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-content --lib`  
Expected: FAIL `mage_cannot_wear_leather_or_sword` (`WrongClass`/`WrongArmor` not yet true because `allowed_classes` is empty and tunic is Cloth). Keep the test file; Task 2 tags items to make it pass. If you tagged in this task, Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/items.rs crates/woc-content/src/items_zone2.rs crates/woc-content/src/lib.rs
git commit -m "feat(content): ItemDef armor class, weapon style, and can_equip"
```

---

### Task 2: Tag existing gear + jewelry slot enum

**Files:**
- Modify: `crates/woc-content/src/items.rs` (`ItemEquipSlot::{Neck,Finger}`, helper args, locked tags, new item rows may wait for Task 8)
- Modify: `crates/woc-content/src/items_zone2.rs`
- Modify: `crates/woc-content/src/lib.rs` (integrity test)
- Test: `crates/woc-content/src/lib.rs` / `items.rs`

**Interfaces:**
- Consumes: `can_equip` from Task 1
- Produces: tagged tables matching spec §5.5; `ItemEquipSlot::Neck` / `Finger`

- [ ] **Step 1: Integrity test**

```rust
#[test]
fn every_gear_item_has_rules() {
    for it in ITEMS.iter() {
        if it.equip_slot.is_none() {
            assert!(it.armor_class.is_none());
            assert!(it.weapon_style.is_none());
            continue;
        }
        match it.kind {
            ItemKind::Weapon => {
                assert!(it.weapon_style.is_some(), "{}", it.id);
                assert!(it.armor_class.is_none(), "{}", it.id);
            }
            ItemKind::Armor => {
                let style = it.weapon_style;
                if matches!(it.equip_slot, Some(ItemEquipSlot::OffHand)) {
                    assert_eq!(style, Some(WeaponStyle::Shield), "{}", it.id);
                    assert!(it.armor_class.is_none(), "{}", it.id);
                } else if matches!(
                    it.equip_slot,
                    Some(ItemEquipSlot::Neck | ItemEquipSlot::Finger)
                ) {
                    assert!(style.is_none(), "{}", it.id);
                    assert!(it.armor_class.is_none(), "{}", it.id);
                } else {
                    assert!(it.armor_class.is_some(), "{}", it.id);
                    assert!(style.is_none(), "{}", it.id);
                }
            }
            _ => panic!("{} is equippable but not weapon/armor", it.id),
        }
    }
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p woc-content every_gear_item_has_rules -- --nocapture`  
Expected: FAIL (`Neck`/`Finger` missing and/or tunic still Cloth / sword still empty `allowed_classes`).

- [ ] **Step 3: Tag tables**

Add to `ItemEquipSlot`: `Neck`, `Finger`.

Change `weapon(...)` to take `style: WeaponStyle` and `allowed: &'static [PlayerClass]`. Change `armor(...)` to take `ArmorClass`. Add:

```rust
const fn shield(
    id: &'static str,
    name: &'static str,
    vendor_sell: u32,
    armor: f32,
    allowed: &'static [PlayerClass],
) -> ItemDef { /* kind Armor, slot OffHand, weapon_style Shield, armor_class None */ }
```

Locked tags (spec §5.5). Use named consts:

```rust
const WARRIOR: &[PlayerClass] = &[PlayerClass::Warrior];
const PALADIN_SHAMAN: &[PlayerClass] = &[PlayerClass::Paladin, PlayerClass::Shaman];
const HUNTER: &[PlayerClass] = &[PlayerClass::Hunter];
const ROGUE: &[PlayerClass] = &[PlayerClass::Rogue];
const CASTERS: &[PlayerClass] = &[
    PlayerClass::Priest,
    PlayerClass::Mage,
    PlayerClass::Warlock,
    PlayerClass::Druid,
];
const WAR_PAL: &[PlayerClass] = &[PlayerClass::Warrior, PlayerClass::Paladin];
const WAR_PAL_ROGUE: &[PlayerClass] = &[
    PlayerClass::Warrior,
    PlayerClass::Paladin,
    PlayerClass::Rogue,
];
const WAR_PAL_SHA: &[PlayerClass] = &[
    PlayerClass::Warrior,
    PlayerClass::Paladin,
    PlayerClass::Shaman,
];
```

`wooden_buckler` uses `shield(..., WAR_PAL_SHA)`. Leather pieces use `ArmorClass::Leather` and `&[]`. Cloth starter pieces use `ArmorClass::Cloth`. `veteran_helm` is `Mail`. Zone2 `marsh_wraps` / `reedwalk_boots` / `mireguard_hood` are Leather.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-content --lib`  
Expected: PASS (`mage_cannot_wear_leather_or_sword`, `every_gear_item_has_rules`, `every_class_start_gear_exists`).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content
git commit -m "feat(content): class and armor tags on existing gear"
```

---

### Task 3: Protocol additive slots and sheet stats

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`
- Test: `crates/woc-protocol/src/lib.rs`

**Interfaces:**
- Consumes: existing `EquipSlot`, `EquipmentSnapshot`, `TickSnapshot`
- Produces: `EquipSlot::{Neck,Finger}`; `EquipmentSnapshot.neck/finger`; `TickSnapshot.{attack_power,armor,spell_power}` all `#[serde(default)]`; `PROTOCOL_REV` still 7

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn equipment_snapshot_omitted_jewelry_defaults() {
    let eq: EquipmentSnapshot = serde_json::from_str(
        r#"{"main_hand":"worn_sword","off_hand":null,"chest":"recruit_tunic"}"#,
    )
    .unwrap();
    assert_eq!(eq.main_hand.as_deref(), Some("worn_sword"));
    assert!(eq.neck.is_none());
    assert!(eq.finger.is_none());
}

#[test]
fn tick_snapshot_omitted_sheet_stats_default_zero() {
    let snap: TickSnapshot = serde_json::from_str(
        r#"{"tick":0,"player_id":1,"entities":[],"progress":{"xp":0,"xp_to_level":0,"level":1,"copper":0},"target_id":null,"ability_ready":false,"ability_cooldown":0.0}"#,
    )
    .unwrap();
    assert_eq!(snap.attack_power, 0.0);
    assert_eq!(snap.armor, 0.0);
    assert_eq!(snap.spell_power, 0.0);
    assert_eq!(snap.protocol_rev, PROTOCOL_REV);
    assert_eq!(PROTOCOL_REV, 7);
}

#[test]
fn unequip_neck_roundtrip() {
    let a = InteractAction::Unequip {
        equip_slot: EquipSlot::Neck,
    };
    let s = serde_json::to_string(&a).unwrap();
    let back: InteractAction = serde_json::from_str(&s).unwrap();
    assert!(matches!(
        back,
        InteractAction::Unequip {
            equip_slot: EquipSlot::Neck
        }
    ));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p woc-protocol equipment_snapshot_omitted_jewelry_defaults -- --nocapture`  
Expected: FAIL (unknown fields / missing struct fields).

- [ ] **Step 3: Implement**

Add variants and fields. Update `Default for TickSnapshot` with `attack_power: 0.0, armor: 0.0, spell_power: 0.0`. Leave `PROTOCOL_REV = 7`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-protocol --lib`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-protocol/src/lib.rs
git commit -m "feat(protocol): jewelry slots and sheet stats, rev 7 additive"
```

---

### Task 4: Sim `Equipment` / `Combat.spell_power` / persist DTO

**Files:**
- Modify: `crates/woc-sim/src/ecs/components.rs`
- Modify: `crates/woc-sim/src/ecs/spawn.rs` (`spell_power: 0.0` on every `Combat {`)
- Modify: `crates/woc-sim/src/instances/mod.rs` (boss `Combat` literal)
- Modify: `crates/woc-sim/src/persist_state.rs` (`is_virgin` jewelry)
- Modify: `crates/woc-persist/src/models.rs`
- Modify: `crates/woc-server/src/bridge.rs`
- Modify: `crates/woc-sim/src/interaction.rs` (`equipment_slot_mut` + `to_protocol_slot`)
- Modify: `crates/woc-sim/src/sim.rs` (snapshot copy)
- Modify: `crates/woc-sim/src/stats.rs` (walk new slots; formula in Task 6 — for now add fields at 0)
- Test: `crates/woc-persist/src/lib.rs` (`inventory_equipment_quests_roundtrip` still compiles)

**Interfaces:**
- Consumes: protocol `EquipSlot::{Neck,Finger}`
- Produces: `Equipment.{neck,finger: Option<String>}`; `Combat.spell_power: f32`; `EquipmentDto` matching fields

- [ ] **Step 1: Persist omit-key test**

In `crates/woc-persist/src/lib.rs`:

```rust
#[test]
fn equipment_dto_omitted_jewelry_defaults() {
    let eq: EquipmentDto = serde_json::from_str(r#"{"main_hand":"worn_sword"}"#).unwrap();
    assert_eq!(eq.main_hand.as_deref(), Some("worn_sword"));
    assert!(eq.neck.is_none());
    assert!(eq.finger.is_none());
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p woc-persist equipment_dto_omitted_jewelry_defaults -- --nocapture`  
Expected: FAIL until `neck`/`finger` exist with `serde(default)`.

- [ ] **Step 3: Wire fields**

`Equipment` and `EquipmentDto`: add `neck`, `finger` with `#[serde(default, skip_serializing_if = "Option::is_none")]` on the DTO (same as other slots).

`Combat.spell_power: f32`. Every `Combat {` in `spawn.rs` (player helper + pet) and `instances/mod.rs` sets `spell_power: 0.0`.

`equipment_slot_mut` / `to_protocol_slot` match Neck/Finger.

`is_virgin`: `&& self.equipment.neck.is_none() && self.equipment.finger.is_none()`.

`bridge.rs` `equip_from_dto` / `equip_to_dto` copy the two fields.

`sim.rs` snapshot:

```rust
neck: bags.equipment.neck.clone(),
finger: bags.equipment.finger.clone(),
```

and `attack_power` / `armor` / `spell_power` from `world.get::<Combat>(player_id)` (zeros if missing).

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace --exclude woc-client`  
Expected: PASS (behavior unchanged aside from new zeros / empty jewelry).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim crates/woc-persist crates/woc-server
git commit -m "feat(sim): jewelry fields and combat.spell_power"
```

---

### Task 5: Equip rules and two-hand occupancy

**Files:**
- Modify: `crates/woc-sim/src/interaction.rs`
- Test: `crates/woc-sim/src/interaction.rs`

**Interfaces:**
- Consumes: `woc_content::can_equip`, `EquipDeny`, `WeaponStyle`
- Produces: `equip_from_bag` that refuses illegal gear, occupies OH for two-hand/ranged, toasts English messages

Toast strings (locked):

| Deny | Toast |
| --- | --- |
| `NotGear` | `Cannot equip that.` (existing) |
| `LevelReq(n)` | `Requires level {n}.` (existing) |
| `WrongClass` | `Your class cannot equip that.` |
| `WrongArmor` | `Your class cannot wear that armor.` |
| two-hand + OH attempt | `Cannot dual-wield a two-handed weapon.` |
| not enough bag holes | `Inventory full.` |

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn mage_refuses_sword() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Cas", PlayerClass::Mage, 0.0, 0.0);
    if let Some(bags) = world.get_mut::<Bags>(1) {
        assert!(grant_into(&mut bags.inventory, "worn_sword", 1));
    }
    let slot = bag_slot_of(&world, 1, "worn_sword");
    let mut events = Vec::new();
    equip_from_bag(&mut world, 1, slot, &mut events);
    assert_ne!(
        world.get::<Bags>(1).unwrap().equipment.main_hand.as_deref(),
        Some("worn_sword")
    );
    assert!(events.iter().any(|e| matches!(
        e,
        SimEvent::Toast { message } if message.contains("class cannot equip")
    )));
}

#[test]
fn two_hand_clears_off_hand_into_bag() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Hunt", PlayerClass::Hunter, 0.0, 0.0);
    if let Some(bags) = world.get_mut::<Bags>(1) {
        bags.equipment.off_hand = Some("wooden_buckler".into());
        // hunter cannot wear buckler — grant a legal OH by skipping can_equip for setup
        // Use spawn then force OH empty and give another hunter-legal scenario:
        // warrior + buckler then we need a two-hand warrior weapon (crypt_cleaver in Task 8).
        // Until Task 8, use worn_bow on hunter after stuffing OH via bags.equipment
        // (sim state injection). Hunter cannot equip buckler; injection is fine.
    }
    let mut events = Vec::new();
    // Equip the already-equipped start bow again from a granted copy to trigger 2H clear.
    if let Some(bags) = world.get_mut::<Bags>(1) {
        assert!(grant_into(&mut bags.inventory, "worn_bow", 1));
    }
    let slot = bag_slot_of(&world, 1, "worn_bow");
    equip_from_bag(&mut world, 1, slot, &mut events);
    assert!(world.get::<Bags>(1).unwrap().equipment.off_hand.is_none());
    assert_eq!(
        world.get::<Bags>(1).unwrap().equipment.main_hand.as_deref(),
        Some("worn_bow")
    );
    assert_eq!(
        count_item(&world.get::<Bags>(1).unwrap().inventory, "wooden_buckler"),
        1
    );
}

#[test]
fn two_hand_refuses_when_bag_cannot_hold_off_hand() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Hunt", PlayerClass::Hunter, 0.0, 0.0);
    if let Some(bags) = world.get_mut::<Bags>(1) {
        bags.equipment.off_hand = Some("wooden_buckler".into());
        for i in 0..bags.inventory.len() {
            if bags.inventory[i].is_none() {
                bags.inventory[i] = Some(InvStack {
                    item_id: format!("wolf_fang"),
                    count: 1,
                });
            }
        }
        // free exactly one hole and put worn_bow there
        bags.inventory[0] = Some(InvStack {
            item_id: "worn_bow".into(),
            count: 1,
        });
        // remaining slots already fang-filled; after removing bow, one hole — MH swap uses it, OH has nowhere
        let mh = bags.equipment.main_hand.clone();
        let _ = mh;
    }
    let slot = bag_slot_of(&world, 1, "worn_bow");
    let mut events = Vec::new();
    equip_from_bag(&mut world, 1, slot, &mut events);
    assert_eq!(
        world.get::<Bags>(1).unwrap().equipment.off_hand.as_deref(),
        Some("wooden_buckler")
    );
    assert!(events.iter().any(|e| matches!(
        e,
        SimEvent::Toast { message } if message.contains("Inventory full")
    )));
}
```

Fill every inventory hole except the bow stack so that after consuming the bow there is one hole (enough for old MH, not for OH). If spawn already filled MH with `worn_bow`, old MH is also a bow — still needs a bag hole. Count carefully: displaced = old MH + OH = 2; holes after remove incoming = 1 → refuse.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p woc-sim mage_refuses_sword -- --nocapture`  
Expected: FAIL (mage currently equips the sword).

- [ ] **Step 3: Implement `equip_from_bag`**

Replace the body after resolving `idef`:

```rust
let class = world
    .get::<ClassKit>(player_id)
    .and_then(|k| k.class_id)
    .unwrap_or(PlayerClass::Warrior);
let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
match can_equip(idef, class, level) {
    Ok(()) => {}
    Err(EquipDeny::NotGear) => {
        events.push(SimEvent::Toast { message: "Cannot equip that.".into() });
        return;
    }
    Err(EquipDeny::LevelReq(n)) => {
        events.push(SimEvent::Toast { message: format!("Requires level {n}.") });
        return;
    }
    Err(EquipDeny::WrongClass) => {
        events.push(SimEvent::Toast {
            message: "Your class cannot equip that.".into(),
        });
        return;
    }
    Err(EquipDeny::WrongArmor) => {
        events.push(SimEvent::Toast {
            message: "Your class cannot wear that armor.".into(),
        });
        return;
    }
}

let two_hand = matches!(
    idef.weapon_style,
    Some(WeaponStyle::TwoHand | WeaponStyle::Ranged)
);
if idef.equip_slot == Some(ItemEquipSlot::OffHand) {
    let mh = world
        .get::<Bags>(player_id)
        .and_then(|b| b.equipment.main_hand.clone());
    if let Some(id) = mh {
        if let Some(cur) = item(&id) {
            if matches!(
                cur.weapon_style,
                Some(WeaponStyle::TwoHand | WeaponStyle::Ranged)
            ) {
                events.push(SimEvent::Toast {
                    message: "Cannot dual-wield a two-handed weapon.".into(),
                });
                return;
            }
        }
    }
}

// Preflight bag holes, then mutate (remove incoming, place, grant displaced).
```

Count empty inventory slots. After a successful `remove_item` of 1, holes increase by 1 if that stack emptied. Displaced items: current target slot contents always; plus `OffHand` if `two_hand`. If `displaced.len() > holes_after_remove`, toast and return **before** mutating.

Then existing replace/grant path. Call `recalc_player_stats`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim --lib interaction::`  
Expected: PASS including `equip_and_unequip_gear`, `refuse_low_level_equip`, new tests.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/interaction.rs
git commit -m "feat(sim): class, armor, and two-hand equip rules"
```

---

### Task 6: Stat recalc, spawn set, snapshot sheet

**Files:**
- Modify: `crates/woc-sim/src/stats.rs`
- Modify: `crates/woc-sim/src/ecs/spawn.rs`
- Modify: `crates/woc-sim/src/sim.rs` (if Task 4 did not yet fill AP/armor/SP)
- Test: `crates/woc-sim/src/stats.rs`

**Interfaces:**
- Consumes: `ItemDef.stamina` / `spell_power`; eight `Equipment` slots
- Produces: `hp_max` includes `sta * 2.0`; `Combat.spell_power`; warrior/mage spawn fills cloth extras

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Combat, Health};
    use crate::ecs::spawn::create_player;
    use crate::ecs::World;
    use woc_content::PlayerClass;

    #[test]
    fn warrior_spawns_full_cloth_extras() {
        let mut world = World::new();
        create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        let eq = &world.get::<Bags>(1).unwrap().equipment;
        assert_eq!(eq.main_hand.as_deref(), Some("worn_sword"));
        assert_eq!(eq.chest.as_deref(), Some("recruit_tunic"));
        assert_eq!(eq.head.as_deref(), Some("recruit_cap"));
        assert_eq!(eq.legs.as_deref(), Some("recruit_pants"));
        assert_eq!(eq.feet.as_deref(), Some("recruit_boots"));
        assert!(eq.off_hand.is_none());
        assert!(eq.neck.is_none());
    }
}
```

Do **not** add `pendant_raises_hp_max` here — `fang_pendant` is authored in Task 8. Recalc still walks `stamina`/`spell_power` (zeros on starter gear).

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p woc-sim warrior_spawns_full_cloth_extras -- --nocapture`  
Expected: FAIL (`head` is `None`).

- [ ] **Step 3: Implement**

`create_player` equipment:

```rust
equipment: Equipment {
    main_hand: Some(def.start_weapon.to_string()),
    chest: Some(def.start_chest.to_string()),
    head: Some("recruit_cap".into()),
    legs: Some("recruit_pants".into()),
    feet: Some("recruit_boots".into()),
    ..Default::default()
},
```

`stats.rs` `add_gear_stats` also accumulates `stamina` and `spell_power`. Walk `neck` and `finger`. `hp_max` uses `sta * 2.0`. Write `c.spell_power = sp`.

Snapshot fields from `Combat`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim warrior_spawns_full_cloth_extras -- --nocapture`  
Expected: PASS. Also `cargo test -p woc-sim --lib` for persist virgin / equip tests.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/stats.rs crates/woc-sim/src/ecs/spawn.rs crates/woc-sim/src/sim.rs
git commit -m "feat(sim): starter paper doll and stamina/spell-power recalc"
```

---

### Task 7: Combat uses spell power

**Files:**
- Modify: `crates/woc-sim/src/combat.rs` (`apply_ability_effect`, `apply_direct_heal`)
- Test: `crates/woc-sim/src/combat.rs`

**Interfaces:**
- Consumes: `Combat.spell_power`
- Produces: heal amount `(def.damage + spell_power * 0.5) * coefficient * heal_mult`; spell damage `def.damage + attack * 0.35 + spell_power * 0.5`

- [ ] **Step 1: Failing test**

Use priest `flash_heal` / `Heal` kit ability. Pattern matches existing heal tests in `combat.rs` (copy the nearest `create_player` + `apply_ability_effect` setup). Assert HP restored is higher when `Combat.spell_power = 10.0` than when `0.0`, same HP-before.

```rust
#[test]
fn spell_power_increases_priest_heal() {
    fn heal_once(sp: f32) -> f32 {
        let mut world = World::new();
        let mut rng = Rng::new(1);
        crate::ecs::spawn::create_player(&mut world, 1, "P", PlayerClass::Priest, 0.0, 0.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 20.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.spell_power = sp;
            c.target = Some(1);
        }
        let def = woc_content::ability("flash_heal").expect("flash_heal");
        let mut events = Vec::new();
        apply_ability_effect(&mut world, &mut rng, 1, def, &mut events);
        world.get::<Health>(1).unwrap().hp
    }
    assert!(heal_once(10.0) > heal_once(0.0) + 4.0);
}
```

If `flash_heal` needs a living friendly target of 1, `heal_target` already allows self. If GCD/resource blocks `apply_ability_effect`, call `apply_direct_heal` if you make it `pub(crate)` — prefer not; `apply_ability_effect` is the public seam.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p woc-sim spell_power_increases_priest_heal -- --nocapture`  
Expected: FAIL (heals ignore `spell_power`).

- [ ] **Step 3: Implement**

In `apply_ability_effect`:

```rust
let spell = world
    .get::<Combat>(src)
    .map(|c| c.spell_power)
    .unwrap_or(0.0);
let weapon = def.damage + attack * 0.35 + spell * 0.5;
```

`WeaponDamage` / Execute / Cleave / auto-attack must **not** use `spell`. Keep a `melee` value:

```rust
let melee = def.damage + attack * 0.35;
let spell_hit = def.damage + attack * 0.35 + spell * 0.5;
```

Use `melee` for `WeaponDamage`, `AoeDamage` (cleave), `Execute`, `Charge`. Use `spell_hit` for `SpellDamage` and harm side of `HealOrHarm`.

`apply_direct_heal`:

```rust
let sp = world.get::<Combat>(src).map(|c| c.spell_power).unwrap_or(0.0);
let amount = match hit {
    HitResult::Miss | HitResult::Hit => (def.damage + sp * 0.5) * coefficient * heal_mult,
    HitResult::Crit => { toast_crit(...); (def.damage + sp * 0.5) * coefficient * CRIT_MULT * heal_mult }
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim spell_power_increases_priest_heal -- --nocapture`  
Expected: PASS. Run existing combat tests: `cargo test -p woc-sim --lib combat::`  
Expected: PASS (melee numbers unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/combat.rs
git commit -m "feat(sim): spell power feeds heals and spell damage"
```

---

### Task 8: Independent loot rolls + locked drops

**Files:**
- Modify: `crates/woc-sim/src/combat.rs` (`spawn_mob_loot`)
- Modify: `crates/woc-content/src/items.rs` / `items_zone2.rs` (new items)
- Modify: `crates/woc-content/src/mobs.rs` (`crypt_warden` + scarred_wolf/boar extra rows)
- Modify: `crates/woc-content/src/mobs_zone2.rs` (`bog_wisp`, `barrow_hag`)
- Modify: `crates/woc-content/src/lib.rs` (integrity: every `DungeonDef.boss_id` resolves in `MOBS`)
- Test: `crates/woc-sim/src/combat.rs`, `crates/woc-content/src/lib.rs`

**Interfaces:**
- Consumes: `LootEntry` slices; `Rng::next_f32`
- Produces: one loot entity per successful entry; copper on the first pile; new item ids from spec §5.5

- [ ] **Step 1: Failing tests**

Content:

```rust
#[test]
fn dungeon_bosses_have_mob_templates() {
    for d in DUNGEONS {
        assert!(
            mob(d.boss_id).is_some(),
            "boss {} missing MobTemplate",
            d.boss_id
        );
    }
}
```

Sim (seeded): force both rolls by stubbing chances to 1.0 in the test via a dedicated template **or** call `spawn_mob_loot` after temporarily relying on real 1.0 rows (`barrow_hag` claw 1.0 + focus 1.0):

```rust
#[test]
fn independent_loot_can_drop_two_items() {
    let mut world = World::new();
    let mut rng = Rng::new(1);
    // barrow_hag: hag_claw 1.0 and hag_focus 1.0 → two piles
    let _ = spawn_mob_loot(&mut world, &mut rng, Some("barrow_hag"), 0.0, 0.0);
    let piles: Vec<_> = world
        .ids::<LootPile>()
        .into_iter()
        .filter_map(|id| world.get::<LootPile>(id).and_then(|p| p.item.clone()))
        .collect();
    assert!(piles.iter().any(|i| i == "hag_claw"));
    assert!(piles.iter().any(|i| i == "hag_focus"));
}

#[test]
fn crypt_warden_drops_cleaver() {
    let mut world = World::new();
    let mut rng = Rng::new(1);
    spawn_mob_loot(&mut world, &mut rng, Some("crypt_warden"), 1.0, 1.0);
    let items: Vec<_> = world
        .ids::<LootPile>()
        .into_iter()
        .filter_map(|id| world.get::<LootPile>(id).and_then(|p| p.item.clone()))
        .collect();
    assert_eq!(items, vec!["crypt_cleaver".to_string()]);
}
```

Also restore Task 6 `pendant_raises_hp_max` here once `fang_pendant` exists (`stamina: 4.0`).

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p woc-content dungeon_bosses_have_mob_templates -- --nocapture`  
Expected: FAIL (`crypt_warden` missing).  
Run: `cargo test -p woc-sim independent_loot_can_drop_two_items -- --nocapture`  
Expected: FAIL (`break` after first entry / missing items).

- [ ] **Step 3: Implement**

`spawn_mob_loot`:

```rust
pub fn spawn_mob_loot(
    world: &mut World,
    rng: &mut Rng,
    template_id: Option<&str>,
    x: f32,
    z: f32,
) -> EntityId {
    let Some(tid) = template_id.and_then(mob) else {
        let copper = rng.gen_range_u32(3, 8);
        let id = world.next_id();
        return crate::ecs::spawn::create_loot(world, id, x, z, copper, None);
    };
    let copper = rng.gen_range_u32(tid.copper_min, tid.copper_max);
    let mut dropped: Vec<String> = Vec::new();
    for entry in tid.loot {
        if rng.next_f32() < entry.chance {
            dropped.push(entry.item_id.to_string());
        }
    }
    if dropped.is_empty() {
        let id = world.next_id();
        return crate::ecs::spawn::create_loot(world, id, x, z, copper, None);
    }
    let mut first = EntityId::default();
    for (i, item_id) in dropped.into_iter().enumerate() {
        let id = world.next_id();
        let c = if i == 0 { copper } else { 0 };
        crate::ecs::spawn::create_loot(world, id, x + i as f32 * 0.4, z, c, Some(item_id));
        if i == 0 {
            first = id;
        }
    }
    first
}
```

Offset piles by `0.4` yd so they are distinct positions but still corpse-adjacent.

Author items with `jewelry(...)` / `weapon(...)` helpers (`fang_pendant`, `boar_tusk_ring`, `crypt_cleaver`, `fen_staff`, `hag_focus`) exactly as spec §5.5.

`crypt_warden` `MobTemplate`: level 3, hp 240, xp 150, copper 20–40, attack 14, loot `crypt_cleaver` 1.0. Numbers may match `DungeonDef` but spawn_boss_shell does not read them except via `spawn_mob_loot` + `template_id`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-content --lib` and `cargo test -p woc-sim crypt_warden_drops_cleaver independent_loot_can_drop_two_items pendant_raises_hp_max -- --nocapture`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content crates/woc-sim/src/combat.rs crates/woc-sim/src/stats.rs
git commit -m "feat(content/sim): gear drops and independent loot rolls"
```

---

### Task 9: Bevy bags and character sheet

**Files:**
- Modify: `crates/woc-client/src/hud.rs`
- Modify: `crates/woc-client/src/input.rs`
- Test: none (GPU). Gate: `cargo check -p woc-client`

**Interfaces:**
- Consumes: `TickSnapshot.{attack_power,armor,spell_power,equipment.neck/finger,inventory[].slot}`; `can_equip`; `progress.class_id/level`
- Produces: C-sheet eight slots + AP/Armor/SP; bags keys 1–9; C-panel keys 1–8 unequip; Q uses `can_equip`

- [ ] **Step 1: Update `first_equippable_bag_stack`**

```rust
pub(crate) fn first_equippable_bag_stack(snap: &TickSnapshot) -> Option<(u8, String)> {
    let class = PlayerClass::parse(&snap.progress.class_id)?;
    let level = snap.progress.level;
    snap.inventory.iter().find_map(|stack| {
        let def = item(&stack.item_id)?;
        can_equip(def, class, level).ok()?;
        Some((stack.slot, stack.item_id.clone()))
    })
}
```

- [ ] **Step 2: Character sheet text**

Replace the equipment block in the C-panel formatter with eight lines plus:

```text
AP: {attack_power:.0}   Armor: {armor:.0}   SP: {spell_power:.0}
[1-8] Unequip slot
```

- [ ] **Step 3: Input**

When `ui.show_bags` and digit 1–9 just pressed: find `inventory` row whose `slot == digit-1`; if `can_equip` ok → `InteractAction::Equip { bag_slot }`; else if consumable → `UseItem`; else toast `Cannot use that.`

When `ui.show_character` and digit 1–8: `Unequip` for `[MainHand, OffHand, Head, Chest, Legs, Feet, Neck, Finger][digit-1]`.

Keep ability keys disabled while bags **or** character sheet is open (character sheet currently must not fire abilities). If `show_character` does not already block 1–5, add `&& !ui.show_character` next to `!ui.show_bags`.

- [ ] **Step 4: Check client**

Run: `cargo check -p woc-client`  
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/hud.rs crates/woc-client/src/input.rs
git commit -m "feat(client): gear sheet stats and numbered equip"
```

---

### Task 10: Version, docs, demo

**Files:**
- Modify: `VERSION.toml` (`rewrite_version = "1.9.0"`, `parity_target = "gear-depth"`)
- Modify: workspace `Cargo.toml` version if it tracks rewrite
- Modify: `crates/woc-version/src/lib.rs` if constants are duplicated
- Modify: `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`, `README.md`, `UPSTREAM.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: shipped Tasks 1–9
- Produces: tag-ready 1.9.0 docs; demo step 8 for gear

- [ ] **Step 1: STATUS table**

Add under a `## Gear depth (\`gear-depth\`)` heading:

| Subsystem | Status | Notes |
| --- | --- | --- |
| `can_equip` class/armor | done | Cloth→Plate caps; weapon `allowed_classes` |
| Two-hand occupancy | done | Bow/staff/cleaver clear OH |
| Jewelry | done | Neck + one Finger |
| Stamina / spell power | done | `sta*2` HP; SP on heal/spell |
| Independent loot | done | One pile per successful `LootEntry` |
| Crypt / hag gear | done | `crypt_cleaver` / `hag_focus` |
| Client sheet | done | AP/Armor/SP; 1–9 bags; 1–8 unequip |

- [ ] **Step 2: ROADMAP row**

`**1.9.0** (this branch) | \`gear-depth\` | Class gear rules, jewelry, secondary stats, upgrade drops`

- [ ] **Step 3: DEMO**

Append: `8. Warrior spawn shows a full cloth extra set; mage cannot equip a sword; crypt warden drops crypt_cleaver; C-sheet AP/Armor/SP update on equip.`

Footer version `1.9.0`.

- [ ] **Step 4: CHANGELOG**

Under `## 1.9.0`, summarize rules, slots, stats, loot, client.

- [ ] **Step 5: Full gate + commit**

Run: `cargo test --workspace --exclude woc-client`  
Expected: PASS.  
Run: `cargo check -p woc-client`  
Expected: success.

```bash
git add VERSION.toml Cargo.toml crates/woc-version docs CHANGELOG.md README.md UPSTREAM.md
git commit -m "docs: mark 1.9.0 gear-depth shipped"
```

---

## Main-agent merge playbook

Serial path ownership (avoid parallel edits on `items.rs` + `combat.rs` in the same wave):

| Wave | Tasks | Exclusive paths |
| --- | --- | --- |
| 1 | Task 1–2 | `crates/woc-content/src/items*.rs`, `lib.rs` |
| 2 | Task 3 | `crates/woc-protocol/src/lib.rs` |
| 3 | Task 4–6 | `woc-sim` ecs/stats/spawn/interaction/persist, `woc-persist`, `woc-server` bridge |
| 4 | Task 7–8 | `combat.rs` + mob/item tables |
| 5 | Task 9 | `woc-client` |
| 6 | Task 10 | docs/version |

After each wave: `cargo test --workspace --exclude woc-client`. After wave 5: `cargo check -p woc-client`.

## Spec coverage

| Spec § | Task |
| --- | --- |
| 5.1 `can_equip` / armor cap | 1–2 |
| 5.2 two-hand | 5 |
| 5.3 stats + combat SP | 6–7 |
| 5.4 spawn set | 6 |
| 5.5 loot + items | 8 |
| 5.6 protocol | 3 |
| 5.7 client | 9 |
| 5.8 persist | 4 |
| DoD / demo / 1.9.0 | 10 |
| Non-goals (durability, quality, dual-wield, extra slots) | none — skipped |
