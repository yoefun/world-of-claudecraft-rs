# Quest Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the playable quest loop: accept only from the giver with prerequisites, credit kill/collect/talk, toast when ready, turn in only at the turn-in NPC, and drive client **E** / log / map from one offer helper.

**Architecture:** Keep `QuestLog` as a player column in `woc-sim`. Add `QuestDef.requires` in `woc-content`. Validate giver / turn-in / prerequisite in `quests.rs`. Export `npc_quest_offers` for Talk toasts, Bevy **E**, and map markers. No protocol rev bump. No new ECS column.

**Tech Stack:** Rust 2021 workspace (`woc-content`, `woc-sim`, `woc-client`), existing `InteractAction` / `SimEvent` / `QuestLogEntry`, Bevy HUD text. Tests: `cargo test -p woc-content -p woc-sim` and `cargo check -p woc-client`.

**Design:** [`docs/superpowers/specs/2026-08-13-quest-loop-design.md`](../specs/2026-08-13-quest-loop-design.md)

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- Client never decides combat/loot/quest/vendor/talent outcomes — intents/actions only.
- Protocol stays at rev **6**. Do not add `QuestReady` or `AbandonQuest`.
- English-only strings.
- New per-actor state is a `World` component column (`AGENTS.md`). This feature does **not** add a column; quest state stays on `QuestLog`.
- Do not add explore/escort/daily/repeatable/abandon/share.
- Before claiming done: `cargo test --workspace --exclude woc-client` + `cargo check -p woc-client`.

## File map

| File | Responsibility |
| --- | --- |
| `crates/woc-content/src/quests.rs` | `QuestDef.requires`; zone1 chains |
| `crates/woc-content/src/quests_zone2.rs` | Eastfen/Mirefen `requires` |
| `crates/woc-content/src/quests_zone3.rs` | Thornpeak `requires` |
| `crates/woc-content/src/lib.rs` | Integrity tests for `requires` |
| `crates/woc-sim/src/quests.rs` | Offer helper, accept/turn-in gates, ready toast, unit tests |
| `crates/woc-sim/src/interaction.rs` | Pass NPC template into accept/turn-in; Talk uses offers |
| `crates/woc-sim/src/sim.rs` | Update `wolf_quest_accept_kill_turnin` to finish breadcrumb first |
| `crates/woc-client/src/input.rs` | Generic **E** accept/turn-in from `npc_quest_offers` |
| `crates/woc-client/src/hud.rs` | Log/tracker names + objective counts |
| `crates/woc-client/src/map.rs` | Yellow/green from offers |
| `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`, `CHANGELOG.md`, `VERSION.toml`, `crates/woc-version/src/lib.rs`, `README.md`, `UPSTREAM.md`, root `Cargo.toml` | `1.4.0` / `quest-loop` |

---

### Task 1: Content `requires` field and chains

**Files:**
- Modify: `crates/woc-content/src/quests.rs`
- Modify: `crates/woc-content/src/quests_zone2.rs`
- Modify: `crates/woc-content/src/quests_zone3.rs`
- Modify: `crates/woc-content/src/lib.rs` (tests)

**Interfaces:**
- Consumes: existing `QuestDef` literals
- Produces: `QuestDef.requires: Option<&'static str>` on every row (table in the spec §5.1)

- [ ] **Step 1: Write the failing integrity tests**

In `crates/woc-content/src/lib.rs` inside `mod tests`, add:

```rust
    #[test]
    fn every_quest_requires_exists_and_is_acyclic() {
        for q in QUESTS.iter() {
            let Some(req) = q.requires else {
                continue;
            };
            assert!(
                QUESTS.iter().any(|o| o.id == req),
                "quest {} requires missing {req}",
                q.id
            );
            let mut seen = vec![q.id];
            let mut cursor = q.requires;
            while let Some(id) = cursor {
                assert!(
                    !seen.contains(&id),
                    "quest {} has a requires cycle at {id}",
                    q.id
                );
                seen.push(id);
                cursor = quest(id).and_then(|d| d.requires);
            }
        }
    }

    #[test]
    fn eastbrook_quest_chain_is_report_wolves_tusks() {
        assert_eq!(quest("report_to_alden").unwrap().requires, None);
        assert_eq!(
            quest("wolves_at_the_gate").unwrap().requires,
            Some("report_to_alden")
        );
        assert_eq!(
            quest("boar_tusks").unwrap().requires,
            Some("wolves_at_the_gate")
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content --lib every_quest_requires_exists_and_is_acyclic eastbrook_quest_chain_is_report_wolves_tusks`

