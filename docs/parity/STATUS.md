# Parity status

## Rewrite 1.0.0-pre ↔ upstream 0.31.0 (`completion`)

Legend: `done` · `partial` · `planned` · `deferred` · `n/a`

Completion design: [`docs/superpowers/specs/2026-07-28-rust-rewrite-completion-design.md`](../superpowers/specs/2026-07-28-rust-rewrite-completion-design.md)  
Completion plan: [`docs/superpowers/plans/2026-07-28-rust-rewrite-completion.md`](../superpowers/plans/2026-07-28-rust-rewrite-completion.md)

### Gameplay-core (completion gate)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Version / upstream pin | done | `1.0.0-pre` / completion |
| `woc-content` Eastbrook tables | done | |
| Deterministic tick (20 Hz) | done | locked phase fingerprint |
| Seeded RNG (mulberry32) | done | |
| SimContext seam | done | emit/lookup/mutate |
| Multi-player Entity economy | done | |
| Sticky WS realm | done | |
| Client online mode | done | |
| Death / spirit / graveyard | done | |
| Combat core (GCD/cast/aura/threat) | done | |
| Deeper bags / consumables | done | |
| Tab targeting | done | |
| Player motion / colliders | done | |
| Mob respawn / social aggro | done | |
| `woc-persist` + auth API | done | R4 fields: talents/bank/honor/zone/professions |
| Client login / char select | done | |
| Multi-ability kits | done | |
| Talents / loadouts | done | spend/respec + damage_pct; client panel N |
| Pets | done | hunter/warlock |
| Zone2 + zone3 / Mirefen | done | Eastfen + Mirefen content + portals |
| Party + chat | done | |
| Group loot rules | done | FFA + Need/Greed |
| Dungeons / instances | done | eastbrook_crypt boss shell |
| Delves | done | eastbrook_hollow 3-room loop + reward |
| Bank + mail | done | sim + client panels K/M |
| Auction market | done | sim + client panel U |
| Professions gather/craft | done | herbalism → alchemy salve |
| Duel + PvP honor | done | |
| World boss + deeds | done | mire_terror + deed honor |
| Client economy/talent chrome | done | N/K/M/U panels |
| Heightfield terrain | partial | not byte-identical |
| Byte-identical terrain/combat | n/a | Explicit non-goal |

### Explicit deferred / non-goals

| Subsystem | Status |
| --- | --- |
| Browser Three.js / Electron / Capacitor | deferred |
| Web3 / wallets / cosmetics shop | deferred |
| Gymnasium RL / headless env | deferred |
| Full i18n catalogs | deferred |
| Admin SPA / Discord OAuth polish | deferred |
| Vale Cup / Card Duel / Fiesta | deferred |

## Prior rewrites

0.3.0 `online-alive`, 0.2.0 `framework`, 0.1.0 `combat-slice` — superseded by the table above.
