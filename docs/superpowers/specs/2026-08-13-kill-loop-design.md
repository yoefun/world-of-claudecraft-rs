# Kill-loop design — `1.14.0` / `kill-loop`

**Status:** Shipped as rewrite 1.14.0 / kill-loop (polish 2026-08-14).  
**Baseline:** rewrite `1.13.0` / `gear-slots` on `develop` (ECS `World` actor store; manufacturing wired).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `kill-loop`.

Post-completion combat-depth (shipped): [`2026-08-13-post-completion-program-design.md`](2026-08-13-post-completion-program-design.md).  
Sim ECS (required): [`2026-08-13-sim-ecs-design.md`](2026-08-13-sim-ecs-design.md).  
Gear loot rolls (shipped): [`2026-08-13-gear-depth-design.md`](2026-08-13-gear-depth-design.md).

## 1. Goal

Close the **spawn → fight → loot → respawn** loop so overworld camps, dungeon trash, and world bosses behave like a game instead of a 30-second checklist.

Today every mob revives in 30 seconds (including crypt trash), leash does not heal, loot piles never expire and ignore `LootEntry.count`, a hunter pet last-hit grants no XP, and mobs only white-swing.

> Pull a camp. The pack leashes and resets if you run. The corpse drops the table (including stack counts). Trash in an instance stays dead until the instance resets. The wolf comes back on its own timer, not the boss’s.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Overworld spawn | `ZoneLayout.mobs: &[MobSpot { mob_id, x, z }]`; `populate_all_overworld` jitters ±0.75 yd |
| Dungeon trash | `DungeonTrashSpot { mob_id, x, z, count }`; spawned on instance enter |
| Respawn | `Respawn.respawn_timer`; `tick_mob_respawns` uses global `MOB_RESPAWN_SEC = 30.0` for **every** `Respawn` column, including instance trash and dungeon bosses |
| Leash | `LEASH_RANGE = 40`; drops target + threat; **does not** restore HP or auras |
| Social aggro | Geometric camp: `CAMP_HOME_RADIUS = 20`, `SOCIAL_AGGRO_RANGE = 16` |
| Mob combat | Auto-attack only (`update_mob_combat`); `Combat.ability_cd` unused on mobs |
| Threat | `add_threat` on damage; `prefer_mob_target` **sticks** to the current living target |
| Loot | `spawn_mob_loot` rolls every `LootEntry`; one pile per success; copper on first pile |
| `LootEntry.count` | Declared, **never applied** (`grant_loot_pile` always grants `1`) |
| Loot lifetime | Piles persist until picked up or Need/Greed resolves |
| `LootTable` | Marker + `xp_value`; `loot_copper` / `loot_item` unused (payout is content lookup at death) |
| Party loot | FFA + Need/Greed; quest `ItemKind::Quest` can still enter a roll |
| Pet last-hit | `SimEvent::Kill.killer` is the pet; XP/quest/party share skip non-players |
| Protocol | Rev **8**; tick fingerprint `3214741777866168171` (10 phases) |

Honest remaining kill-loop debt:

1. **One respawn number.** Wolves, world bosses, and crypt trash all revive in 30s. Instance trash coming back while the party is still inside is a bug, not a feature.
2. **Leash is a kite cheese.** Running 40 yd drops aggro but the mob keeps missing HP, so the next pull is free.
3. **Camps are seven hardcoded points.** `DungeonTrashSpot` already has `count`; overworld `MobSpot` does not, so density means listing every wolf.
4. **Loot is a leak.** `count` is ignored; piles never despawn; `create_loot` stamps `zone_id = "eastbrook"` even in Eastfen or an instance.
5. **Pets steal the kill.** Hunter/warlock last-hit yields copper piles but no XP, no quest credit, no Need/Greed.
6. **Mobs have no kit.** `AbilityEffect` exists; only players dispatch it. Bosses are inflated auto-attackers.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Content only** | More `MobSpot`s and loot rows | Fast; 30s crypt respawn, leash cheese, pet last-hit, and `count` stay broken | Reject |
| **B. Full spawn engine** | Spawn groups, waypoints, phases, conditions, master loot, dodge/parry | Fights the locked tick contract; weeks of tables | Reject |
| **C. Kill-loop polish on existing columns (recommended)** | Per-template respawn + instance skip, leash reset, `MobSpot` count/radius, loot count + TTL, pet credit, one mob ability seam | One content pass; no new tick phase; no fat actor | **Adopt** |

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.13.0** | `gear-slots` | Dual-wield, Finger2, quality, MH enchant (shipped) |
| **1.14.0** | `kill-loop` | Spawn timers, leash reset, loot count/TTL, pet credit, mob abilities |