Expected: FAIL compile (`no field requires` on `QuestDef`) or FAIL assert if the field exists but is still `None`.

- [ ] **Step 3: Add the field and author chains**

In `crates/woc-content/src/quests.rs`, add `requires` after `turn_in_npc`:

```rust
pub struct QuestDef {
    pub id: &'static str,
    pub name: &'static str,
    pub giver_npc: &'static str,
    pub turn_in_npc: Option<&'static str>,
    pub requires: Option<&'static str>,
    pub blurb: &'static str,
    pub objectives: &'static [QuestObjective],
    pub reward: QuestReward,
}
```

Zone1 literals:

```rust
    QuestDef {
        id: "wolves_at_the_gate",
        name: "Wolves at the Gate",
        giver_npc: "captain_alden",
        turn_in_npc: Some("captain_alden"),
        requires: Some("report_to_alden"),
        blurb: "Slay young wolves north of town.",
        // objectives + reward unchanged
    },
    QuestDef {
        id: "boar_tusks",
        // ...
        requires: Some("wolves_at_the_gate"),
        // ...
    },
    QuestDef {
        id: "report_to_alden",
        // ...
        requires: None,
        // ...
    },
```

Zone2 (`quests_zone2.rs`) — insert `requires:` on every `QuestDef`:

| id | requires |
| --- | --- |
| `report_to_selene` | `None` |
| `crawler_cull` | `Some("report_to_selene")` |
| `toad_bile_harvest` | `None` |
| `wisps_in_the_mist` | `Some("crawler_cull")` |
| `silk_for_bandages` | `None` |
| `ember_offering` | `None` |
| `into_mirefen` | `Some("wisps_in_the_mist")` |
| `leeches_at_the_landing` | `Some("into_mirefen")` |
| `spores_for_the_ferryman` | `None` |
| `terror_beneath_the_reeds` | `Some("leeches_at_the_landing")` |

Zone3 (`quests_zone3.rs`):

| id | requires |
| --- | --- |
| `stalkers_on_the_ridge` | `None` |
| `tusks_for_highwatch` | `None` |
| `harpies_over_highwatch` | `Some("stalkers_on_the_ridge")` |

Every existing `QuestDef {` must set `requires`. Missing field = compile error.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-content --lib`

Expected: PASS (including the two new tests and `every_quest_npc_exists` / `every_quest_objective_refs_exist`).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/quests.rs crates/woc-content/src/quests_zone2.rs crates/woc-content/src/quests_zone3.rs crates/woc-content/src/lib.rs
git commit -m "feat(content): add quest prerequisite chains"
```

---

### Task 2: Sim offer helper and accept/turn-in gates

**Files:**
- Modify: `crates/woc-sim/src/quests.rs`
- Modify: `crates/woc-sim/src/interaction.rs`
- Modify: `crates/woc-sim/src/sim.rs` (`wolf_quest_accept_kill_turnin`)

**Interfaces:**
- Consumes: `QuestDef.requires`; `QuestLog` column; `woc_protocol::QuestLogEntry`
- Produces:
  - `pub struct NpcQuestOffers { pub accept: Vec<&'static woc_content::QuestDef>, pub turn_in: Vec<&'static woc_content::QuestDef> }`
  - `pub fn npc_quest_offers(npc_template_id: &str, log: &[QuestLogEntry]) -> NpcQuestOffers`
  - `pub fn accept_quest(world, player_id, quest_id, giver_template_id, events) -> bool`
  - `pub fn turn_in_quest(world, player_id, quest_id, turner_template_id, events) -> bool`
  - `pub fn turn_in_npc_id(def: &QuestDef) -> &str` → `def.turn_in_npc.unwrap_or(def.giver_npc)`

- [ ] **Step 1: Write the failing tests**

