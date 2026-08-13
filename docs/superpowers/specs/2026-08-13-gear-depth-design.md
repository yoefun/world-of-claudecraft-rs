# Gear-depth design — `1.9.0` / `gear-depth`

**Status:** Proposed (planning deliverable 2026-08-13).  
**Baseline:** rewrite `1.8.0` / `class-forms` on `develop` (ECS `World` actor store).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `gear-depth`.

Post-completion program (shipped): [`2026-08-13-post-completion-program-design.md`](2026-08-13-post-completion-program-design.md).  
Sim ECS (required): [`2026-08-13-sim-ecs-design.md`](2026-08-13-sim-ecs-design.md).  
Related (do **not** reimplement here): durability/repair is owned by the NPC-services draft (`cursor/npc-services-plan-a147`); item quality/enchants are owned by the manufacturing draft (`cursor/manufacturing-system-plan-573b`).

## 1. Goal

Equipment becomes a **class-visible progression loop**: the paper doll has rules, upgrades drop in the world, and the character sheet shows the stats those items actually grant.

Today a mage can wear a sword and a shield, a two-hand staff still leaves the off-hand free, `recalc_player_stats` only sums `attack_power` + `armor`, starter spawn fills two of six slots, and almost every mob drops quest junk rather than gear. `Equip` / `Unequip` / level-req already work.

> Loot a piece that your class can wear. Equip it. See attack power, armor, and spell power change. A two-hander occupies both hands.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Slots | `MainHand`, `OffHand`, `Head`, `Chest`, `Legs`, `Feet` |
| `ItemDef` | `kind`, `equip_slot`, `level_req`, `attack_power`, `armor` — no class, no armor type, no secondary stats |
| Equip | `InteractAction::Equip { bag_slot }` / `Unequip { equip_slot }`; level-req toast |
| Stats | `recalc_player_stats`: class AP + gear AP (off-hand × 0.25) + armor; HP from level + `armor * 0.5` + talent % |
| Combat | Weapon/spell/heal math uses `Combat.attack_damage`; heals ignore gear entirely |
| Spawn | `start_weapon` + `start_chest` only (cap/pants/boots exist but stay in the item table) |
| Loot | `spawn_mob_loot` rolls `LootEntry` rows then **`break`s on the first success** — one item per kill |
| Dungeon bosses | `crypt_warden` has **no** `MobTemplate`, so boss loot is 3–8c and no item |
| Client | Bags **Q** = first equippable stack (no class filter); **C** lists six item ids, not stats |
| Protocol | Rev **7**; `EquipmentSnapshot` six named fields |
| Persist | `EquipmentDto` six optional strings |

Honest remaining gear debt:

1. **No identity.** Every class can equip every slot-matching item. Cloth/leather/mail/plate do not exist.
2. **No two-hand rule.** `worn_staff` / `worn_bow` occupy `MainHand` only; a buckler still fits.
3. **Paper doll is sparse.** Recruits spawn half-naked; jewelry slots do not exist; zone loot is tusks and ichor.
4. **Stats are two numbers.** Casters gain nothing from “caster gear”; stamina does not exist.
5. **Loot cannot mix junk + gear.** First successful `LootEntry` wins; adding a pendant behind `wolf_fang` never drops.
6. **Client cannot pick a bag slot.** Q always takes the first `equip_slot.is_some()` stack, including gear the class cannot wear (once rules exist).

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Content only** | More items and drops; same rules | Fast; mage still dual-wields sword + shield | Reject |
| **B. Full WoW paper doll** | 16 slots, sockets, sets, hit rating, weapon DPS, bind rules | Fights NPC-services durability and manufacturing quality; weeks of tables | Reject |
| **C. Rules + short ladder + sheet (recommended)** | Armor class / weapon style / two-hand / two jewelry slots / stamina + spell power / independent loot rolls / numbered bag equip | One content pass; additive protocol; no new actor column | **Adopt** |

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.8.0** | `class-forms` | Class signatures (shipped) |
| **1.9.0** | `gear-depth` | Equip rules, jewelry, secondary stats, upgrade ladder, sheet |

`PROTOCOL_REV` stays **7** (additive snapshot fields with `#[serde(default)]`; new `EquipSlot` variants are only sent by this client). Upstream pin stays **0.31.0**. This planning change does **not** bump `VERSION.toml`; the implementation wave tags `1.9.0`.

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
- Client never decides combat / loot / quest / **equip** outcomes.
- All sim RNG via mulberry32 on `Sim` only.
- English-only strings.
- New *per-actor* state is a `World` column. Equipment stays on `Bags` (player-only). Do **not** add a parallel `Gear` column or a fat `Entity`.
- Tick-phase fingerprint stays `15038642330132466611`. Equip is an interact; stat recalc is already called from spawn / equip / talents / persist. No new named phase.

