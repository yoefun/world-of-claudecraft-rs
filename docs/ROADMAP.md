# Roadmap

| Rewrite | Parity target | Intent |
| --- | --- | --- |
| **0.1.0** (shipped) | `combat-slice` | Bevy offline Warrior combat: wolves, XP/loot, thin server health |
| **0.2.0** (shipped on `develop`) | `framework` | Content tables, 9 classes, inventory, quests, vendor, WS host |
| **0.3.0** (shipped on branch) | `online-alive` | SimContext + multi-player Entity; online client; death; combat/motion/bags core |
| **0.4.0** (next) | `online-persist` | Postgres `woc-persist`, auth, character CRUD |
| **0.5.0** | `class-depth` | Multi-ability kits, talents, pets |
| **0.6.0** | `open-world` | Zone2/3, graveyards, denser quests |
| **0.7.0** | `group-pve` | Party/chat, loot rules, dungeon (+ delve) |
| **0.8.0** | `economy` | Bank, mail, market |
| **0.9.0** | `professions-pvp` | Gather/craft, duel/honor |
| **1.0.0-pre** | `completion` | Light world boss/deeds + STATUS core rows green |

## Completion program

**Definition of done:** [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](superpowers/plans/2026-07-28-rust-rewrite-completion.md)

Upstream pin remains **0.31.0** unless explicitly bumped. Browser/Electron/Web3/RL/admin/i18n are non-goals.

## Parallel execution

Main agent freezes protocol/sim contracts per wave, dispatches subagents on isolated branches with exclusive path ownership, then merges by dependency and runs workspace tests. See plan §“Main-agent merge playbook”.
