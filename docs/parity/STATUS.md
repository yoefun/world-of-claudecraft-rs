# Parity status

**Current rewrite:** `1.8.0` / `class-forms`.  
**Post-completion program:** closed through `online-hard` — see [`docs/ROADMAP.md`](../ROADMAP.md).  
**Runbook:** [`../client-update.md`](../client-update.md). Class identity is `1.6.0`–`1.8.0`.  
**Next:** `1.9.0` / `quest-loop` (planned) — [`quest-loop design`](../superpowers/specs/2026-08-13-quest-loop-design.md) · [`implementation plan`](../superpowers/plans/2026-08-13-quest-loop.md)

## Client version gate (`client-compat`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| `check_compat` policy | done | `woc-version`; prerelease suffix stripped |
| `/version` protocol + min client | done | `WOC_MIN_CLIENT_VERSION` |
| Hello identity | done | Additive; missing → reject |
| Title Online preflight | done | Fail-closed |
| Welcome kick | done | `version:` → Title |

## Client update packages (`client-update`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Full `.tar.zst` pack / unpack | done | `woc-update::pack_full` |
| Per-file bsdiff delta | done | One predecessor (`N-1 → N`); skip-version uses full |
| ed25519 signed manifest | done | `WOC_UPDATE_SIGNING_KEY` / `WOC_UPDATE_PUBKEY` |
| `woc-updater` apply + exec client | done | Staging swap; `--once` for title launch |
| `/version` `update_manifest_url` | done | `WOC_UPDATE_MANIFEST_URL` on server |
| Title **Update** button | done | Sibling `woc-updater` when incompatible |
| Linux tag publish workflow | done | `.github/workflows/client-release.yml` |
| Runbook | done | [`docs/client-update.md`](../client-update.md) |

## Class identity (`class-engine` → `class-forms`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Protocol rev 7 identity snapshot | done | `combo_points` / `stealthed` / `stance_id` / `absorb`; `ToggleStealth` / `CycleStance` / `ToggleForm`. |
| Absorb before HP | done | Shield auras soak; depleted shields pop. |
| Interrupt lockout | done | Kick / Earth Shock / Counterspell set `cast_lockout` (≥1.5s). |
| Combo points | done | Sinister Strike builds; Eviscerate spends (`combo_per_point`). |
| Stealth | done | Rogue **Z**; aggro skip until melee; 0.7 move; breaks on damage / ability. |
| Charge / Blink / Convert | done | Engine verbs; Charge/Blink/Life Tap stubs off-kit until 1.7. |
| Hunter mana | done | Hunter `resource_type` is Mana. |
| Self-AoE | done | Frost Nova fires without a hostile target. |
| Rage from taken + Execute dump | done | Warriors gain rage when hit; Execute dumps leftover rage into damage. |
| 1.7 kit swaps (Charge/Blink/Shield on bar) | done | Warrior Charge; mage Blink; priest PW:S; hunter Aspect (1.1 damage). Dropped on-bar: rend, SW:P, Counterspell, Multi-Shot (still in `ABILITIES`). |
| 1.8 stance / form / shout / fear | done | **F** CycleStance / ToggleForm; Lightning Shield thorns; Fear/Travel Form break on damage. |

### Nine-class signatures

| Class | Signature |
| --- | --- |
| Warrior | Charge; **F** battle/defensive (Battle Shout 1.1 / armor_flat + outgoing 0.9) |
| Paladin | Devotion Aura at spawn; Crusader Strike seal DoT |
| Hunter | Aspect of the Hawk (1.1 dmg); mana |
| Rogue | **Z** stealth, combo, Cheap Shot |
| Priest | Power Word: Shield |
| Shaman | Lightning Shield thorns; **F** Ghost Wolf |
| Mage | Blink; Frost Nova self-AoE |
| Warlock | Life Tap; Fear (stun, breaks on damage) |
| Druid | **F** Travel Form (1.4 move, breaks on hit) |

## Next (`quest-loop`)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Giver / turn-in NPC checks | planned | Accept/turn-in fail unless the target template matches the table |
| `QuestDef.requires` chains | planned | Breadcrumb → hub sequences; integrity + acyclic tests |
| Talk / collect coverage | planned | Sim tests beyond the wolf kill path; ready toast |
| Generic **E** accept/turn-in | planned | Drop Captain Alden hardcoded ids |
| Named log + objective counts | planned | HUD uses `woc-content` `QuestDef` |
| Offer-aware map markers | planned | Yellow/green from `npc_quest_offers`, not raw table membership |

## Post-completion (`stable` → `online-hard`)

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

