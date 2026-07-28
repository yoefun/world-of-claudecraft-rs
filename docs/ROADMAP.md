# Roadmap

| Rewrite | Parity target | Intent |
| --- | --- | --- |
| **0.1.0** (shipped on branch) | `combat-slice` | Bevy offline Warrior combat: wolves, XP/loot, thin server health |
| **0.2.0** (next) | `framework` | Basic framework complete — see below |
| 0.3.x (later) | `online-persist` (tentative) | Postgres characters/auth, durable realm |
| 0.4.x+ (later) | content systems | Talents, multi-zone, dungeons, social, market, … |

## 0.2.0 — framework complete

**Definition of done:** [`docs/superpowers/specs/2026-07-28-rust-rewrite-framework-design.md`](superpowers/specs/2026-07-28-rust-rewrite-framework-design.md) §3  
**Implementation plan:** [`docs/superpowers/plans/2026-07-28-rust-rewrite-framework.md`](superpowers/plans/2026-07-28-rust-rewrite-framework.md)

Phases: **A** content + SimContext + WorldHost → **B** inventory/equip → **C** 9 classes → **D** quests/NPC → **E** vendor/camps → **F** WebSocket host + release.

Upstream pin remains **0.31.0** unless explicitly bumped. Full MMO parity is not the 0.2 goal.
