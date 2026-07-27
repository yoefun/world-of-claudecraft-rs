# Parity status (rewrite 0.1.0 ↔ upstream 0.31.0)

Legend: `done` · `partial` · `deferred` · `n/a`

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
