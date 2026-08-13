# Task 8 Report: Independent loot rolls + locked drops

## Status

**Complete.** All required tests pass; changes committed on `cursor/gear-depth-plan-a6b7`.

## Commit

- `287e59b` — `feat(content/sim): gear drops and independent loot rolls`

## TDD Evidence

### Step 1 — Failing tests added

- `dungeon_bosses_have_mob_templates` in `crates/woc-content/src/lib.rs`
- `independent_loot_can_drop_two_items`, `crypt_warden_drops_cleaver` in `crates/woc-sim/src/combat.rs`
- `pendant_raises_hp_max` in `crates/woc-sim/src/stats.rs`

### Step 2 — Red (expected failures)

```
cargo test -p woc-content dungeon_bosses_have_mob_templates -- --nocapture
→ FAILED: boss crypt_warden missing MobTemplate

cargo test -p woc-sim independent_loot_can_drop_two_items -- --nocapture
→ FAILED: assertion failed: piles.iter().any(|i| i == "hag_focus")

cargo test -p woc-sim crypt_warden_drops_cleaver -- --nocapture
→ FAILED: left: [], right: ["crypt_cleaver"]

cargo test -p woc-sim pendant_raises_hp_max -- --nocapture
→ FAILED: expected +8 hp_max from sta 4, got base 132 with 132
```

### Step 3 — Implementation

**Content (`woc-content`):**

- Added `jewelry(...)` and `weapon_gear(...)` helpers in `items.rs`.
- New items: `fang_pendant`, `boar_tusk_ring`, `crypt_cleaver` (zone1); `fen_staff`, `hag_focus` (zone2).
- Mob loot tables: independent rows on `scarred_wolf`, `young_boar`, `bog_wisp`, `barrow_hag`.
- New `crypt_warden` `MobTemplate` (lvl 3, hp 240, xp 150, copper 20–40, AP 14, `crypt_cleaver` 1.0).

**Sim (`woc-sim`):**

- `spawn_mob_loot` rolls every `LootEntry` independently; copper on first pile only; piles offset `x + i * 0.4`; returns first pile id.

### Step 4 — Green

```
cargo test -p woc-content --lib
→ 43 passed; 0 failed

cargo test -p woc-sim --lib
→ 225 passed; 0 failed

cargo test -p woc-sim independent_loot_can_drop_two_items -- --nocapture  → ok
cargo test -p woc-sim crypt_warden_drops_cleaver -- --nocapture           → ok
cargo test -p woc-sim pendant_raises_hp_max -- --nocapture                 → ok
cargo test -p woc-content dungeon_bosses_have_mob_templates -- --nocapture → ok
```

Integrity tests `every_gear_item_has_rules` and `every_mob_loot_item_exists` remain green.

## Files changed

| File | Change |
|------|--------|
| `crates/woc-content/src/items.rs` | Helpers + zone1 gear items |
| `crates/woc-content/src/items_zone2.rs` | `fen_staff`, `hag_focus` |
| `crates/woc-content/src/mobs.rs` | Loot arrays, `crypt_warden` |
| `crates/woc-content/src/mobs_zone2.rs` | `bog_wisp`, `barrow_hag` multi-roll loot |
| `crates/woc-content/src/lib.rs` | `dungeon_bosses_have_mob_templates` test |
| `crates/woc-sim/src/combat.rs` | Independent `spawn_mob_loot` + tests |
| `crates/woc-sim/src/stats.rs` | `pendant_raises_hp_max` test |

## Concerns

None. Need/Greed unchanged; extra piles are separate loot entities as intended.

---

## Review fix: instance-tag every independent loot pile

### Status

**Complete.** Kill-reward path now tags and rolls every pile spawned by `spawn_mob_loot`, not only the first.

### Commit

- `fix(sim): tag and roll every independent loot pile` (pending hash after commit)

### TDD Evidence

**Red (pre-fix):** `instance_independent_loot_tags_all_piles` would fail — second `hag_focus` pile had `instance_id == None` while first had the killer's instance.

**Green:**

```
cargo test -p woc-sim independent_loot_can_drop_two_items -- --nocapture  → ok
cargo test -p woc-sim crypt_warden_drops_cleaver -- --nocapture           → ok
cargo test -p woc-sim instance_independent_loot_tags_all_piles -- --nocapture → ok
cargo test -p woc-sim --lib                                               → 226 passed; 0 failed
```

### Change

`sim.rs` kill-reward: record `LootPile` ids before `spawn_mob_loot`, then for each new pile copy killer `InstanceAt` and call `maybe_start_party_roll`.
