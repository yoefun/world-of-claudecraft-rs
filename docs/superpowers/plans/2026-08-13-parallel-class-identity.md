# Class-identity — max-parallel dispatch schedule

**Base tip:** rewrite `1.3.0` / `online-hard` on `develop` **plus** [PR #20](https://github.com/yoefun/world-of-claudecraft-rs/pull/20) merged.  
**Parent plan:** [`2026-08-13-class-identity-program.md`](2026-08-13-class-identity-program.md)  
**Design:** [`docs/superpowers/specs/2026-08-13-class-identity-program-design.md`](../specs/2026-08-13-class-identity-program-design.md)

## Principle

Maximize concurrent subagents by **exclusive path ownership**. Serial choke points:

1. `crates/woc-protocol/src/lib.rs` — one PROTO agent per freeze (rev 7 at 1.6)
2. `ClassKit` / `Combat` / `AuraInstance` field adds — **KIT in the same batch as first COMBAT use**
3. `crates/woc-sim/src/sim.rs` snapshot + interact — CORE only
4. Version bump / CHANGELOG / STATUS — main agent at wave gate
5. Do not collide with `1.4.0` / `1.5.0` client-compat/update PRs (different files, different versions)

## Dependency DAG

```text
[1.3.0 + PR #20]
    │
    ├─► PROTO rev 7
    │       │
    │       ├─► KIT fields ∥ CONTENT flags/stubs
    │       │         └─► COMBAT dispatch (absorb, lockout, combo, charge, blink, convert)
    │       │                    ├─► MOB stealth aggro ∥ MOTION stealth speed
    │       │                    └─► CORE snapshot + interact ─► CLIENT Z/HUD
    │       └─► version 1.6.0 class-engine
    │
    └─► [after 1.6.0]
            CONTENT kit swaps ─► COMBAT class tests ─► CLIENT hints ── 1.7.0
                    │
                    └─► [after 1.7.0]
                            thorns/form auras ─► paladin/shaman/warlock/druid/warrior
                            PERSIST stance_id ∥ CLIENT F ── 1.8.0
```

## Batch S1 — `1.6.0` (after PR #20)

| # | Workstream | Branch | Exclusive paths | Notes |
| --- | --- | --- | --- | --- |
| 1 | `ws-proto-rev7` | `cursor/ws-proto-rev7-67ff` | `woc-protocol/src/lib.rs` | **First** |
| 2 | `ws-kit-fields` | `cursor/ws-kit-fields-67ff` | `ecs/components.rs`, `ecs/spawn.rs` | after 1 |
| 3 | `ws-engine-content` | `cursor/ws-engine-content-67ff` | `woc-content/src/abilit*.rs`, `classes.rs`, `lib.rs` | ∥ 2 |
| 4 | `ws-engine-combat` | `cursor/ws-engine-combat-67ff` | `woc-sim/src/combat.rs`, `types.rs` | after 2+3 |
| 5 | `ws-stealth-mob` | `cursor/ws-stealth-mob-67ff` | `woc-sim/src/mob.rs` | after 2 |
| 6 | `ws-stealth-motion` | `cursor/ws-stealth-motion-67ff` | `player_motion.rs` | after 2 |
| 7 | `ws-identity-snap` | `cursor/ws-identity-snap-67ff` | `woc-sim/src/sim.rs` | after 1+2+4 |
| 8 | `ws-identity-hud` | `cursor/ws-identity-hud-67ff` | `woc-client/src/input.rs`, `hud.rs` | after 1, ∥ 7 |
| 9 | `ws-rel-160` | main | `VERSION.toml`, STATUS, CHANGELOG, ROADMAP | last |

## Batch S2 — `1.7.0`

| # | Workstream | Exclusive paths |
| --- | --- | --- |
| 1 | `ws-identity-kits` | `woc-content` classes/abilities |
| 2 | `ws-identity-tests` | `woc-sim/src/combat.rs` tests only if 1 is merged, else same PR as 1 |
| 3 | `ws-rel-170` | version docs |

Prefer **one PR** for S2 (kit swaps + tests) — the files overlap.

## Batch S3 — `1.8.0`

| # | Workstream | Exclusive paths |
| --- | --- | --- |
| 1 | `ws-forms-content-combat` | content auras + combat thorns/fear-break + kits |
| 2 | `ws-forms-persist` | `woc-persist/src/models.rs` `stance_id` |
| 3 | `ws-forms-client` | client **F** |
| 4 | `ws-rel-180` | version docs |

1 and 2 may run in parallel; 3 after 1 (needs `ToggleForm` / `CycleStance` already dispatched in sim).

## Merge gates

- **S1:** `absorb_soaks_damage_before_hp` + `stealth_skips_wolf_aggro_at_range` + protocol rev 7 roundtrip
- **S2:** five class tests in the parent plan §1.7.2
- **S3:** five form tests in §1.8.3; STATUS lists a signature for all nine classes
