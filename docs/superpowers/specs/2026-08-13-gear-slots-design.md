# Gear-slots design — `1.13.0` / `gear-slots`

**Status:** Proposed.  
**Baseline:** rewrite `1.12.0` / `gear-depth` on `develop` (ECS `World`; durability already in `1.11.0`).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `gear-slots`.

Related: gear-depth [`2026-08-13-gear-depth-design.md`](2026-08-13-gear-depth-design.md); NPC-services durability [`2026-08-13-npc-services-design.md`](2026-08-13-npc-services-design.md).

## 1. Goal

The paper doll grows by **one ring** and **off-hand weapons for dual-wield classes**. Gear shows a **catalog quality** and a **main-hand enchant** applied from a vendor oil/stone. Durability/repair is already shipped — this program does not reimplement wear.

> Equip a second one-hander as a rogue. Wear two rings. Buy a stone, use it, see AP rise.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Slots | MH, OH, Head, Chest, Legs, Feet, Neck, Finger |
| Off-hand | Shields only (`WeaponStyle::Shield`) |
| Dual-wield | Blocked; toast `"Cannot dual-wield a two-handed weapon."` |
| Durability | `InvStack.durability` + `EquipmentWear`; RepairAll at smith |
| Quality / enchants | None |
| Protocol | Rev **8**; additive `#[serde(default)]` |

## 3. Approaches considered

| Approach | What it does | Verdict |
| --- | --- | --- |
| **A. Full manufacturing** | Crafted quality rolls, sockets, profession enchants | Separate program; too large |
| **B. 16-slot WoW doll** | Shoulders, back, wrists, trinkets | YAGNI vs one extra ring |
| **C. Dual-wield + Finger2 + catalog quality + MH enchant (recommended)** | Class dual-wield, second Finger, `ItemQuality` multiplier, one MH enchant from consumables | **Adopt** |

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.12.0** | `gear-depth` | Rules, jewelry, stamina/SP (shipped) |
| **1.13.0** | `gear-slots` | Dual-wield, Finger2, quality, MH enchant |

`PROTOCOL_REV` stays **8**. New fields use `#[serde(default)]`. Upstream pin stays **0.31.0**. Equipment stays on `Bags`. No new ECS actor column. Tick fingerprint unchanged.

## 5. Architecture

### 5.1 Dual-wield

`can_dual_wield(class) -> bool` is true only for **Warrior** and **Rogue**.

One-hand weapons remain `ItemEquipSlot::MainHand` / `WeaponStyle::OneHand` in the catalog. The sim **routes** a legal OneHand into `OffHand` when:

1. `can_equip` succeeds.
2. `can_dual_wield(class)`.
3. `MainHand` is occupied by a OneHand (not TwoHand/Ranged).
4. `OffHand` is empty **or** the incoming item is being placed because MH is full — if OH is empty, fill OH; if both full, replace MH (current replace behavior).

If a non-dual-wield class would be routed to OH, do not route; replace MH as today.

Putting a shield or any item into OH while MH is TwoHand/Ranged still toasts `"Cannot dual-wield a two-handed weapon."`

Non-dual-wield class attempting to put a OneHand into OH (if we ever send `EquipSlot::OffHand` for a MH item): toast `"Your class cannot dual-wield."`

OH one-hand AP remains **× 0.25**. Shields remain 0 AP.

Off-hand catalog items (`equip_slot: OffHand`) stay shields only. Integrity test unchanged: every `ItemEquipSlot::OffHand` row is `WeaponStyle::Shield`.

### 5.2 Second ring (`Finger2`)

`ItemEquipSlot` stays `Finger` on ring defs. Protocol / sim gain `EquipSlot::Finger2` and `Equipment.finger2`.

When equipping a Finger item:

1. If `finger` empty → `Finger`.
2. Else if `finger2` empty → `Finger2`.
3. Else replace `Finger`.

Unequip keys on the character sheet: **1–9** = MH, OH, Head, Chest, Legs, Feet, Neck, Finger, Finger2.

`recalc_player_stats` sums `finger2`. Jewelry still has `max_durability = 0` (no wear column).

### 5.3 Quality

```rust
pub enum ItemQuality { Poor, Common, Uncommon, Rare }
pub fn quality_mult(q: ItemQuality) -> f32 {
    match q {
        ItemQuality::Poor => 0.9,
        ItemQuality::Common => 1.0,
        ItemQuality::Uncommon => 1.1,
        ItemQuality::Rare => 1.2,
    }
}
```

