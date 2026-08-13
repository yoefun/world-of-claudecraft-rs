# Parity status

## Rewrite 1.0.0-pre ↔ upstream 0.31.0 (`completion`)

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

Completion design: [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](../superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
Completion plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](../superpowers/plans/2026-07-28-rust-rewrite-completion.md)

### Gameplay-core (completion gate)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `1.0.0-pre` / completion |
| `woc-content` Eastbrook tables | done | |
| Deterministic tick (20 Hz) | done | locked phase fingerprint |
| Seeded RNG (mulberry32) | done | |
| SimContext seam | done | emit/lookup/mutate |
| Multi-player Entity economy | done | |
| Sticky WS realm | done | authenticated Hello + per-player snapshots |
| Client online mode | done | token + character_id Hello |
| Death / spirit / graveyard | done | |
| Combat core (GCD/cast/aura/threat) | done | DoT + consumable HoT |
| Deeper bags / consumables | done | absolute inventory/bank slot indices |
| Tab targeting | done | |
| Player motion / colliders | done | |
| Mob respawn / social aggro | done | |
| `woc-persist` + auth API | done | R4 fields + deeds; WS load/save loop |
| Client login / char select | done | |
| Multi-ability kits | done | |
| Talents / loadouts | done | 3/class; tier gates; numbered spend + pet key; damage/hp/armor/resource effects |
| Pets | done | hunter/warlock |
| Zone2 + zone3 / Mirefen | done | Eastfen + Mirefen + Thornpeak quests/mobs |
| Party + chat | done | kill credit within 40 yd + same instance |
| Group loot rules | done | FFA + Need/Greed |
| Dungeons / instances | done | unique instance keys; party share; overworld preserved |
| Delves | done | eastbrook_hollow 3-room loop + reward |
| Bank + mail | done | durable character bank; mail keyed by character UUID |
| Auction market | done | durable listings; offline proceed/return via mail |
| Professions gather/craft | done | herbalism → alchemy salve |
| Duel + PvP honor | done | |
| World boss + deeds | done | one-shot deed completion persisted |
| Client economy/talent chrome | done | N/K/I/U panels; **M** world map + minimap |
| Heightfield terrain | done | Continuous strip seed `20061`; golden ε≈1e-3 vs upstream pin; editor custom maps deferred |
| Procedural character / scene visuals | done | Class/template mesh recipes; buildings, portals, zone atmosphere; create preview |
| In-world NPC/mob scene load | done | Nameplates, quest/vendor markers, target ring, gather herbs, spawn lifecycle |
| Entity walk / remove presentation | done | Locomotion hysteresis, procedural gait limbs, corpse tip, soft despawn fade |
| Jump / swim / travel flight | done | Coyote jump, gravity + fall damage, lake tread, V-toggle flight |
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
