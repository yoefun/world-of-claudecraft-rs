# World of ClaudeCraft (Rust)

[![CI](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml)
[![Rewrite](https://img.shields.io/badge/rewrite-0.1.0-blue)](VERSION.toml)
[![Upstream](https://img.shields.io/badge/upstream-0.31.0-informational)](UPSTREAM.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Rust rewrite of [World of ClaudeCraft](https://github.com/levy-street/world-of-claudecraft).

**Rewrite `0.1.0`** is pinned to upstream **`0.31.0`**
(`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`). See [`UPSTREAM.md`](UPSTREAM.md)
and [`docs/parity/STATUS.md`](docs/parity/STATUS.md).

## What works in 0.1 (combat slice)

- Native Bevy client embedding a shared deterministic sim
- Create a **Warrior**, walk an Eastbrook-like open area
- Fight **wolves** with auto-attack + Heroic Strike
- Gain XP, level up, pick up simple loot
- Version footer: `WoC-rs 0.1.0 · upstream 0.31.0`

## Crates

| Crate | Role |
| --- | --- |
| `woc-version` | Embeds rewrite + upstream pin constants |
| `woc-protocol` | Intents, snapshots, events |
| `woc-sim` | Deterministic game core (no Bevy) |
| `woc-client` | Bevy offline host |
| `woc-server` | Thin HTTP health/version scaffold |

## Quick start

```bash
# Play offline combat slice
cargo run -p woc-client

# Run sim / unit tests (no GPU)
cargo test --workspace --exclude woc-client

# Server stub
cargo run -p woc-server
# curl http://127.0.0.1:8787/version
```

Controls in-world: **WASD** move, **mouse** look (hold right button), **left click** target/attack, **1** Heroic Strike, **Esc** release cursor.

## Architecture

One sim, multiple hosts (same idea as upstream):

- Offline Bevy host runs `woc-sim` in-process at 20 Hz
- Online `woc-server` will host the same sim later (stubbed in 0.1)
- Client never decides combat outcomes — only sends intents and renders snapshots

## License

MIT — see [`LICENSE`](LICENSE). Upstream project is also MIT.
