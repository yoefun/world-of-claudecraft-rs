# Post-completion program design — `1.0.0` → `1.3.0`

**Status:** Proposed (planning deliverable 2026-08-13).  
**Baseline:** rewrite `1.0.0-pre` / parity `completion` on `develop` (ECS `World` actor store + loot/combat polish).  
**Upstream pin (unchanged unless bumped later):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `stable` then `combat-depth` / `content-depth` / `online-hard`.

Completion design (shipped): [`2026-07-28-rust-rewrite-completion-design.md`](2026-07-28-rust-rewrite-completion-design.md).

## 1. Goal

The completion program delivered a **playable gameplay-core rewrite**: one deterministic `woc-sim`, offline + online Bevy hosts, persist, nine classes, four overworld bands, party/instances/economy/professions/light PvP.

This program does **not** restart that rewrite. It turns `1.0.0-pre` into a **honest 1.0.0**, then deepens the systems that shipped as thin-but-green slices.

> Close the contract between docs, tick order, and code; then make combat, content, and online play feel like a game instead of a checklist.

## 2. Baseline (already shipped on `develop`)

| Piece | State |
| --- | --- |
| Version | `1.0.0-pre` / parity `completion` / protocol rev **6** |
| Crates | `woc-version`, `woc-protocol`, `woc-content`, `woc-sim`, `woc-persist`, `woc-server`, `woc-client` |
| Sim | 20 Hz, mulberry32, typed sparse-column `World`, sticky WS realm |
| World | Eastbrook / Eastfen / Mirefen / Thornpeak + crypt dungeon + 3-room delve |
| Combat | GCD, casts, DoT/HoT, kits 1–5, Tab/Esc, talent % stats, hunter/warlock pets |
| Economy | Bags, bank+copper vault, mail, AH, Need/Greed |
| Persist | Memory default; Postgres optional via `DATABASE_URL` |
| Client | Bevy offline+online; minimap/world map; procedural silhouettes |

### Honest remaining debt (why `pre` is still correct)

These are **not** new features. They are mismatches between “done” and what the code actually does:

1. **Tick-phase contract is stale.** `TICK_PHASES` lists six names. `Sim::tick_all` also runs pet AI, aura decay, mob respawn, death finalize, PvP honor, and auction expiry — deliberately *outside* the fingerprint so it stays stable. That is a lie the next combat/content wave will trip over.
2. **Ability hits are damage + hardcoded DoT ids.** `resolve_ability_hit` always deals `def.damage + 0.35 * attack_damage`. Cleave is not AoE. Priest/Paladin “heals” still require a living **mob** target. No miss/crit, interrupt, shield, or taunt.
3. **Dungeon is a boss shell.** `eastbrook_crypt` spawns one `crypt_warden`. No trash packs, no trash loot, no boss abilities.
4. **Professions are one loop.** Herbalism → two alchemy recipes. Skill cap 75 exists; the ladder is unused.
5. **Talents are stat multipliers.** Three talents/class, `damage_pct` / `max_hp_pct` / `armor_*` / `resource_pct`. No ability-modifying ranks.
6. **Online is co-presence, not a realm.** Full-zone snapshots, `MAX_REALM_PLAYERS = 8`, no reconnect resume, CI does not run on `develop`.
7. **Actor store is ECS columns (required).** Fat `Entity` is deleted. New per-actor state is a component in `ecs/components.rs`, never a blob vec. Combat-depth must query columns (`Health`, `Combatant`, `ClassKit`, …) and must not reintroduce `&mut [Entity]`.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Content first** | Add zones/dungeons/professions on the current tick lie and damage-only combat | Fast visible content; every new ability still special-cases `combat.rs` | Reject |
| **B. Reintroduce a fat actor** | Put new combat fields on a blob `Entity` / `Vec<Actor>` | Violates `AGENTS.md`; rejected | Reject |
| **C. Contract-close then depth on ECS columns (recommended)** | Ship `1.0.0` by telling the truth about tick/docs/CI, then data-driven combat as systems on `World` | Slightly slower first tag; later waves stay on the required actor model | **Adopt** |

## 4. Version map

| Rewrite | Parity label | Theme | Gate |
| --- | --- | --- | --- |
| **1.0.0-pre** | `completion` | Gameplay-core checklist (shipped) | STATUS core rows `done` |
| **1.0.0** | `stable` | Tick-phase truth, docs/CI, protocol comment hygiene, demo script | Fingerprint matches `tick_all`; `develop` in CI |
| **1.1.0** | `combat-depth` | Data-driven ability effects: heal, AoE, miss/crit, interrupt, taunt | Kits use `AbilityEffect`; PvP/party heals work |
| **1.2.0** | `content-depth` | Second profession pair, dungeon trash, second dungeon or delve, talent procs | Two full profession loops; crypt has trash |
| **1.3.0** | `online-hard` | Reconnect, per-player AOI radius, persist production notes | Disconnect → Hello resume same entity |

Upstream pin stays **0.31.0** unless a dedicated bump PR says otherwise.

## 5. Architecture (unchanged invariants)

One sim, multiple hosts:

- `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
- Client never decides combat / loot / quest / vendor / talent outcomes.
- All sim RNG via mulberry32 on `Sim` only; no wall clock in sim.
- Prefer additive `#[serde(default)]` protocol fields; bump `PROTOCOL_REV` on breaking wire changes.
- English-only strings.

### 5.1 Target tick phases (1.0.0 must document the real order)

`Sim::tick_all` today, which 1.0.0 locks:

