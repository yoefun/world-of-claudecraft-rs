# World of ClaudeCraft (Rust)

[![CI](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/yoefun/world-of-claudecraft-rs/actions/workflows/ci.yml)
[![Rewrite](https://img.shields.io/badge/rewrite-1.17.0-blue)](VERSION.toml)
[![Upstream](https://img.shields.io/badge/upstream-0.31.0-informational)](UPSTREAM.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Rust rewrite of [World of ClaudeCraft](https://github.com/levy-street/world-of-claudecraft).

**Rewrite `1.17.0`** is pinned to upstream **`0.31.0`**
(`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`). Parity target: **`parcel-bank`**.
See [`UPSTREAM.md`](UPSTREAM.md) and [`docs/parity/STATUS.md`](docs/parity/STATUS.md).
Packaged updates: [`docs/client-update.md`](docs/client-update.md).

## What works in 1.17.0 (parcel-bank)

- Native Bevy client embedding a shared deterministic sim (offline + online)
- Create any of **9 classes**, multi-ability kits, talents, hunter/warlock pets
- Walk Eastbrook Vale, Eastfen, Mirefen, and Thornpeak; Eastbrook Crypt + Mirefen Barrow + 3-room delve
- Talk to NPCs (E), quests (abandon **L+X**, share **L+Y**, daily/explore/escort, choice rewards **1/2/3**), vendor buyback, repair, trainers, hearth, combat, party/chat, Need/Greed loot (1/2/3 rolls; [ ] loot mode)
- Hub reputation (Watch / Circle / Ferry / Highwatch); Friendly vendor discounts and gated Watch Signet; **C** sheet lists standing
- Bank (Banker Holme; instance-preserving deposit/withdraw + copper vault; **G** any non-quest stack), mail (Eastbrook Post; offline send **S**/**Y**, numbered collect **1–9**, return **X**), auction house (Auctioneer Lise; bids; 12/24/48 h; soulbound; instance listings; 5% cut; mail proceeds); herbalism → alchemy; mining → blacksmithing
- Duels, PvP flag, honor; Mire Terror deed (one-shot)
- Client panels: talents / bank / mail / market / bags (equip·use·sell); **character sheet** (C: extra slots, quality, MH/OH enchant, AP/armor/SP); **minimap** + **world map** (M)
- Class armor caps, two-hand occupancy, warrior/rogue/hunter dual-wield, two rings, two trinkets, catalog + instance loot quality, MH/OH oils; stamina → HP, spell power on heals/spells; independent loot piles
- Procedural class/creature silhouettes + scene props (buildings, portals, zone sky)
- Entity walk presentation (locomotion hysteresis + limb gait) and soft visual remove / corpse tip
- Jump (Space), lake swim, travel flight (V; Space/Ctrl vertical)
- Version footer: `WoC-rs 1.17.0 · upstream 0.31.0`
- `woc-server` sticky multi-player realm over WebSocket (`/ws/game`) with authenticated Hello; disconnect parks the player for resume
- Persist auth + character CRUD including talents/bank/honor/zone/deeds (memory default; `DATABASE_URL` Postgres is production)

## Crates

| Crate | Role |
| --- | --- |
| `woc-version` | Embeds rewrite + upstream pin constants |
| `woc-content` | Data tables (classes, items, mobs, NPCs, quests) |
| `woc-protocol` | Intents, snapshots, events, `WorldHost`, WS msgs |
| `woc-sim` | Deterministic game core (no Bevy); live professions ECS |
| `woc-manufacturing` | Typed professions oracle (not on the sim tick loop) |
| `woc-client` | Bevy offline + online host |
| `woc-server` | HTTP + WebSocket sim host |
| `woc-update` | Pack, delta, and apply signed client updates ([runbook](docs/client-update.md)) |

## Quick start

```bash
# Play offline framework slice (title: press 1 / Offline)
cargo run -p woc-client

# Run sim / unit tests (no GPU)
cargo test --workspace --exclude woc-client

# Server (health + WS game host)
cargo run -p woc-server
# curl http://127.0.0.1:8787/version
# GET /version includes protocol_rev and min_client_version.
# Online title probes this before Login (fail-closed).
# WOC_MIN_CLIENT_VERSION=1.4.0 overrides the floor (default: rewrite version).
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
`DELETE /api/characters/{id}` removes a character (confirm in the client UI).  
Default WS: `ws://127.0.0.1:8787/ws/game` (`ONLINE_WS_URL` in `crates/woc-client/src/online.rs`).
Online IO uses dedicated OS threads + sync `tungstenite` / `ureq` bridged via `std::sync::mpsc`.

### Persistence (production vs dev)

`DATABASE_URL` is the durable realm path. When it is set, `woc-server` / `woc-persist` use **Postgres** for accounts, character saves, mail, and the auction house (migrations in `crates/woc-persist/migrations/`). When it is unset, the in-memory store remains the **zero-config dev default**.

```bash
DATABASE_URL=postgres://woc:woc@127.0.0.1:5432/woc cargo run -p woc-server
```

Controls: **WASD** move, **Space** jump / swim hop / fly up, **Ctrl** descend (swim/fly), **V** travel flight toggle, **mouse** look (hold right), **left click** / **F** attack, **Tab** cycle target, **1–5** abilities (or Need/Greed/Pass while rolling), **T** pet summon/dismiss (hunter/warlock), **Esc** clear target / stop attack / release cursor, **E** interact/loot, **B** bags (**Q** equip / **F** use / **V** sell junk at vendor), **L** quests, **C** character, **N** talents (**1–3** spend / Y first available / R respec), **K** bank (**G** deposit / **H**/**1–9** withdraw / **J**/**Y** copper vault), **I** mail (**S**/**Y** send / **P**/**1–9** collect / **X** return / Enter compose), **M** world map, **U** market (**L** list / **O** buy / **X** cancel), **[**/**]** party loot mode. Title: **1/2** or click Offline|Online, **Enter**/Continue. Offline create: **click** or **←/→** class grid, type name, **Enter**. Online login: **Tab** field, **F2**/tabs login|register, **Enter**/Sign in (register asks for password confirm). Char select: click roster or **↑/↓**, **Enter world**, **N**/New character (class grid), **D**/Delete (confirm twice), **Esc** logout.

## Architecture

One sim, multiple hosts:

- Offline Bevy host runs `woc-sim` in-process at 20 Hz (`GameHost`)
- Online Bevy host sends `Hello` / `Intent` / `Interact` and applies `Snapshot` / `Events`
- Online `woc-server` embeds the same sim over WebSocket
- Client never decides combat outcomes — only sends intents/actions and renders snapshots

## Roadmap

See [`docs/ROADMAP.md`](docs/ROADMAP.md). **Shipped:** `1.17.0` / `parcel-bank`. Manual demo: [`docs/parity/DEMO.md`](docs/parity/DEMO.md).
Online play persists characters (enter injects save; disconnect autosaves) and realm mail/auction. Post-completion program: [`docs/superpowers/specs/2026-08-13-post-completion-program-design.md`](docs/superpowers/specs/2026-08-13-post-completion-program-design.md).

## License

MIT — see [`LICENSE`](LICENSE). Upstream project is also MIT.


## Manufacturing

Live path is `woc-sim` professions (ECS): gathering, blacksmithing, skinning, leatherworking, tailoring, jewelcrafting, enchanting, engineering, and alchemy. Content lives in `woc-content`; denials are `ProfessionDeny` ids.

`woc-manufacturing` remains the typed oracle crate and is not wired into `Sim::tick_all`.

- Design: `docs/superpowers/specs/2026-08-13-manufacturing-system-design.md`
- ECS wiring: `docs/superpowers/specs/2026-08-13-manufacturing-ecs-design.md`

```sh
cargo test -p woc-sim --lib professions
cargo test -p woc-manufacturing
```
