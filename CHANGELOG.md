# Changelog

## 0.1.0 — 2026-07-27

### Added

- Cargo workspace scaffold: `woc-version`, `woc-protocol`, `woc-sim`, `woc-server`, `woc-client`.
- Upstream pin tracking via `VERSION.toml` / `UPSTREAM.md` against TypeScript World of ClaudeCraft **0.31.0** (`a3e5e9596a8e`).
- Deterministic 20 Hz sim with mulberry32 RNG, Eastbrook-like heightfield, Warrior motion.
- Combat slice: wolf camp, auto-attack, Heroic Strike, XP/loot/level-up.
- Bevy offline client (title → character create → in-world) with minimal HUD.
- Thin `woc-server` exposing `/health` and `/version`.
- Parity checklist in `docs/parity/STATUS.md`.