1. `apply_intents_motion`
2. `player_combat`
3. `pet_ai`
4. `mob_ai_combat`
5. `aura_decay` (includes mob respawn timers)
6. `kill_rewards` (XP/quest/deed + loot spawn + Need/Greed start + death finalize)
7. `pvp_and_market` (duel resolve + listing expiry)
8. `loot_pickup`
9. `build_snapshot`

Do **not** reorder these once the 1.0.0 fingerprint lands. New systems hook *inside* an existing phase or append a named phase with a deliberate fingerprint update.

### 5.2 Combat-depth seam (1.1)

Keep `AbilityDef` in `woc-content`. Add an effect enum (not stringly `abil_id` matches):

```text
AbilityEffect =
  WeaponDamage { coefficient }
| SpellDamage { school }
| Heal { coefficient }
| AoeDamage { radius, max_targets }
| ApplyAura { aura_id }
| Interrupt
| Taunt { threat }
```

`resolve_ability_hit` becomes `apply_ability_effect`. Hardcoded Rend/Ignite/Sting id lists move to content tables. Player combat targeting: hostile for damage, friendly (self/party/pet) for heals.

### 5.3 Required actor model (ECS columns)

`woc-sim` actors live in `crates/woc-sim/src/ecs/` (`World` + `SparseSet` columns). Rules from `AGENTS.md`:

- New *per-actor* combat state → new component in `ecs/components.rs` (e.g. hit-result is snapshot-only unless a system needs it).
- New *per-realm* state → field on `Sim`.
- Query the columns a system needs. Do not branch on `Identity.kind` to skip missing data.
- Do not reintroduce a fat `Entity` / `Vec<Actor>`.
- Combat-depth implements `apply_ability_effect(world: &mut World, …)`.

## 6. Definition of done per rewrite

### 6.1 `1.0.0` / `stable`

1. `TICK_PHASES` + fingerprint test match the real `tick_all` order in §5.1.
2. `sim.rs` module docs, `STATUS.md`, `ROADMAP.md`, `README.md` crate blurb, `Cargo.toml` workspace description no longer say “framework slice” or “deed stub” as if current.
3. Protocol comments that still say “stub” for shipped actions (`ReleaseSpirit`, `TrainProfession`, `Gather`) are corrected.
4. CI runs on `develop` (push + PR target), not only `main`.
5. Completion design §8 demo script is copied into `docs/parity/DEMO.md` as the 1.0.0 acceptance script (manual; no GPU in CI).

### 6.2 `1.1.0` / `combat-depth`

1. Every class kit ability is described by `AbilityEffect` in content; `combat.rs` has no per-ability-id DoT match arms.
2. At least one heal hits self or a party player (priest or paladin).
3. At least one ability hits ≥2 mobs in radius (cleave or equivalent).
4. Auto-attack and abilities roll miss/crit via sim RNG (rates in content or `types`).
5. One interrupt and one taunt exist and have unit tests.
6. Player combat may target players when duel/PvP rules already allow damage.

### 6.3 `1.2.0` / `content-depth`

1. Second gathering + crafting pair (mining → blacksmithing **or** skinning → leatherworking — pick one in the implementation plan).
2. `eastbrook_crypt` spawns ≥2 trash packs before the boss; trash uses party loot.
3. A second dungeon **or** a second delve (not both required).
4. ≥1 talent per class changes an ability effect (proc, extra radius, reduced CD) — not only a stat %.
5. Content integrity tests cover the new tables.

### 6.4 `1.3.0` / `online-hard`

1. Disconnect parks the player entity; a later authenticated Hello with the same `character_id` resumes it (position/combat state) instead of always `spawn_player`.
2. `snapshot_for_player` can omit entities beyond a radius (AOI); tests lock “nearby mob in, far mob out”.
3. README documents Postgres as the durable production path; memory remains the zero-config default.
4. `MAX_REALM_PLAYERS` stays 8 unless a measured change says otherwise.

## 7. Explicit non-goals (still skipped)

Same list as completion, plus:

| Skip | Rationale |
| --- | --- |
| Map editor / custom heightfields | Still authoring, not sim |
| Browser / Electron / Capacitor | Bevy remains the client |
| Web3 / cosmetics shop | Not gameplay |
| Full i18n / admin SPA / Discord OAuth | Product shell |
| Vale Cup / Card Duel / Fiesta | Minigames |
| Byte-identical combat vs TypeScript | Rewrite non-goal |
| Full DESIGN.md chrome / authored 3D packs | Functional Bevy + procedural meshes |
| Reintroducing a fat actor struct | Violates required ECS model |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Fingerprint update looks like a behavior change | 1.0.0 only *names* existing calls; no reorder |
| Ability-effect enum balloons | Ship the seven variants in §5.2; no school-resistance matrix |
| Combat-depth adds a blob `Entity` field | New combat state is a column or a `Sim` resource (`AGENTS.md`) |
| Content-depth without combat-depth recreates id match arms | 1.2 depends on 1.1 |
| AOI breaks quest NPC talk | AOI applies to mobs/loot/pets; NPCs in the current zone always snapshot |

## 9. Success demo (human, end state of 1.3)

1.0.0: two clients still complete the completion demo; tick fingerprint test matches `tick_all`.  
1.1.0: priest heals a party mate; warrior cleave hits two wolves; a swing can miss.  
1.2.0: gather ore (or hides), craft a piece of gear, clear crypt trash then boss.  
1.3.0: kill a wolf, Alt-F4 client, log back in — same HP, same bag, same position, wolf corpse still there.

When 1.0.0’s contract items are green, tag `1.0.0`. Later tags wait on their own DoD.
