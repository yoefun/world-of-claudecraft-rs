# Mounts and riding design — `1.21.0` / `mounts`

**Status:** Shipped (rewrite `1.21.0` / `mounts`).  
**Baseline:** rewrite `1.13.0` / `gear-slots` on `develop` (ECS `World`; NPC services shipped).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `mounts`.

Related: NPC services deferred riding ([`2026-08-13-npc-services-design.md`](2026-08-13-npc-services-design.md) §7); travel flight today is a convenience toggle in `player_motion.rs`, not a mount system.

## 1. Goal

Players **train riding**, **learn mount items**, and **toggle a mount** for overworld speed. Flying is a **gated flying mount**, not a free **V** cheat. The client never decides whether a mount succeeds.

> Talk to the stable master, buy a pony, press **V**, outrun a wolf. Expert riding + gryphon replaces free travel flight.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Travel | `RUN_SPEED = 7`; aura `move_mult` (Travel Form / Ghost Wolf = 1.4) |
| **V** | `PlayerIntent.fly_toggle` → `Motion.flying` with no skill, item, or zone gate; `FLY_SPEED_MULT = 1.15` |
| Pets | Separate `Owner` actor; hunter/warlock only |
| NPC services | Vendor / repair / profession / class / inn. **No** `RidingTrainer`. Explicit non-goal in 1.11.0 |
| Items | Weapon / Armor / Consumable / Junk / Quest. No mount kind |
| Persist | Character JSON completion blob (`quests_json`); additive `#[serde(default)]` fields |
| Protocol | Rev **8**; `fly_toggle` already on the intent |
| Level curve | Tiny table levels 1–10 (`xp_to_next`) |
| Tick fingerprint | `3214741777866168171` (10 named phases including `profession_casts`) |

Honest remaining travel debt:

1. **Free flight.** Anyone presses **V** and ignores terrain. `player_motion.rs` documents this as “a rewrite convenience mode rather than a full mount/form system.”
2. **No riding skill.** NPC services deferred “flight masters / mounts / stables” and “riding trainers.”
3. **Class forms are the only speed fantasy.** Travel Form / Ghost Wolf (1.4, breaks on damage) are class identity, not a shared mount loop.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Aura-only whistle** | Use a mount item → speed aura; no visual, keep free **V** flight | Fast; looks like a potion; **V** still a cheat | Reject |
| **B. Mount as a pet actor** | Spawn a `EntityKind::Mount` with `Owner`; parent motion | Tab-target / AOI / combat vs the horse; second actor for cosmetics | Reject |
| **C. Player `Riding` column + snapshot visual (recommended)** | One player-only component; **V** toggles last known mount; flying mounts set `Motion.flying`; client draws a child silhouette | One column, additive protocol, replaces free flight | **Adopt** |

Do **not** put mount state on `Bags` equipment. Mounts are not paper-doll slots. Do **not** reuse `Progress.professions` — riding has ranks and gold gates, not a gather/craft ladder.

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.13.0** | `gear-slots` | Dual-wield, Finger2, quality, MH enchant (shipped) |
| **1.21.0** | `mounts` | Riding ranks, three mounts, combat dismount, gated flight |

`PROTOCOL_REV` stays **10**. New fields use `#[serde(default)]`. Upstream pin stays **0.31.0**. Tick fingerprint stays `3214741777866168171`. No new named tick phase. Implementation tags `1.21.0` (after parcel-bank `1.20.0`).

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
- Client never decides combat / loot / quest / **mount / riding** outcomes.
- All sim RNG via mulberry32 on `Sim` only; riding has **no** RNG.
- English-only strings.
- New *per-actor* state is a `World` column. Do not reintroduce a fat `Entity`.
- Mount toggle hooks **inside** `apply_intents_motion`. Dismount hooks existing combat / death / instance paths.

```
woc-content MOUNTS / RIDING_RANKS     woc-sim mount.rs
        │                                    │
        ▼                                    ▼
 UseItem (mount) → learn + summon     fly_toggle (V) → toggle last
 TrainRiding at stable master         Motion.flying iff flying mount
        │                                    │
        ▼                                    ▼
 Riding column (rank, known, last, active)
        │
        ▼
 TickSnapshot.mounted + EntitySnapshot.mounted → Bevy child mesh
```

### 5.1 Content: riding ranks

