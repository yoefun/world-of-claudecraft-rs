# Changelog

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
