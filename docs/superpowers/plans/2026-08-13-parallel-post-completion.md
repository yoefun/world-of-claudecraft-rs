# Post-completion — max-parallel dispatch schedule

**Base tip:** rewrite `1.0.0-pre` / `completion` on `develop`.  
**Parent plan:** [`2026-08-13-post-completion-program.md`](2026-08-13-post-completion-program.md)  
**Design:** [`docs/superpowers/specs/2026-08-13-post-completion-program-design.md`](../specs/2026-08-13-post-completion-program-design.md)

## Principle

Maximize concurrent subagents by **exclusive path ownership**. Serial choke points:

1. `crates/woc-protocol/src/lib.rs` — one PROTO agent per freeze
2. `crates/woc-sim/src/sim.rs` tick wiring and `TICK_PHASES` — **main agent / CORE only**
3. Version bump / CHANGELOG / STATUS — main agent at wave gate
4. `entity.rs` field adds — CORE in the same batch; never two agents

ECS columns are **required** (`AGENTS.md`). Do not dispatch a competing actor-store workstream. New per-actor state = new component column.

## Dependency DAG

```text
[1.0-pre tip]
    │
    ├─► tick-phases (CORE) ─► docs/CI/copy ∥ demo.md ─► version bump ── 1.0.0 stable
    │
    └─► [after 1.0.0]
            PROTO? (hit-result snapshot, optional)
            ├─► ability-effects content ─► miss/crit ─► heal/aoe/interrupt/taunt ── 1.1.0
            │                                              │
            │                                              ├─► mining/smith ∥ crypt trash ∥ 2nd instance
            │                                              └─► talent ability_mod ────────── 1.2.0
            └─► park/resume ∥ AOI (after 1.1 targeting is stable) ─► persist docs ── 1.3.0
```

## Batch S0 — `1.0.0` (now)

| # | Workstream | Branch | Exclusive paths | Notes |
| --- | --- | --- | --- | --- |
| 1 | `ws-tick-phases` | `cursor/ws-tick-phases-9630` | `woc-sim/src/context.rs`, `sim.rs` docs+test only | **First; no siblings in sim.rs** |
| 2 | `ws-ci-develop` | `cursor/ws-ci-develop-9630` | `.github/workflows/ci.yml` | after or ∥ 1 |
| 3 | `ws-copy-hygiene` | `cursor/ws-copy-hygiene-9630` | protocol comments, crate docs, `Cargo.toml` description | ∥ 2 |
| 4 | `ws-demo-doc` | `cursor/ws-demo-doc-9630` | `docs/parity/DEMO.md`, README link | ∥ 2 |
| 5 | `ws-release-100` | integration | VERSION, CHANGELOG, STATUS, ROADMAP, README badges | **last** |

**Merge order S0:** tick-phases → (ci ∥ copy ∥ demo) → release bump.

### S0 DoD

- [ ] Nine named tick phases; fingerprint test matches `tick_all`
- [ ] CI on `develop`
- [ ] No “stub” comments on shipped interact actions
- [ ] `docs/parity/DEMO.md` exists
- [ ] `rewrite_version = "1.0.0"`, `parity_target = "stable"`

## Batch S1 — `1.1.0` (after 1.0.0)

| # | Workstream | Depends | Exclusive paths |
| --- | --- | --- | --- |
| 1 | `ws-ability-effects` | S0 | `woc-content/src/abilities.rs`, new `ability_effects.rs` |
| 2 | `ws-hit-table` | S0 | `woc-sim/src/types.rs` + combat hit-roll helpers (coordinate with 3) |
| 3 | `ws-combat-effects` | 1, 2 | `woc-sim/src/combat.rs` |
| 4 | `ws-combat-hud` | 3 optional PROTO | `woc-client/src/hud.rs` miss/crit/heal toasts only |

If 2 and 3 would both edit `combat.rs`, **merge 2 into 3** and dispatch one COMBAT agent.

### S1 DoD

- [ ] No ability-id DoT match arms in `combat.rs`
- [ ] Cleave hits ≥2; priest/paladin heal hits a player
- [ ] Interrupt + taunt unit tests
- [ ] Seeded miss/crit

## Batch S2 — `1.2.0` (after 1.1.0)

| # | Workstream | Exclusive paths |
| --- | --- | --- |
| 1 | `ws-mining-smith` | content professions/nodes/recipes/items + `woc-sim/src/professions/**` |
| 2 | `ws-crypt-trash` | `woc-content/src/dungeons.rs` + `woc-sim/src/instances/**` |
| 3 | `ws-second-instance` | new dungeon **or** delve tables + instance/delve enter tests |
| 4 | `ws-talent-mods` | `woc-content/src/talents.rs` + `woc-sim/src/talents.rs` (combat call sites via CORE) |

**Merge order S2:** mining-smith ∥ crypt-trash ∥ second-instance ∥ talent tables → CORE wires talent bonuses in `combat.rs` once.

### S2 DoD

- [ ] Mining → craft → equip
- [ ] Crypt trash + boss
- [ ] One extra dungeon or delve
- [ ] ≥1 ability-mod talent wired

## Batch S3 — `1.3.0` (after 1.1.0; can overlap late S2 if paths stay disjoint)

| # | Workstream | Exclusive paths |
| --- | --- | --- |
| 1 | `ws-park-resume` | `woc-server/src/game_ws.rs` + `Sim` park/resume helpers (CORE) |
| 2 | `ws-aoi` | `snapshot_for_player` in `sim.rs` — **not parallel with 1** |
| 3 | `ws-persist-docs` | README + `woc-persist` module docs |

Run 1 then 2 on CORE; 3 always parallel.

### S3 DoD

- [ ] Hello resumes parked `character_id`
- [ ] Far mobs omitted; zone NPCs always present
- [ ] Postgres documented as durable path

## Main-agent rules

1. Never let two agents edit `sim.rs` or `woc-protocol` in the same batch.
2. Leaf agents expose `pub fn` hooks; main agent inserts one call site per merge.
3. After each batch: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client`.
4. New per-actor state is a `World` column (`AGENTS.md`). Do not invent a second component layout.