```
woc-content ItemDef + can_equip     woc-sim interaction / stats / combat / spawn_mob_loot
        │                                         │
        ▼                                         ▼
 Equip { bag_slot }  →  Bags.equipment  →  recalc_player_stats
        │                                         │
        ▼                                         ▼
 TickSnapshot.equipment + attack_power/armor/spell_power
        │
        ▼
 Bevy C-sheet / B-bags (presentation only)
```

### 5.1 Content: armor class, weapon style, `can_equip`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorClass {
    Cloth,
    Leather,
    Mail,
    Plate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStyle {
    OneHand,
    TwoHand,
    Ranged, // occupies MainHand + OffHand like TwoHand
    Shield,
}

pub enum EquipDeny {
    NotGear,
    LevelReq(u32),
    WrongClass,
    WrongArmor,
}

pub fn can_equip(def: &ItemDef, class: PlayerClass, level: u32) -> Result<(), EquipDeny>
```

`ItemDef` gains (all `const`-friendly):

| Field | Default for non-gear |
| --- | --- |
| `armor_class: Option<ArmorClass>` | `None` |
| `weapon_style: Option<WeaponStyle>` | `None` |
| `allowed_classes: &'static [PlayerClass]` | empty slice = **all classes** (still subject to armor class) |
| `stamina: f32` | `0.0` |
| `spell_power: f32` | `0.0` |

Do **not** add `max_durability` here. NPC-services owns wear/repair. Jewelry never takes durability in that program either.

`ItemEquipSlot` gains **`Neck`** and **`Finger`** (one ring). No second ring, trinket, back, shoulder, wrist, waist, or hands.

Armor proficiency (inclusive — a plate class may wear cloth):

| Class | Highest armor | Weapons (via `allowed_classes` on each item) |
| --- | --- | --- |
| Warrior | Plate | swords, maces; two-hand `crypt_cleaver` |
| Paladin | Plate | maces; two-hand `crypt_cleaver`; shields |
| Hunter | Mail | bows (`Ranged`) |
| Rogue | Leather | daggers |
| Priest / Mage / Warlock | Cloth | staves (`TwoHand`) |
| Shaman | Mail | maces; shields |
| Druid | Leather | staves (`TwoHand`) |

`can_equip` order: not gear → level → `allowed_classes` (if non-empty) → armor class (if `Some`). Shields use `weapon_style = Shield` and `allowed_classes`; they have no `armor_class`. Jewelry has neither armor class nor weapon style; empty `allowed_classes` means every class.

Helpers `class_armor_cap(class) -> ArmorClass` and `armor_rank(a) -> u8` (`Cloth=0 … Plate=3`). Wear is legal iff `armor_rank(item) <= armor_rank(cap)`.

### 5.2 Two-hand / ranged occupancy

Equipping an item whose `weapon_style` is `TwoHand` or `Ranged`:

1. Validate `can_equip`.
2. Count displaced items: current `MainHand` (if any) + current `OffHand` (if any). The bag slot being consumed supplies **one** free hole.
3. If displaced count > free holes after removing the incoming stack, toast `"Inventory full."` and change nothing.
4. Else: remove incoming stack, place it in `MainHand`, move previous MH and OH into the bag, clear `OffHand`.

Equipping anything into `OffHand` while `MainHand` is two-hand/ranged: toast `"Cannot dual-wield a two-handed weapon."` and change nothing.

One-hand weapons stay `MainHand` only. Dual-wield (off-hand weapons) is out of scope; off-hand remains shields.

### 5.3 Stats

`recalc_player_stats` walks all eight slots. Off-hand still contributes **0.25 × `attack_power`** (shields are 0 AP). Two-hand/ranged never leave an off-hand item, so the 0.25 path does not double-dip.

```
ap     = class.attack_power + Σ slot.attack_power   (OH × 0.25)
armor  = (Σ slot.armor + talent armor_flat) * (1 + talent armor_pct)
sta    = Σ slot.stamina
sp     = Σ slot.spell_power
hp_max = (player_hp(base, level) + armor * 0.5 + sta * 2.0) * (1 + talent max_hp_pct)
```

Write `Combat.attack_damage = ap`, `Combat.armor = armor`, **`Combat.spell_power = sp`** (new field, default `0.0` on mobs/pets/bosses). HP ratio preservation is unchanged.

