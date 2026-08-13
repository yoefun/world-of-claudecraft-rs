# Gear-slots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rewrite `1.13.0` / `gear-slots`: warrior/rogue dual-wield into OffHand, Finger2, catalog `ItemQuality` multipliers, and a main-hand enchant from vendor oils.

**Architecture:** Equipment stays on `Bags`. OneHand items stay `ItemEquipSlot::MainHand` in content; the sim routes a second OneHand to `OffHand` when `can_dual_wield`. Finger items fill `finger` then `finger2`. Quality multiplies gear stats in `add_gear_stats`. Enchants live on `InvStack.enchant_id` and `Bags.equipment_enchants.main_hand`. Protocol rev stays 8 with serde defaults.

**Tech Stack:** Rust 2021, existing crates, Bevy 0.16 client, protocol rev 8, upstream 0.31.0.

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- `PROTOCOL_REV` remains **8**. New fields `#[serde(default)]`.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- Equipment stays on `Bags`. Do not add a fat `Entity` or a parallel Gear column.
- Tick-phase fingerprint stays `15038642330132466611`.
- English-only toasts. Locked copy: `"Your class cannot dual-wield."`, `"Nothing to enchant."`, `"Enchanted {name}."`
- Dual-wield classes: Warrior, Rogue only.
- Do not change durability/repair math except skip MH enchant AP/sta/SP when MH wear is `Some(0)`.
- Author for commits: `yoefun <xinglinsky@outlook.com>`.
- Branch: `cursor/gear-slots-a6b7`.

## File map

- `crates/woc-content/src/items.rs` — quality, enchant_id, can_dual_wield, ENCHANTS, oils
- `crates/woc-content/src/lib.rs` — re-exports + integrity
- `crates/woc-content/src/npcs.rs` — Brann stock
- `crates/woc-protocol/src/lib.rs` — Finger2, snapshot/stack fields
- `crates/woc-sim/src/ecs/components.rs` — finger2, InvStack.enchant_id, EquipmentEnchants
- `crates/woc-sim/src/interaction.rs` — dual-wield route, Finger fill, Use enchant
- `crates/woc-sim/src/stats.rs` — quality + finger2 + enchant
- `crates/woc-sim/src/persist_state.rs` / `woc-persist` — DTO fields
- `crates/woc-sim/src/sim.rs` — snapshot
- `crates/woc-client/src/hud.rs` / `input.rs` — 9 slots, labels
- `VERSION.toml`, `Cargo.toml`, changelog, README, ROADMAP, DEMO, STATUS

---

### Task 1: Content — quality, dual-wield helper, enchants, oils

**Files:**
- Modify: `crates/woc-content/src/items.rs`
- Modify: `crates/woc-content/src/items_zone2.rs` (quality on zone2 gear)
- Modify: `crates/woc-content/src/lib.rs` (re-export + tests)
- Modify: `crates/woc-content/src/npcs.rs` (Brann vendor_stock)

**Produces:** `ItemQuality`, `quality_mult`, `can_dual_wield`, `EnchantDef`, `ENCHANTS`, `enchant()`, `ItemDef.quality`, `ItemDef.enchant_id`

- [ ] **Step 1: Failing tests** in `crates/woc-content/src/lib.rs`:

```rust
#[test]
fn dual_wield_classes() {
    assert!(can_dual_wield(PlayerClass::Warrior));
    assert!(can_dual_wield(PlayerClass::Rogue));
    assert!(!can_dual_wield(PlayerClass::Mage));
    assert!(!can_dual_wield(PlayerClass::Hunter));
}

#[test]
fn rare_hag_focus_and_oils() {
    assert_eq!(item("hag_focus").unwrap().quality, ItemQuality::Rare);
    assert_eq!(quality_mult(ItemQuality::Rare), 1.2);
    assert_eq!(item("coarse_whetstone").unwrap().enchant_id, Some("coarse_sharpening"));
    assert!(enchant("coarse_sharpening").is_some());
}
```

- [ ] **Step 2: Run** `cargo test -p woc-content dual_wield_classes -- --nocapture` — FAIL (unresolved)

- [ ] **Step 3: Implement**

Add to `items.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemQuality { Poor, Common, Uncommon, Rare }

pub fn quality_mult(q: ItemQuality) -> f32 {
    match q {
        ItemQuality::Poor => 0.9,
        ItemQuality::Common => 1.0,
        ItemQuality::Uncommon => 1.1,
        ItemQuality::Rare => 1.2,
    }
}

pub fn can_dual_wield(class: PlayerClass) -> bool {
    matches!(class, PlayerClass::Warrior | PlayerClass::Rogue)
}

pub struct EnchantDef {
    pub id: &'static str,
    pub name: &'static str,
    pub attack_power: f32,
    pub stamina: f32,
    pub spell_power: f32,
}

pub static ENCHANTS: &[EnchantDef] = &[
    EnchantDef { id: "coarse_sharpening", name: "Coarse Sharpening", attack_power: 6.0, stamina: 0.0, spell_power: 0.0 },
    EnchantDef { id: "minor_wizard_oil", name: "Minor Wizard Oil", attack_power: 0.0, stamina: 0.0, spell_power: 6.0 },
];

pub fn enchant(id: &str) -> Option<&'static EnchantDef> {
    ENCHANTS.iter().find(|e| e.id == id)
}
```