| Subsystem | Status | Notes |
| --- | --- | --- |
| Tick-phase contract vs `tick_all` | done | Nine named phases matching `tick_all` (pets/auras/PvP/market). |
| CI on `develop` | done | Workflow push/PR includes `develop`. |
| Protocol/crate “stub” / “framework slice” copy | done | Shipped interact actions; crate blurbs. |
| 1.0.0 acceptance demo doc | done | `docs/parity/DEMO.md`. |
| Data-driven `AbilityEffect` | done | Content tables; combat dispatches on the enum. |
| Heal / AoE / interrupt / taunt | done | Priest `flash_heal`; paladin `holy_light` / `holy_shock`; shaman `healing_wave`; druid `rejuvenation`; warrior cleave AoE + `taunt`; shaman `earth_shock` / rogue `kick` / mage `counterspell` interrupt. |
| Miss / crit hit table | done | 5% miss / 10% crit via sim RNG; heals do not miss. |
| Mining + blacksmithing | done | Copper veins in Eastbrook + Eastfen; smelt bar; craft/equip `copper_shortsword`. |
| Dungeon trash packs | done | Crypt and barrow spawn `DungeonTrashSpot` packs on enter. |
| Second dungeon or delve | done | `mirefen_barrow` (boss `barrow_hag`). |
| Ability-modifying talents | done | 4th talent/class: cleave targets / heal% / crit%. |
| Park / resume on reconnect | done | WS close parks the entity; Hello with the same `character_id` resumes it. |
| Snapshot AOI | done | 80 yd for other players/mobs/pets; zone NPCs and pending loot always included. |
| Postgres as documented production path | done | `DATABASE_URL` is the durable realm path; memory is the dev default. |
| ECS `Entity` split | done | Sparse-column `World` is the required actor store. |

## Rewrite 1.0.0-pre ↔ upstream 0.31.0 (`completion`)

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

Completion design: [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](../superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
Completion plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](../superpowers/plans/2026-07-28-rust-rewrite-completion.md)

Sim ECS (internal, post-completion): [`../superpowers/specs/2026-08-13-sim-ecs-design.md`](../superpowers/specs/2026-08-13-sim-ecs-design.md) · [`../superpowers/plans/2026-08-13-sim-ecs.md`](../superpowers/plans/2026-08-13-sim-ecs.md)

### Gameplay-core (completion gate)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `1.8.0` / class-forms (upstream still 0.31.0) |
| `woc-content` Eastbrook tables | done | |
| Deterministic tick (20 Hz) | done | locked phase fingerprint |
| Seeded RNG (mulberry32) | done | |
| SimContext seam | done | emit/lookup/mutate |
| Multi-player actor economy | done | typed ECS columns in `World` (source of truth) |
| Sticky WS realm | done | authenticated Hello + per-player snapshots |
| Client online mode | done | token + character_id Hello |
| Death / spirit / graveyard | done | |
| Combat core (GCD/cast/aura/threat) | done | DoT + consumable HoT; ability bar + Tab/Esc combat UX; class-kit identity (Execute, CC, dual Holy Shock) |
| Deeper bags / consumables | done | absolute inventory/bank slot indices |
| Tab targeting | done | |
| Player motion / colliders | done | |
| Mob respawn / social aggro | done | |
| `woc-persist` + auth API | done | R4 fields + deeds; WS load/save loop |
| Client login / char select | done | |
| Multi-ability kits | done | 4–5 slots/class; Execute / HealOrHarm / named DoT-HoT-CC auras |
| Talents / loadouts | done | 4/class; tier gates; numbered spend + pet key; stat + ability-mod effects |
| Pets | done | hunter/warlock |
| Zone2 + zone3 / Mirefen | done | Eastfen + Mirefen + Thornpeak quests/mobs |
| Party + chat | done | kill credit within 40 yd + same instance |
| Group loot rules | done | FFA + Need/Greed; rolls start on mob loot; pending in snapshot |
| Dungeons / instances | done | unique instance keys; party share; overworld preserved; crypt/barrow trash |
| Delves | done | eastbrook_hollow 3-room loop + reward |
| Bank + mail | done | durable character bank + copper vault; mail keyed by character UUID |
| Auction market | done | durable listings; list/buy/cancel from client; offline proceed/return via mail |
| Professions gather/craft | done | herbalism → alchemy; mining → blacksmithing sword |
| Duel + PvP honor | done | |
| World boss + deeds | done | one-shot deed completion persisted |
| Client economy/talent chrome | done | N/K/I/U panels; **M** world map + minimap |
| Heightfield terrain | done | Continuous strip seed `20061`; golden ε≈1e-3 vs upstream pin; editor custom maps deferred |
| Procedural character / scene visuals | done | Class/template mesh recipes; buildings, portals, zone atmosphere; create preview |
| In-world NPC/mob scene load | done | Nameplates, quest/vendor markers, target ring, gather herbs, spawn lifecycle |
| Entity walk / remove presentation | done | Locomotion hysteresis, procedural gait limbs, corpse tip, soft despawn fade |
| Jump / swim / travel flight | done | Coyote jump, gravity + fall damage, lake tread, V-toggle flight |
| Sim typed ECS columns | done | Sparse `World` in `woc-sim`; Bevy stays presentation-only |
| Byte-identical terrain/combat | n/a | Explicit non-goal |
| Minimap / world map UI | done | Functional Bevy paint (not full DESIGN.md chrome) |

### Explicit deferred / non-goals

| Subsystem | Status |
| --- | --- |
| Map editor / `setActiveWorldContent` custom heightfields | deferred |
| Browser Three.js / Electron / Capacitor | deferred |
| Web3 / wallets / cosmetics shop | deferred |
| Gymnasium RL / headless env | deferred |
| Full i18n catalogs | deferred |
| Admin SPA / Discord OAuth polish | deferred |
| Vale Cup / Card Duel / Fiesta | deferred |

## Prior rewrites

0.3.0 `online-alive`, 0.2.0 `framework`, 0.1.0 `combat-slice` — superseded by the table above.
