# Quest loop — accept, progress, complete

**Status:** Approved for implementation planning (2026-08-13). Cloud-agent planning deliverable; implement from the paired plan, not from this spec alone.  
**Rewrite target:** `1.9.0` / parity `quest-loop` (after shipped `1.8.0` / `class-forms`).  
**Upstream pin:** unchanged (`0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Protocol:** stay on rev **6**. No new `InteractAction` / `SimEvent` variants.

## 1. Goal

Make the existing quest kernel a **playable loop** on every authored NPC, not only Captain Alden’s two Eastbrook quests.

A player must be able to:

1. **Accept** a quest only from its giver, only when prerequisites are complete.
2. **Progress** kill / collect / talk objectives with credit, toasts, and log counts.
3. **Ready** when every objective is met (tracker + map marker + toast).
4. **Turn in** only at the turn-in NPC, consume collect items, grant XP / copper / item, persist `Completed`.

Sim remains the authority. The client sends `AcceptQuest` / `TurnInQuest` and renders `TickSnapshot` + `woc-content` tables.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Content | `QuestDef` in `woc-content` (zone1 + Eastfen/Mirefen + Thornpeak). Objectives: `Kill`, `Collect`, `Talk`. |
| Sim | `QuestLog` column on players. `accept_quest` / `on_mob_killed` / `on_inventory_changed` / `on_talked_to` / `recompute_ready` / `turn_in_quest`. |
| Credit | Kill credit shares to party mates within 40 yd (same as XP). Collect recounts bag stacks. Talk sets count `1`. |
| Persist | `quests_json` round-trips `quest_id` / `state` / `counts`. |
| Protocol | `InteractAction::{AcceptQuest, TurnInQuest}`; snapshot `quest_log`; events `QuestAccepted` / `QuestProgress` / `QuestCompleted`. |
| Client | **L** log, tracker strip, map yellow/green markers, **E** interact. |
| Tests | `wolf_quest_accept_kill_turnin`; content integrity for giver/turn-in NPCs and objective refs. |

`QuestState`: `Active` → `Ready` → `Completed`. Completed quests stay in the log (non-repeatable).

## 3. Gaps (why this exists)

The kernel is a checklist slice. These holes make “talk to any quest NPC” fail:

1. **No giver / turn-in check.** `AcceptQuest` succeeds against any in-range NPC. `TurnInQuest` does not require `turn_in_npc`.
2. **No prerequisites.** Tables already tell a story (`report_to_alden` → wolves → tusks; `report_to_selene` → crawler cull → Mirefen). The sim ignores that.
3. **Client **E** is hardcoded** to `captain_alden` + `wolves_at_the_gate` / `boar_tusks`. Town Crier, Selene, Orla, Elara, etc. only `Talk`. `report_to_alden` is unplayable from the client.
4. **HUD shows ids**, not names or `current/required` objective text.
5. **Map “available”** is “quest id not in log”, so locked follow-ups still light yellow.
6. **Talk toast** lists every quest tied to the NPC, including completed and locked ones.
7. **Coverage:** only the kill path is integration-tested. Collect and talk have no sim test. Ready has no toast (`recompute_ready` ignores `events`).

## 4. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Snapshot offer list** | Each tick, put `available_quests` / `turn_in_quests` on `TickSnapshot` (or per nearby NPC). | Protocol rev 7; every host must understand the new fields. | Reject — client already has `woc-content` + `quest_log`. |
| **B. Client-only dialog script** | Keep sim loose; teach the client more hardcoded ids per zone. | Fast for Eastbrook; repeats the Alden bug in every zone. Client would pick which quest exists. | Reject |
| **C. Sim gates + shared offer helper (recommended)** | Validate giver / turn-in / `requires` in `woc-sim`. Export `npc_quest_offers` for client **E**, talk toasts, and map markers. HUD looks up `QuestDef` locally. | No protocol bump. One filter function. Existing wolf test must complete the breadcrumb first. | **Adopt** |

## 5. Architecture

Unchanged invariants: `QuestLog` stays a player column; no Bevy quest components; client never decides credit or rewards; mulberry32 / tick order untouched.

```
woc-content QuestDef (id, giver, turn-in, requires, objectives, reward)
        │
        ▼
woc-sim quests.rs  ── accept / credit / ready / turn-in ──► QuestLog column
        │                                                    │
        │ npc_quest_offers(npc, log)                         │ snapshot.quest_log
        ▼                                                    ▼
  interaction.rs Talk/Accept/TurnIn              Bevy HUD + map + E interact
```

New per-actor state is **not** required. Do not add a quest-dialog component, a second log, or a fat actor.

### 5.1 Content: `requires`

Add one field to `QuestDef`:

```rust
pub struct QuestDef {
    pub id: &'static str,
    pub name: &'static str,
    pub giver_npc: &'static str,
    pub turn_in_npc: Option<&'static str>,
    pub requires: Option<&'static str>, // completed quest id, or None
    pub blurb: &'static str,
    pub objectives: &'static [QuestObjective],
    pub reward: QuestReward,
}
```

`turn_in_npc = None` means turn in at `giver_npc`.

Chains (only breadcrumb / hub sequences that the blurbs already imply):

| Quest | `requires` |
| --- | --- |
| `report_to_alden` | none |
| `wolves_at_the_gate` | `report_to_alden` |
| `boar_tusks` | `wolves_at_the_gate` |
| `report_to_selene` | none |
| `crawler_cull` | `report_to_selene` |
| `toad_bile_harvest` | none (side) |
| `wisps_in_the_mist` | `crawler_cull` |
| `silk_for_bandages` | none (side) |
| `ember_offering` | none (side) |
| `into_mirefen` | `wisps_in_the_mist` |
| `leeches_at_the_landing` | `into_mirefen` |
| `spores_for_the_ferryman` | none (side) |
| `terror_beneath_the_reeds` | `leeches_at_the_landing` |
| `stalkers_on_the_ridge` | none |
| `tusks_for_highwatch` | none (side) |
| `harpies_over_highwatch` | `stalkers_on_the_ridge` |

Integrity tests: every `requires` id exists in `QUESTS`; following `requires` hits `None` without repeating an id (no cycles).

### 5.2 Sim: accept

`accept_quest(world, player_id, quest_id, giver_template_id, events) -> bool`

Fail (toast, no log change) when:

| Condition | Toast |
| --- | --- |
| Unknown `quest_id` | silent `false` (no table row) |
| `giver_template_id != def.giver_npc` | `That NPC does not offer this quest.` |
| Log has this id in `Active` or `Ready` | `You have already accepted this quest.` |
| Log has this id in `Completed` | `You have already completed this quest.` |
| `requires` is `Some(id)` and that id is missing or not `Completed` | `You do not meet the requirements.` |

Success: push `QuestProgress { Active, counts: zeros }`, `QuestAccepted`, `Accepted: {name}` toast, then `recompute_ready` (a talk-only quest whose talk NPC is the giver may become `Ready` immediately via the existing `on_talked_to` after accept — only if the giver **is** the talk target; `report_to_alden` talks to Alden, not the Crier, so it stays `Active`).

### 5.3 Sim: progress

Keep the three credit functions. No new objective kinds.

`recompute_ready` must use `events`. When a quest flips `Active` → `Ready`, emit `Toast { "Ready to turn in: {name}" }`. Do not add `QuestReady` (would bump protocol). Existing `QuestProgress` events still fire on each credit.

Selling or consuming a collect item already drops the count through `on_inventory_changed` and can move `Ready` back to `Active`. Keep that.

### 5.4 Sim: turn-in

`turn_in_quest(world, player_id, quest_id, turner_template_id, events) -> bool`

Fail when the quest is not `Ready`, or `turner_template_id` is not `def.turn_in_npc.unwrap_or(def.giver_npc)`. Toast: `This NPC cannot take this quest.` / `Nothing to turn in.`

Success path unchanged: consume `Collect` counts, mark `Completed`, copper + XP + optional item, `QuestCompleted` + `Completed: {name}`.

If the reward item does not fit, keep today’s behavior (quest still completes; `grant_item` error ignored). No mailbox fallback.

### 5.5 Shared offer helper

```rust
pub struct NpcQuestOffers {
    pub accept: Vec<&'static QuestDef>,
    pub turn_in: Vec<&'static QuestDef>,
}

pub fn npc_quest_offers(npc_template_id: &str, log: &[QuestLogEntry]) -> NpcQuestOffers
```

- `accept`: `giver_npc == npc`, not in log as active/ready/completed, `requires` completed or `None`.
- `turn_in`: log state `ready` and turn-in NPC matches.

`quests_for_npc` stays as the raw table filter (giver or turn-in). Talk toasts, **E**, and map markers must call `npc_quest_offers`, not the raw filter.

A World convenience wrapper may build `QuestLogEntry`s from the `QuestLog` column so interaction tests do not go through snapshots.

### 5.6 Client

**E** on the nearest in-range NPC (after loot/corpse): `Talk`, then `TurnInQuest` for every `offers.turn_in`, then `AcceptQuest` for every `offers.accept`. Offers are computed from the **pre-interaction** snapshot, so one **E** cannot turn in a quest that becomes ready during the same `Talk` and immediately accept the next chained quest — those take two **E** presses (turn-in first, then accept on the next). No Alden special case.

**L** log: `Name [state] — objective label current/required; …` using `quest(&id)` from content. Tracker (log closed): same line for the first `active` or `ready` entry. Unknown ids fall back to the raw id.

Map: green if `turn_in` non-empty; yellow if `accept` non-empty; otherwise plain NPC. Overhead `!` / `?` cues stay as they are (template `is_quest_giver`); do not add Bevy gameplay state.

Toasts already handle `QuestAccepted` / `QuestProgress` / `QuestCompleted`. Ready uses the new sim toast.

## 6. Error handling

- Out of range: existing `Too far away.`
- Dead player: existing early return.
- Unknown quest id: `false`, no toast (treat as a bad client).
- Collect turn-in without enough items: `take_item` fails → `false` (should not happen if `Ready` is honest; if it does, leave the quest `Ready`).

English-only strings.

## 7. Testing

Sim (`crates/woc-sim/src/quests.rs` `#[cfg(test)]` plus the existing wolf test):

1. Accept from the giver succeeds; from a different NPC fails.
2. Accept without `requires` completed fails; after completing the prereq succeeds.
3. Re-accept active or completed fails.
4. Talk path: `report_to_alden` accept at Town Crier → talk Alden → ready → turn in at Alden → completed; XP/item granted.
5. Collect path: accept `boar_tusks` (after wolves completed) → `grant_item` two `boar_tusk` → ready → turn in → tusks consumed.
6. Kill path: existing `wolf_quest_accept_kill_turnin` first completes `report_to_alden`.
7. Turn-in at the wrong NPC fails; quest stays `Ready`.
8. Ready toast appears when the last objective completes.

Content: existing NPC/objective ref tests plus `requires` existence + acyclic.

Client: pure functions `format_quest_log_line` and a small interact-offer unit test (given a log + NPC id, which actions fire). `cargo check -p woc-client` remains the GPU-free gate.

## 8. Explicit non-goals

| Skip | Why |
| --- | --- |
| Abandon / share / party quest log | Not in accept/progress/complete |
| Daily, repeatable, item-choice rewards | YAGNI |
| New objective kinds (explore, escort, use, gather-node) | Kill/collect/talk already cover authored tables |
| `min_level`, class gates | No content needs them yet |
| `QuestReady` event / protocol rev 7 | Toast + snapshot `ready` is enough |
| Full DESIGN.md quest chrome / multi-page gossip | Functional Bevy text |
| Mail overflow for reward items | Keep ignore-on-full |
| Reintroducing a fat `Entity` | `AGENTS.md` |

## 9. Definition of done (`1.9.0` / `quest-loop`)

1. Accept / turn-in are rejected unless the target NPC matches the table.
2. `requires` is enforced; the chain table in §5.1 is authored.
3. **E** on any quest-giver NPC accepts available and turns in ready quests from `npc_quest_offers`.
4. Log and tracker show names + objective `current/required`.
5. Map yellow/green respects offers (not raw table membership).
6. Tests in §7 pass: `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client`.
7. `VERSION.toml` rewrite `1.9.0`, parity `quest-loop`; `STATUS.md` / `ROADMAP.md` / `CHANGELOG.md` / `DEMO.md` mention the loop (Town Crier → Alden → wolves → tusks).

## 10. Success demo (human)

1. Eastbrook: **E** Town Crier → accept Report to Alden → **E** Captain Alden (talk) → **E** turn-in → **E** accept Wolves → kill 3 young wolves → tracker `3/3` and “Ready to turn in” → **E** turn-in → **E** accept Boar Tusks.
2. Collect 2 tusks (loot or grant) → ready → **E** turn in; tusks leave the bag.
3. Travel Eastfen: **E** Scout Darian for Report to Selene; locked cull quests do not yellow-ping until the breadcrumb is complete.
4. Relog: completed and active rows restore.
