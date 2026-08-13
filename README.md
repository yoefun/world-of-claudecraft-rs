# world-of-claudecraft-rs

Rust rewrite of World of ClaudeCraft. v1 starts with the manufacturing sim.

## Manufacturing plan (this branch)

Design and implementation plan for gathering, forging, skinning, leatherworking, tailoring, jewelcrafting, enchanting, engineering, and alchemy:

- Design spec: [`docs/superpowers/specs/2026-08-13-manufacturing-system-design.md`](docs/superpowers/specs/2026-08-13-manufacturing-system-design.md)
- Implementation plan: [`docs/superpowers/plans/2026-08-13-manufacturing-system.md`](docs/superpowers/plans/2026-08-13-manufacturing-system.md)
- Short pointer: [`docs/design/manufacturing.md`](docs/design/manufacturing.md)

The sim crate (`crates/woc-sim`) is created in Task 1 of the plan.

```sh
cargo test -p woc-sim
```
