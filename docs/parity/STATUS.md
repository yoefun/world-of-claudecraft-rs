# Parity status

## Rewrite 0.2.0 ↔ upstream 0.31.0 (`framework`)

Legend: `done` · `partial` · `deferred` · `n/a`

Plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-framework.md`](../superpowers/plans/2026-07-28-rust-rewrite-framework.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `0.2.0` / framework |
| `woc-content` data tables | done | classes/items/mobs/npcs/quests/zone1 |
| Deterministic tick (20 Hz) | done | phased tick in `woc-sim` |
| Seeded RNG (mulberry32) | done | |
| SimContext seam | partial | thin emit context; leaves still take slices |
| WorldHost trait | done | offline Bevy + server |
| Nine classes + starter kits | done | one primary ability each |
| Inventory backpack + stacking | done | 16-slot bag |
| Equipment + `recalc_player_stats` | done | MainHand / Chest |
| Quest accept / credit / turn-in | done | 3 Eastbrook quests |
| NPC talk + dialog events | done | |
| Vendor buy/sell | done | Trader Wilkes |
| Eastbrook multi-camp spawn | done | wolves + boars from tables |
| Client UI windows | partial | bags/quest HUD toggles; no full window chrome |
| `woc-server` WebSocket sim host | done | in-memory; `/ws/game` |
| Client online mode | deferred | server ready; Bevy still offline-primary |
| Heightfield terrain | partial | not byte-identical |
| Player motion | partial | no full parkour |
| Postgres auth / characters | deferred | |
| Talents / dungeons / PvP / market / professions | deferred | |
| i18n / Web3 / RL / Electron | deferred | |
| Byte-identical terrain/combat | n/a | Explicit non-goal |

## Rewrite 0.1.0 baseline (`combat-slice`)

Superseded by 0.2.0 framework tables above. Historical note: first playable Warrior wolf combat on Bevy offline host.
