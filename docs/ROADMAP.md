# Roadmap

| Rewrite | Parity target | Intent |
| --- | --- | --- |
| **0.1.0** (shipped) | `combat-slice` | Bevy offline Warrior combat: wolves, XP/loot, thin server health |
| **0.2.0** (shipped) | `framework` | Content tables, 9 classes, inventory, quests, vendor, WS host |
| **0.3.0** (shipped) | `online-alive` | SimContext + multi-player Entity; online client; death; combat/motion/bags core |
| **0.4–0.9** (folded into 1.0-pre) | persist → professions-pvp | Landed via R1–R3 parallel batches on `develop` |
| **1.0.0-pre** (current) | `completion` | Talents, loot rules, bank/mail/market, professions, zones, dungeon, PvP, deed stub |

## Completion program

**Definition of done:** [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](superpowers/plans/2026-07-28-rust-rewrite-completion.md)

Upstream pin remains **0.31.0** unless explicitly bumped. Browser/Electron/Web3/RL/admin/i18n are non-goals.

## Internal: sim ECS columns (done)

Gameplay actors in `woc-sim` live in a typed sparse-column `World` (simpler systems, O(1) lookup, sparse loot/NPC). The fat `Vec<Entity>` path is deleted. Parity/protocol unchanged.

**Design:** [`docs/superpowers/specs/2026-08-13-sim-ecs-design.md`](superpowers/specs/2026-08-13-sim-ecs-design.md)  
**Plan (historical):** [`docs/superpowers/plans/2026-08-13-sim-ecs.md`](superpowers/plans/2026-08-13-sim-ecs.md)  
**Rules:** [`docs/architecture/ecs.md`](architecture/ecs.md) · [`AGENTS.md`](../AGENTS.md)

## Parallel execution

Main agent freezes protocol/sim contracts per wave, dispatches subagents on isolated branches with exclusive path ownership, then merges by dependency and runs workspace tests. See plan §“Main-agent merge playbook”.
