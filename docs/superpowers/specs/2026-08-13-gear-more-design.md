# Gear-more design — `1.15.0` / `gear-more`

**Status:** Implemented.  
**Baseline:** rewrite `1.14.0` / `reputation` on `develop` (after `1.13.0` / `gear-slots`).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `gear-more`.

Related: [`2026-08-13-gear-slots-design.md`](2026-08-13-gear-slots-design.md) §7 leftovers.

## 1. Goal

Finish the leftover paper-doll / dual-wield / enchant / loot-quality slice that 1.13.0 explicitly deferred.

> Hunter dual-wields two hatchets. Wear a cloak and a trinket. Oil both weapons. A wolf drop can be Uncommon even when the catalog row is Common.

## 2. Baseline (shipped 1.13.0)

| Piece | State |
| --- | --- |
| Slots | MH, OH, Head, Chest, Legs, Feet, Neck, Finger, Finger2 |
| Dual-wield | Warrior + Rogue only |
| Enchants | MH apply + persist; OH id stored but not shown / not in recalc |
| Quality | Catalog `ItemDef.quality` only |
| Protocol | Rev **8** |

## 3. Approaches considered

| Approach | Verdict |
| --- | --- |
| **A. Full 19-slot WoW doll + manufacturing sockets** | Too large; sockets stay manufacturing |
| **B. Hunter DW + OH sheet enchant only** | Leaves empty doll slots the user named |
| **C. Extra armor+trinket slots + Hunter DW + OH enchant + instance loot quality (recommended)** | Covers every leftover named in 1.13.0 §7 except Shaman DW / sockets |

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.14.0** | `reputation` | Hub factions (shipped) |
| **1.15.0** | `gear-more` | Extra slots, Hunter DW, OH enchant on sheet, loot quality rolls |

`PROTOCOL_REV` stays **8**. New fields `#[serde(default)]`. Equipment stays on `Bags`. Tick fingerprint unchanged. Upstream pin stays **0.31.0**.

## 5. Architecture

### 5.1 Extra doll slots

Add catalog + protocol + sim slots:

`Shoulder`, `Back`, `Wrist`, `Hands`, `Waist`, `Trinket` (+ routed `Trinket2`).

- Shoulder/Back/Wrist/Hands/Waist: armor, `max_durability = 30`, wear columns, included in combat armor-wear rotation.
- Trinket: jewelry (`max_durability = 0`, no wear). Fill `trinket` then `trinket2`, else replace Trinket (same as Finger).
- Integrity: Trinket rows have no `armor_class` / `weapon_style` (with Neck/Finger). Other new slots require `armor_class`.

Locked catalog rows (Common unless noted):

| id | name | slot | notes |
| --- | --- | --- | --- |
| `padded_shoulders` | Padded Shoulders | Shoulder | Cloth, armor 4 |
| `wool_cloak` | Wool Cloak | Back | Cloth, armor 3, Uncommon |
| `frayed_cuffs` | Frayed Cuffs | Wrist | Cloth, armor 2 |
| `work_gloves` | Work Gloves | Hands | Leather, armor 3, Uncommon |
| `frayed_belt` | Frayed Belt | Waist | Cloth, armor 2 |
| `lucky_pebble` | Lucky Pebble | Trinket | sta 2 |

Smith Brann stocks `wool_cloak` and `work_gloves` (count 8). Scarred wolf loot gains `work_gloves` at chance 0.08 (in addition to existing fangs/pelts).

Unequip while C-sheet is open (does not steal WASD):

| Key | Slot |
| --- | --- |
| 1–9 | existing core nine |
| 0 | Shoulder |
| `-` | Back |
| `=` | Wrist |
| `[` | Hands |
| `]` | Waist |
| `;` | Trinket |
| `'` | Trinket2 |

### 5.2 Hunter dual-wield

`can_dual_wield` is Warrior **or** Rogue **or** Hunter.

Hunter still spawns with `worn_bow` (Ranged occupies OH). Dual-wield only after a OneHand is in MH and OH is empty.

New weapon: `worn_hatchet` / Worn Hatchet, OneHand, AP 7, allowed **Hunter** (and Warrior). Wilkes stocks one. Hunter can also equip `copper_shortsword` (add Hunter to its `allowed_classes`).

Shaman dual-wield stays out.

### 5.3 Off-hand enchant on the sheet

Use-oil rule:

1. If MH is a weapon and MH enchant is empty → apply to MH (today).
2. Else if OH is a weapon → apply to OH.
3. Else `"Nothing to enchant."`

Recalc adds OH enchant AP/sta/SP when OH is not broken (full value, not ×0.25; the oil is a second consumable). Broken OH skips OH weapon AP **and** OH enchant.

Protocol `EquipmentSnapshot.off_hand_enchant`. C-sheet Off line shows `[enchant name]` like Main.

### 5.4 Random loot quality

Quality can exist **on the stack**, not only the catalog.

- `InvStack.quality: Option<ItemQuality>` — `None` means use `ItemDef.quality`.
- `EquipmentQualities` on `Bags` (parallel to wear/enchants) copies on equip/unequip like durability.
- `LootPile.quality` set when spawning gear loot.

`roll_loot_quality(rng, catalog) -> ItemQuality`:

```
r = rng.next_f32()
rolled = Poor if r < 0.05 else Common if r < 0.70 else Uncommon if r < 0.92 else Rare
return max(catalog, rolled)  // never downgrade a Rare catalog row
```

Only Weapon/Armor piles roll. Junk/quest/consumable stay `None`.

Stats: `quality_mult(instance.or(catalog))`. HUD prefix uses instance quality when present.

Drop *identity* (which item) is unchanged: quality consumes RNG **after** the drop list is built.

### 5.5 Protocol / persist

Rev **8**. Additive `EquipSlot` variants + snapshot/DTO fields for new slots, `off_hand_enchant`, per-stack `quality` (`snake_case` string), `EquipmentQualities` mapped onto DTO `*_quality` fields with serde default.

## 6. Definition of done

1. `can_dual_wield(Hunter)`; hunter with OneHand MH equips a second OneHand into OH; shaman still does not.
2. Equip cloak → Back; two `lucky_pebble` → Trinket then Trinket2.
3. Second whetstone with dual-wield weapons sets OH enchant; C-sheet Off line shows `[Coarse Sharpening]`; AP includes +6.
4. Forced `rng` sequence can produce Uncommon `work_gloves` on a Common catalog row; persist roundtrip keeps stack quality.
5. `PROTOCOL_REV == 8`. Tick fingerprint `15038642330132466611`.
6. Footer `1.15.0`.

## 7. Out of scope

Shaman dual-wield. Ranged slot (bow stays MH). Sockets/sets. Profession-crafted enchants (`woc-manufacturing`). Extra unequip mouse targeting. Re-doing durability math.