Add `quality: ItemQuality` and `enchant_id: Option<&'static str>` to `ItemDef`. Update every constructor to set `quality: ItemQuality::Common` and `enchant_id: None`. Tag Uncommon/Rare per spec. Add consumables:

```rust
ItemDef {
    id: "coarse_whetstone",
    name: "Coarse Whetstone",
    kind: ItemKind::Consumable,
    stack_size: 5,
    max_durability: 0,
    vendor_buy: 15,
    vendor_sell: 3,
    /* zeros for combat fields */
    enchant_id: Some("coarse_sharpening"),
    quality: ItemQuality::Common,
    /* remaining fields as other consumables */
}
```

Same for `minor_wizard_oil`. Append both to Brann `vendor_stock` with count 20.

Every existing `ItemDef {` literal (zone2 included) must set the two new fields.

- [ ] **Step 4:** `cargo test -p woc-content` PASS; `cargo clippy -p woc-content -- -D warnings` PASS

- [ ] **Step 5: Commit** `feat(content): quality, dual-wield helper, and MH enchant oils`

---

### Task 2: Protocol additive Finger2 + enchant fields

**Files:** Modify `crates/woc-protocol/src/lib.rs`

**Produces:** `EquipSlot::Finger2`; `EquipmentSnapshot.finger2`, `main_hand_enchant`; `InvSlotSnapshot.enchant_id`; `PROTOCOL_REV` still 8

- [ ] **Step 1: Tests**

```rust
#[test]
fn finger2_and_enchant_defaults() {
    let eq: EquipmentSnapshot = serde_json::from_str(r#"{"main_hand":null,"off_hand":null,"chest":null}"#).unwrap();
    assert!(eq.finger2.is_none());
    assert!(eq.main_hand_enchant.is_none());
    let slot: InvSlotSnapshot = serde_json::from_str(r#"{"item_id":"x","count":1}"#).unwrap();
    assert!(slot.enchant_id.is_none());
    assert_eq!(PROTOCOL_REV, 8);
}

#[test]
fn unequip_finger2_roundtrip() {
    let a = InteractAction::Unequip { equip_slot: EquipSlot::Finger2 };
    let v = serde_json::to_value(&a).unwrap();
    let back: InteractAction = serde_json::from_value(v).unwrap();
    assert!(matches!(back, InteractAction::Unequip { equip_slot: EquipSlot::Finger2 }));
}
```

- [ ] **Step 2:** FAIL
- [ ] **Step 3:** Add enum variant + serde default fields. Update `Default for TickSnapshot` if it constructs EquipmentSnapshot by field.
- [ ] **Step 4:** `cargo test -p woc-protocol` PASS
- [ ] **Step 5: Commit** `feat(protocol): Finger2 and main-hand enchant snapshot fields`

---

### Task 3: Sim storage, persist, dual-wield + Finger routing

**Files:**
- Modify: `crates/woc-sim/src/ecs/components.rs`
- Modify: `crates/woc-sim/src/interaction.rs`
- Modify: `crates/woc-sim/src/persist_state.rs`
- Modify: `crates/woc-persist/src/models.rs` (+ serialize tests)
- Modify: `crates/woc-server/src/bridge.rs` if it maps EquipmentDto
- Modify: `crates/woc-sim/src/sim.rs` snapshot mapping

**Produces:** `Equipment.finger2`, `InvStack.enchant_id`, `EquipmentEnchants { main_hand }`, routing helpers

- [ ] **Step 1: Tests** in `interaction.rs`:

```rust
#[test]
fn rogue_second_dagger_goes_off_hand() { /* spawn rogue, put worn_dagger on MH, grant second worn_dagger, Equip bag 0, assert OH Some */ }

#[test]
fn mage_second_weapon_replaces_main_hand() { /* mage with staff; grant worn_dagger if can_equip fails that's ok — use worn_staff already MH, grant another worn_staff, equip, OH still none */ }

#[test]
fn second_ring_fills_finger2() { /* equip fang_pendant? that's neck. Use boar_tusk_ring twice */ }
```

Need a second ring id for two rings: equip `boar_tusk_ring` then another `boar_tusk_ring` (same id twice is allowed).

- [ ] **Step 2:** FAIL (finger2 field missing)
- [ ] **Step 3:** Wire fields. Routing:

