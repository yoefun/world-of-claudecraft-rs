# Upstream pin

This repository is a **Rust rewrite** of
[levy-street/world-of-claudecraft](https://github.com/levy-street/world-of-claudecraft).

| Field | Value |
| --- | --- |
| Rewrite version | `1.23.0` |
| Upstream repo | https://github.com/levy-street/world-of-claudecraft |
| Upstream version | `0.31.0` |
| Upstream commit | `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9` |
| Parity target | `delve-depth` |

Machine-readable source of truth: [`VERSION.toml`](VERSION.toml).

## How to bump the pin

1. Choose the upstream tag/commit to track.
2. Update `VERSION.toml` (`upstream_version`, `upstream_commit`).
3. Note behavioral deltas in [`docs/parity/STATUS.md`](docs/parity/STATUS.md).
4. Mention the pin change in `CHANGELOG.md`.

## Relationship to upstream

- We reimplement behavior inspired by the TypeScript `src/sim/` core.
- Heightfield (`terrainHeight` / `groundHeight` / noise) is contract-aligned to this pin with seed `20061`; golden samples tolerate ε≈1e-3 (`f32` vs JS `number`). Byte-identical floats remain a non-goal.
- Map-editor custom worlds (`setActiveWorldContent`) stay deferred.
- The client is a native Bevy host; the browser/Three.js stack remains out of scope.