Append to `crates/woc-sim/src/quests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Identity, Transform};
    use crate::sim::Sim;
    use woc_content::PlayerClass;
    use woc_protocol::{InteractAction, QuestLogEntry, SimEvent};

    fn find_template(sim: &Sim, template: &str) -> EntityId {
        sim.world
            .live_ids()
            .find(|&id| {
                sim.world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some(template)
            })
            .expect(template)
    }

    fn place_at_template(sim: &mut Sim, template: &str) {
        let id = find_template(sim, template);
        let (x, z) = {
            let t = sim.world.get::<Transform>(id).unwrap();
            (t.x, t.z)
        };
        let y = crate::ecs::spawn::ground_at(x, z);
        if let Some(t) = sim.world.get_mut::<Transform>(sim.player_id) {
            t.x = x;
            t.z = z;
            t.y = y;
        }
    }

    fn log_entries(sim: &Sim) -> Vec<QuestLogEntry> {
        sim.snapshot_for_player(sim.player_id).quest_log
    }

    #[test]
    fn offers_hide_locked_and_completed() {
        let empty: Vec<QuestLogEntry> = vec![];
        let crier = npc_quest_offers("town_crier", &empty);
        assert!(crier.accept.iter().any(|q| q.id == "report_to_alden"));
        assert!(crier.turn_in.is_empty());

        let alden = npc_quest_offers("captain_alden", &empty);
        assert!(
            !alden.accept.iter().any(|q| q.id == "wolves_at_the_gate"),
            "wolves locked until report_to_alden is completed"
        );

        let after_report = vec![QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "completed".into(),
            counts: vec![1],
        }];
        let alden = npc_quest_offers("captain_alden", &after_report);
        assert!(alden.accept.iter().any(|q| q.id == "wolves_at_the_gate"));
    }

    #[test]
    fn accept_rejects_wrong_npc_and_missing_prereq() {
        let mut sim = Sim::new_eastbrook("Qgate", PlayerClass::Warrior);
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
        );
        assert!(log_entries(&sim).is_empty());

        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
        );
        assert!(log_entries(&sim).is_empty());
    }
}
```

Also add a helper in the existing `wolf_quest_accept_kill_turnin` in `sim.rs` **before** accepting wolves: complete `report_to_alden`. Write that change in Step 3 so this task’s tests and the old test land together.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib offers_hide_locked_and_completed accept_rejects_wrong_npc_and_missing_prereq`

Expected: FAIL (`npc_quest_offers` not found, and/or wolves still accept from Alden without the breadcrumb).

- [ ] **Step 3: Implement helper + gates**

In `crates/woc-sim/src/quests.rs` add (near `quests_for_npc`):

```rust
use woc_protocol::QuestLogEntry;

pub struct NpcQuestOffers {
    pub accept: Vec<&'static woc_content::QuestDef>,
    pub turn_in: Vec<&'static woc_content::QuestDef>,
}

pub fn turn_in_npc_id(def: &woc_content::QuestDef) -> &'static str {
    def.turn_in_npc.unwrap_or(def.giver_npc)
}

fn log_state<'a>(log: &'a [QuestLogEntry], quest_id: &str) -> Option<&'a str> {
    log.iter()
        .find(|e| e.quest_id == quest_id)
        .map(|e| e.state.as_str())
}

fn requires_met(def: &woc_content::QuestDef, log: &[QuestLogEntry]) -> bool {
    match def.requires {
        None => true,
        Some(id) => log_state(log, id).is_some_and(|s| s.eq_ignore_ascii_case("completed")),
    }
}

pub fn npc_quest_offers(npc_template_id: &str, log: &[QuestLogEntry]) -> NpcQuestOffers {
    let mut accept = Vec::new();
    let mut turn_in = Vec::new();
    for def in QUESTS.iter() {
        if def.giver_npc == npc_template_id
            && log_state(log, def.id).is_none()
            && requires_met(def, log)
        {
            accept.push(def);
        }
        if turn_in_npc_id(def) == npc_template_id
            && log_state(log, def.id).is_some_and(|s| s.eq_ignore_ascii_case("ready"))
        {
            turn_in.push(def);
        }
    }
    NpcQuestOffers { accept, turn_in }
}

