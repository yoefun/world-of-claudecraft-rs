# Rust rewrite completion design — remaining features

**Status:** Shipped as rewrite `1.0.0-pre` / parity `completion` (2026-07-28 program; polish continued on `develop`).  
**Successor:** [`2026-08-13-post-completion-program-design.md`](2026-08-13-post-completion-program-design.md).  
**Baseline (when written):** rewrite `0.2.0` / parity `framework` on `develop`.  
**Upstream pin (unchanged unless bumped later):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `completion` — gameplay-core remaining systems rewritten in Rust (not byte-identical / not full platform parity).

## 1. Goal

Finish the **remaining gameplay-core rewrite** after the Eastbrook framework slice so that:

> offline + online Bevy clients share one deterministic `woc-sim`, with durable characters, class depth (abilities/talents/pets), multi-zone content, parties, instances, economy, professions, and light PvP — without porting browser/Electron/Web3/RL/admin surfaces.

`0.1.0` proved combat.  
`0.2.0` proved the MMO skeleton (content tables, inventory, quests, vendor, WS host).  
**This program** ports the rest of the *game*, not the *product shell*.

## 2. Baseline (already shipped on `develop`)

| Piece | State |
| --- | --- |
| Crates | `woc-version`, `woc-protocol`, `woc-content`, `woc-sim`, `woc-server`, `woc-client` |
| Content | Eastbrook tables: 9 classes, items, mobs, NPCs, ≥3 quests, wolves+boars |
| Sim | 20 Hz, mulberry32, inventory/equip/quests/interact/vendor |
| Protocol | rev 2 — intents, interactions, snapshots, `WorldHost`, WS envelopes |
| Server | `/health`, `/version`, `/ws/game` (in-memory; **effectively single-player**) |
| Client | Bevy offline host; bags/quest toggles; **no online mode** |
| Debt | `SimContext` unused; `main.rs` god-file; Hello resets realm; economy fields on `Sim` not `Entity` |

## 3. Definition of done — “剩下功能全部重写完成”

All of the following must be true at rewrite **`1.0.0-pre`** (or tagged `completion`):

1. **One sim, multiple hosts** still holds: `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
2. **Online co-presence:** ≥2 Bevy clients on one realm see each other move; Hello does not wipe the world.
3. **Persistence:** register/login + character CRUD + load/save (pos/xp/inventory/quests/talents) via Postgres (`woc-persist`).
4. **Combat depth:** GCD, cast bar, auras (DoT/HoT), swing timer, threat-driven mob targeting.
5. **Class depth:** ≥3 abilities per class; talent spend changes measurable combat; hunter/warlock pet summon.
6. **World:** zone2 (and zone3 or documented stub portal) with graveyards, denser quests/mobs, death→spirit→respawn.
7. **Social PvE:** party invite, party chat, kill credit / XP share, FFA vs Need/Greed loot.
8. **Instances:** ≥1 dungeon instance shell + boss kill under party loot; delve run optional same shell.
9. **Economy:** bank + mail + auction list/buy durable across restart.
10. **Professions:** gather node → craft recipe → item in bag (one gathering + one crafting path).
11. **PvP light:** duel + open-world PvP flag + honor currency (no arena required).
12. **Docs/CI:** `VERSION.toml` / `docs/parity/STATUS.md` / `CHANGELOG.md` updated; non-GPU workspace tests green; client `cargo check`.

### Explicit non-goals (permanent / long-term skip)

| Skip | Rationale |
| --- | --- |
| Browser Three.js client | Bevy is the client |
| Electron / Steam / Capacitor shells | Native binary only |
| Web3 / wallets / Claudium shop / cosmetics | Not gameplay-core |
| Gymnasium / RL headless | Research surface |
| Full i18n catalogs | English strings only |
| Admin SPA / Discord bot / OAuth polish | Optional later; password auth first |
| Antibot / Meta CAPI / Turnstile | Platform fluff |
| Map/music editors, guide site, MediaWiki | Authoring/docs, not sim |
| Vale Cup / Card Duel / Fiesta / Yumi maze | Minigame fluff |
| Byte-identical terrain/combat vs TS | Explicit rewrite non-goal |
| Full DESIGN.md UI chrome parity | Functional Bevy UI only |

## 4. Architecture

### 4.1 Crate layout (target)

```
crates/
  woc-version/      # pin constants
  woc-content/      # data tables (expand zones/talents/dungeons/…)
  woc-protocol/     # intents, snapshots, events, WorldHost, WS/REST DTOs
  woc-sim/          # deterministic core (module tree mirrors upstream areas)
  woc-persist/      # NEW — Postgres schema, auth, character save/load
  woc-server/       # axum HTTP + WS realm host
  woc-client/       # Bevy offline | online host (split modules)
