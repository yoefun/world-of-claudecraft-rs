# 职业系统完善设计 — `1.25.0` / `class-depth`

**Status:** Proposed (planning deliverable 2026-08-14).  
**Baseline:** rewrite `1.24.0` / `delve-depth` on `develop` (ECS `World`; class-engine/identity/forms shipped as `1.6.0`–`1.8.0`).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `class-depth`.  
**Protocol:** stays **10**. No new `InteractAction` / snapshot fields. Paladin **F** reuses `CycleStance`. HUD already has `stance_id`.

If another depth wave lands on `develop` before this tag, shift the number by one. Do not reuse a shipped label (`1.24.0` is `delve-depth`).

Prior class program (shipped as `1.6.0`–`1.8.0`, design in [PR #21](https://github.com/yoefun/world-of-claudecraft-rs/pull/21)). This spec does **not** reopen that DoD. It closes the gap between “STATUS says class-identity done” and nine classes that still play as distinct **resources + bars + HUD + pets**.

## 1. Goal

`1.6.0`–`1.8.0` landed the combat *engine* (combo, stealth, absorb, Charge, Blink, Life Tap) and **one signature per class**. That program is shipped. It is not a complete class system.

Honest playable completeness against this spec’s scorecard (§2.2): **68%**. Not 100%.

This program makes the nine classes feel like classes in a short Eastbrook session, without porting upstream 15–40 ability spellbooks:

1. Resources actually differ (energy ticks fast, mana slow, rage decays out of combat).
2. Every kit has five live buttons; leftover signatures that already exist in `ABILITIES` go back on the bar.
3. Paladin **F** cycles auras (today **F** is unbound for paladin).
4. HUD paints the current stance / form / aura name from `stance_id`.
5. Hunter wolf and warlock imp each fire one pet ability, not only a white swing.

Sim stays authoritative. Bevy only sends existing intents and paints `TickSnapshot`.

> 把 STATUS 里「Class identity: done」和九个职业在客户端里真正打起来的差别补上。

## 2. Completeness audit (2026-08-14)

### 2.1 What `1.6.0`–`1.8.0` already shipped

| Layer | State |
| --- | --- |
| 9 `PlayerClass` + create + starter gear | done |
| Kits 4–5 slots, `AbilityEffect` dispatch | done |
| Combo / stealth / absorb / interrupt lockout / self-AoE | done |
| Charge / Blink / Convert / Execute / HealOrHarm / Taunt | done |
| Hunter mana; rage from taken + white swing | done |
| Rogue Z stealth + combo; priest PW:S; warrior Charge; mage Blink + Frost Nova; hunter Aspect | done |
| Paladin Devotion-at-spawn + Crusader seal DoT; shaman Lightning Shield + Ghost Wolf; warlock Life Tap + Fear; druid Travel Form; warrior Battle/Defensive | done |
| `TrainClass` toast; talents 4/class with one ability-mod; hunter wolf / warlock imp T summon | done |
| Gear `can_equip` / armor cap / dual-wield | done (`1.12.0`–`1.15.0`) |
| Protocol rev 7 identity snapshot | done; current rev **10** |

`1.6.0`–`1.8.0` DoD is ~**95%**. Remaining nits from that program: HUD never paints `stance_id`; paladin **F** was specified “auto or F” and the implementation picked auto only.

### 2.2 Playable class-system scorecard (this program’s 100%)

Weights sum to 100. **Current 68.** Gate for `1.25.0`: **100** on this card. Rows marked n/a are explicit non-goals and do not count.

| # | Slice | Wt | Now | After `1.25.0` | Honest debt |
| --- | --- | --- | --- | --- | --- |
| 1 | Nine-class create, HP/resource types, starter kit | 10 | 10 | 10 | — |
| 2 | Data-driven `AbilityEffect` (no per-id combat arms) | 10 | 10 | 10 | Comment still says “stubs”; delete it |
| 3 | Five live bar slots / class | 10 | 6 | 10 | Rogue kit is 4; hunter Multi-Shot, priest SW:P, mage Counterspell exist off-kit |
| 4 | Distinct resource regen | 12 | 4 | 12 | Mana and energy share `1.5/s`; rage never decays |
| 5 | Signature identity (Z/F + on-bar) | 12 | 9 | 12 | Paladin **F** unbound; HUD ignores `stance_id` |
| 6 | Pet combat identity | 10 | 4 | 10 | T summon + white swing only |
| 7 | Client HUD / keys | 10 | 6 | 10 | Combo/stealth/absorb yes; stance/form name no; paladin F no |
| 8 | Gear class rules | 8 | 8 | 8 | Shipped |
| 9 | Talents (4/class, one ability-mod) | 8 | 8 | 8 | Shipped; three specs stay out |
| 10 | Class trainer confirmation | 5 | 3 | 3 | Toast-only by `1.11.0` design; **not** this wave |
| 11 | Persist `stance_id` | 5 | 5 | 5 | Round-trips |
| | **Total** | **100** | **68** | **98** | Trainer stays 3/5 (confirmation seam, not ranks) |

`1.25.0` does not claim 100 on row 10. Full 100 on this card would need ability-rank trainers (explicit non-goal). **Ship bar for this program: rows 1–9 + 11 green (98/100).** Call that “class-depth done”; do not pretend trainers teach ranks.

### 2.3 Honest remaining debt (why 68 is correct)

1. **Mana ≈ energy.** `update_player_combat` does `gain_resource(kit, 1.5 * DT)` for both. Rogue energy takes ~67 s to fill from empty. Rage never decays out of combat.
2. **Rogue has four buttons.** `every_class_has_multi_ability_kit` only requires `>= 4`.
3. **1.7 kit swaps left signatures on the floor.** `rend`, `shadow_word_pain`, `counterspell`, `multi_shot`, `battle_shout` are in `ABILITIES` but not on a live slot (Charge/Blink/Aspect took the slot).
4. **Paladin F is dead.** `input.rs` binds **F** only for warrior / shaman / druid. `cycle_stance` rejects non-warriors with `"You cannot change stance."`
5. **HUD does not show stance/form.** Snapshot has `stance_id`; HP line and action bar never print it. Warrior hint is the static string `[F] Stance`.
6. **Pets are white swings.** `tick_pets` only `deal_damage(..., "pet")`. Imp walks to melee.
7. **Class trainer is a no-op vs level-up.** `known_abilities_at_level` already runs on ding. `TrainClass` refreshes the same list and toasts. `1.11.0` called this a confirmation seam for a later rank trainer — still later.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Port full spellbooks / 3 talent specs / bear+cat** | Copy upstream `classes.ts` | Explodes `AbilityEffect`; fights the 1.6 “enum balloons” risk; years of ranks | Reject |
| **B. More damage buttons only** | Grow kits to 8 generic strikes | HUD locked 1–5; no resource identity; paladin F still dead | Reject |
| **C. Resource + bar + HUD + one pet ability (recommended)** | Fix regen, restore off-kit signatures into 5 slots, paladin F, paint `stance_id`, pet Bite / Firebolt | Fits existing columns and `AbilityEffect`; one protocol-idle wave | **Adopt** |

Do not add a 6th `AbilitySlot`. Do not add `CycleAura` to the wire — paladin reuses `CycleStance`. Do not add a `PetKit` column; pet ability id lives on `PetDef`, cooldown uses existing `Combat.ability_cd`.

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.6.0–1.8.0** | `class-engine` → `class-forms` | Engine + one signature / class (shipped) |
| **1.24.0** | `delve-depth` | Isolated Hollow (shipped; current tag) |
| **1.25.0** | `class-depth` | Distinct regen, 5-slot kits, paladin aura cycle, pet ability, HUD stance |

`PROTOCOL_REV` stays **10**. Tick fingerprint stays `3214741777866168171u64`. No new named phase. Resource regen stays inside `player_combat` (`update_player_combat`). Pet ability stays inside `pet_ai` (`tick_pets`). Planning commit does not bump `VERSION.toml`; the implementation wave tags `1.25.0`.

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` 不依赖 Bevy / wgpu / axum / tokio。
- 客户端从不决定命中、资源回复、姿态、宠物技能。
- 全部 sim RNG 走 mulberry32。本程序资源回复**不抽随机**。宠物技能沿用现有白字路径（`deal_damage`，不走 miss/crit 表），与当前 pet 白字一致。
- 禁止墙钟。
- English-only 玩家可见字符串，文案锁死见 §6.8。
- 新 per-actor 状态才是 `World` 列。本程序**不新增列**。
- 不要把脂肪 `Entity` 请回来。
- Action bar stays **1–5**. Identity toggles stay **Z** / **F**.

```
woc-content CLASSES / ABILITIES / AURAS / PETS
        │
        ▼
woc-sim combat::update_player_combat   resource regen (energy / mana / rage)
        combat::cycle_stance           warrior stances + paladin auras
        pet::tick_pets                 optional PetDef.ability_id
        │
        ▼
TickSnapshot.stance_id / combo / stealthed / absorb / ability_bar   (rev 10, already there)
        │
        ▼
Bevy HUD paints stance/form name; F sends CycleStance for paladin too
```

## 6. `1.25.0` / `class-depth`

### 6.1 Resource regen

Replace the shared `1.5 * DT` branch in `update_player_combat`. Constants in `crates/woc-sim/src/types.rs`:

```rust
pub const ENERGY_REGEN_PER_SEC: f32 = 10.0;
pub const MANA_REGEN_OOC_PER_SEC: f32 = 8.0;
pub const MANA_REGEN_COMBAT_PER_SEC: f32 = 2.0;
pub const RAGE_DECAY_OOC_PER_SEC: f32 = 3.0;
```

Rules, evaluated every player-combat tick **before** spending:

| Resource | In combat | Out of combat |
| --- | --- | --- |
| Energy | +10 / s | +10 / s |
| Mana | +2 / s | +8 / s |
| Rage | no regen here (taken + white swing + ability hit stay) | −3 / s, floor 0 |

**In combat** = `Combat.auto_attack || Combat.target` points at a living hostile (`LootTable` or opposing `ClassKit`). Dead/missing target is out of combat.

Rage-from-taken (`RAGE_FROM_TAKEN`) and +5 rage on white swing / rage ability hit stay. Life Tap `Convert` stays.

Do not add spirit-stat scaling. Do not add 5-second rule. One in/out pair is enough.

### 6.2 Five-slot kits (restorations)

`every_class_has_multi_ability_kit` must require `kit.len() == 5` and slots `{1,2,3,4,5}`.

Locked swaps (bar still 1–5; drop the least distinctive filler):

| Class | Slot | Was | Becomes | Why |
| --- | --- | --- | --- | --- |
| Rogue | 5 | *(missing)* | `sprint` | Escape identity; new content (see §6.3) |
| Hunter | 3 | `aimed_shot` | `multi_shot` | AoE already in `ABILITIES`; Aimed Shot is a slower Arcane Shot |
| Priest | 3 | `mind_blast` | `shadow_word_pain` | Signature DoT already in `ABILITIES` |
| Mage | 3 | `arcane_missiles` | `counterspell` | Interrupt already in `ABILITIES`; missiles were never a channel |

Warrior / paladin / shaman / warlock / druid kits **do not swap**. Charge, Taunt, Cleave, Execute, Holy Shock, Earth Shock, Life Tap, Fear, Travel Form stay.

`aimed_shot`, `mind_blast`, `arcane_missiles` remain in `ABILITIES` (tests and aura table may still mention them) but leave the live bars. `rend` stays off-kit (Charge took that slot in 1.7; do not drop Charge/Taunt/Cleave).

### 6.3 Rogue `sprint`

New ability + aura. No new `AbilityEffect` variant (`ApplyAura` + self-buff targeting already works when `move_mult >= 1.0` and `is_self_buff()`).

```text
AbilityDef id=sprint  name=Sprint  cost=40  cooldown=20  range=0  min_level=1
  effect=ApplyAura  aura=Some("sprint")  flags=DEFAULT

AuraDef id=sprint  duration=8  move_mult=1.5  breaks_on_damage=false
```

Energy 40, 20 s CD, 8 s 1.5× move. Does **not** break stealth (`breaks_stealth` stays default true only if the rogue *casts* it — Sprint **does** break stealth, classic-like). Document that in CHANGELOG.

### 6.4 Paladin aura cycle (**F** → `CycleStance`)

Extend `cycle_stance`:

| Class | Current `stance_id` | Next | Aura on | Aura off | Toast |
| --- | --- | --- | --- | --- | --- |
| Warrior | `defensive` | `battle` | `battle_shout` | `defensive_stance` | `Battle Stance.` |
| Warrior | other | `defensive` | `defensive_stance` | `battle_shout` | `Defensive Stance.` |
| Paladin | `retribution` | `devotion` | `devotion_aura` | `retribution_aura` | `Devotion Aura.` |
| Paladin | other | `retribution` | `retribution_aura` | `devotion_aura` | `Retribution Aura.` |
| else | — | — | — | — | `You cannot change stance.` |

New aura `retribution_aura`: `duration=3600`, `damage_mult=1.1`, `armor_flat=0`. `devotion_aura` stays `armor_flat=20`.

`apply_spawn_identity` for paladin: set `stance_id = Some("devotion")` then apply `devotion_aura` (today it applies the aura with `stance_id` left `None`). Persist already stores `stance_id`; load already calls `apply_spawn_identity`.

Client: `input.rs` **F** match arm adds `"paladin" => CycleStance`. Action-bar hint: paladin `[F] Devotion` / `[F] Retribution` from `stance_id`.

### 6.5 HUD stance / form

No protocol change. `TickSnapshot.stance_id` is already filled.

HP line (after combo dots) appends `   {label}` when `stance_id` is non-empty, using:

| `stance_id` | Label |
| --- | --- |
| `battle` | `Battle` |
| `defensive` | `Defensive` |
| `devotion` | `Devotion` |
| `retribution` | `Retribution` |
| `ghost_wolf` | `Ghost Wolf` |
| `travel_form` | `Travel Form` |
| other | the raw id |

Action-bar hint `class_interact_hint` becomes dynamic:

| Class | Hint |
| --- | --- |
| rogue | `[Z] STEALTH` / `[Z] Stealth` (unchanged) |
| warrior | `[F] Battle` / `[F] Defensive` |
| paladin | `[F] Devotion` / `[F] Retribution` |
| shaman | `[F] Ghost Wolf` when on, else `[F] Form` |
| druid | `[F] Travel Form` when on, else `[F] Form` |
| other | empty |

### 6.6 Pet signature ability

`PetDef` gains:

```rust
    /// Optional extra hit while the pet is on a living target. `None` = white swing only.
    pub ability_id: Option<&'static str>,
```

| Pet | `ability_id` | Range | CD |
| --- | --- | --- | --- |
| `hunter_wolf` | `wolf_bite` (already in `ABILITIES`) | melee (`MELEE_RANGE`) | 6 s |
| `warlock_imp` | `imp_firebolt` (new) | 14 yd | 6 s |

`imp_firebolt`: `SpellDamage { Fire }`, damage 14, cost 0, cooldown 6, range 14, min_level 1. Imp **walks to 14 yd**, not melee, when this ability is set. White swing still happens in melee if the imp is already that close; Firebolt is the identity hit.

`tick_pets` keeps today’s `deal_damage(owner_id, …)` kill-credit path. When `Combat.ability_cd <= 0` and distance ≤ ability range, fire `deal_damage(..., Some(ability_name), true, events)` for the ability’s `damage + 0.35 * pet attack_damage`, then set `ability_cd = 6.0`. Decrement `ability_cd` by `DT` each pet tick. **No `Rng` in `tick_pets`.** No miss/crit (same as current pet white swing).

Do not add a pet action bar. **T** remains summon/dismiss.

### 6.7 Class trainer (explicitly unchanged)

`TrainClass` stays the `1.11.0` confirmation toast. Do **not** gate `min_level` 3/6 abilities behind Alden. Do not teach ranks. Row 10 of the scorecard stays 3/5.

### 6.8 Locked copy (English)

| When | Toast / HUD |
| --- | --- |
| Paladin cycle → devotion | `Devotion Aura.` |
| Paladin cycle → retribution | `Retribution Aura.` |
| Warrior (unchanged) | `Battle Stance.` / `Defensive Stance.` |
| Non-warrior, non-paladin **F** / `CycleStance` | `You cannot change stance.` |
| Sprint | no extra toast (GCD + aura strip) |
| Pet ability | no extra toast (damage event is enough) |

### 6.9 Tests (names the plan must land)

| Test | Gate |
| --- | --- |
| `energy_regens_ten_per_second` | rogue +20 ticks ≈ +10 energy |
| `mana_regens_slower_in_combat` | mage OOC +8/s, `auto_attack` +2/s |
| `rage_decays_out_of_combat` | warrior OOC loses rage; in combat does not |
| `every_class_has_five_kit_slots` | all nine `kit.len()==5` |
| `hunter_bar_has_multi_shot` / `priest_bar_has_shadow_word_pain` / `mage_bar_has_counterspell` | slot 3 ids |
| `rogue_sprint_buffs_move_speed` | slot 5 aura `sprint`, `move_speed_mult` 1.5 |
| `paladin_cycle_stance_swaps_devotion_and_retribution` | F cycle + spawn `stance_id=="devotion"` |
| `hunter_pet_bites_on_cooldown` / `warlock_imp_firebolts_at_range` | pet ability |
| `format_action_bar_paints_stance_name` | client HUD |

Fingerprint test must still equal `3214741777866168171u64`. `PROTOCOL_REV` test still `10`.

## 7. Explicit non-goals

| Skip | Why |
| --- | --- |
| Ability ranks / trainer-gated ranks | `1.6` and `1.11` already deferred; row 10 stays confirmation |
| Three talent specs / 27 signatures | Other program; talents stay 4/class |
| Bear / cat / moonkin, Ice Block, Polymorph, totems, blessings, soul shards | 1.8 travel-form-only still holds |
| Pet roster, tame, pet bar, pet revive | Keep **T** |
| Channel (Mind Flay / old Arcane Missiles) | Needs a new cast mode |
| Growing `AbilitySlot` to 6–10 | Protocol + HUD rewrite |
| Dodge / parry / block / school resist | combat-depth rejected this |
| Class-specific graveyards / racials | Not class combat identity |
| Byte-identical vs TypeScript | Permanent rewrite non-goal |
| Reintroducing a fat `Entity` | `AGENTS.md` |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Faster energy makes Cheap Shot infinite | 40 energy + 6 s CD + stealth gate stay; regen 10/s still needs a builder |
| Rage decay nerfs Execute dummies | Decay only when **out of combat**; tests lock in-combat hold |
| Paladin `CycleStance` surprises old clients | Same action; old clients simply never send it for paladin. Additive behavior |
| Imp range 14 yd kites poorly in crypt halls | Walk-to-range uses existing `step_toward`; if blocked, Firebolt waits |
| Kit-swap breaks 1.7 slot tests | Update those tests in the same task; CHANGELOG lists the swaps |
| Fingerprint drift | Regen and pet ability hook **inside** existing phases |

## 9. Success demo (human)

1. Create a Rogue: energy climbs visibly out of combat; **1–4** as today; **5** Sprint, run 1.5× for 8 s.
2. Create a Mage: **3** is Counterspell (lockout); mana crawls in a fight, fills faster in town.
3. Create a Paladin: spawn shows `Devotion` on the HP line; **F** → `Retribution Aura.` toast and outgoing damage 1.1×; **F** back.
4. Create a Warrior: **F** still Battle/Defensive; leave combat, rage ticks down; take a hit, rage comes back.
5. Hunter **T**: wolf Bites every 6 s in melee. Warlock **T**: imp Firebolts from ~14 yd.
6. Priest **3** is Shadow Word: Pain. Hunter **3** is Multi-Shot.

Footer still `WoC-rs 1.25.0 · upstream 0.31.0` when the implementation wave tags.

When rows 1–9 and 11 of §2.2 are green in STATUS, tag `1.25.0`.
