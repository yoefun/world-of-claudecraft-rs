# Roadmap

| Rewrite | Parity target | Intent |
| --- | --- | --- |
| **0.1.0** (shipped) | `combat-slice` | Bevy offline Warrior combat: wolves, XP/loot, thin server health |
| **0.2.0** (shipped) | `framework` | Content tables, 9 classes, inventory, quests, vendor, WS host |
| **0.3.0** (shipped) | `online-alive` | SimContext + multi-player Entity; online client; death; combat/motion/bags core |
| **0.4–0.9** (folded into 1.0-pre) | persist → professions-pvp | Landed via R1–R3 parallel batches on `develop` |
| **1.0.0-pre** (shipped) | `completion` | Talents, loot rules, bank/mail/market, professions, zones, dungeon, PvP, deeds |
| **1.0.0** (this branch) | `stable` | Tick-phase contract, CI on `develop`, docs/demo hygiene |
| **1.1.0** (this branch) | `combat-depth` | Data-driven ability effects: heal, AoE, miss/crit, interrupt, taunt |
| **1.2.0** | `content-depth` | Mining/smith, dungeon trash, second instance, ability-mod talents |
| **1.3.0** | `online-hard` | Reconnect park/resume, snapshot AOI, Postgres production notes |

## Completion program (closed)

**Definition of done:** [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](superpowers/plans/2026-07-28-rust-rewrite-completion.md)

Gameplay-core rewrite against upstream **0.31.0** is **shipped** as `1.0.0-pre`. Remaining work is contract-close and depth, not a second port.

## Post-completion program (current)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-post-completion-program-design.md`](superpowers/specs/2026-08-13-post-completion-program-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-08-13-post-completion-program.md`](superpowers/plans/2026-08-13-post-completion-program.md)  
**Max-parallel schedule:** [`docs/superpowers/plans/2026-08-13-parallel-post-completion.md`](superpowers/plans/2026-08-13-parallel-post-completion.md)

Upstream pin remains **0.31.0** unless explicitly bumped. Browser/Electron/Web3/RL/admin/i18n stay non-goals. New per-actor gameplay state must be a `World` component column (`AGENTS.md`); do not reintroduce a fat `Entity`.

## Internal: sim ECS columns (done)

Gameplay actors in `woc-sim` live in a typed sparse-column `World` (simpler systems, O(1) lookup, sparse loot/NPC). The fat `Vec<Entity>` path is deleted. Parity/protocol unchanged.

**Design:** [`docs/superpowers/specs/2026-08-13-sim-ecs-design.md`](superpowers/specs/2026-08-13-sim-ecs-design.md)  
**Plan (historical):** [`docs/superpowers/plans/2026-08-13-sim-ecs.md`](superpowers/plans/2026-08-13-sim-ecs.md)  
**Rules:** [`docs/architecture/ecs.md`](architecture/ecs.md) · [`AGENTS.md`](../AGENTS.md)

## Parallel execution

Main agent freezes protocol/sim contracts per wave, dispatches subagents on isolated branches with exclusive path ownership, then merges by dependency and runs workspace tests. See the active plan’s “Main-agent merge playbook”.
