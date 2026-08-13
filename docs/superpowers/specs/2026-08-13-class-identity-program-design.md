# Class-identity program design — `1.6.0` → `1.8.0`

**Status:** Proposed (planning deliverable 2026-08-13).  
**Baseline:** rewrite `1.3.0` / `online-hard` on `develop`, plus class-kit identity ([PR #20](https://github.com/yoefun/world-of-claudecraft-rs/pull/20): named auras, Execute, HealOrHarm, stun/slow, 4–5 kit slots).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `class-engine` then `class-identity` / `class-forms`.

Orthogonal (do not steal versions): `1.4.0` `client-compat` and `1.5.0` `client-update` are a separate program.

## 1. Goal

PR #20 made the nine kits *distinct buttons*. This program makes them *distinct classes*, using a thin slice of upstream `src/sim/content/classes.ts` identity — not a second port of the 15–40 ability spellbooks.

> Land the combat *engine* pieces every class needs (combo, stealth, absorb, lockout, self-AoE, gap-close), then give each class one signature system. Stop before talent specs, ability ranks, and full pet/form matrices.

## 2. Baseline vs upstream (honest gap)

| Layer | Rewrite now (after PR #20) | Upstream 0.31.0 |
| --- | --- | --- |
| Kit size | 4–5 slots / class | 15–40 abilities / class, ranks as you level |
| Effects | Weapon/Spell/Heal/HealOrHarm/AoE/Aura/Interrupt/Taunt/Execute + stun/slow auras | `weaponStrike`, `chainDamage`, `dispel`, `silence`, `aoeFear`, `blinkForward`, absorb, seals, forms, … |
| Resources | Rage / Energy / Mana; hunter is Energy | Hunter is **mana**; rogue **combo points**; rage from damage taken |
| Identity | None (no stealth, stance, form, shield, charge) | Stealth, 3 warrior stances, druid forms, paladin seals/auras, shaman imbues, mage blink/ice block |
| Pets | One hunter wolf / warlock imp, T to summon | Tame/dismiss/revive; voidwalker through doomguard |
| Talents | 4/class, mostly stat % | Three specs + signatures + mastery |

Completion DoD was “≥3 abilities / class”. That bar is met. This program is **depth**, not a new rewrite.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Port full spellbooks** | Copy every ability id from `classes.ts` | Explodes `AbilityEffect`; fights “enum balloons” risk from combat-depth; years of ranks/specs | Reject |
| **B. More damage buttons** | Grow kits to 8 generic strikes | HUD already 1–5; no new identity | Reject |
| **C. Engine then one signature / class (recommended)** | 1.6 ships combo/stealth/absorb/lockout/self-AoE/charge/blink/life-tap as data-driven effects; 1.7 wires rogue/priest/warrior/mage/hunter; 1.8 wires paladin/shaman/warlock/druid + one warrior stance | Fits ECS columns; each wave has a playable demo | **Adopt** |

## 4. Version map

| Rewrite | Parity label | Theme | Gate |
| --- | --- | --- | --- |
| **1.3.0** | `online-hard` | Park/AOI/Postgres notes (shipped) | STATUS online-hard rows `done` |
| **PR #20** | (no bump) | Named auras, Execute, HealOrHarm, CC, 4–5 kits | Merge to `develop` **before** 1.6 implementation |
| **1.4.0 / 1.5.0** | `client-compat` / `client-update` | Version gate + updater (other program) | Independent of this plan |
| **1.6.0** | `class-engine` | Combo, stealth, absorb, interrupt lockout, self-AoE, charge, blink, life-tap, hunter mana, rage-from-taken | Effects exist; unit tests; kits may still be 4–5 slots |
| **1.7.0** | `class-identity` | Rogue stealth+combo, priest shield, warrior charge, mage blink+self Frost Nova, hunter mana+Aspect | Each of those five classes uses the new engine in a playable kit |
| **1.8.0** | `class-forms` | Paladin aura+seal, shaman lightning shield+ghost wolf, warlock life tap+fear, druid travel form, warrior defensive stance | Remaining four classes have a signature; stance/form persist |

Upstream pin stays **0.31.0**.

## 5. Architecture

Unchanged invariants: one sim, ECS columns, mulberry32, client never decides combat, English-only, no Bevy in `woc-sim`.

### 5.1 Where new state lives

| State | Home | Who has it |
| --- | --- | --- |
| Combo points (0–5) | `ClassKit` fields | player |
| Stealthed | `ClassKit.stealthed` (bool) | player |
| Stance / form id | `ClassKit.stance_id: Option<String>` | player |
| Absorb | `AuraDef.absorb` + `AuraInstance.absorb` | anyone with `Auras` |
| Interrupt lockout | `Combat.cast_lockout: f32` | player, mob, pet |
| Charge / blink | no extra column — `AbilityEffect` + `Transform` | — |

Do **not** add a new catch-all `ClassState` blob. Do **not** grow `AbilitySlot` past 5 in this program (HUD and protocol already lock 1–5). Identity toggles (stealth / stance / form) are `InteractAction` variants, same pattern as `TogglePvp`.

### 5.2 `AbilityEffect` additions (1.6)

Keep the existing nine variants. Add only these:

```text
AbilityEffect =
  …existing…
| ComboBuilder { points: u8 }
| ComboSpend { per_point: f32 }
| Absorb { amount: f32 }
| Charge { gap: f32 }
| Blink { distance: f32 }
| Convert { hp_cost: f32, resource_gain: f32 }   // Life Tap
| RequiresStealth                                // Cheap Shot gate; still deals WeaponDamage via sibling? 
```

**Do not** add `RequiresStealth` as a sole effect. Cheap Shot stays `WeaponDamage` + stun aura; the stealth gate is `AbilityDef.requires_stealth: bool`. Combo builder/spend are flags on the existing hit:

Prefer **fields on `AbilityDef`** over enum explosion when the primary hit is still damage/heal:

| Field on `AbilityDef` | Meaning |
| --- | --- |
| `requires_stealth: bool` | Fail unless `ClassKit.stealthed` |
| `breaks_stealth: bool` | Default true; Cheap Shot opener may keep it true after the hit |
| `combo_add: u8` | After a successful harm hit |
| `combo_spend: bool` | Eviscerate: damage *= 1 + per_point * combo; then combo = 0 |
| `self_aoe: bool` | `AoeDamage` origin is caster if no hostile / always caster |
| `interrupt_lockout: f32` | Seconds of `cast_lockout` on the target |
| `rage_dump: bool` | Execute: spend remaining rage after the base cost; scale damage |

`AbilityEffect` only grows when the *primary* action is not already covered: `Absorb`, `Charge`, `Blink`, `Convert`.

### 5.3 Targeting / combat rules (1.6)

- **Self-AoE:** Frost Nova / Thunder Clap fire with origin = caster. Hostile target optional.
- **Interrupt:** existing clear-cast **plus** `Combat.cast_lockout`. Starting a cast while lockout > 0 fails.
- **Absorb:** `deal_damage` subtracts from the highest remaining `AuraInstance.absorb` before HP. Shield expires when absorb hits 0 or duration elapses.
- **Stealth:** `stealthed` players are skipped by mob aggro (`mob.rs`) unless distance ≤ melee. Any harm taken or `breaks_stealth` ability clears it. Move speed 0.7 while stealthed.
- **Charge:** if distance in `(MELEE_RANGE, gap]`, snap/step to melee then weapon hit; fail if already in melee or out of gap.
- **Blink:** displace along facing by `distance`, then `clamp_to_world` + `ground_height`. Does not break stun in 1.6 (Ice Block is out of scope).
- **Rage from taken:** when a player with `ResourceType::Rage` receives `deal_damage`, `gain_resource(damage * 0.05)` capped at max (tune in content constant `RAGE_FROM_TAKEN`).
- **Hunter mana:** `ClassDef.resource_type = Mana` for hunter. Existing energy hunters on disk: on load, if class is hunter, reset resource_type from content (already how kits refresh).

### 5.4 Protocol (prefer additive)

Bump **`PROTOCOL_REV` 6 → 7** once, at the start of 1.6, for:

- `TickSnapshot`: `combo_points: u8`, `stealthed: bool`, `stance_id: String`, `absorb: f32` — all `#[serde(default)]`
- `InteractAction`: `ToggleStealth`, `CycleStance`, `ToggleForm`
- Client HUD: combo dots, stealth tint, absorb on portrait; keys **Z** stealth, **F** stance/form (not overlapping T pet / V fly / N talents)

Older peers with missing fields stay playable. New actions on an old server fail closed (unknown variant) — that is why the rev bumps.

### 5.5 Persist

- **Do not** persist combo, stealth, absorb, cast_lockout.
- **Do** persist `stance_id` / travel-form as optional `#[serde(default)]` on `Character` / `CharacterSave` (empty = class default). No migration file beyond JSON default.

## 6. Definition of done per rewrite

### 6.1 `1.6.0` / `class-engine`

1. PR #20 is on `develop`.
2. `AbilityDef` has the gate/combo/self_aoe/lockout/rage_dump fields; `Absorb` / `Charge` / `Blink` / `Convert` exist.
3. `deal_damage` honors absorb; interrupts set lockout; rage-from-taken works; hunter is mana.
4. Unit tests (names in the implementation plan) for each engine piece, using existing kit ids plus one new ability each (`power_word_shield`, `charge`, `blink`, `life_tap`, `battle_shout` may be added as content stubs even if kits wait until 1.7).
5. Protocol rev 7; snapshot roundtrip test; CI workspace tests green.

### 6.2 `1.7.0` / `class-identity`

Each of these is in the class kit **or** bound to Z/F and covered by a sim test:

| Class | Signature |
| --- | --- |
| Rogue | Stealth (Z); Sinister Strike adds combo; Eviscerate spends combo (more combo → more damage); Cheap Shot requires stealth |
| Priest | Power Word: Shield on a kit slot (absorb); Flash Heal unchanged |
| Warrior | Charge on a kit slot (replaces Rend on the bar); rage-from-taken already from 1.6 |
| Mage | Blink on a kit slot; Frost Nova `self_aoe` |
| Hunter | Mana; Aspect of the Hawk (self ranged damage buff) on a kit slot |

Kits stay ≤5 slots: drop the least distinctive current filler if a slot is full (e.g. hunter `multi_shot` or warrior `rend` may yield to Charge / Aspect). Document the swap in CHANGELOG.

### 6.3 `1.8.0` / `class-forms`

| Class | Signature |
| --- | --- |
| Paladin | Devotion Aura (armor self-buff, auto or F); Seal of Righteousness as on-hit aura on Crusader Strike / auto |
| Shaman | Lightning Shield (thorns-style reflect via aura tick or on-hit); Ghost Wolf (move_mult > 1, cancelled by harm) |
| Warlock | Life Tap; Fear (stun-like aura `breaks_on_damage: true`) |
| Druid | Travel Form (F): move_mult 1.4, cancelled by harm or ability |
| Warrior | CycleStance battle (default) / defensive (`damage_mult` 0.9, armor_flat +20). Battle Shout self-buff applied while in battle stance |

Fear **breaks on damage** (unlike Cheap Shot stun). Ghost Wolf / Travel Form are not full shapeshift combat (no bear tank kit).

## 7. Explicit non-goals

| Skip | Why |
| --- | --- |
| Ability ranks (rank 2/3 numbers) | Content explosion; one rank is enough |
| Three talent specs / 27 signatures / Chronomancy | Other program; talents stay 4/class |
| Full pet roster, tame beast, pet bar | Keep T summon/dismiss |
| Bear/cat combat forms, Ice Block, Polymorph, Ice Barrier | 1.8 is one travel form only |
| Dodge / parry / block / school resist matrix | Combat-depth already rejected this |
| Channel (Arcane Missiles, Mind Flay) | Needs a new cast mode; later |
| Growing `AbilitySlot` to 6–10 | Protocol + HUD rewrite |
| Byte-identical vs TypeScript | Permanent rewrite non-goal |
| Reintroducing a fat `Entity` | `AGENTS.md` |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Effect enum balloons | Prefer `AbilityDef` flags; only four new primary variants |
| Stealth breaks open-world aggro forever | Aggro skip only while stealthed; melee proximity still pulls |
| Charge teleports through walls | Reuse `step_toward` / sweep; if blocked, fail the charge (toast) |
| Stance/form persist surprises returning players | Default empty = Battle / caster; document on character sheet |
| Parallel 1.4/1.5 client PRs | This program starts at **1.6.0**; no `VERSION.toml` edit until 1.6 lands |
| Kit slot pressure | Explicit swap list in 1.7/1.8 tasks; never a 6th slot |

## 9. Success demo (human)

**1.6:** priest shield soaks a wolf swing; rogue stealth walks past a wolf until melee; mage blinks 10 yd; warrior takes damage and rage ticks up.  
**1.7:** rogue opens Cheap Shot from stealth, builds 3 combo, Eviscerate hits harder than 1 combo; warrior Charges from 8 yd.  
**1.8:** druid F → travel form outruns a leash; paladin Devotion Aura shows on the aura strip; warlock Life Tap trades HP for mana then Fear a wolf (wolf starts chasing again after a hit).

Footer still `WoC-rs 1.8.0 · upstream 0.31.0` when the last wave tags.