pub fn quest_log_entries(world: &World, player_id: EntityId) -> Vec<QuestLogEntry> {
    world
        .get::<QuestLog>(player_id)
        .map(|q| {
            q.quest_log
                .iter()
                .map(|quest| QuestLogEntry {
                    quest_id: quest.quest_id.clone(),
                    state: match quest.state {
                        QuestState::Active => "active",
                        QuestState::Ready => "ready",
                        QuestState::Completed => "completed",
                    }
                    .to_string(),
                    counts: quest.counts.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}
```

Change `accept_quest` signature to take `giver_template_id: &str`. After looking up `def`:

```rust
    if giver_template_id != def.giver_npc {
        events.push(SimEvent::Toast {
            message: "That NPC does not offer this quest.".into(),
        });
        return false;
    }
    let log_entries = quest_log_entries(world, player_id);
    if let Some(state) = log_state(&log_entries, quest_id) {
        let msg = if state.eq_ignore_ascii_case("completed") {
            "You have already completed this quest."
        } else {
            "You have already accepted this quest."
        };
        events.push(SimEvent::Toast {
            message: msg.into(),
        });
        return false;
    }
    if !requires_met(def, &log_entries) {
        events.push(SimEvent::Toast {
            message: "You do not meet the requirements.".into(),
        });
        return false;
    }
```

Keep the existing `QuestLog` push + `QuestAccepted` + `Accepted: {name}` toast.

Change `turn_in_quest` to take `turner_template_id: &str`. After finding `Ready` + `def`:

```rust
    if turner_template_id != turn_in_npc_id(def) {
        events.push(SimEvent::Toast {
            message: "This NPC cannot take this quest.".into(),
        });
        return false;
    }
```

If no `Ready` row:

```rust
        events.push(SimEvent::Toast {
            message: "Nothing to turn in.".into(),
        });
        return false;
```

Update `interaction.rs`:

```rust
        InteractAction::AcceptQuest { quest_id } => {
            let template = world
                .get::<Identity>(target_id)
                .and_then(|i| i.template_id.clone());
            if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
                return;
            }
            let Some(tid) = template.as_deref() else {
                return;
            };
            if accept_quest(world, player_id, &quest_id, tid, events) {
                on_talked_to(world, player_id, tid, events);
            }
        }
        InteractAction::TurnInQuest { quest_id } => {
            if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
                return;
            }
            let Some(tid) = world
                .get::<Identity>(target_id)
                .and_then(|i| i.template_id.clone())
            else {
                return;
            };
            let _ = turn_in_quest(world, player_id, &quest_id, &tid, events);
        }
```

In `talk()`, replace the raw `quests_for_npc` name dump with:

```rust
        let offers = npc_quest_offers(&template_id, &quest_log_entries(world, player_id));
        let mut names: Vec<&str> = offers.accept.iter().map(|q| q.name).collect();
        names.extend(offers.turn_in.iter().map(|q| q.name));
        if !names.is_empty() && d.is_quest_giver {
            events.push(SimEvent::Toast {
                message: format!("Quests: {}", names.join(", ")),
            });
        }
```

Add `npc_quest_offers` and `quest_log_entries` to the `interaction.rs` import from `crate::quests`.

Update `wolf_quest_accept_kill_turnin` in `sim.rs` — before accepting wolves:

```rust
        let crier = find_template(&sim, "town_crier").unwrap();
        let (cx, cz) = {
            let t = sim.world.get::<Transform>(crier).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, cx, cz);
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        let (gx, gz) = {
            let t = sim.world.get::<Transform>(giver).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, gx, gz);
        sim.interact(giver, InteractAction::Talk);
        sim.interact(
            giver,
            InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
            },
        );
```

Then the existing wolves accept / kill / turn-in.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --lib offers_hide_locked_and_completed accept_rejects_wrong_npc_and_missing_prereq wolf_quest_accept_kill_turnin`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/quests.rs crates/woc-sim/src/interaction.rs crates/woc-sim/src/sim.rs
git commit -m "feat(sim): gate quest accept and turn-in on NPC and prerequisites"
```

---

### Task 3: Talk/collect paths, ready toast, wrong turn-in

**Files:**
- Modify: `crates/woc-sim/src/quests.rs` (`recompute_ready` + tests)

**Interfaces:**
- Consumes: Task 2 signatures
- Produces: `recompute_ready` emits `Toast { "Ready to turn in: {name}" }` on `Active` → `Ready`

- [ ] **Step 1: Write the failing tests**

In the `quests.rs` test module from Task 2, add:

```rust
    #[test]
    fn talk_quest_accept_talk_turnin() {
        let mut sim = Sim::new_eastbrook("Qtalk", PlayerClass::Warrior);
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(alden, InteractAction::Talk);
        let ready = log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "report_to_alden" && q.state == "ready");
        assert!(ready);
        sim.interact(
            alden,
            InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "report_to_alden" && q.state == "completed"));
        assert!(sim
            .snapshot_for_player(sim.player_id)
            .inventory
            .iter()
            .any(|s| s.item_id == "baked_bread"));
    }

    #[test]
    fn collect_quest_grant_ready_turnin_consumes() {
        let mut sim = Sim::new_eastbrook("Qcol", PlayerClass::Warrior);
        // complete report + wolves via log injection so this test does not fight wolves
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: "report_to_alden".into(),
                state: QuestState::Completed,
                counts: vec![1],
            });
            log.quest_log.push(QuestProgress {
                quest_id: "wolves_at_the_gate".into(),
                state: QuestState::Completed,
                counts: vec![3],
            });
        }
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "boar_tusks".into(),
            },
        );
        sim.grant_item(sim.player_id, "boar_tusk", 2).unwrap();
        let ready = log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "boar_tusks" && q.state == "ready");
        assert!(ready);
        sim.interact(
            alden,
            InteractAction::TurnInQuest {
                quest_id: "boar_tusks".into(),
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "boar_tusks" && q.state == "completed"));
        assert_eq!(
            crate::inventory::player_item_count(&sim.world, sim.player_id, "boar_tusk"),
            0
        );
    }

    #[test]
    fn turn_in_rejects_wrong_npc() {
        let mut sim = Sim::new_eastbrook("Qwrong", PlayerClass::Warrior);
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: "report_to_alden".into(),
                state: QuestState::Ready,
                counts: vec![1],
            });
        }
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "report_to_alden" && q.state == "ready"));
    }

    #[test]
    fn ready_toast_on_last_objective() {
        let mut sim = Sim::new_eastbrook("Qready", PlayerClass::Warrior);
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: "report_to_alden".into(),
                state: QuestState::Active,
                counts: vec![0],
            });
        }
        let mut events = Vec::new();
        on_talked_to(
            &mut sim.world,
            sim.player_id,
            "captain_alden",
            &mut events,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Ready to turn in: Report to Alden"
        )));
    }
