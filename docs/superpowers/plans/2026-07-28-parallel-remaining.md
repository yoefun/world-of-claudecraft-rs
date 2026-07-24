# Remaining rewrite — max-parallel dispatch schedule

**Base tip:** rewrite `0.3.0` / `online-alive` (`cursor/ws-framework-polish-0a-8e8e`).  
**Parent plan:** [`2026-07-28-rust-rewrite-completion.md`](2026-07-28-rust-rewrite-completion.md)

## Principle

Maximize concurrent subagents by **exclusive path ownership**. Only these are serial choke points:

1. `crates/woc-protocol/src/lib.rs` — one PROTO agent per freeze
2. `crates/woc-sim/src/sim.rs` tick wiring — **main agent only** after leaf PRs land
3. Version bump / CHANGELOG / STATUS — main agent at wave gate

Everything else fans out.

## Dependency DAG (remaining)

```text
                    ┌─ mob-ai ─────────────────────────────┐
                    ├─ motion/colliders ───────────────────┤
[0.3 tip] ─┬─► PROTO freeze ─┬─► ability-kits ─► talents ──┼─► 0.5 gate
           │                 ├─► party/chat ─► loot-rules ─┼─► dungeon ─► delve
           │                 └─► pets (after mob-ai) ──────┤
           ├─► zone2/3 content ────────────────────────────┼─► zone transition (CORE)
           ├─► persist ─► auth API ─► client login ────────┼─► 0.4 gate
           │         └─► bank ∥ mail ─► market ────────────┼─► 0.8
           ├─► professions content ─► (after bags+zones) gather/craft
           ├─► client UI chrome ───────────────────────────┤
           └─► pvp (after combat+online — ready) ∥ worldboss
```

## Batch R1 — launch immediately (7-wide, no shared files)

| # | Workstream | Branch | Exclusive paths | Needs PROTO? |
| --- | --- | --- | --- | --- |
| 1 | `ws-proto-remaining` | `cursor/ws-proto-remaining-8e8e` | `woc-protocol` only | — |
| 2 | `ws-mob-ai` | `cursor/ws-mob-ai-8e8e` | `woc-sim/src/mob.rs` (+ optional `mob/`) | no |
| 3 | `ws-motion` | `cursor/ws-motion-8e8e` | `player_motion.rs`, `physics/**` | no |
| 4 | `ws-zone2-content` | `cursor/ws-zone2-content-8e8e` | `zone2.rs`, new `quests_zone2.rs` / mobs/npcs rows in separate files if possible | no |
| 5 | `ws-persist` | `cursor/ws-persist-8e8e` | `crates/woc-persist/**`, workspace `Cargo.toml` members, server `auth*.rs` | no |
| 6 | `ws-client-ui-chrome` | `cursor/ws-client-ui-8e8e` | `woc-client/src/**` | no |
| 7 | `ws-professions-content` | `cursor/ws-prof-content-8e8e` | `woc-content` new `professions.rs` `recipes.rs` `gather_nodes.rs` only | no |

**Merge order R1:** PROTO → persist (Cargo.toml) → content streams (zone2, professions) → mob-ai → motion → client UI → main wires `sim.rs` hooks.

## Batch R2 — after R1 merge (5-wide)

| # | Workstream | Depends | Exclusive paths |
| --- | --- | --- | --- |
| 1 | `ws-ability-kits` | PROTO AbilitySlot | content abilities/classes + `combat.rs` |
| 2 | `ws-party-chat` | PROTO party/chat | `woc-sim/src/social/**` + client chat |
| 3 | `ws-pets` | mob-ai | `woc-sim/src/pet/**` + content pets |
| 4 | `ws-zone-transition` | zone2 content | CORE `sim.rs` zone change (main or single agent) |
| 5 | `ws-client-login` | persist auth | `woc-client` login UI |

## Batch R3 — after R2 (parallel where noted)

| Streams | Parallel? |
| --- | --- |
| talents (after kits) | with pets polish |
| loot-rules (after party) | alone then dungeons |
| bank ∥ mail (after persist) | yes |
| market (after bank) | |
| professions sim (after content + zones) | with market |
| pvp duel/honor | with professions |
| dungeon shell → delve | sequential-ish |
| worldboss/deeds | with pvp |

## Main-agent rules

1. Never let two agents edit `sim.rs` or `woc-protocol` in the same batch.
2. Leaf agents expose `pub fn` hooks; main agent inserts one call site per merge.
3. After each batch: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client`.
4. Prefer one integration branch (`cursor/ws-framework-polish-0a-8e8e` or successor) receiving merges.

## What cannot be fully parallel

| Item | Why |
| --- | --- |
| Talent procs | needs ability kit IDs |
| Loot Need/Greed | needs party |
| Dungeon instances | needs party + loot + combat |
| Character save shape | stabilize after bags/talents or version migrations |
| `sim.rs` phase list changes | single writer |