Combat:

- `WeaponDamage` / auto-attack / Execute / Cleave: unchanged (`attack_damage`).
- `SpellDamage` / `HealOrHarm` harm side: `weapon = def.damage + attack * 0.35 + spell_power * 0.5`.
- `Heal` / `HealOrHarm` heal side: `amount = (def.damage + spell_power * 0.5) * coefficient * heal_mult` (heals still do not miss; crit path unchanged).

### 5.4 Spawn

`create_player` still sets `start_weapon` / `start_chest` from `ClassDef`. It also equips these **Cloth** pieces for every class (they already exist):

- `head` = `recruit_cap`
- `legs` = `recruit_pants`
- `feet` = `recruit_boots`

Do not add `start_head` fields to `ClassDef`. Jewelry stays empty at spawn.

### 5.5 Loot

`spawn_mob_loot` rolls **every** `LootEntry` independently. Each success grants that item. If several succeed, spawn **one loot pile per item** (plus one copper pile, or copper on the first pile). Preferred: copper on the first pile created; additional piles are item-only (`copper = 0`). Need/Greed already keys by loot entity, so extra piles are extra rolls — that is intended.

Add `MobTemplate` `crypt_warden` (loot only; `spawn_boss_shell` still uses `DungeonDef` for HP/name). `Identity.template_id` is already `boss_id`, so `spawn_mob_loot` will resolve it.

Locked new items:

| Id | Slot / style | Stats | Source |
| --- | --- | --- | --- |
| `fang_pendant` | Neck | sta 4 | `scarred_wolf` chance **0.12** (fang row stays) |
| `boar_tusk_ring` | Finger | sta 3, AP 1 | `young_boar` chance **0.12** (tusk row stays) |
| `crypt_cleaver` | MH TwoHand | AP 16, lvl 3; warrior+paladin | `crypt_warden` chance **1.0** |
| `fen_staff` | MH TwoHand | AP 9, SP 6, lvl 4; priest/mage/warlock/druid | `bog_wisp` chance **0.15** |
| `hag_focus` | Neck | sta 2, SP 8, lvl 5; all | `barrow_hag` chance **1.0** (`hag_claw` stays as a second independent roll) |

Retag existing gear (no id changes):

| Id | armor_class / style | `allowed_classes` |
| --- | --- | --- |
| `worn_sword` | OneHand | warrior |
| `worn_mace` | OneHand | paladin, shaman |
| `worn_bow` | Ranged | hunter |
| `worn_dagger` | OneHand | rogue |
| `worn_staff` | TwoHand | priest, mage, warlock, druid |
| `copper_shortsword` | OneHand | warrior, paladin, rogue |
| `wooden_buckler` | Shield | warrior, paladin, shaman |
| `recruit_tunic` / `eastbrook_greaves` / `marsh_wraps` / `reedwalk_boots` / `mireguard_hood` | Leather | (empty — armor cap applies) |
| `recruit_robe` / `recruit_cap` / `recruit_pants` / `recruit_boots` | Cloth | empty |
| `veteran_helm` | Mail | empty (cap applies; lvl 5 already) |

Quest rewards that are gear (`eastbrook_greaves`, `reedwalk_boots`, `marsh_wraps`, `mireguard_hood`, `veteran_helm`) stay. A mage turning in a leather reward **receives** the item (quest grant does not call `can_equip`) but cannot equip it — toast on Equip. That is correct.

### 5.6 Protocol (additive, rev 7)

`EquipSlot` gains `Neck`, `Finger`. Old clients never send them. Snapshot:

```rust
pub struct EquipmentSnapshot {
    pub main_hand: Option<String>,
    pub off_hand: Option<String>,
    #[serde(default)] pub head: Option<String>,
    pub chest: Option<String>,
    #[serde(default)] pub legs: Option<String>,
    #[serde(default)] pub feet: Option<String>,
    #[serde(default)] pub neck: Option<String>,
    #[serde(default)] pub finger: Option<String>,
}
```

`TickSnapshot` gains sheet numbers (so the client does not reimplement recalc):

```rust
#[serde(default)] pub attack_power: f32,
#[serde(default)] pub armor: f32,
#[serde(default)] pub spell_power: f32,
```

`InteractAction` is unchanged (`Equip` still infers slot from `ItemDef`). Persist `EquipmentDto` gains the same two optional strings with `serde(default)`.

### 5.7 Client (presentation only)

