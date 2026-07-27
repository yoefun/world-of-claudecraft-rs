# Parity status

## Rewrite 0.3.0 ↔ upstream 0.31.0 (`online-alive`)

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

Completion design: [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](../superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
Completion plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](../superpowers/plans/2026-07-28-rust-rewrite-completion.md)

### Shipped through Wave 1

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `0.3.0` / online-alive |
| `woc-content` Eastbrook tables | done | |
| Deterministic tick (20 Hz) | done | locked phase fingerprint |
| Seeded RNG (mulberry32) | done | |
| SimContext seam | done | emit/lookup/mutate |
| Multi-player Entity economy | done | xp/copper on Entity; intent map |
| Sticky WS realm | done | Hello spawn/despawn; no full reset |
| Client online mode | done | title Offline\|Online; tungstenite thread |
| Death / spirit / graveyard | done | `release_spirit` → eastbrook graveyard |
| Combat core (GCD/cast/aura/threat) | done | DoTs + timed cast + GCD |
| Deeper bags / consumables | done | Head/OH/Legs/Feet; UseItem; level_req |
| Tab targeting | done | `Sim::tab_target` / `clear_target` |
| Client module split | done | title/char_create/world/input/hud/online |
| Content stubs (talents/zone2/…) | done | |
| Protocol death/aura/party fields | done | additive; PROTOCOL_REV 2 |
| Nine classes + starter kits | done | |
| Inventory / equipment / quests / vendor | done | framework |
| Heightfield terrain | partial | not byte-identical |
| Player motion / colliders | done | Eastbrook inn AABB + sweep (R1) |
| Mob respawn / social aggro | done | (R1) |
| `woc-persist` + auth API | partial | memory default; Postgres optional (R1) |
| Zone2 content tables | partial | Eastfen filled; transition later |
| Professions content | partial | herbalism/alchemy tables (R1) |
| Client UI chrome | done | char/vendor/cast/action bar (R1) |
| Mob respawn / social aggro | planned | Wave 1B |
| Byte-identical terrain/combat | n/a | Explicit non-goal |

### Completion backlog

| Subsystem | Target rewrite | Status |
| --- | --- | --- |
| Mob respawn / social aggro | 0.3.x | done (R1) |
| Postgres auth + character CRUD | 0.4 | done (R1+R2 login UI) |
| Multi-ability kits | 0.5 | done (R2) |
| Talents / loadouts | 0.5 | planned |
| Pets | 0.5 | done (R2) |
| Zone2 / Zone3 | 0.6 | partial (zone2 content R1) |
| Party + chat | 0.7 | done (R2) |
| Group loot rules | 0.7 | planned |
| Dungeons / instances | 0.7 | planned |
| Delves | 0.7.x | planned |
| Bank + mail | 0.8 | planned |
| Auction market | 0.8 | planned |
| Professions gather/craft | 0.9 | partial (content R1) |
| Duel + PvP honor | 0.9 | planned |
| World boss + deeds | 1.0-pre | planned |

### Explicit deferred / non-goals

| Subsystem | Status |
| --- | --- |
| Browser Three.js / Electron / Capacitor | deferred |
| Web3 / wallets / cosmetics shop | deferred |
| Gymnasium RL / headless env | deferred |
| Full i18n catalogs | deferred |
| Admin SPA / Discord OAuth polish | deferred |
| Vale Cup / Card Duel / Fiesta | deferred |

## Rewrite 0.2.0 / 0.1.0

Superseded by tables above.
