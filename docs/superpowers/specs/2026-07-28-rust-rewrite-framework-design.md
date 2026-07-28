# Rust rewrite 0.2 — basic framework design

**Status:** Proposed (planning deliverable 2026-07-28).  
**Rewrite target:** `0.2.0`  
**Upstream pin (unchanged unless bumped later):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`)  
**Parity target label:** `framework`

## 1. Goal

Finish the **basic game framework** for the Rust rewrite so that offline (and a thin online host) can run the classic Eastbrook loop end-to-end:

> pick a class → enter Eastbrook Vale → talk to NPCs → accept quests → kill/collect → loot into bags → equip → vendor → turn in → level → repeat

`0.1.0` already proved the combat kernel (Warrior + wolves + XP).  
`0.2.0` makes that kernel a **reusable MMO skeleton**: data-driven content, inventory, quests, interaction, host facade, and a server that embeds the same sim.

This is **not** full upstream feature parity (dungeons, talents trees, PvP, market, professions, Web3, RL, i18n, Electron).

## 2. Current baseline (0.1.0)

| Piece | State |
| --- | --- |
| Workspace crates | `woc-version`, `woc-protocol`, `woc-sim`, `woc-client`, `woc-server` |
| Sim | Deterministic 20 Hz, mulberry32, Warrior-only combat slice |
| Protocol | Minimal `PlayerIntent` / `TickSnapshot` / `SimEvent` |
| Client | Bevy offline host + combat HUD |
| Server | HTTP `/health` + `/version` only |
| Content | Hardcoded wolf camp in `Sim::new_combat_slice` |

## 3. Definition of done — “基本框架全部重写完成”

All of the following must be true for rewrite `0.2.0`:

1. **One sim, multiple hosts** still holds: `woc-sim` has no Bevy / wgpu / net deps.
2. **Content is data**, not hardwired camps: classes, abilities, items, mobs, NPCs, and quests live in `woc-content` tables loaded by sim.
3. **Nine classes** can be created offline with starter kits (resource type + one primary ability each; simplified kits OK).
4. **Inventory + equipment** work: backpack slots, equip weapon/chest, `recalc_player_stats`, loot goes into bags.
5. **Quest pipeline** works: accept → kill/collect credit → ready → turn-in → XP/copper/item rewards, with ≥3 authored Eastbrook starter quests.
6. **NPC interaction** works: talk / quest dialog / simple vendor buy-sell within interact range.
7. **Eastbrook zone scaffold** loads town NPCs + ≥2 mob camps from content (wolves + boars), not a single hardcoded spawn list.
8. **Host facade** (`WorldHost` trait in `woc-protocol` or thin `woc-host`) is implemented by offline Bevy and by `woc-server`.
9. **Online skeleton**: `woc-server` hosts `woc-sim` over WebSocket, applies intents, streams snapshots/events (in-memory sessions; **no Postgres** in 0.2).
10. **Client UI framework**: character create (9 classes), bags, quest log, character sheet, vendor panel — functional, not full DESIGN.md chrome parity.
11. **Docs/CI**: `VERSION.toml` → `0.2.0`, `docs/parity/STATUS.md` updated, determinism tests cover quest+inventory paths, CI green for non-GPU crates.

### Explicit non-goals (deferred past 0.2)

- Full talent trees / 27 specs  
- Zones 2–3, dungeons, delves, raids, world bosses  
- Parties, guilds, mail, market, professions, PvP, Vale Cup, Card Duel, deeds  
- Postgres auth/characters, Discord OAuth, Steam, Web3, RL env, Electron packaging  
- Byte-identical terrain / combat vs TypeScript  
- Full i18n catalog / Three.js browser client  

## 4. Architecture

### 4.1 Crate layout (0.2)

```
crates/
  woc-version/     # pin constants (rewrite 0.2.0, upstream 0.31.0)
  woc-content/     # NEW — pure data tables (classes, items, mobs, npcs, quests, zone1)
  woc-protocol/    # intents, snapshots, events, WorldHost trait, WS message envelopes
  woc-sim/         # deterministic core; depends on content + protocol only
  woc-client/      # Bevy offline (and later online) host
  woc-server/      # axum HTTP + WebSocket sim host