```rust
pub struct RidingRankDef {
    pub rank: u8,           // 1, 2, 3
    pub id: &'static str,   // apprentice | journeyman | expert
    pub name: &'static str,
    pub level_req: u32,
    pub copper: u32,
    pub ground_speed_mult: f32, // applied while a ground mount is active
}

pub static RIDING_RANKS: &[RidingRankDef] = &[
    RidingRankDef { rank: 1, id: "apprentice", name: "Apprentice Riding", level_req: 2, copper: 10, ground_speed_mult: 1.6 },
    RidingRankDef { rank: 2, id: "journeyman", name: "Journeyman Riding", level_req: 5, copper: 50, ground_speed_mult: 2.0 },
    RidingRankDef { rank: 3, id: "expert",     name: "Expert Riding",     level_req: 8, copper: 200, ground_speed_mult: 2.0 },
];
```

Rank 0 = untrained. Training is sequential: rank *n* requires rank *n−1*. Expert does not raise ground speed further; it **unlocks flying mounts**.

Helpers: `riding_rank(id) -> Option<&RidingRankDef>`, `riding_rank_by_n(n: u8) -> Option<&RidingRankDef>`.

Classic 20/40/60 gates do not fit a 1–10 XP table. Locked gates above are the 1.16.0 numbers.

### 5.2 Content: mounts

```rust
pub enum MountKind { Ground, Flying }

pub struct MountDef {
    pub id: &'static str,
    pub name: &'static str,
    pub item_id: &'static str,
    pub kind: MountKind,
    pub riding_rank: u8,          // minimum Riding.rank
    pub speed_mult: f32,          // horizontal; flying also uses existing FLY_VERTICAL_SPEED
    pub visual_key: &'static str, // visual_catalog key
}

pub static MOUNTS: &[MountDef] = &[
    MountDef { id: "brown_pony",      name: "Brown Pony",      item_id: "brown_pony",      kind: MountKind::Ground,  riding_rank: 1, speed_mult: 1.6, visual_key: "mount_pony" },
    MountDef { id: "swift_bay_steed", name: "Swift Bay Steed", item_id: "swift_bay_steed", kind: MountKind::Ground,  riding_rank: 2, speed_mult: 2.0, visual_key: "mount_steed" },
    MountDef { id: "tawny_gryphon",   name: "Tawny Gryphon",   item_id: "tawny_gryphon",   kind: MountKind::Flying,  riding_rank: 3, speed_mult: 2.0, visual_key: "mount_gryphon" },
];
```

`speed_mult` on the mount is the live multiplier. Rank `ground_speed_mult` is documentation / UI; the mount row is authoritative so a Journeyman player on the pony still runs at **1.6** (the pony), not 2.0.

`ItemKind::Mount`. `stack_size = 1`, `max_durability = 0`, `heal_hp = 0`, not equippable. First **UseItem** **consumes** the stack, inserts `id` into `Riding.known`, summons it. A second copy of the same item toasts `"You already know that mount."` and is not consumed.

Locked vendor prices:

| Item | Buy | Sell |
| --- | --- | --- |
| `brown_pony` | 25 | 5 |
| `swift_bay_steed` | 150 | 30 |
| `tawny_gryphon` | 300 | 60 |

### 5.3 NPC: stable master

New `NpcService::RidingTrainer`.

```rust
NpcDef {
    id: "stable_master_ross",
    name: "Stable Master Ross",
    greeting: "A horse knows the road better than most maps.",
    services: &[NpcService::RidingTrainer, NpcService::Vendor],
    vendor_stock: &[
        VendorOffer { item_id: "brown_pony", count: 1 },
        VendorOffer { item_id: "swift_bay_steed", count: 1 },
        VendorOffer { item_id: "tawny_gryphon", count: 1 },
    ],
    trains: &[], // professions only; riding is the service flag
}
```

Eastbrook spot: `x: 4.0`, `z: 9.0` (south-east of Innkeeper Mara at `2, 8`). No other-zone riding trainer in 1.16.0.

`NpcDef` helpers: `is_riding_trainer()`. Session snapshot additive `train_riding: bool` (default false). Client shows **Train riding** when true.

New interact:

```rust
InteractAction::TrainRiding
```

Requires an open NPC session whose def `is_riding_trainer()`, same `INTERACT_RANGE` path as profession training (session already implies Talk). Trains the **next** rank if level and copper allow.

Locked toasts:

| Condition | Copy |
| --- | --- |
| No riding-trainer session | `"Talk to a riding trainer."` |
| Already Expert | `"You already know that rank."` |
| Level too low | `"You are too low level."` |
| Not enough copper | `"Not enough copper."` |
| Success | `"Learned {rank name}."` (e.g. `"Learned Apprentice Riding."`) |

### 5.4 ECS: `Riding` column (players only)

```rust
pub struct Riding {
    pub rank: u8,                     // 0..=3
    pub known: BTreeSet<String>,      // mount ids
    pub last_id: Option<String>,
    pub active_id: Option<String>,    // currently mounted
}
```

