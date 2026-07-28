# Changelog

## 1.0.0-pre — 2026-07-28

### Added

- Talents: class trees, spend/respec, damage_pct multipliers on combat.
- Party loot rules: FFA + Need/Greed rolls via sim RNG.
- Economy: personal bank deposit/withdraw; mail send/collect; auction list/buy/cancel/expire.
- Professions sim: train herbalism/alchemy → gather nodes → craft recipes.
- Zone transition Eastbrook ↔ Eastfen ↔ Mirefen without wiping player progression.
- Dungeon instance shell (`eastbrook_crypt`) with boss spawn + leave.
- Delve loop (`eastbrook_hollow`): 3 rooms → advance → reward.
- Light PvP: duel challenge/accept, open-world PvP flag, honor currency.
- World boss deed (`mire_terror` / eastfen_mire_terror) granting honor on kill.
- Protocol additive interact/events/snapshot fields for the above (rev still 2).
- Persist R4 character fields: zone, talents, bank, honor, professions (backward-compatible JSON).
- Bevy client panels: talents (N), bank (K), mail (M), market (U).

### Changed

- Parity target `online-alive` → `completion`.
- Rewrite version `0.3.0` → `1.0.0-pre`.
- Mirefen filled from placeholder to open-world content.

## 0.3.0 — 2026-07-28

### Added

- Multi-player sticky realm: per-player xp/copper, intent map, spawn/despawn without resetting Eastbrook.
- Expanded `SimContext` + locked tick-phase fingerprint.
- Death / spirit release / Eastbrook graveyard respawn.
- Combat core: GCD, timed casts, DoT auras, threat stub; snapshot auras/cast/`is_dead`.
- Deeper bags: Head/OffHand/Legs/Feet, `UseItem` consumables, level-req equip.
- Tab targeting + clear target.
- Bevy client online mode (`ws://127.0.0.1:8787/ws/game`) alongside offline.
- Content stubs: talents, zone2 placeholders, graveyards, dungeons.
- Client module split (title / char create / world / input / hud / online).

### Changed

- Parity target `framework` → `online-alive` (upstream pin remains 0.31.0).
- Rewrite version `0.2.0` → `0.3.0`.

## 0.2.0 — 2026-07-28

### Added

- `woc-content` crate: classes, abilities, items, mobs, NPCs, quests, Eastbrook layout.
- Protocol rev 2: interactions, inventory/equipment/quest snapshots, `WorldHost`, WS envelopes.
- Framework sim: content-driven Eastbrook spawn, backpack inventory, equipment + stat recalc.
- Nine class create path with starter kits and primary abilities.
- Quest accept / kill-collect credit / turn-in (≥3 Eastbrook quests).
- NPC talk + vendor buy/sell.
- `woc-server` WebSocket host at `/ws/game` embedding `woc-sim`.
- Bevy client: class select, E interact, B bags, L quest log, quest tracker.

### Changed

- Parity target `combat-slice` → `framework` (upstream pin remains 0.31.0).

## 0.1.0 — 2026-07-27

### Added

- Cargo workspace scaffold: `woc-version`, `woc-protocol`, `woc-sim`, `woc-server`, `woc-client`.
- Upstream pin tracking via `VERSION.toml` / `UPSTREAM.md` against TypeScript World of ClaudeCraft **0.31.0** (`a3e5e9596a8e`).
- Deterministic 20 Hz sim with mulberry32 RNG, Eastbrook-like heightfield, Warrior motion.
- Combat slice: wolf camp, auto-attack, Heroic Strike, XP/loot/level-up.
- Bevy offline client (title → character create → in-world) with minimal HUD.
- Thin `woc-server` exposing `/health` and `/version`.
- Parity checklist in `docs/parity/STATUS.md`.