```

### 4.2 Hard sequencing rules

1. **`ws-simcontext` + multi-player Entity migration** land before parallel sim feature leaves that touch economy/combat tick order.
2. **`woc-protocol` is a choke point** — freeze additive rev bumps in short protocol PRs before consumer waves.
3. **Content-only** and **Bevy UI-only** streams may always run in parallel with each other.
4. **Persistence** waits on sticky online sessions (no Hello-reset).
5. **Instances** wait on party + loot rules + combat core.
6. Tick phase order, once locked by hash tests, **must not reorder**.

### 4.3 Target tick phases (after combat core)

1. Drain queued intents / interactions  
2. Player motion / physics  
3. Player combat (GCD/cast/auto/auras)  
4. Pet AI  
5. Mob AI + mob combat  
6. Aura/timer decay  
7. Loot despawn / pickup / rolls  
8. Quest ready recompute  
9. Snapshot + drain events  

All RNG through `Sim.rng` (mulberry32). No wall clock in sim.

### 4.4 Multi-player model

- One realm `Sim` with many player `Entity`s (cap e.g. 16 for early online).
- Per-player: inventory, equipment, quest log, xp/copper, talents on **entity / player component**, not on `Sim`.
- `push_intent(player_id, …)` / `interact(player_id, …)` always keyed.
- `snapshot_for(player_id)` may still be full-zone early; AOI later optional.
- Disconnect: park or despawn per policy; **never** recreate entire Eastbrook on Hello.

## 5. Version map

| Rewrite | Parity label | Theme |
| --- | --- | --- |
| **0.2.x** | `framework-polish` | SimContext, client split, UI chrome, multi-session WS |
| **0.3.0** | `online-alive` | Online client, death loop, combat/motion/bags foundation |
| **0.4.0** | `online-persist` | `woc-persist`, auth, character CRUD |
| **0.5.0** | `class-depth` | Multi-ability, talents, pets |
| **0.6.0** | `open-world` | Zone2/3, denser quests, graveyards |
| **0.7.0** | `group-pve` | Party/chat, loot rules, dungeon (+ delve) |
| **0.8.0** | `economy` | Bank, mail, market |
| **0.9.0** | `professions-pvp` | Gather/craft, duel/honor |
| **1.0.0-pre** | `completion` | World boss/deeds light + STATUS all core rows green |

Upstream pin stays **0.31.0** unless explicitly bumped in a dedicated change.

## 6. Parallel execution model

See implementation plan:

- [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](../plans/2026-07-28-rust-rewrite-completion.md)

**Model:** main agent owns merge-base (`develop` or integration branch), freezes protocol/sim contracts per wave, dispatches **N subagents on isolated branches/worktrees**, then merges sequentially by dependency, runs full workspace tests, updates STATUS.

## 7. Risks

| Risk | Mitigation |
| --- | --- |
| Scope = full upstream port | Hard non-goals; DoD is gameplay-core |
| Protocol / `sim.rs` merge hell | Protocol PR first; one core owner for `sim.rs` |
| Parallel agents thrash `SimContext` | Foundation batch lands alone before leaf adoption |
| Hello-reset + persist race | Sticky realm before `woc-persist` |
| Bevy god-file conflicts | Split `woc-client` modules before online/UI features |
| Over-faithful combat port | Simplified hit tables / effect_dispatch; tests lock behavior |

## 8. Success demo (human, end state)

1. Two clients online on one server; both see each other.  
2. Create chars, quit, re-login — gear/quests/talents restored.  
3. Spend talents, cast 3+ abilities, summon pet.  
4. Travel to zone2, die, respawn at graveyard.  
5. Party dungeon boss → Need/Greed loot.  
6. Bank item, mail copper, list auction, gather+craft one recipe.  
7. Duel a player; honor increments.

When that script works and CI is green, rewrite completion is achieved under this design.