```

Optional later (not required for 0.2): `woc-persist` for Postgres.

### 4.2 Sim internals (inspired by upstream, simplified)

Upstream pattern to mirror at small scale:

- Thin `Sim` facade + phased `tick()`
- Leaf modules behind a `SimContext` seam (callbacks for emit / lookup / mutate)
- Content tables (`CLASSES`, `ABILITIES`, `ITEMS`, `MOBS`, `NPCS`, `QUESTS`) in `woc-content`
- Interaction commands as free functions (`interact`, `loot_corpse`, `buy`, `sell`)
- Quest credit pure updaters (`on_mob_killed`, `on_inventory_changed`, `check_ready`)

**0.2 tick phases (fixed order — do not reorder once tests lock it):**

1. Apply pending intents / interactions queued this tick  
2. Player motion  
3. Player combat (cast / auto-attack / resource regen)  
4. Mob AI + mob combat  
5. Aura/timer decay (minimal; stubs OK if unused)  
6. Loot despawn / pickup proximity  
7. Quest ready recompute (if dirty)  
8. Build snapshot + drain events  

All RNG through `Sim.rng` (mulberry32). No `thread_rng`, no wall clock in sim.

### 4.3 Host facade

```rust
/// Implemented by Bevy offline host and woc-server session.
pub trait WorldHost {
    fn push_intent(&mut self, player_id: EntityId, intent: PlayerIntent);
    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction);
    fn tick(&mut self) -> (TickSnapshot, Vec<SimEvent>);
    fn snapshot(&self, player_id: EntityId) -> TickSnapshot;
}
```

Client never decides combat, loot, quest credit, or vendor prices — only sends intents/actions and renders snapshots/events.

### 4.4 Online skeleton

- `GET /health`, `GET /version` (keep)  
- `WS /ws/game` — client connects, sends `{Hello, Intent, Interact}`, receives `{Welcome, Snapshot, Events}`  
- One in-memory realm: single `Sim`, multiple player entities (cap small, e.g. 8)  
- No auth: display name from Hello; disconnect removes or parks character  
- Same tick rate 20 Hz on a tokio interval  

### 4.5 Client UI framework

Keep Bevy. Expand beyond combat HUD:

- **Title / character create**: name + 9-class grid  
- **In-world HUD**: HP/resource/XP/target (existing) + quest tracker strip  
- **Windows** (toggle keys): bags (`B`), character (`C`), quest log (`L`), vendor (on interact)  
- Windows are host-side views of snapshot fields; mutations go through `InteractAction` / intents  

Visual polish follows “functional first”; do not block 0.2 on upstream DESIGN.md token parity.

## 5. Content scope (Eastbrook scaffold)

Minimum authored content for 0.2:

| Table | Minimum rows |
| --- | --- |
| Classes | 9 (warrior…warlock) with starter weapon/chest/ability/resource |
| Abilities | ≥9 primary abilities (one per class) + shared auto-attack rules |
| Items | starter gear, bread/water, wolf/boar junk, 2–3 vendor goods, quest rewards |
| Mobs | Young Wolf, Scarred Wolf, Young Boar (+ templates with XP/loot tables) |
| NPCs | ≥1 quest giver, ≥1 vendor, ≥1 flavor greeter in town |
| Quests | ≥3: kill wolves, collect boar tusks (or hides), talk/turn-in chain starter |
| Zone | Eastbrook spawn + town NPC spots + wolf camp + boar meadow |

Zone 2/3 portals may exist as **non-functional markers** or be omitted.

## 6. Protocol expansion (summary)

New / extended types (serde JSON for WS + offline):

- `PlayerClass` enum (9 variants)  
- `InteractAction`: `Talk`, `AcceptQuest { quest_id }`, `TurnInQuest { quest_id }`, `Buy { item_id, count }`, `Sell { bag_slot, count }`, `Equip { bag_slot }`, `Unequip { equip_slot }`, `LootCorpse { target_id }`  
- Snapshot fields: `inventory`, `equipment`, `quest_log`, `vendor_stock` (when open), `resource_type`  
- Events: `QuestAccepted`, `QuestProgress`, `QuestCompleted`, `ItemGained`, `ItemLost`, `Equipped`, `VendorOpen`, `ChatNpc`  

Wire compatibility with 0.1 is **not** required (rewrite still pre-1.0).

## 7. Versioning & tracking

| Artifact | 0.2 change |
| --- | --- |
| `VERSION.toml` | `rewrite_version = "0.2.0"`, `parity_target = "framework"` |
| Workspace `version` | `0.2.0` |
| `UPSTREAM.md` | note framework target; pin stays 0.31.0 unless intentionally bumped |
| `docs/parity/STATUS.md` | mark framework rows done/partial; keep deferred rows |
| `CHANGELOG.md` | 0.2.0 section |

## 8. Testing strategy

- Unit tests in `woc-content` (table integrity: every class start item exists, every quest NPC exists)  
- Determinism: same seed + intent script → identical snapshots/events hash  
- Quest path integration in `woc-sim`: accept → kill N → turn-in  
- Inventory path: loot → equip → vendor sell  
- Server: WS hello + one tick roundtrip (tokio test)  
- Client: `cargo check -p woc-client` in CI (no GPU playtest required in CI)

## 9. Phased delivery (maps to implementation plans)

| Phase | Outcome | Suggested rewrite tag |
| --- | --- | --- |
| A | `woc-content` + `SimContext` + phased tick + `WorldHost` | 0.2.0-dev (internal) |
| B | Inventory / equipment / loot-into-bags |  |
| C | 9 classes + ability kits + ability model |  |
| D | Quests + NPC talk + Eastbrook content tables |  |
| E | Vendor + richer Eastbrook camps |  |
| F | `woc-server` WS host + client online mode toggle | **0.2.0** release |

Phases A→F are sequential dependencies; B/C may partially overlap after A lands.

## 10. Risks & mitigations

| Risk | Mitigation |
| --- | --- |
| Scope creep into full MMO | Hard non-goals list; reject content beyond Eastbrook scaffold |
| God-file `sim.rs` returns | Enforce module leaves + SimContext from Phase A |
| Protocol churn breaks client often | Expand snapshot behind versioned `protocol_rev` field |
| Online without auth abused | Dev-only bind localhost default; document as scaffold |
| Bevy UI cost | Prefer simple node UI; one window module at a time |

## 11. Success demo script (human)

1. `cargo run -p woc-client` → create Mage (or any non-Warrior) → spawn in Eastbrook.  
2. Talk to quest NPC → accept “Young Wolves”.  
3. Kill required wolves → loot → see bag update + quest progress.  
4. Turn in → XP/reward item → equip if gear.  
5. Buy bread from vendor with copper.  
6. Separately: `cargo run -p woc-server` + client online mode → second window sees the same world tick.

When that script works and CI is green, **基本框架重写完成**.
