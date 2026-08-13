# Sim ECS (required actor model)

Authoritative game state in `woc-sim` is a **typed sparse-column World**, not a fat `Entity` bag and not Bevy’s ECS.

```
woc-sim World  →  TickSnapshot  →  Bevy presentation ECS
(columns)         (protocol)       (meshes, UI, camera)
```

## Why

- **Simpler:** a loot pile is `Identity + Transform + LootPile`. A player is those plus bags, kit, motion, … Systems name the columns they touch.
- **Faster at this scale:** O(1) id lookup; combat iterates combatant columns; NPCs/loot do not allocate backpack/bank.
- **Stable later:** new state is a new column or a `Sim` resource. God-objects are forbidden.

## Rules

1. `woc-sim` must not depend on Bevy.
2. Do not add fields to `Entity`. Add a component or a `Sim` resource.
3. Do not use `kind` as a substitute for “has this data”. Query the column.
4. Tick stays sequential and deterministic (insertion order, seeded RNG).
5. Client Bevy components are presentation only.

## Catalog

See the table in [`docs/superpowers/specs/2026-08-13-sim-ecs-design.md`](../superpowers/specs/2026-08-13-sim-ecs-design.md) §4.4.

## Migration

Implementation tasks: [`docs/superpowers/plans/2026-08-13-sim-ecs.md`](../superpowers/plans/2026-08-13-sim-ecs.md).

Until Task 12 lands, `Sim.entities` still exists. Dual-write any live-state change into columns. The `Entity` stack-size ceiling test must not go up.
