# Parity status

## Rewrite program ↔ upstream 0.31.0

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

Completion design: [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](../superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
Completion plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](../superpowers/plans/2026-07-28-rust-rewrite-completion.md)

### Framework (0.2.0) — shipped

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `0.2.0` / framework |
| `woc-content` Eastbrook tables | done | classes/items/mobs/npcs/quests/zone1 |
| Deterministic tick (20 Hz) | done | |
| Seeded RNG (mulberry32) | done | |
| SimContext seam | partial | unused by leaves — Wave 0 |
| WorldHost trait | done | offline Bevy + server |
| Nine classes + starter kits | done | one primary ability each |
| Inventory backpack + stacking | done | 16-slot bag |
| Equipment + `recalc_player_stats` | done | MainHand / Chest |
| Quest accept / credit / turn-in | done | 3 Eastbrook quests |
| NPC talk + dialog events | done | |
| Vendor buy/sell | done | Trader Wilkes |
| Eastbrook multi-camp spawn | done | wolves + boars |
| Client UI windows | partial | bags/quest toggles — Wave 0 |
| `woc-server` WebSocket sim host | partial | in-memory; Hello resets realm — Wave 0 |
| Client online mode | planned | Wave 1 |
| Heightfield terrain | partial | not byte-identical |
| Player motion | partial | Wave 1 colliders |
| Byte-identical terrain/combat | n/a | Explicit non-goal |

### Completion backlog (planned)

| Subsystem | Target rewrite | Status |
| --- | --- | --- |
| Multi-player Entity + sticky realm | 0.2.x / 0.3 | planned |
| Death / spirit / graveyard | 0.3 | planned |
| Combat core (GCD/cast/aura/threat) | 0.3 | planned |
| Deeper bags / consumables | 0.3 | planned |
| Tab targeting | 0.3 | planned |
| Mob respawn / social aggro | 0.3 | planned |
| Postgres auth + character CRUD | 0.4 | planned |
| Multi-ability kits | 0.5 | planned |
| Talents / loadouts | 0.5 | planned |
| Pets | 0.5 | planned |
| Zone2 / Zone3 | 0.6 | planned |
| Party + chat | 0.7 | planned |
| Group loot rules | 0.7 | planned |
| Dungeons / instances | 0.7 | planned |
| Delves | 0.7.x | planned |
| Bank + mail | 0.8 | planned |
| Auction market | 0.8 | planned |
| Professions gather/craft | 0.9 | planned |
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

## Rewrite 0.1.0 baseline (`combat-slice`)

Superseded by 0.2.0 framework tables above.
