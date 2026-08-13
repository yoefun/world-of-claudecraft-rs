# Changelog

## Unreleased

### Added

- Procedural character / creature visual catalog (`woc-sim::visual_catalog`): class-, template-, and role-keyed mesh recipes (players, NPCs, mobs, pets, loot).
- Bevy scene loading: Eastbrook building meshes from physics AABBs, hub beacons, zone-gate portal arches, campfire props; biome-tinted terrain bands + zone sky/ambient on travel.
- Character create: rotating 3D class preview silhouette behind the UI panel.
- In-world scene load for NPCs/mobs/herbs: nameplates, quest/vendor overhead markers, target ground ring, idle bob; gather nodes spawned into the realm with herb visuals.
- Enter-world toast summarizing NPC / foe / herb counts; visual spawn/despawn lifecycle for corpses, loot, and pets.

### Fixed

- Clippy `too_many_arguments` on mail send / market list (nightly).
- Gather nodes are not auto-looted on proximity (Interact / Gather only).

## 1.0.0-pre — 2026-07-28

### Added

- Upstream-aligned continuous strip heightfield (seed `20061`, pin `a3e5e959`): biome bands, hubs, lakes, camps, Sowfield flatten, ridge/rim walls, terrace.
- Golden harness `crates/woc-sim/tests/data/terrain_golden.json` (noise + height/steepness; ε≈1e-3 vs JS).
- `WorldSpatial` content layer + absolute strip coordinates for Eastbrook / Eastfen / Mirefen / Thornpeak.
- Chunked Bevy terrain/water sampling via the same `terrain_height` / `ground_height` functions.
- Talents: class trees (3/class, tiers 1–2), spend/respec; damage/hp/armor/resource effects.
- Party loot rules: FFA + Need/Greed rolls via sim RNG; kill credit share within 40 yd.
- Economy: personal bank deposit/withdraw; mail send/collect; auction list/buy/cancel/expire.
- Durable realm economy (`realm_economy` / in-memory): mail + auction survive restart; offline AH settlement via system mail.
- Online persist loop: authenticated WS Hello (`token` + `character_id`), inject CharacterSave on enter, autosave on disconnect + periodic economy checkpoint.
- Professions sim: train herbalism/alchemy → gather nodes → craft recipes.
- Zone transition Eastbrook ↔ Eastfen ↔ Mirefen ↔ Thornpeak without wiping player progression.
- Thornpeak zone3 NPCs/mobs/quests; deeper talent trees.
- Dungeon instances with unique keys (party-shared); overworld actors preserved.
- Delve loop (`eastbrook_hollow`): 3 rooms → advance → reward.
- Light PvP: duel challenge/accept, open-world PvP flag, honor currency.
- World boss deed (`mire_terror` / eastfen_mire_terror): one-shot honor, persisted `completed_deeds`.
- Consumable HoT linger after rations/salves.
- Protocol rev 3: authenticated Hello fields; absolute inventory/bank `slot` indices.
- Persist R4+ character fields: zone, talents, bank, honor, professions, deeds (backward-compatible JSON).
- Bevy client panels: talents (N), bank (K), mail (M), market (U).

### Changed

- World bounds: `WORLD_MAX_X=180`, `z ∈ [-180, 900]` (replaces square `WORLD_HALF=120` bowl).
- Zone portals teleport on the shared coordinate system without wiping other-zone actors.
- Player/mob/pet motion climb limit `1.5` rise/run + `ground_height` footing.
- Parity target `online-alive` → `completion`.
- Rewrite version `0.3.0` → `1.0.0-pre`.
- Mirefen filled from placeholder to open-world content.
- Online WS snapshots are per-player (no longer broadcast primary-player view to all sockets).

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