```rust
fn resolve_equip_slot(world: &World, player_id: EntityId, idef: &ItemDef, class: PlayerClass) -> EquipSlot {
    if idef.equip_slot == Some(ItemEquipSlot::Finger) {
        let eq = &world.get::<Bags>(player_id).unwrap().equipment;
        if eq.finger.is_none() { return EquipSlot::Finger; }
        if eq.finger2.is_none() { return EquipSlot::Finger2; }
        return EquipSlot::Finger;
    }
    if idef.weapon_style == Some(WeaponStyle::OneHand) && can_dual_wield(class) {
        if let Some(bags) = world.get::<Bags>(player_id) {
            if bags.equipment.main_hand.is_some() && bags.equipment.off_hand.is_none() {
                if let Some(mh) = bags.equipment.main_hand.as_deref().and_then(item) {
                    if !matches!(mh.weapon_style, Some(WeaponStyle::TwoHand | WeaponStyle::Ranged)) {
                        return EquipSlot::OffHand;
                    }
                }
            }
        }
    }
    to_protocol_slot(idef.equip_slot.unwrap())
}
```

Move `enchant_id` like durability on equip/unequip for MH only.

Snapshot: `finger2`, `main_hand_enchant`. Persist DTO same.

Extend slot match arms (Finger2) everywhere Equipment is matched (interaction, combat wear — Finger2 has no wear, return None).

- [ ] **Step 4:** `cargo test -p woc-sim rogue_second_dagger_goes_off_hand second_ring_fills_finger2` PASS
- [ ] **Step 5: Commit** `feat(sim): dual-wield OffHand routing and Finger2`

---

### Task 4: Stats — quality, finger2, enchant, broken MH skips enchant

**Files:** Modify `crates/woc-sim/src/stats.rs`

- [ ] **Step 1:** Update `hag_focus_raises_spell_power` expected delta to `8.0 * 1.2`. Add:

```rust
#[test]
fn whetstone_enchant_adds_attack_power() {
    let mut world = World::new();
    create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
    recalc_player_stats(&mut world, 1);
    let base = world.get::<Combat>(1).unwrap().attack_damage;
    if let Some(bags) = world.get_mut::<Bags>(1) {
        bags.equipment_enchants.main_hand = Some("coarse_sharpening".into());
    }
    recalc_player_stats(&mut world, 1);
    let enchanted = world.get::<Combat>(1).unwrap().attack_damage;
    assert!((enchanted - base - 6.0).abs() < 0.01);
}

#[test]
fn broken_mh_skips_enchant() {
    /* set wear.main_hand = Some(0) and enchant; AP should not include weapon AP nor +6 */
}
```

- [ ] **Step 2:** FAIL (9.6 vs 8)
- [ ] **Step 3:** `add_gear_stats` multiply by `quality_mult(it.quality)`. Sum `finger2`. After slots, if MH not broken, add enchant stats from `equipment_enchants.main_hand`.
- [ ] **Step 4:** `cargo test -p woc-sim stats::` PASS
- [ ] **Step 5: Commit** `feat(sim): quality multipliers and main-hand enchants in recalc`

---

### Task 5: Use-item applies MH enchant

**Files:** Modify `crates/woc-sim/src/interaction.rs` (use-item path)

- [ ] **Step 1:** Test `use_whetstone_enchants_main_hand` via InteractAction::Use
- [ ] **Step 2:** FAIL
- [ ] **Step 3:** In Use handler, if `def.enchant_id` is Some: require MH weapon; set `equipment_enchants.main_hand`; consume 1; toast `Enchanted {name}.` else `Nothing to enchant.`
- [ ] **Step 4:** PASS
- [ ] **Step 5: Commit** `feat(sim): apply main-hand enchant from oils`

---

### Task 6: Bevy HUD / input

**Files:** `crates/woc-client/src/hud.rs`, `input.rs`

- [ ] Character sheet 9 slots + quality prefix + MH enchant label
- [ ] Unequip keys 1–9 including Finger2
- [ ] `cargo check -p woc-client`
- [ ] Commit `feat(client): Finger2 sheet slot and enchant/quality labels`

---

### Task 7: Version 1.13.0 + docs

**Files:** `VERSION.toml`, `Cargo.toml`, `Cargo.lock`, `crates/woc-version/src/lib.rs`, `CHANGELOG.md`, `README.md`, `UPSTREAM.md`, `docs/ROADMAP.md`, `docs/parity/DEMO.md`, `docs/parity/STATUS.md`

- [ ] rewrite_version `1.13.0`, parity `gear-slots`
- [ ] `cargo test -p woc-version`
- [ ] Commit `docs: mark 1.13.0 gear-slots shipped`

---

## Self-review

1. Spec coverage: dual-wield, Finger2, quality, MH enchant, persist, client, version — tasks 1–7.
2. Durability not reimplemented.
3. Names: `can_dual_wield`, `quality_mult`, `EquipmentEnchants.main_hand`, `EquipSlot::Finger2` consistent.