```

`talk_quest_accept_talk_turnin` needs `snapshot_for_player` — `Sim` already has it (used by `vendor_buy_spend_copper`). Import `QuestProgress` / `QuestLog` / `QuestState` in the test module (already used by parent).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib talk_quest_accept_talk_turnin collect_quest_grant_ready_turnin_consumes turn_in_rejects_wrong_npc ready_toast_on_last_objective`

Expected: `ready_toast_on_last_objective` FAIL (no such toast). The others may already pass from Task 2; if `turn_in_rejects_wrong_npc` already passes, keep it.

- [ ] **Step 3: Emit ready toast in `recompute_ready`**

Replace `recompute_ready` so it reads previous state, writes new state, and toasts on the edge:

```rust
pub fn recompute_ready(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    let Some(log) = world.get_mut::<QuestLog>(player_id) else {
        return;
    };
    let mut became_ready: Vec<String> = Vec::new();
    for qp in log.quest_log.iter_mut() {
        if qp.state == QuestState::Completed {
            continue;
        }
        let Some(def) = quest(&qp.quest_id) else {
            continue;
        };
        let done = def.objectives.iter().enumerate().all(|(i, obj)| {
            let need = match obj {
                QuestObjective::Kill { count, .. } => *count,
                QuestObjective::Collect { count, .. } => *count,
                QuestObjective::Talk { .. } => 1,
            };
            qp.counts.get(i).copied().unwrap_or(0) >= need
        });
        let next = if done {
            QuestState::Ready
        } else {
            QuestState::Active
        };
        if qp.state != QuestState::Ready && next == QuestState::Ready {
            became_ready.push(def.name.to_string());
        }
        qp.state = next;
    }
    for name in became_ready {
        events.push(SimEvent::Toast {
            message: format!("Ready to turn in: {name}"),
        });
    }
}
```

Do **not** rename the `_events` parameter — it is now used.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --lib talk_quest_accept_talk_turnin collect_quest_grant_ready_turnin_consumes turn_in_rejects_wrong_npc ready_toast_on_last_objective wolf_quest_accept_kill_turnin`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/quests.rs
git commit -m "test(sim): cover talk and collect quests; toast when ready"
```