`PROTOCOL_REV` stays **8**. New snapshot/event fields use `#[serde(default)]`. Upstream pin stays **0.31.0**. Do **not** bump `VERSION.toml` / workspace version in the planning change; the implementation wave tags `1.14.0`.

Tick-phase fingerprint stays `3214741777866168171`. Loot expiry hooks inside `loot_pickup`. Respawn duration stays inside `aura_decay` (`tick_mob_respawns`). Mob abilities hook inside `mob_ai_combat`. Pet credit hooks inside `kill_rewards`. No new named phase.

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
- Client never decides combat / loot / spawn / respawn outcomes.
- All sim RNG via mulberry32 on `Sim` only; no wall clock in sim (respawn and loot TTL are **seconds / ticks**).
- English-only strings.
- New *per-actor* state is a field on an existing `World` column (`Respawn`, `LootPile`) or content on `MobTemplate` / `MobSpot`. Do **not** add a `SpawnGroup` blob actor or a fat `Entity`.
- Query columns. Living overworld mob = `Respawn` + `Health.alive`. Instance skip = `InstanceAt.instance_id.is_some()`, not `Identity.kind`.

```
woc-content MobTemplate / MobSpot     woc-sim spawn / mob / combat / loot
        │                                         │
        ▼                                         ▼
 populate_all_overworld  →  Home + Respawn.delay_sec
        │                                         │
        ▼                                         ▼
 leash / death → reset or arm timer → revive at Home
 kill_rewards → owner credit → spawn_mob_loot (count, zone, TTL)
 mob_ai_combat → white swing + optional AbilityEffect
```

### 5.1 Spawn and respawn

`MobTemplate` gains:

```text
respawn_seconds: f32    // 0 = never (used at spawn to fill Respawn.delay_sec)
ability_id: Option<&'static str>  // optional AbilityDef id; None = white hits only
```

`Respawn` becomes `{ respawn_timer: f32, delay_sec: f32 }`. `delay_sec == 0.0` means **never revive**. `create_mob_from_template` copies `MobTemplate.respawn_seconds`. `spawn_boss_shell` / `spawn_trash_spot` set `delay_sec = 0.0` even if the template would respawn in the overworld.

`tick_mob_respawns`:

1. Skip living actors (`Health.alive`).
2. Skip `delay_sec <= 0.0` (dead instance actors stay corpses until the instance despawns).
3. On first observation of death, arm `respawn_timer = delay_sec` (not the global 30s constant).
4. On expiry, `revive_mob` (existing: HP full, pose = Home, clear combat/threat/auras).

Keep `MOB_RESPAWN_SEC = 30.0` as the **default** overworld value. Do not delete the constant; templates that omit an explicit number use it.

World boss `mire_terror`: `respawn_seconds = 300.0` (five minutes). Deed completion stays one-shot per player (`worldboss.rs` unchanged).

Dungeon / delve: trash and bosses never auto-respawn. Leaving the instance already despawns instance-keyed actors; the next enter spawns a fresh pack. That is the reset.

`MobSpot` gains `count: u32` and `radius: f32`. Helpers keep existing tables readable:

```rust
const fn mob(id: &'static str, x: f32, z: f32) -> MobSpot {
    MobSpot { mob_id: id, x, z, count: 1, radius: 1.5 }
}
```

