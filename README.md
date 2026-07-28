# World of ClaudeCraft (Rust)

[![CI](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml)
[![Rewrite](https://img.shields.io/badge/rewrite-0.2.0-blue)](VERSION.toml)
[![Upstream](https://img.shields.io/badge/upstream-0.31.0-informational)](UPSTREAM.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Rust rewrite of [World of ClaudeCraft](https://github.com/levy-street/world-of-claudecraft).

**Rewrite `0.2.0`** is pinned to upstream **`0.31.0`**
(`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`). Parity target: **`framework`**.
See [`UPSTREAM.md`](UPSTREAM.md) and [`docs/parity/STATUS.md`](docs/parity/STATUS.md).

## What works in 0.2 (framework)

- Native Bevy client embedding a shared deterministic sim
- Create any of **9 classes**, walk Eastbrook Vale
- Talk to NPCs (E), accept/turn in quests, fight wolves & boars
- Backpack inventory, equipment, vendor buy, XP/loot/level-up
- Version footer: `WoC-rs 0.2.0 · upstream 0.31.0`
- `woc-server` hosts the same sim over WebSocket (`/ws/game`)

## Crates

| Crate | Role |
| --- | --- |
| `woc-version` | Embeds rewrite + upstream pin constants |
| `woc-content` | Data tables (classes, items, mobs, NPCs, quests) |
| `woc-protocol` | Intents, snapshots, events, `WorldHost`, WS msgs |
| `woc-sim` | Deterministic game core (no Bevy) |
| `woc-client` | Bevy offline host |
| `woc-server` | HTTP + WebSocket sim host |

## Quick start

```bash
# Play offline framework slice
cargo run -p woc-client

# Run sim / unit tests (no GPU)
cargo test --workspace --exclude woc-client

# Server (health + WS game host)
cargo run -p woc-server
# curl http://127.0.0.1:8787/version
# WS: ws://127.0.0.1:8787/ws/game
```

Controls: **WASD** move, **mouse** look (hold right), **left click** attack, **1** ability, **E** interact, **B** bags, **L** quests, **Esc** release cursor. Character create: **←/→** change class.

## Architecture

One sim, multiple hosts:

- Offline Bevy host runs `woc-sim` in-process at 20 Hz
- Online `woc-server` embeds the same sim over WebSocket
- Client never decides combat outcomes — only sends intents/actions and renders snapshots

## Roadmap

See [`docs/ROADMAP.md`](docs/ROADMAP.md). **Next:** completion program — online, persist, class depth, zones, group PvE, economy, professions, light PvP (`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`).

## License

MIT — see [`LICENSE`](LICENSE). Upstream project is also MIT.
