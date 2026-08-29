# Quest depth — abandon, share, daily, escort/explore, choice rewards

**Status:** Approved for implementation (cloud-agent continuation of `quest-loop`).  
**Rewrite target:** `1.10.0` / parity `quest-depth` (after shipped `1.9.0` / `quest-loop`).  
**Upstream pin:** unchanged (`0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Protocol:** bump to rev **8**. Additive `TurnInQuest.reward_choice`; new `AbandonQuest` / `ShareQuest`; `QuestAbandoned` event.

## 1. Goal

The accept → progress → ready → turn-in loop is playable. This wave adds the remaining log verbs and objective/reward kinds that `quest-loop` explicitly deferred:

1. **Abandon** an active/ready quest (log **X**).
2. **Share** an active/ready quest with in-range party mates (log **Y**).
3. **Daily** repeatable quests that clear after a sim-tick epoch (no wall clock).
4. **Explore** and **escort** objective kinds, each with one Eastbrook demo quest.
5. **Choice rewards** at turn-in (keys **1/2/3** while targeting the turn-in NPC).

Sim remains the authority. Client never decides credit, fail, reset, or which item is granted.

## 2. Baseline (already shipped)

`QuestLog` player column; `AcceptQuest` / `TurnInQuest`; kill / collect / talk; `requires`; `npc_quest_offers`; protocol rev **7**; party kill credit within `PARTY_CREDIT_RANGE` (40 yd). Completed once-quests stay in the log forever.

## 3. Approaches considered

| Approach | Verdict |
| --- | --- |
| **A. Client-only abandon/share; fake dailies in HUD** | Reject — client would decide log membership. |
| **B. Wall-clock daily reset** | Reject — sim must not use wall clock (`post-completion` invariant). |
| **C. Sim verbs + tick epoch + two new objective kinds (recommended)** | Adopt. Protocol rev 8 for new interacts; escort is an `Escort` column, not `Owner` (pets keep `Owner`). |

## 4. Architecture

Unchanged: no fat `Entity`; no Bevy quest components; mulberry32 / tick phase names untouched. New systems hook **inside** existing phases (`apply_intents_motion`, `pet_ai`).

```
woc-content QuestDef (repeat, Explore/Escort, reward.choices)
        │
        ▼
woc-sim quests.rs  ── abandon / share / daily refresh / explore / escort / choice
        │                 QuestLog column + Escort column (NPC only)
        ▼
protocol rev 8     ── AbandonQuest, ShareQuest, TurnInQuest.reward_choice
        ▼
Bevy HUD/input     ── L+X abandon, L+Y share, 1/2/3 choice, named log lines
```

### 4.1 Content

```rust
pub enum QuestRepeat { Once, Daily }

pub enum QuestObjective {
    Kill { mob_id: &'static str, count: u32, label: &'static str },
    Collect { item_id: &'static str, count: u32, label: &'static str },
    Talk { npc_id: &'static str, label: &'static str },
    Explore { x: f32, z: f32, radius: f32, label: &'static str },
    Escort { npc_id: &'static str, dest_x: f32, dest_z: f32, radius: f32, label: &'static str },
}

pub struct QuestReward {
    pub xp: u32,
    pub copper: u32,
    pub item_id: Option<&'static str>,           // always granted on turn-in
    pub choices: &'static [&'static str],        // pick exactly one; empty = none
}

pub struct QuestDef {
    // existing fields …
    pub repeat: QuestRepeat,                     // default Once on all 1.9 quests
}
```

`DAILY_PERIOD_TICKS = 12_000` (10 minutes at 20 Hz). Epoch = `tick / DAILY_PERIOD_TICKS`.

Demo rows (do not change existing rewards/requires of 1.9 quests):

| Id | Kind | Giver | Requires | Notes |
| --- | --- | --- | --- | --- |
| `scout_north_road` | Explore `(-8, 40) r=12` | `town_crier` | `report_to_alden` | North road between square and Wolf Run |
| `wolf_patrol` | Daily kill 2 `young_wolf` | `captain_alden` | `wolves_at_the_gate` | `QuestRepeat::Daily` |
| `courier_to_the_gate` | Escort `eastbrook_courier` to `(-8, 50) r=8` | `captain_alden` | `boar_tusks` | Unlocks after tusks so Alden’s post-report **E** still only offers wolves |
| `arms_of_the_watch` | Talk `captain_alden`; choices `travelers_ration` / `spring_water` / `baked_bread` | `trader_wilkes` | `report_to_alden` | Wilkes `is_quest_giver: true` |

New NPC `eastbrook_courier` (not a world spawn; spawned on escort accept). Integrity: every `requires` / NPC / mob / item / choice id exists; explore/escort coords inside world bounds; `repeat: Daily` quests are allowed to be re-accepted after epoch rollover.

### 4.2 Protocol rev 8

```rust
AcceptQuest { quest_id: String }
TurnInQuest { quest_id: String, #[serde(default)] reward_choice: Option<u32> }
AbandonQuest { quest_id: String }
ShareQuest { quest_id: String }

QuestAbandoned { player: EntityId, quest_id: String }
```

Old `{"type":"turn_in_quest","quest_id":"x"}` still deserializes (`reward_choice: None`). `AbandonQuest` / `ShareQuest` are dispatched from `WorldHost::interact` **without** an NPC range check (same as `SummonPet`).

### 4.3 Sim: abandon

`abandon_quest(world, player_id, quest_id, events) -> bool`

- Fail (toast `Nothing to abandon.`) if missing, or state is `Completed`.
- Success: remove the log row; despawn any `Escort` for `(player, quest)`; `QuestAbandoned` + `Abandoned: {name}`. Collect items stay in the bag.

### 4.4 Sim: share

`share_quest(world, parties, player_id, quest_id, events) -> bool`

- Sharer must have the quest in `Active` or `Ready`. Else toast `You cannot share that quest.`
- No party → `You are not in a party.`
- For each other member within `PARTY_CREDIT_RANGE` (same instance): call `accept_quest` with `giver_template_id = def.giver_npc` (bypasses standing at the giver; still enforces requires / not already in log / not completed-this-epoch).
- Out of range mates: toast `{name} is too far away.`
- At least one successful accept → `true`. Recipients get the normal `QuestAccepted` path (including escort spawn).

### 4.5 Sim: daily

`QuestProgress.completed_tick: u64` (0 when never completed). Persist DTO field `completed_tick` with `#[serde(default)]`.

At the start of `tick_all` after `tick += 1`, `refresh_daily_quests(world, now_tick)` for every player: if a row is `Completed`, `def.repeat == Daily`, and `completed_tick / PERIOD != now_tick / PERIOD`, **remove** the row.

`accept_quest` still rejects a same-epoch completed daily (`You have already completed this quest.`). After refresh the id is absent, so accept works. Once-quests never refresh.

Turn-in writes `completed_tick = now_tick`.

### 4.6 Sim: explore

After player motion (phase 1), `credit_explore(world, player_id, events)`: for each `Active` explore objective, if player Transform is within `radius` of `(x, z)`, set count to 1 and emit `QuestProgress`. Then `recompute_ready`.

### 4.7 Sim: escort

New component (NPC only — **not** `Owner`):

```rust
pub struct Escort {
    pub player_id: EntityId,
    pub quest_id: String,
    pub dest_x: f32,
    pub dest_z: f32,
    pub radius: f32,
}
```

On accept of a quest with an `Escort` objective: spawn `create_npc_from_template` at the player’s pose, insert `Escort`, 80 HP, no `LootTable` (mobs do not treat it as a camp mob).

Inside phase 3 (`pet_ai`), `tick_escorts`:

- Dead escort → fail: remove Active/Ready row, despawn, toast `Escort failed: {name}.`
- Else follow the player with `step_toward` at `MOB_SPEED` when farther than 3 yd.
- If escort Transform is inside dest radius → credit count 1 + `recompute_ready`.

Abandon / successful turn-in / fail despawns matching escort entities. Relog does **not** respawn an in-progress escort (player must abandon and re-accept). YAGNI: no escort combat AI.

### 4.8 Sim: choice rewards

If `def.reward.choices` is non-empty and `reward_choice` is `None` or out of range: toast `Choose a reward.` and **do not** complete.

On success: grant `item_id` (if any) **and** `choices[reward_choice]`. Bag-full still ignores the grant (same as 1.9).

### 4.9 Client

| Key | When | Action |
| --- | --- | --- |
| **L** then **X** | quest log open | `AbandonQuest` for the first `active`/`ready` row |
| **L** then **Y** | quest log open | `ShareQuest` for that same tracked row |
| **1/2/3** | nearest in-range NPC has a ready choice turn-in; no pending loot; talents/bank closed | `TurnInQuest { reward_choice: Some(i) }` |
| **E** | choice turn-in | Talk only — do **not** auto-`TurnInQuest` when `choices` is non-empty |

HUD log lines include Explore/Escort labels and `current/required`. When a choice turn-in is available, tracker/hint shows `1 {item} · 2 {item} · 3 {item}`. Daily completed rows may appear until epoch rollover.

## 5. Error handling

English-only toasts. Unknown quest id: silent `false` (bad client). Escort template missing: accept still adds the log row but toasts `Escort NPC is missing.` and does not spawn.

## 6. Testing

Content: objective/choice refs; Wilkes `is_quest_giver`; courier exists; daily flagged.

Sim:

1. Abandon active removes the row; abandon completed fails; escort despawns.
2. Share in party + range accepts for mate; out of range / no party fails; completed cannot share.
3. Daily: complete → re-accept fails → `tick` advanced by `DAILY_PERIOD_TICKS` → refresh removes row → re-accept succeeds. Once-quests stay completed.
4. Explore: move player into radius → ready.
5. Escort: accept spawns NPC; move escort (or player, then tick) to dest → ready; kill escort → quest gone.
6. Choice: turn-in without index fails and stays Ready; with `0` grants that item.

Protocol: rev 8; old TurnIn JSON still parses; Abandon/Share roundtrip.

Client: `quest_interact_actions` omits choice turn-ins; `format_quest_log_line` covers Explore; `cargo check -p woc-client`.

## 7. Explicit non-goals

Weekly/repeatable-on-abandon-only; escort combat/leash; gossip multi-page; `QuestReady` event; mailbox overflow; min_level/class gates; protocol fields on snapshot for offers.

## 8. Definition of done (`1.10.0` / `quest-depth`)

1. Abandon / share / daily / explore / escort / choice behave as §4.
2. Demo content in §4.1 is authored and integrity-tested.
3. Protocol rev **8**; rewrite `1.10.0`; parity `quest-depth`.
4. `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client` pass.
5. STATUS / ROADMAP / CHANGELOG / DEMO / README mention the verbs and keys.