Insert on `create_player` as `Riding::default()` (rank 0, empty known). Persist `rank` + `known` + `last_id`. **Do not persist `active_id`** — login always starts dismounted (matches death/instance safety; park/resume in the same session may keep `Motion` but 1.16.0 still dismounts on export/import to avoid flying through a load). Park/resume of a live entity without persist keeps the column as-is.

Module: `crates/woc-sim/src/mount.rs`. Re-export from `lib.rs`.

### 5.5 Toggle, summon, dismount

**V** stays `PlayerIntent.fly_toggle` on the wire (rev 8, no bump). Sim meaning becomes **toggle mount**, not free flight.

`step_player_motion` **stops reading `fly_toggle`**. `Sim` `apply_intents_motion` calls `mount::toggle_mount` **before** `step_player_motion` when `intent.fly_toggle`. Flight kinematics (`Motion.flying`, Space/Ctrl vertical, ground land) stay in `player_motion` and apply only when `Motion.flying` is already true.

`toggle_mount`:

1. If `active_id` is Some → `dismount` (toast `"You dismount."`).
2. Else summon `last_id`, else the only known mount, else toast `"You do not know a mount."`
3. Summon path: `summon_mount(world, player_id, mount_id, events)`.

`summon_mount` fails (no mutate) with locked copy:

| Condition | Copy |
| --- | --- |
| Dead / spirit | `"You cannot mount here."` |
| In an instance (`InstanceAt.instance_id` Some) | `"You cannot mount here."` |
| Swimming (ground mount) | `"You cannot mount here."` |
| Stealthed | `"You cannot mount here."` |
| Rank 0 | `"You need riding training."` |
| `Riding.rank < MountDef.riding_rank` | `"Your riding skill is too low."` |
| Flying mount while rank < 3 | `"Your riding skill is too low."` |
| Unknown mount id | `"You do not know a mount."` |

On success: set `active_id` / `last_id`; clear Travel Form / Ghost Wolf (`remove_named_auras` + clear those `stance_id` values); if flying mount, set `Motion.flying = true` and lift `y` by 1.5 (same as today’s engage); if ground mount, `Motion.flying = false`. Toast `"You mount up."`

`UseItem` on `ItemKind::Mount`: if not in `known`, consume 1 and insert id, toast `"You learn to ride the {name}."`, then `summon_mount`. If already known, do not consume; `summon_mount` that id (or dismount if already on it).

### 5.6 Speed

In `step_player_motion`, after swim/fly/run base speed and **after** `combat::move_speed_mult`:

```text
if Riding.active_id → MountDef.speed_mult
else 1.0
```

Multiply the already-computed horizontal speed. Slow auras still apply (chill while mounted is allowed). Stealth cannot coexist with a mount (summon refused; stealth toggle dismounts first — see §5.7).

Ground mounted: existing jump/gravity. Flying mounted: existing flight vertical pass. Deep water while ground-mounted: `dismount` then swim. Flying mount over water stays flying.

### 5.7 Forced dismount

Call `mount::dismount(world, player_id, events)` (silent if not mounted; toast `"You dismount."` when it was):

- Player auto-attack engages (`intent.attack` / sticky auto-attack start).
- Player fires an ability.
- Player takes HP damage (`deal_damage` when the **target** has `Riding` and `active_id` is Some). Absorb-only hits that never reduce HP still dismount (the hit connected).
- `toggle_stealth` on, or stealth already on when mounting (mounting refused).
- `toggle_form` / `cycle_stance` (forms and mounts are exclusive).
- Death (`on_player_death_check`).
- `enter_dungeon` / `enter_delve` / any path that sets `InstanceAt.instance_id`.
- Ground mount begins swimming.

Falling after a flying dismount uses the existing land/fall path (`Motion.flying = false`, keep current `y`).

### 5.8 Protocol / persist

Rev stays **8**. Additive:

- `InteractAction::TrainRiding`
- `EntitySnapshot.mounted: Option<String>` (mount id; default `None`)
- `TickSnapshot.riding_rank: u8` (default 0)
- `TickSnapshot.known_mounts: Vec<String>` (default empty)
- `TickSnapshot.mounted: Option<String>` (local player; default `None`)
- `NpcSessionSnapshot.train_riding: bool` (default false)

Persist (serde default 0 / empty):

- `Character.riding_rank: u8`
- `Character.known_mounts: Vec<String>`
- `Character.last_mount: String` (empty = none)
- Same fields on `CharacterSave` and `CharacterCompletionDto`

No Postgres column migration: completion JSON already lives in `quests_json`.

### 5.9 Client