---

### Task 4: Generic client **E** interact

**Files:**
- Modify: `crates/woc-client/src/input.rs`

**Interfaces:**
- Consumes: `woc_sim::quests::npc_quest_offers`, `TickSnapshot.quest_log`, NPC `template_id`
- Produces: `pub(crate) fn quest_interact_actions(template_id: &str, log: &[QuestLogEntry]) -> Vec<InteractAction>` — `TurnInQuest` for each `offers.turn_in`, then `AcceptQuest` for each `offers.accept`

- [ ] **Step 1: Write the failing test**

In `crates/woc-client/src/input.rs` `mod tests`, add:

```rust
    use woc_protocol::QuestLogEntry;
    use super::quest_interact_actions;

    #[test]
    fn e_on_crier_accepts_report_only() {
        let actions = quest_interact_actions("town_crier", &[]);
        assert_eq!(
            actions,
            vec![InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            }]
        );
    }

    #[test]
    fn e_on_alden_turns_in_then_accepts_next() {
        let log = vec![QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "ready".into(),
            counts: vec![1],
        }];
        let actions = quest_interact_actions("captain_alden", &log);
        assert_eq!(
            actions,
            vec![InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
            }]
        );

        let log = vec![QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "completed".into(),
            counts: vec![1],
        }];
        let actions = quest_interact_actions("captain_alden", &log);
        assert_eq!(
            actions,
            vec![InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            }]
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-client --lib e_on_crier_accepts_report_only e_on_alden_turns_in_then_accepts_next`

Expected: FAIL (`quest_interact_actions` not found). If GPU/link issues, `cargo test -p woc-client --lib --offline` is still expected to compile the lib tests (this crate’s unit tests do not need a GPU).

- [ ] **Step 3: Implement helper and replace the Alden special case**

Above the interact **E** handler in `input.rs`:

```rust
use woc_protocol::QuestLogEntry;
use woc_sim::quests::npc_quest_offers;

pub(crate) fn quest_interact_actions(
    template_id: &str,
    log: &[QuestLogEntry],
) -> Vec<InteractAction> {
    let offers = npc_quest_offers(template_id, log);
    let mut out = Vec::new();
    for q in offers.turn_in {
        out.push(InteractAction::TurnInQuest {
            quest_id: q.id.to_string(),
        });
    }
    for q in offers.accept {
        out.push(InteractAction::AcceptQuest {
            quest_id: q.id.to_string(),
        });
    }
    out
}
```

Replace the block that sets `best: Option<(EntityId, f32, bool)>` and the `is_alden` accept/turn-in with:

```rust
    let mut best: Option<(EntityId, f32, Option<String>)> = None;
    for e in &host.snapshot.entities {
        if e.kind != EntityKind::Npc || !e.alive {
            continue;
        }
        let dx = e.x - player.x;
        let dz = e.z - player.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d < 5.0 && best.as_ref().map(|(_, bd, _)| d < *bd).unwrap_or(true) {
            best = Some((e.id, d, e.template_id.clone()));
        }
    }
    let Some((nid, _, template_id)) = best else {
        host.recent_toasts.push(("No NPC nearby.".into(), 2.0));
        return;
    };

    host.interact(nid, InteractAction::Talk);

    if let Some(template_id) = template_id.as_deref() {
        for action in quest_interact_actions(template_id, &host.snapshot.quest_log) {
            host.interact(nid, action);
        }
    }
```

Delete the `has_wolves` / `boar_tusks` branches.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-client --lib e_on_crier_accepts_report_only e_on_alden_turns_in_then_accepts_next first_available_talent_uses_class_and_skips_max_rank`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/input.rs
git commit -m "feat(client): E accepts and turns in any NPC quest offers"
```

---

### Task 5: Quest log, tracker, map markers

**Files:**
- Modify: `crates/woc-client/src/hud.rs`
- Modify: `crates/woc-client/src/map.rs`

**Interfaces:**
- Consumes: `woc_content::{quest, QuestObjective}`, `QuestLogEntry`
- Produces:
  - `pub(crate) fn format_quest_log_line(entry: &QuestLogEntry) -> String`
  - `npc_quest_marker` uses `npc_quest_offers` (yellow = `accept` non-empty, green = `turn_in` non-empty)

- [ ] **Step 1: Write the failing tests**

