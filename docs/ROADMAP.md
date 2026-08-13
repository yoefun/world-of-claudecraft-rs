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
| **1.2.0** (this branch) | `content-depth` | Mining/smith, dungeon trash, second instance, ability-mod talents |
| **1.3.0** (shipped) | `online-hard` | Reconnect park/resume, snapshot AOI, Postgres production notes |
| **1.4.0** (shipped) | `client-compat` | Online version gate (title preflight + Hello identity) |
| **1.5.0** (shipped) | `client-update` | Signed full + bsdiff delta packages; `woc-updater` launcher (Linux x86_64) |
| **1.6.0** (shipped) | `class-engine` | Combo, stealth, absorb, interrupt lockout, Charge/Blink/Life Tap, hunter mana |
| **1.7.0** (shipped) | `class-identity` | Rogue stealth+combo, priest shield, warrior Charge, mage Blink, hunter Aspect |
| **1.8.0** (this branch) | `class-forms` | Warrior stances, shaman/druid forms, paladin aura/seal, warlock Fear |
| **1.9.0** (planned) | `gear-depth` | Class/armor equip rules, jewelry, stamina/spell power, gear drops |

## Completion program (closed)

**Definition of done:** [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](superpowers/plans/2026-07-28-rust-rewrite-completion.md)

Gameplay-core rewrite against upstream **0.31.0** is **shipped** as `1.0.0-pre`. Remaining work is contract-close and depth, not a second port.

## Post-completion program (current)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-post-completion-program-design.md`](superpowers/specs/2026-08-13-post-completion-program-design.md)  
**Implementation + parallel dispatch:** [`docs/superpowers/plans/2026-08-13-post-completion-program.md`](superpowers/plans/2026-08-13-post-completion-program.md)  
**Max-parallel schedule:** [`docs/superpowers/plans/2026-08-13-parallel-post-completion.md`](superpowers/plans/2026-08-13-parallel-post-completion.md)

Upstream pin remains **0.31.0** unless explicitly bumped. Browser/Electron/Web3/RL/admin/i18n stay non-goals. New per-actor gameplay state must be a `World` component column (`AGENTS.md`); do not reintroduce a fat `Entity`.

## Gear depth (planned)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-gear-depth-design.md`](superpowers/specs/2026-08-13-gear-depth-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-gear-depth.md`](superpowers/plans/2026-08-13-gear-depth.md)

Equipment stays on `Bags`. `can_equip` is the single class/armor/level gate. Two-hand and ranged weapons occupy the off-hand. Neck + one Finger. Stamina and spell power are sim-authoritative. Durability/repair and crafted quality stay out of this program.

## Client version gate (current)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-client-version-update-design.md`](superpowers/specs/2026-08-13-client-version-update-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-client-version-update.md`](superpowers/plans/2026-08-13-client-version-update.md)

Online Bevy clients must not enter a realm with a mismatched rewrite version or `protocol_rev`. Hello identity fields stay additive; class-identity snapshot is protocol rev **7**. Packaged incremental updates shipped in **1.5.0**.

## Client update packages (current)

**Definition of done:** [`docs/superpowers/specs/2026-08-13-client-update-packages-design.md`](superpowers/specs/2026-08-13-client-update-packages-design.md)  
**Implementation:** [`docs/superpowers/plans/2026-08-13-client-update-packages.md`](superpowers/plans/2026-08-13-client-update-packages.md)  
**Runbook:** [`docs/client-update.md`](client-update.md)

Players start `woc-updater`. CI on version tags packs a zstd full archive plus a per-file bsdiff from the previous GitHub Release. Skip-version downloads full. Windows/macOS and Velopack/Electron stay out of scope.

## Internal: sim ECS columns (done)

Gameplay actors in `woc-sim` live in a typed sparse-column `World` (simpler systems, O(1) lookup, sparse loot/NPC). The fat `Vec<Entity>` path is deleted. Parity/protocol unchanged.

**Design:** [`docs/superpowers/specs/2026-08-13-sim-ecs-design.md`](superpowers/specs/2026-08-13-sim-ecs-design.md)  
**Plan (historical):** [`docs/superpowers/plans/2026-08-13-sim-ecs.md`](superpowers/plans/2026-08-13-sim-ecs.md)  
**Rules:** [`docs/architecture/ecs.md`](architecture/ecs.md) · [`AGENTS.md`](../AGENTS.md)

## Parallel execution

Main agent freezes protocol/sim contracts per wave, dispatches subagents on isolated branches with exclusive path ownership, then merges by dependency and runs workspace tests. See the active plan’s “Main-agent merge playbook”.
