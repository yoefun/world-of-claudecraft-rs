# Manufacturing ECS Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire v1 manufacturing onto `woc-sim` ECS using `woc-content` tables and stable `ProfessionDeny` ids.

**Architecture:** Content tables expand in `woc-content`. Protocol grows deny + skin/enchant actions. `woc-sim` adds `ProfessionCast` / `GatherNodeState` / `Skinnable` and rewrites `professions` against `World`. `woc-manufacturing` stays as the typed oracle crate.

**Tech Stack:** Rust workspace, existing custom ECS (`ecs::World`), `woc-protocol` serde, 20 Hz ticks.

## Global Constraints

- Do not depend on `woc-manufacturing` from `woc-sim`.
- Profession denials are `ProfessionDeny` (no English profession toasts).
- Blacksmithing id stays `blacksmithing` (not `forging`).
- Gathering cap 100, crafting cap 125.
- Known recipes have no skillReq admission; nodes are tool-gated.
- Gather success = 2 RNG draws; craft success = 1 masterwork draw; disenchant = 0.
- Recipe economy: input reagent_unit_value > output vendor_sell.
- Gathered mats must not appear on NPC vendor stock.
- Author commits as `yoefun <xinglinsky@outlook.com>`.

---

### Task 1: Content tables

**Files:** `crates/woc-content/src/{professions,recipes,gather_nodes,items,npcs,lib}.rs`; create `stations.rs`, `enchants.rs`.

- [ ] Expand professions, recipes (station + budget), nodes (tool/tier/respawn/fine), items, trainers, stations, enchants.
- [ ] Tests: ten professions, recipe economy, vendors never stock gathered mats.
- [ ] `cargo test -p woc-content --lib`

### Task 2: Protocol

**Files:** `crates/woc-protocol/src/lib.rs`

- [ ] Add `ProfessionDeny`, `ProfessionDenied`, `Skin` / `Disenchant` / `ApplyEnchant`.
- [ ] Roundtrip tests.
- [ ] `cargo test -p woc-protocol --lib`

### Task 3: ECS components + sim professions

**Files:** `crates/woc-sim/src/ecs/{components,world,spawn}.rs`, `professions/**`, `sim.rs`, `host.rs`, `rng.rs`, `combat.rs` loot, existing profession tests.

- [ ] Components, tick phase `profession_casts`, deny ids, tools, stations, skin, enchant, casts.
- [ ] Update existing gather/craft tests (tools, gold, flux).
- [ ] `cargo test -p woc-sim --lib` and `cargo test -p woc-manufacturing`
