# World of ClaudeCraft (Rust)

[![CI](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml)
[![Rewrite](https://img.shields.io/badge/rewrite-1.0.0--pre-blue)](VERSION.toml)
[![Upstream](https://img.shields.io/badge/upstream-0.31.0-informational)](UPSTREAM.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Rust rewrite of [World of ClaudeCraft](https://github.com/levy-street/world-of-claudecraft).

**Rewrite `1.0.0-pre`** is pinned to upstream **`0.31.0`**
(`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`). Parity target: **`completion`**.
See [`UPSTREAM.md`](UPSTREAM.md) and [`docs/parity/STATUS.md`](docs/parity/STATUS.md).

## What works in 1.0-pre (completion)

- Native Bevy client embedding a shared deterministic sim (offline + online)
- Create any of **9 classes**, multi-ability kits, talents, hunter/warlock pets
- Walk Eastbrook Vale and travel to Eastfen; dungeon crypt instance
- Talk to NPCs (E), quests, combat, party/chat, Need/Greed loot
- Bank, mail, auction house; herbalism → alchemy craft loop
- Duels, PvP flag, honor; deed stub on world-boss template kill
- Version footer: `WoC-rs 1.0.0-pre · upstream 0.31.0`
- `woc-server` sticky multi-player realm over WebSocket (`/ws/game`)
- Persist auth + character CRUD (memory default; Postgres optional)

## Crates

| Crate | Role |
| --- | --- |
| `woc-version` | Embeds rewrite + upstream pin constants |
| `woc-content` | Data tables (classes, items, mobs, NPCs, quests) |
| `woc-protocol` | Intents, snapshots, events, `WorldHost`, WS msgs |
| `woc-sim` | Deterministic game core (no Bevy) |
| `woc-client` | Bevy offline + online host |
| `woc-server` | HTTP + WebSocket sim host |

## Quick start

```bash
# Play offline framework slice (title: press 1 / Offline)
cargo run -p woc-client

# Run sim / unit tests (no GPU)
cargo test --workspace --exclude woc-client

# Server (health + WS game host)
cargo run -p woc-server
# curl http://127.0.0.1:8787/version
# WS: ws://127.0.0.1:8787/ws/game
```

### Online play (two clients)

```bash
# Terminal A — authoritative realm
cargo run -p woc-server

# Terminal B — Bevy client
cargo run -p woc-client
# Title: 2 Online → Login/Register (F2) → Character Select → Enter world

# Terminal C — optional second client (same Online path) for co-presence
cargo run -p woc-client
```

REST: `http://127.0.0.1:8787/api/{register,login,characters}` (blocking `ureq` on a worker thread).  
Default WS: `ws://127.0.0.1:8787/ws/game` (`ONLINE_WS_URL` in `crates/woc-client/src/online.rs`).
Online IO uses dedicated OS threads + sync `tungstenite` / `ureq` bridged via `std::sync::mpsc`.

Controls: **WASD** move, **mouse** look (hold right), **left click** attack, **1** ability, **E** interact, **B** bags, **L** quests, **Esc** release cursor. Title: **1/2** Offline|Online. Offline create: **←/→** class. Online login: **Tab** field, **F2** login/register, **Enter** submit. Char select: **↑/↓**, **N** create, **Enter** enter world.

## Architecture

One sim, multiple hosts:

- Offline Bevy host runs `woc-sim` in-process at 20 Hz (`GameHost`)
- Online Bevy host sends `Hello` / `Intent` / `Interact` and applies `Snapshot` / `Events`
- Online `woc-server` embeds the same sim over WebSocket
- Client never decides combat outcomes — only sends intents/actions and renders snapshots

## Roadmap

See [`docs/ROADMAP.md`](docs/ROADMAP.md). **Current:** `1.0.0-pre` / `completion`. Remaining polish: dedicated delve loop, denser zone3, Bevy UI for new systems.

## License

MIT — see [`LICENSE`](LICENSE). Upstream project is also MIT.