`populate_all_overworld` / `ensure_zone_population`: for each spot, spawn `count` actors. Offset `i` uses mulberry32 inside `radius` (replace the hardcoded `(rng - 0.5) * 1.5` jitter). `Home` is the actual spawn pose (unchanged). Eastbrook Wolf Run: two `young_wolf` spots use `count: 2` so the camp is five wolves + one scarred, not four loners.

`create_mob_from_template` currently hardcodes `zone_id = "eastbrook"`. Callers already overwrite. Change the factory to take `zone_id: &str` so loot/combat never see a lying zone. Existing test call sites pass `"eastbrook"` or the layout tag.

### 5.2 Leash reset

When `d_home > LEASH_RANGE`, call a shared `reset_mob_to_home(world, id)` used by both leash and (without the move) revive:

- `Combat.target = None`, clear cast/swing/gcd/ability_cd
- `Threat.threat.clear()`
- `Auras.auras.clear()`
- `Health.hp = hp_max` (leash only; revive already does this)
- step toward Home (leash) or snap to Home (revive)

Do **not** start the respawn timer on leash. The mob is still alive.

### 5.3 Loot

Honor `LootEntry.count`. `LootPile` becomes `{ copper, item, count, expires_tick }`.

- `count` defaults to `1`. `grant_loot_pile` / Need/Greed `resolve` grant `count` (not `1`).
- `expires_tick = 0` means never (gather nodes). Kill piles use `Sim.tick + LOOT_PILE_TTL_TICKS` where `LOOT_PILE_TTL_TICKS = 2_400` (120 s at 20 Hz).
- `tick_all` phase `loot_pickup`: **first** despawn piles with `expires_tick != 0 && tick >= expires_tick`, **then** `try_pickup_loot`. Pending Need/Greed piles that expire: drop the pending row and despawn (toast `"Loot expired."`).
- `create_loot` takes `zone_id` from the victim’s `Identity.zone_id`. `kill_rewards` already copies `InstanceAt` from the killer; also copy from the **victim** when the killer has none (pet / overworld).
- Quest items (`ItemKind::Quest`): `maybe_start_party_roll` skips the pile (killer / FFA proximity pickup still works). Gear and junk still roll.

`LootTable.loot_copper` / `loot_item` stay unused. Payout remains `spawn_mob_loot` + `MobTemplate`. Do not dual-write.

Skinning stays on the first kill pile (`maybe_mark_skinnable`). 120 s TTL is the skin window. Do not move `Skinnable` onto the dead mob in this program.

### 5.4 Pet kill credit

`collect_pending_mob_kills` rewrites `killer` through `Owner`:

```text
credit_actor(world, id) = world.get::<Owner>(id).owner_id  else id
```

XP, quest `on_mob_killed`, deeds, loot instance key, and Need/Greed eligibility all use the credited player. `SimEvent::Kill` still records the actual source (pet or player) for combat log truth; rewards use the credited actor. If the owner is missing, credit stays with the pet, so the existing player-kind check skips XP but loot still spawns at the corpse. A dead-but-present owner remains the credited player and receives XP through that player-kind check.

### 5.5 Mob abilities and threat

`MobTemplate.ability_id` optional. `update_mob_combat` keeps the white swing, then if `ability_id` resolves, the mob is in ability range, not stunned, and `Combat.ability_cd <= 0`, call existing `apply_ability_effect` and set `ability_cd = def.cooldown`. Pass `&mut Sim.rng` into `update_mob_combat` (miss/crit already live there for players).

Ship **three** mob abilities (new `AbilityDef` rows, not on any class kit):

| id | Who | Effect |
| --- | --- | --- |
| `wolf_bite` | `scarred_wolf` | `WeaponDamage { 1.2 }`, cooldown 6 s, range 3 |
| `warden_smash` | `crypt_warden` | `AoeDamage { radius: 4, max_targets: 3 }`, cooldown 8 s |
| `terror_slam` | `mire_terror` | `WeaponDamage { 1.5 }`, cooldown 8 s, range 3 |

Young wolves / crawlers stay white-hit only. `every_ability_declares_an_effect` currently locks `ABILITIES.len() == 51`; bump to `54`.

