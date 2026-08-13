# Agent instructions

These rules override convenience. They apply to every change in this repository.

## Sim storage is ECS columns

`woc-sim` gameplay actors live in a typed sparse-column `World` (`crates/woc-sim/src/ecs/`). That is the required model.

**Do not:**

- Add a field to `crates/woc-sim/src/entity.rs` `struct Entity` (frozen; being deleted).
- Introduce a new catch-all actor struct (`struct Actor`, `struct Unit`, another `Vec<Entity>`).
- Depend on Bevy / `bevy_ecs` / wgpu / axum / tokio from `woc-sim` or `woc-content`.
- Put HP, inventory, quests, or combat into Bevy components on the client. Those stay in the sim (or on `TickSnapshot` for display).

**Do:**

- New *per-actor* state → new component in `ecs/components.rs` + `SparseSet` field on `World` + `Component` impl + `insert` only on the actor kinds that need it.
- New *per-realm* state → field on `Sim` (like `Mailbox`, `AuctionHouse`, `PartyRoster`).
- New *visual-only* state → Bevy component in `woc-client`.
- Query the columns a system needs. If you branch on `EntityKind` / `Identity.kind` to skip missing data, you wanted a component query instead.
- Keep tick phase order, mulberry32 RNG, and “client never decides combat/loot/quests”.

Until `Entity` is deleted, dual-write: if you must touch live actor state, update the matching column in the same change. See `docs/superpowers/specs/2026-08-13-sim-ecs-design.md` and `docs/superpowers/plans/2026-08-13-sim-ecs.md`.

Human-facing summary: `docs/architecture/ecs.md`.