- **V** already sends `fly_toggle` when bags are closed. Keep it.
- NPC session: **Train riding** button when `train_riding`.
- Bags **F** / Use on a mount item sends `UseItem` (extend the consumable branch to `ItemKind::Mount`).
- HUD: character sheet line `Riding: {rank name}` plus known mount names; while mounted, a short `Mounted: {name}` near the XP bar.
- Presentation: if `EntitySnapshot.mounted` is Some, spawn a child mesh from `visual_spec` using `MountDef.visual_key` (`mount_pony` / `mount_steed` reuse Boar family; `mount_gryphon` reuses Harpy family with a tan palette). Raise the player visual by `0.55` yards while mounted. No new `EntityKind`.

Locked visual keys: `mount_pony`, `mount_steed`, `mount_gryphon`.

### 5.10 Class forms

Travel Form and Ghost Wolf stay. They are worse than Journeyman mounts and remain the no-gold speed option. Mounting clears those forms. Toggling a form dismounts.

## 6. Definition of done

1. Content integrity: every `MountDef.item_id` exists as `ItemKind::Mount`; every mount item maps to a `MountDef`; Ross stocks all three; Eastbrook layout includes `stable_master_ross`; every `RidingTrainer` is a vendor with non-empty mount stock.
2. Untrained **V** toasts `"You do not know a mount."` and does **not** set `Motion.flying`.
3. Level 2 + 10c at Ross learns Apprentice; buying and using `brown_pony` consumes it, persists known, summons; horizontal speed is `RUN_SPEED * 1.6` times aura mult.
4. Pony at rank 1 cannot use the steed (`"Your riding skill is too low."`); Journeyman at 5 + 50c unlocks the steed (2.0).
5. Expert at 8 + 200c; using `tawny_gryphon` sets `Motion.flying`; Space still ascends. Rank 2 **V** with only the pony does not fly.
6. Auto-attack, ability, and taking damage dismount. Entering crypt/barrow/delve dismounts and refuses remount (`"You cannot mount here."`).
7. Export/import round-trip keeps `riding_rank`, `known_mounts`, `last_mount`; old completion JSON without those keys loads as rank 0.
8. Bevy: Ross session Train riding; **V** after learning; other players see `mounted` on the snapshot. `cargo check -p woc-client` green.
9. `TICK_PHASES` fingerprint unchanged. `PROTOCOL_REV` remains **8**.
10. `docs/parity/STATUS.md`, `ROADMAP.md`, `DEMO.md`, README controls, changelog, `VERSION.toml` **1.16.0** / `mounts` when the implementation wave lands.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Flight masters / taxi nodes / flight paths | Travel network; separate program |
| Stables, mount equipment, passenger mounts | YAGNI |
| Paladin/Warlock class mounts / mount quests | Class identity already shipped |
| Aquatic / indoor-only mounts | Swim dismounts ground; instances refuse |
| Mount journal UI / favorites | **V** + last_id is enough |
| Daze / dismount chance on hit | Always dismount on damage |
| Skill-up from riding around | Rank is purchased, not ground |
| Replacing Travel Form / Ghost Wolf | Class signatures stay |
| `EntityKind::Mount` actor | Presentation child only |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |
| New tick phase | Toggle in `apply_intents_motion` |
| Protocol rev 9 | Additive fields |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Old clients still send `fly_toggle` expecting free flight | 1.16.0 min-client is the rewrite version; untrained **V** toasts instead of flying |
| `fly_toggle_enables_vertical_ascend` goes red | Rewrite against Expert + gryphon; keep flight kinematics tests |
| `PlayerPersistentState { ... }` literal sites miss new fields | Default empty; update every constructor in the persist task |
| Mount speed stacks with Travel Form 1.4 | Summoning clears those auras |
| Flying dismount fall damage surprises | Same path as today’s **V** off; test gryphon dismount near ground does not smash |
| Vendor **V** sell-junk vs world **V** | Already gated on `!ui.show_bags` |

## 9. Success demo (human)

1. Untrained **V** — toast, still on foot.
2. Talk to Stable Master Ross (east of Mara). Train riding at level 2 (10c). Buy Brown Pony (25c). Use it — learned, mounted, faster than a wolf.
3. **V** dismounts; **V** remounts the pony.
4. Hit a wolf — dismounted. Ability 1 — dismounted.
5. At 5, train Journeyman, buy Swift Bay Steed, use it — faster than the pony.
6. At 8, train Expert, buy Tawny Gryphon, **V** — Space/Ctrl vertical flight. Land, enter Eastbrook Crypt — cannot remount.
7. Relog: still know the mounts; start on foot; **V** summons last mount.

When §6 is green, tag `1.16.0`.
