# Sim ECS — typed sparse columns

**Status:** Approved for implementation planning (2026-08-13).  
**Rewrite target:** stay on `1.0.0-pre` / parity `completion`. This is an internal architecture change, not a content/protocol bump.  
**Upstream pin:** unchanged (`0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).

## 1. Goal

Replace the fat `Entity` bag in `woc-sim` with a **typed sparse-column World** so that:

1. **The framework is simpler** — a new actor type is “spawn id + insert the components it actually has”, not another `Option` on a 90-field struct. Systems declare the columns they touch.
2. **Hot paths are cheaper** — O(1) id lookup; combat/AI iterate only combatant columns; loot/NPC/pets do not allocate backpack, bank, talents, or threat tables.
3. **Later work cannot slide back** — new gameplay state is a new component column. Adding a field to a god-object is a hard fail (docs + size ceiling test + agent instructions).

This is **not** “put the sim in Bevy ECS”. The client already uses Bevy for presentation. The sim stays host-agnostic and deterministic.

## 2. Current problem (why this exists)

`Sim.entities: Vec<Entity>` is a homogeneous AoS list. Every mob, NPC, loot pile, and pet carries player-only heap (`inventory` 16 slots, `bank` 24 slots, `talents`, `professions`, …). Leaf modules look like systems (`update_mob_ai`, `tick_auras`) but they:

- scan the whole vec and branch on `EntityKind`;
- re-`find` by id inside per-entity loops (dozens of O(n) lookups per tick);
- cannot express “this function only needs Transform + Health”.

Bevy on the client is a visual projection of `TickSnapshot`. That split is correct and stays.

## 3. Approaches considered

| Approach | Verdict |
| --- | --- |
| **A. `bevy_ecs` inside `woc-sim`** | Reject. Violates “one sim, no Bevy/wgpu”. Change detection + parallel schedules fight the locked 20 Hz deterministic tick. Online server would pull Bevy. |
| **B. `hecs` / `shipyard` / `specs`** | Reject for v1. Extra sim dependency; iteration order and `Query` borrows are easy to make non-deterministic or borrow-checker hostile. The current “collect ids, then mutate” pattern would fight the library. Revisit only if the typed World grows a generic `Any` registry. |
| **C. Nested `Option` blobs still on `Vec<Entity>`** | Reject as the end state. Removes some heap from loot, but keeps O(n) lookup, kind-branching, and the “just add a field” habit. Allowed only as a *micro-step* inside a column migration, not as the architecture. |
| **D. Typed sparse-column `World` in `woc-sim` (no crate)** | **Accept.** Explicit catalog, dense iteration, O(1) lookup, full control of order, zero new deps, matches “simplify the framework”. |

## 4. Architecture

### 4.1 Two ECS worlds, one boundary

```
woc-sim World          protocol           woc-client Bevy World
(typed columns)   →   TickSnapshot   →   SimVisual / meshes / UI
authoritative         (unchanged)        presentation only
```

- Gameplay state lives only in `woc-sim` columns.
- Wire types (`EntitySnapshot`, `TickSnapshot`, `WorldHost`) do **not** become Bevy components.
- Client systems keep reading `GameHost.snapshot`. Do not mirror HP/inventory into Bevy components unless a presentation system truly needs a local cache.

### 4.2 `World` (sim)

`World` owns:

- live `EntityId`s (monotonic `u32`, **never reused** — same as today);
- one `SparseSet<C>` per component type;
- `next_id`.

`Sim` becomes:

```text
Sim { tick, seed, rng, world, player_id, events, intents, parties, mail, market, loot_rules, pvp }
```

`Sim.entities: Vec<Entity>` is deleted at the end of the migration. During the migration it may exist as a compatibility shim (see §6).

`SimContext` becomes a borrow of `World` + `events` + `rng` (no separate `next_id`; it lives on `World`).

### 4.3 Sparse set (required properties)

```text
SparseSet<T> {
  sparse: HashMap<EntityId, usize>,  // O(1) lookup
  dense_ids: Vec<EntityId>,          // insertion order
  dense: Vec<T>,                     // parallel to dense_ids
}
```

Invariants:

- **Insertion order** of `dense_ids` matches today’s `Vec<Entity>` scan order for that component (spawn order). Do not sort by id on insert; determinism tests assume spawn order.
- `insert` overwrites in place if the id already has `T`.
- `remove` swap-removes and patches `sparse`.
- Iteration is `dense_ids` zip `dense` (cache-friendly for that column).
- Join query: iterate the **smaller** participating column, `sparse.get` the others; skip if any column misses.

No generational indices in v1 (ids are never reused). No archetypes in v1 (joins are fine at Eastbrook/multi-zone scale: hundreds of entities, not tens of thousands).

### 4.4 Component catalog (locked for the migration)

Group today’s `Entity` fields. **Do not** invent extra gameplay components in this program.

| Component | Fields (from `Entity`) | Who has it |
| --- | --- | --- |
| `Identity` | `kind`, `name`, `template_id`, `zone_id` | all |
| `Transform` | `x`, `y`, `z`, `yaw` | all |
| `Health` | `hp`, `hp_max`, `alive`, `level` | player, mob, npc, pet (not loot) |
| `Combat` | `attack_damage`, `armor`, `swing_timer`, `ability_cd`, `auto_attack`, `target`, `gcd`, `cast` | player, mob, pet |
| `Auras` | `auras` | player, mob, pet |
| `Home` | `home_x`, `home_z` | mob (leash / respawn) |
| `Threat` | `threat` | mob |
| `LootTable` | `loot_copper`, `loot_item`, `xp_value` | mob (death payout) |
| `Respawn` | `respawn_timer` | mob |
| `LootPile` | `copper`, `item` | loot entity |
| `Owner` | `owner_id` | pet |
| `ClassKit` | `class_id`, `resource`, `resource_max`, `resource_type`, `primary_ability`, `known_abilities`, `ability_cds` | player |
| `Bags` | `inventory`, `equipment`, `open_vendor_npc` | player |
| `QuestLog` | `quest_log` | player |
| `Progress` | `xp`, `copper`, `talent_points`, `talents`, `honor`, `pvp_flagged`, `professions`, `completed_deeds` | player |
| `Bank` | `bank` | player |
| `Motion` | `vx`, `vz`, `vy`, `on_ground`, `jumping`, `fall_start_y`, `flying` | player (pets/mobs keep ground snaps via `Transform` only unless a later task proves they need `Motion`) |
| `Spirit` | `corpse_x`, `corpse_z` | player |
| `InstanceAt` | `instance_id`, `delve_room` | player |
| `Durable` | `durable_id` | player (online) |

`EntityKind` stays on `Identity` for snapshots and visuals. Gameplay systems **must not** use `kind` as a substitute for “has this column”. Example: loot pickup queries `LootPile`, not `kind == Loot`.

Realm-global state stays **off** entities (unchanged): `intents`, `parties`, `mail`, `market`, `loot_rules`, `pvp`.

### 4.5 System style

Keep the locked tick phases (`TICK_PHASES` fingerprint must stay `1724209595281213949` unless a later change is explicitly a phase reorder).

Replace `&mut [Entity]` with `&mut World`. The allowed mutation pattern (borrow-checker + determinism) remains:

1. Collect `EntityId`s from a column (or a join).
2. For each id, `get` / `get_mut` the needed columns.

Do **not** introduce parallel `SystemParam` schedules. Sequential phases are a feature.

Helper queries (names are normative for the plan):

```rust
world.get::<Transform>(id) -> Option<&Transform>
world.get_mut::<Health>(id) -> Option<&mut Health>
world.ids::<LootPile>() -> impl Iterator<Item = EntityId>  // insertion order
```

`Component` is a trait implemented per column (associated `column` / `column_mut` on `World`). No `TypeId` map.

### 4.6 Factories

`create_player` / `create_mob_from_template` / `create_npc_from_template` / `create_loot` / pet summon become `World` methods (or free functions taking `&mut World`) that `spawn()` then `insert` only the rows in §4.4.

`Entity::blank` is deleted with the fat struct.

## 5. Performance budget (what “optimized” means here)

This program is **not** a renderer or SIMD pass. Success is:

| Metric | Today | After |
| --- | --- | --- |
| Lookup by id | O(n) scan of fat rows | O(1) `sparse` |
| Combat/AI scan | every entity, ~90 fields + heap headers | `Health` ⋈ `Transform` ⋈ `Combat` dense columns |
| Loot/NPC heap | backpack + bank allocated | those columns absent |
| Tick parallelism | none | still none (deterministic) |
| Protocol / snapshot size | unchanged | unchanged |

Optional follow-on (out of scope unless a test shows it matters): spatial hash for `nearest_mob` / aggro. Do not build it in the first World.

No micro-benchmark gate in CI. Guard with:

- existing determinism / gameplay tests green;
- a unit test that a loot entity has no `Bags` / `Bank` / `ClassKit`;
- a unit test that `world.get::<Identity>(id)` is O(1) relative to world size (e.g. lookup 10k dummy ids still cheap — optional, keep small).

## 6. Migration strategy (behavior-preserving)

Big-bang rewrite of every `&mut [Entity]` call is rejected. Phases:

0. **Freeze** the fat `Entity` (size ceiling test + `AGENTS.md`). New gameplay state cannot be a new `Entity` field.
1. **`SparseSet` + `World` + `Component` trait** beside `Vec<Entity>`. Not wired to `Sim` yet.
2. **Id index on `Sim`** (`HashMap<EntityId, usize>` into the existing vec) so lookups become O(1) immediately. This is a valid intermediate; it does not replace columns.
3. **Dual-write**: factories insert columns *and* still fill `Entity`. Systems still read `Entity`. Snapshot still from `Entity`.
4. **Cut systems over** one module at a time (combat → motion → mob/pet → interaction/quests → social/economy → snapshot builder). Each cut: tests in that module pass without reading the fat struct for that concern.
5. **Delete `Entity`**, `Sim.entities`, and the index map. Snapshot builder reads columns.
6. **Ratchet** the size ceiling to `0` (struct gone) and keep the “no god object” tests.

Protocol rev stays at 5. Snapshot field set stays. `WorldHost` signatures stay.

## 7. Follow-on principle (later iterations)

Hard rules, also copied into `AGENTS.md` and `docs/architecture/ecs.md`:

1. `woc-sim` MUST NOT depend on Bevy, `bevy_ecs`, wgpu, axum, or tokio.
2. Gameplay state is a **component column** or **realm resource** on `Sim` (mail, market, parties). Never a new field on a catch-all actor struct.
3. A system may query only the columns it needs. If a function takes `Identity` solely to branch on `kind`, that is a bug — add or use the missing component.
4. Client Bevy components are presentation (`SimVisual`, UI markers). Do not duplicate sim columns there.
5. Tick order, seeded RNG, and “client never decides combat/loot/quests” are unchanged.
6. Prefer inserting/removing a component at runtime (buff as `Auras`, corpse as `Spirit`) over boolean flags on an unrelated column.

When a later feature needs state:

- **Per-actor, sparse** → new component file + `SparseSet` field + `Component` impl + factory `insert`.
- **Per-realm, unique** → field on `Sim` (like `Mailbox`).
- **Visual only** → Bevy component in `woc-client`.

## 8. Testing

- Keep `tick_phase_fingerprint() == 1724209595281213949`.
- Keep `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client`.
- Add column-presence tests (loot has `LootPile` + `Transform` + `Identity` only among the catalog).
- Add `SparseSet` unit tests: insert, overwrite, remove, iterate insertion order, join skip missing.
- Golden terrain tests are unrelated and must stay green (no `World` in heightfield).

## 9. Non-goals

- Unifying sim `World` with Bevy’s `World`.
- Parallel system schedules / job graphs.
- Archetype ECS, generational ids, change detection, scenes, networking inside ECS.
- Protocol or HUD redesign.
- Spatial partitioning (unless a later measured tick cost demands it).
- Byte-identical combat vs TypeScript (already n/a).

## 10. Risks

| Risk | Mitigation |
| --- | --- |
| Borrow checker pushes “collect ids” everywhere | That is the supported pattern; document it. |
| Dual-write drift | Phase 4 module cuts must drop reads of the fat field in the same PR as the column write. |
| Determinism (iteration order) | Preserve spawn/insertion order; fingerprint test stays. |
| Huge mechanical PR | Phases 1–5 are separately mergeable; each ends with workspace tests. |
| Agents add `Entity` fields during the freeze | Size ceiling test + `AGENTS.md` + review checklist in the plan. |