- Character sheet (**C**): eight slots + `AP` / `Armor` / `SP` from the snapshot. Do not compute from item tables on the client for the headline numbers.
- Bags (**B**): keys **1–9** send `Equip` if that absolute bag slot is gear, else `UseItem` if consumable. **Q** still means “first stack this class *can* equip” (`can_equip` using `progress.class_id` + `progress.level`).
- Character sheet keys **1–8** send `Unequip` for MH, OH, Head, Chest, Legs, Feet, Neck, Finger in that order (empty slot is a no-op).
- Hint line: `[1-9] Equip/Use slot · [Q] first legal gear`.

Client does not bypass `can_equip`. Illegal Q targets toast from the sim.

### 5.8 Persist

Old JSON without `neck` / `finger` loads as `None`. Virgin detection treats empty jewelry as virgin-compatible (`is_none()`). No schema migration beyond JSON keys.

## 6. Definition of done

1. Every gear `ItemDef` has a slot; weapons have `weapon_style`; armor has `armor_class`; `can_equip` unit tests cover level, class, armor cap, and “empty allowed_classes + leather on a mage → WrongArmor”.
2. Mage cannot equip `worn_sword` or `wooden_buckler` or `recruit_tunic`; warrior cannot equip `worn_staff`; hunter bow equip clears a buckler into the bag.
3. Equipping `worn_staff` with a full bag and an occupied off-hand toasts inventory full and leaves both slots unchanged.
4. `recalc_player_stats`: `fang_pendant` raises `hp_max` by 8 (sta 4 × 2) at equal armor; `hag_focus` raises `Combat.spell_power` by 8; priest heal amount increases.
5. Warrior spawn snapshot has MH + chest + cap + pants + boots; mage spawn has staff + robe + cloth extras; off-hand / jewelry empty.
6. `scarred_wolf` can drop fang **and** pendant in one kill (independent rolls; seeded test forces both).
7. Killing a `crypt_warden` identity drops `crypt_cleaver`; `barrow_hag` can drop `hag_claw` and `hag_focus`.
8. Bevy: C-sheet shows AP/armor/SP and eight slots; bags 1–9 equip/use. `cargo check -p woc-client` green.
9. `TICK_PHASES` fingerprint unchanged. `PROTOCOL_REV` remains **7**. Old equipment JSON without `neck` still deserializes.
10. `docs/parity/STATUS.md` + `ROADMAP.md` + demo step updated when the implementation wave lands.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Durability / repair | NPC-services program |
| Item quality, masterwork, enchants, gems, sets | Manufacturing draft / YAGNI |
| Dual-wield, weapon skill, swing-speed DPS | One-hand MH + shield OH is enough |
| Second ring, trinket, back, shoulders, wrists, waist, hands | Eight slots close the paper doll |
| Bind-on-pickup / unique-equipped | AH already lists junk/consumable only |
| Client-side stat preview from item tables | Snapshot AP/armor/SP is the sheet |
| Gating quest *grants* on `can_equip` | Bags may hold unusable rewards |
| New tick phase / fat `Entity` / Bevy gameplay components | `AGENTS.md` |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |
| `PROTOCOL_REV` 8 | Additive fields only |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| NPC-services adds `max_durability` on `ItemDef` in parallel | This program does not touch that field; constructors stay explicit so a merge fills both |
| Manufacturing rewrite of `ItemDef` | Gear-depth lands on current `woc-content` tables; do not adopt the empty-repo profession crate layout |
| Independent loot creates two Need/Greed windows | Intended; tests assert two piles when both rolls succeed |
| `EquipSlot` match exhaustiveness | One task updates protocol + sim + persist + client together for Neck/Finger |
| Two-hand + full bags eats the incoming item | Pre-flight displaced vs holes; mutate only after it fits |
| Fingerprint churn | No phase rename; combat formula change is not a phase change |
| Mage starter `worn_staff` + accidental buckler in `start_items` | Starters never include a shield; spawn test locks OH `None` |

## 9. Success demo (human)

1. Create a warrior — C-sheet shows sword, tunic, cap, pants, boots, AP > class baseline.
2. Create a mage — cannot Q-equip a `worn_sword` granted via debug/loot; toast names class/armor.
3. Equip a wooden buckler on the warrior, then a `crypt_cleaver` (or `worn_bow` on a hunter) — shield returns to bags, OH empty, AP jumps.
4. Kill scarred wolves until a `fang_pendant` drops; equip it; HP max ticks up.
5. Priest with `hag_focus` (or `fen_staff`) heals for more than a naked priest at the same level.
6. Crypt Warden drops `crypt_cleaver`; Barrow Hag can drop `hag_focus`.

When §6 is green, tag `1.9.0`.
