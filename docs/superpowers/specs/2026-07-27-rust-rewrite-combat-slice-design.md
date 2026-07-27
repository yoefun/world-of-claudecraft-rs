# Rust rewrite combat-slice design

**Status:** Approved for implementation (plan locked 2026-07-27).  
**Rewrite:** `0.1.0`  
**Upstream pin:** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`)

## Goal

Ship a Cargo workspace that tracks upstream version explicitly and delivers an offline Bevy combat slice: Warrior creation, Eastbrook-like terrain, wolf combat, XP/loot/level-up.

## Architecture

- `woc-sim`: pure deterministic 20 Hz core
- `woc-protocol`: intents / snapshots / events
- `woc-client`: Bevy offline host embedding sim
- `woc-server`: HTTP `/health` + `/version` scaffold
- `woc-version`: pin constants synced with `VERSION.toml`

## Invariants

- No Bevy/wgpu/net deps inside `woc-sim`
- All randomness via seeded mulberry32
- Client sends intents only; sim decides outcomes

## Out of scope (0.1)

Quests, multiplayer play, full content, byte-identical terrain/combat parity, i18n, Web3, RL, Electron.
