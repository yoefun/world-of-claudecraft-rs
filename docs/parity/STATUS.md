# Parity status

## Rewrite 0.1.0 ↔ upstream 0.31.0 (`combat-slice`)

Legend: `done` · `partial` · `deferred` · `n/a` · `planned` (targeted for rewrite 0.2 framework)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `VERSION.toml`, `woc-version`, HUD footer |
| Deterministic tick (20 Hz) | done | `woc-sim` |
| Seeded RNG (mulberry32) | done | Compatible mulberry32 |
| Heightfield terrain | partial | Eastbrook-like procedural field; not byte-identical |
| Warrior class create | partial | Single class, starter stats |
| Player motion (WASD) | partial | Wish vector + ground clamp; no full parkour |
| Wolf mob + aggro/chase | done | Combat-slice camp |
| Auto-attack | done | Melee swing timer |
| Starter ability (Heroic Strike) | done | Slot 1 |
| XP / level-up | done | Simple XP table |
| Loot drop | partial | Copper + stub item on kill |
| HUD (HP/rage/XP/target) | done | Bevy UI |
| Offline in-process host | done | Bevy embeds `Sim` |
| Authoritative multiplayer server | deferred | `/health` + version stub only |
| Quests | deferred | |
| Talents / 9 classes | deferred | |
| Dungeons / PvP / market | deferred | |
| i18n / Web3 / RL / Electron | deferred | |
| Full content tables | deferred | |

## Rewrite 0.2.0 target ↔ upstream 0.31.0 (`framework`)

Plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-framework.md`](../superpowers/plans/2026-07-28-rust-rewrite-framework.md)  
Design: [`docs/superpowers/specs/2026-07-28-rust-rewrite-framework-design.md`](../superpowers/specs/2026-07-28-rust-rewrite-framework-design.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| `woc-content` data tables | planned | classes/items/mobs/npcs/quests/zone1 |
| SimContext + phased tick | planned | Upstream-inspired seam; simplified phases |
| WorldHost trait (offline + online) | planned | Shared host API |
| Nine classes + starter kits | planned | One primary ability each |
| Inventory backpack + stacking | planned | Replace stub `bag_item` |
| Equipment + `recalc_player_stats` | planned | MainHand/OffHand/Chest |
| Quest accept / credit / turn-in | planned | ≥3 Eastbrook quests |
| NPC talk + dialog events | planned | |
| Vendor buy/sell | planned | |
| Eastbrook multi-camp spawn | planned | Wolves + boars from tables |
| Client UI windows (bags/quest/char/vendor) | planned | Functional Bevy UI |
| `woc-server` WebSocket sim host | planned | In-memory; no Postgres |
| Client online mode | planned | WS consumer of snapshots |
| Postgres auth / characters | deferred | Post-0.2 |
| Talents / dungeons / PvP / market / professions | deferred | Post-0.2 |
| i18n / Web3 / RL / Electron | deferred | Post-0.2 |
| Byte-identical terrain/combat | n/a | Explicit non-goal |