In `crates/woc-client/src/hud.rs` `mod tests`:

```rust
    use woc_protocol::QuestLogEntry;
    use super::format_quest_log_line;

    #[test]
    fn quest_log_line_uses_name_and_counts() {
        let line = format_quest_log_line(&QuestLogEntry {
            quest_id: "wolves_at_the_gate".into(),
            state: "active".into(),
            counts: vec![1],
        });
        assert!(line.contains("Wolves at the Gate"));
        assert!(line.contains("active"));
        assert!(line.contains("1/3"));
        assert!(line.contains("Young Wolves slain"));
        assert!(!line.starts_with("wolves_at_the_gate"));
    }
```

In `crates/woc-client/src/map.rs`, if there is no test module, add:

```rust
#[cfg(test)]
mod tests {
    use super::npc_quest_marker;
    use woc_protocol::{QuestLogEntry, TickSnapshot};
    use woc_sim::map_view::MapMarkerKind;

    #[test]
    fn alden_is_plain_until_report_completed() {
        let snap = TickSnapshot::default();
        assert_eq!(
            npc_quest_marker(&snap, Some("captain_alden")),
            Some(MapMarkerKind::Npc)
        );

        let mut snap = TickSnapshot::default();
        snap.quest_log.push(QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "completed".into(),
            counts: vec![1],
        });
        assert_eq!(
            npc_quest_marker(&snap, Some("captain_alden")),
            Some(MapMarkerKind::QuestAvailable)
        );
    }
}
```

`npc_quest_marker` is currently a private `fn` — keep it `fn` in the same module so the test submodule can call `super::npc_quest_marker`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-client --lib quest_log_line_uses_name_and_counts alden_is_plain_until_report_completed`

Expected: FAIL (`format_quest_log_line` missing; Alden currently yellow because wolves is “not in log”).

- [ ] **Step 3: Implement formatting and marker filter**

In `hud.rs` (add `use woc_content::{item, quest, QuestObjective, ...}` and `QuestLogEntry`):

```rust
pub(crate) fn format_quest_log_line(entry: &QuestLogEntry) -> String {
    let Some(def) = quest(&entry.quest_id) else {
        return format!("{} [{}]", entry.quest_id, entry.state);
    };
    let objs: Vec<String> = def
        .objectives
        .iter()
        .enumerate()
        .map(|(i, obj)| {
            let (label, need) = match obj {
                QuestObjective::Kill { label, count, .. } => (*label, *count),
                QuestObjective::Collect { label, count, .. } => (*label, *count),
                QuestObjective::Talk { label, .. } => (*label, 1u32),
            };
            let have = entry.counts.get(i).copied().unwrap_or(0);
            format!("{label} {have}/{need}")
        })
        .collect();
    format!("{} [{}] — {}", def.name, entry.state, objs.join("; "))
}
```

In `update_hud` quest branch:

```rust
            if snap.quest_log.is_empty() {
                **t = "Quests: (none — talk to a quest giver with E)".into();
            } else {
                let lines: Vec<String> = snap.quest_log.iter().map(format_quest_log_line).collect();
                **t = format!("Quests: {}", lines.join(" · "));
            }
        } else {
            let active = snap
                .quest_log
                .iter()
                .find(|q| q.state == "active" || q.state == "ready");
            **t = match active {
                Some(q) => format!("{} (L list)", format_quest_log_line(q)),
                None => "Quest: — (E talk · L list)".into(),
            };
        }
```

In `map.rs`, replace `npc_quest_marker` body:

```rust
fn npc_quest_marker(snap: &TickSnapshot, template_id: Option<&str>) -> Option<MapMarkerKind> {
    let template_id = template_id?;
    let def = npc(template_id)?;
    if !def.is_quest_giver {
        return None;
    }
    let offers = npc_quest_offers(template_id, &snap.quest_log);
    if !offers.turn_in.is_empty() {
        return Some(MapMarkerKind::QuestReady);
    }
    if !offers.accept.is_empty() {
        Some(MapMarkerKind::QuestAvailable)
    } else {
        Some(MapMarkerKind::Npc)
    }
}
```

Switch the import from `woc_sim::quests::quests_for_npc` to `woc_sim::quests::npc_quest_offers`. Drop unused `quest` import if nothing else in `map.rs` needs it (markers still use `npc`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-client --lib quest_log_line_uses_name_and_counts alden_is_plain_until_report_completed`