`ItemDef.quality` defaults to `Common` for gear, ignored for non-gear. `add_gear_stats` multiplies `attack_power`, `armor`, `stamina`, `spell_power` by `quality_mult` **before** the off-hand 0.25 AP factor.

Locked tags:

| Quality | Items |
| --- | --- |
| Common | starter worn_* / recruit_* / class start chest+weapon |
| Uncommon | `copper_shortsword`, leather zone pieces, `fang_pendant`, `boar_tusk_ring`, `veteran_helm` |
| Rare | `crypt_cleaver`, `fen_staff`, `hag_focus` |

No Poor rows required. HUD shows quality as a prefix on equipped names when not Common (`Uncommon Fang Pendant`).

### 5.4 Main-hand enchant

Content table `ENCHANTS: &[EnchantDef]`:

```rust
pub struct EnchantDef {
    pub id: &'static str,
    pub name: &'static str,
    pub attack_power: f32,
    pub stamina: f32,
    pub spell_power: f32,
}
```

Locked rows:

| id | name | stats |
| --- | --- | --- |
| `coarse_sharpening` | Coarse Sharpening | AP +6 |
| `minor_wizard_oil` | Minor Wizard Oil | SP +6 |

Consumables with `ItemDef.enchant_id: Option<&'static str>` (None for non-oils). Using the stack applies that enchant to **equipped MainHand** if MH is a weapon (`kind == Weapon`). Otherwise toast `"Nothing to enchant."`

`Bags.equipment_enchants` is `EquipmentEnchants { main_hand: Option<String> }` only. Recalc adds the enchant's AP/sta/SP (not quality-multiplied) after gear sums.

On unequip MH, copy `equipment_enchants.main_hand` onto `InvStack.enchant_id` the same way durability moves. On equip MH, copy stack `enchant_id` onto `equipment_enchants.main_hand`. Replacing MH with a new weapon drops the old enchant onto the displaced stack.

Vendor: Smith Brann stocks `coarse_whetstone` (applies `coarse_sharpening`) and `minor_wizard_oil` (applies `minor_wizard_oil`), `vendor_buy` 15, `vendor_sell` 3, `stack_size` 5.

Toasts (English, locked): `"Enchanted {item name}."` / `"Nothing to enchant."`

### 5.5 Durability

No change to wear math, repair, or `max_durability`. Finger2 has no wear field. Enchant consumables have `max_durability = 0`.

### 5.6 Protocol / persist

Rev stays **8**. Additive:

- `EquipSlot::Finger2`
- `EquipmentSnapshot.finger2: Option<String>`
- `EquipmentSnapshot.main_hand_enchant: Option<String>`
- `InvSlotSnapshot.enchant_id: Option<String>`
- persist `EquipmentDto.finger2`, `main_hand_enchant`; inventory JSON already stores whole stacks — add `enchant_id` on persist inventory entries if they are structured; if stacks are `{item_id,count,durability}`, add `enchant_id` with serde default.

Old JSON without these keys deserializes to `None`.

### 5.7 Client

Character sheet lists Finger2 and MH enchant name. Unequip **1–9**. Bags still **1–9** while bags are open (C closed). Quality prefix on gear labels. Digit keys still blocked when C is open.

## 6. Definition of done

1. Rogue with MH dagger equips a second OneHand into OH; mage with staff cannot put a dagger in OH.
2. Two Finger items occupy Finger then Finger2; third replaces Finger.
3. `hag_focus` quality Rare: SP contribution is `8 * 1.2 = 9.6` (existing +8 test updates).
4. Use whetstone with a sword equipped: `Combat.attack_damage` rises by 6; persist roundtrip keeps `main_hand_enchant`.
5. Broken MH (durability 0) still grants no weapon AP; enchant AP is also skipped while MH is broken.
6. `PROTOCOL_REV == 8`. Tick fingerprint unchanged.
7. Footer `1.13.0`. Demo line: rogue dual-wield + two rings + Brann whetstone.

## 7. Out of scope

Shoulders/back/wrists/waist/hands/trinkets. Dual-wield for Hunter/Shaman. Off-hand enchant. Random loot quality rolls. Profession-crafted enchants. Sockets/sets. Re-implementing durability/repair.
