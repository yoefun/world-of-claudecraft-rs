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
2. Do not add a fat actor struct or grow `Sim.entities: Vec<…>`. Add a component column or a `Sim` resource.
3. Do not use `kind` as a substitute for “has this data”. Query the column.
4. Tick stays sequential and deterministic (insertion order, seeded RNG).
5. Client Bevy components are presentation only.

## Catalog

See the table in [`docs/superpowers/specs/2026-08-13-sim-ecs-design.md`](../superpowers/specs/2026-08-13-sim-ecs-design.md) §4.4 and `crates/woc-sim/src/ecs/components.rs` module docs.

Player columns include `Hearth` (`zone_id`, bind `x`/`z`, `ready_tick`) for innkeeper binding and tick-based hearthstone cooldowns, and `Riding` (`rank`, `known`, `last_id`, `active_id`) for mount training and summon state; see `crates/woc-sim/src/ecs/components.rs`.

## Status

The ECS column program is **done**. `World` is the actor store; the fat `Entity` / `Sim.entities` path is deleted. New per-actor state is a new column only.