Threat: `prefer_mob_target` switches when another living player’s threat is `> THREAT_SWITCH_RATIO * current` (`1.1`). Current target still wins ties. Taunt already writes target + threat; the ratio keeps a tank once they lead.

### 5.6 Protocol and client

Rev stays **8**. Additive:

- `SimEvent::Loot` gains `#[serde(default)] count: u32` (0 means “treat as 1” for old peers).
- `PendingLootSnapshot` unchanged (still one `item_id`); the granted stack uses `LootPile.count`.

No Bevy combat/loot/HP components. Client already despawns snapshot-missing entities, so expired piles vanish without a client change. Optional nameplate `xN` is out of scope.

## 6. Definition of done

1. Overworld `young_wolf` revives at Home after **30 s**; `mire_terror` after **300 s**.
2. Crypt / barrow trash and bosses have `Respawn.delay_sec == 0` and do **not** revive while the instance exists.
3. Leash beyond 40 yd restores HP to max and clears auras/threat; a second pull is a full-HP wolf.
4. `MobSpot.count` / `radius` spawn N actors; Eastbrook Wolf Run has ≥5 living `young_wolf` after populate.
5. A `LootEntry` with `count: 2` grants two of that item (bag stack or Need/Greed award).
6. An unlooted kill pile is gone by tick `spawn_tick + 2400`.
7. Quest-item piles never enter Need/Greed pending.
8. Hunter pet last-hit grants the owner XP and quest kill credit.
9. `scarred_wolf` uses `wolf_bite` on cooldown in melee; unit test sees a `Damage.ability` of `Wolf Bite` (or the def name).
10. Two players on one wolf: after a 1.1× threat lead, the mob’s `Combat.target` is the leader.
11. `tick_phase_order_fingerprint_locked` still equals `3214741777866168171`.
12. `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client` green.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Spawn groups / waypoints / patrol | Geometric social aggro is enough |
| Master loot / round-robin | FFA + Need/Greed already ship |
| Dodge / parry / block / school resist | Combat-depth hit table is miss/crit only |
| Moving `Skinnable` onto the corpse | 120 s pile TTL is the skin window |
| Filling `LootTable.loot_copper` | Dual-write with `MobTemplate` |
| New tick phase | Hook inside `aura_decay` / `kill_rewards` / `mob_ai_combat` / `loot_pickup` |
| Fat `Entity` / Bevy gameplay HP | `AGENTS.md` |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |
| Bind-on-pickup, loot sparkles, corpse-window UI | Presentation |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Instance skip also blocks an overworld mob that borrowed `InstanceAt` | Only skip when `instance_id.is_some()`; overworld default is `None` |
| `apply_ability_effect` spends combo/rage on mobs | Mobs have no `ClassKit`; those branches no-op |
| Loot TTL races Need/Greed | Expire path removes `pending` first; toast `"Loot expired."` |
| `ABILITIES.len() == 51` lock | Implementation updates the count in the same commit as the three rows |
| Factory `zone_id` signature churn | One task updates `create_mob_from_template` / `create_loot` and all call sites |
| Fingerprint drift | Do not add a phase; tests assert the existing hash |

## 9. Success demo (human)

1. Kill a Young Wolf at Wolf Run; wait 30 s — it stands up at its Home with full HP. Leave the fang on the ground for 2 minutes — the pile is gone.
2. Pull the wolf past 40 yd from Home — it walks back **full HP**. Re-pull is not a leftover 10 HP cheese.
3. Enter Eastbrook Crypt, kill trash, wait 30 s — trash stays dead. Leave and re-enter — a fresh pack.
4. Hunter: pet gets the last hit on a wolf — owner gains XP and Wolves quest credit; loot still spawns.
5. Party Need/Greed on `crypt_cleaver`; a `boar_tusk` quest pile does **not** open 1/2/3.
6. Scarred Wolf in melee — combat log shows Wolf Bite on a 6 s cadence, not only white hits.
7. Two players on one scarred wolf; warrior Taunt — mob stays on the warrior while threat leads by 1.1×.

When §6 is green, tag `1.14.0`.