Expected: PASS.

Run: `cargo check -p woc-client`

Expected: finished `Ok`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/hud.rs crates/woc-client/src/map.rs
git commit -m "feat(client): quest log names, objective counts, offer-aware map markers"
```

---

### Task 6: Version, roadmap, demo, changelog

**Files:**
- Modify: `VERSION.toml`
- Modify: `Cargo.toml` (`workspace.package.version`)
- Modify: `crates/woc-version/src/lib.rs` (`REWRITE_VERSION`, `PARITY_TARGET`)
- Modify: `docs/ROADMAP.md`
- Modify: `docs/parity/STATUS.md`
- Modify: `docs/parity/DEMO.md`
- Modify: `CHANGELOG.md`
- Modify: `README.md` (badge + “What works” heading)
- Modify: `UPSTREAM.md` (rewrite version row)

**Interfaces:**
- Consumes: Tasks 1–5 behavior
- Produces: rewrite `1.4.0`, parity `quest-loop`

- [ ] **Step 1: Confirm workspace version field**

Set `VERSION.toml`:

```toml
rewrite_version = "1.4.0"
parity_target = "quest-loop"
```

Keep `upstream_version` / `upstream_commit` unchanged. Also set:

- `Cargo.toml` `workspace.package.version = "1.4.0"`
- `crates/woc-version/src/lib.rs`: `REWRITE_VERSION = "1.4.0"`, `PARITY_TARGET = "quest-loop"`

`woc-version` has `constants_match_version_toml`; it will fail if toml and the crate drift.

- [ ] **Step 2: Docs**

`docs/ROADMAP.md` — add a row after `1.3.0`:

```markdown
| **1.4.0** (this branch) | `quest-loop` | Accept/progress/complete gates, prerequisite chains, generic E/log/map |
```

`docs/parity/STATUS.md` — current rewrite line `1.4.0` / `quest-loop`. Add a gameplay-core row:

```markdown
| Quest accept / progress / turn-in loop | done | Giver/turn-in/requires gates; talk+collect tests; generic E; named log |
```

`docs/parity/DEMO.md` — footer `WoC-rs 1.4.0`. Add step:

```markdown
8. Town Crier **E** → Report to Alden → **E** Captain Alden turn-in → **E** Wolves → kill 3 → ready toast → **E** turn-in → **E** Boar Tusks.
```

`README.md` — badge `rewrite-1.4.0`; heading “What works in 1.4.0 (quest-loop)”; footer example `1.4.0`.

`UPSTREAM.md` — rewrite version row `1.4.0`.

`CHANGELOG.md` under Unreleased / Added:

```markdown
- Quest loop (`1.4.0` / `quest-loop`): giver and turn-in NPC checks, `QuestDef.requires` chains, ready toast, generic **E** accept/turn-in, quest log names + objective counts, offer-aware map markers.
```

- [ ] **Step 3: Full verification**

Run:

```bash
cargo test --workspace --exclude woc-client
cargo check -p woc-client
```

Expected: all tests PASS; client check `Ok`.

- [ ] **Step 4: Commit**

```bash
git add VERSION.toml Cargo.toml crates/woc-version/src/lib.rs docs/ROADMAP.md docs/parity/STATUS.md docs/parity/DEMO.md CHANGELOG.md README.md UPSTREAM.md
git commit -m "docs: mark 1.4.0 quest-loop accept/progress/complete"
```

---

## Self-review

**Spec coverage**

| Spec § | Task |
| --- | --- |
| 5.1 `requires` + chain table | Task 1 |
| 5.2 accept gates | Task 2 |
| 5.3 progress + ready toast | Task 3 (`recompute_ready`) |
| 5.4 turn-in gates | Task 2 + Task 3 wrong-NPC test |
| 5.5 `npc_quest_offers` | Task 2 |
| 5.6 client E / HUD / map | Tasks 4–5 |
| §7 tests | Tasks 1–5 |
| §8 non-goals | no abandon/protocol bump/new objectives |
| §9 DoD + version | Task 6 |

**Placeholder scan:** none. **Type names:** `NpcQuestOffers`, `npc_quest_offers`, `turn_in_npc_id`, `quest_log_entries`, `quest_interact_actions`, `format_quest_log_line` are consistent across tasks.
