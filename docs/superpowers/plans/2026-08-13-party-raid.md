# Party Depth + Raid Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make 5-man parties playable in the Bevy client (`1.14.0` / `party-depth`), then convert a full party into a 10-player raid of two groups (`1.15.0` / `raid`).

**Architecture:** Keep `PartyRoster` as a per-realm `Sim` field. Add verbs, tick-TTL, classic XP split, and a snapshot roster. Raid is the same `Party` row with `GroupKind::Raid`. Client only sends `WsClientMsg` and paints `TickSnapshot`.

**Tech Stack:** Rust 2021 workspace crates (`woc-protocol`, `woc-sim`, `woc-server`, `woc-client`). No new dependencies. No Bevy inside sim.

**Design spec:** `docs/superpowers/specs/2026-08-13-party-raid-design.md`

## Global Constraints

- `woc-sim` and `woc-content` MUST NOT depend on Bevy, `bevy_ecs`, wgpu, axum, or tokio.
- Client never decides membership, XP, ready-check outcome, or raid convert.
- All timers are sim ticks (`INVITE_TTL_TICKS = 600`, `READY_CHECK_TTL_TICKS = 300`). No wall clock.
- Tick fingerprint must remain `3214741777866168171u64`. Invite/ready expiry runs at the start of `tick_all` next to `refresh_daily_quests`. No new named phase.
- `PROTOCOL_REV` becomes `9` in Task 1 and stays 9 for raid.
- Upstream pin stays `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- English-only player-facing strings (exact copies from the spec).
- `PartyRoster` stays a `Sim` field. Do not add a party component column. Do not reintroduce a fat `Entity`.
- If `develop` has already used `1.14.0` for another wave, shift both tags by one. Do not reuse `1.13.0` (`gear-slots`).
- Every task ends with `cargo test --workspace --exclude woc-client` green, and `cargo check -p woc-client` green when client files change.
- Do not bump workspace `version` / `VERSION.toml` until the matching implementation wave is ready to tag.

---

## File map (create / own)

| Path | Responsibility |
| --- | --- |
| `crates/woc-protocol/src/lib.rs` | Rev 9; roster snapshot types; new `WsClientMsg` variants |
| `crates/woc-sim/src/social/party.rs` | Verbs, TTL, `GroupKind`, `group_xp`, raid convert |
| `crates/woc-sim/src/social/mod.rs` | Re-exports (`group_xp`, `MAX_RAID_SIZE`, `GroupKind`) |
| `crates/woc-sim/src/social/chat.rs` | `raid` channel; raid `party` = subgroup (1.15.0) |
| `crates/woc-sim/src/sim.rs` | `party_*` API, expire hooks, snapshot fill, XP split, `MAX_REALM_PLAYERS` |
| `crates/woc-server/src/game_ws.rs` | Route new `WsClientMsg` like existing party verbs |
| `crates/woc-client/src/main.rs` | `GameHost::send_party` |
| `crates/woc-client/src/input.rs` | Click-target players; G/O/P/panel keys |
| `crates/woc-client/src/hud.rs` | Frames + party panel |
| `crates/woc-client/src/world_setup.rs` | Spawn frame/panel nodes |
| `crates/woc-sim/src/map_view.rs` | `MapMarkerKind::Party` paint |
| `crates/woc-client/src/map.rs` | Party blips from snapshot roster |
| `docs/parity/{STATUS,DEMO}.md`, `docs/ROADMAP.md`, `CHANGELOG.md`, `README.md` | Version rows |

---

### Task 1: Protocol rev 9 roster + verbs

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`

**Interfaces:**
- Consumes: existing `TickSnapshot`, `WsClientMsg`, `WsServerMsg`
- Produces: `PROTOCOL_REV = 9`, `PartyMemberSnapshot`, `ReadyCheckSnapshot`, new client variants listed below

- [ ] **Step 1: Write the failing protocol tests**

In `crates/woc-protocol/src/lib.rs` tests, change both `assert_eq!(PROTOCOL_REV, 8)` to `9`, extend `party_chat_ws_msg_roundtrip` with the new variants, and add:

```rust
#[test]
fn party_roster_snapshot_defaults() {
    let snap: TickSnapshot = serde_json::from_str(
        r#"{"tick":0,"player_id":1,"entities":[],"progress":{"xp":0,"xp_to_level":0,"level":1,"copper":0},"target_id":null,"ability_ready":false,"ability_cooldown":0.0}"#,
    )
    .unwrap();
    assert!(snap.party_members.is_empty());
    assert!(snap.pending_invite_from.is_empty());
    assert!(snap.party_kind.is_empty());
    assert!(snap.party_leader_id.is_none());
    assert!(snap.ready_check.is_none());
    assert_eq!(PROTOCOL_REV, 9);
}

#[test]
fn party_depth_ws_msg_roundtrip() {
    let msgs = vec![
        WsClientMsg::PartyDecline,
        WsClientMsg::PartyKick { name: "Bob".into() },
        WsClientMsg::PartyPromote { name: "Bob".into() },
        WsClientMsg::PartyDisband,
        WsClientMsg::PartyReadyCheck,
        WsClientMsg::PartyReadyRespond { ready: true },
        WsClientMsg::ConvertToRaid,
        WsClientMsg::ConvertToParty,
    ];
    for msg in msgs {
        let s = serde_json::to_string(&msg).unwrap();
        let back: WsClientMsg = serde_json::from_str(&s).unwrap();
        assert_eq!(format!("{back:?}"), format!("{msg:?}"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-protocol party_roster_snapshot_defaults party_depth_ws_msg_roundtrip --offline`

Expected: FAIL compiling (`PartyDecline` not found) or `PROTOCOL_REV` still 8.

- [ ] **Step 3: Implement protocol types**

Set `PROTOCOL_REV` to `9` and add a rev-9 comment after the rev-8 line:

```rust
/// Rev 9: party roster snapshot + kick/promote/disband/ready/raid convert verbs.
pub const PROTOCOL_REV: u32 = 9;
```

Add after `PendingLootSnapshot`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PartyMemberSnapshot {
    pub id: EntityId,
    pub name: String,
    #[serde(default)]
    pub class_id: String,
    #[serde(default)]
    pub hp: f32,
    #[serde(default)]
    pub hp_max: f32,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub raid_group: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ReadyCheckSnapshot {
    pub expires_tick: u64,
    #[serde(default)]
    pub you_responded: bool,
    #[serde(default)]
    pub ready_count: u32,
    #[serde(default)]
    pub total: u32,
}
```

On `TickSnapshot` (all `#[serde(default)]`), after `party_id`:

```rust
    #[serde(default)]
    pub party_leader_id: Option<EntityId>,
    #[serde(default)]
    pub party_kind: String,
    #[serde(default)]
    pub party_members: Vec<PartyMemberSnapshot>,
    #[serde(default)]
    pub pending_invite_from: String,
    #[serde(default)]
    pub ready_check: Option<ReadyCheckSnapshot>,
```

Add the same fields to `TickSnapshot::default` (`party_leader_id: None`, `party_kind: String::new()`, `party_members: Vec::new()`, `pending_invite_from: String::new()`, `ready_check: None`).

Add the same fields to the `tick_snapshot_death_aura_party_roundtrip` literal (empty/None defaults) and assert `back.party_members.is_empty()`.

On `WsClientMsg`, after `PartyLeave`:

```rust
    PartyDecline,
    PartyKick { name: String },
    PartyPromote { name: String },
    PartyDisband,
    PartyReadyCheck,
    PartyReadyRespond { ready: bool },
    ConvertToRaid,
    ConvertToParty,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-protocol --offline`

Expected: PASS. Workspace sim tests will fail to compile until Task 4 fills the new `TickSnapshot` fields — that is Task 4. For this task, `cargo test -p woc-protocol` is the gate. If `cargo test --workspace --exclude woc-client` fails only on missing struct fields in `sim.rs` / client HUD tests, proceed to Task 4 in the same sitting after committing protocol if you prefer one commit per task: **do not commit a red workspace**. Fill `TickSnapshot` literals in `sim.rs` and `crates/woc-client/src/hud.rs` tests with the Default-matching empty fields as a mechanical compile fix in this task (no behavior).

Search: `rg "TickSnapshot \{" crates` and add the five fields to every struct literal.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-protocol/src/lib.rs crates/woc-sim/src/sim.rs crates/woc-client/src/hud.rs
git commit -m "feat(protocol): rev 9 party roster snapshot and group verbs"
```

---

### Task 2: PartyRoster verbs (decline, kick, promote, disband, TTL, ready)

**Files:**
- Modify: `crates/woc-sim/src/social/party.rs`
- Modify: `crates/woc-sim/src/social/mod.rs` (re-export `group_xp` in Task 3; this task re-exports nothing new except keep `PartyRoster` public)

**Interfaces:**
- Consumes: Task 1 types (not required inside `party.rs` yet)
- Produces: `invite(..., now_tick: u64)`, `decline`, `kick`, `promote`, `disband`, `ready_check`, `ready_respond`, `expire_invites`, `expire_ready_check`, `INVITE_TTL_TICKS`, `READY_CHECK_TTL_TICKS`, `GroupKind`, `PendingInvite`, `ReadyCheck`

- [ ] **Step 1: Write the failing tests**

Append to `party.rs` tests (reuse `world_with_players` / `form_party`; change `form_party` to `roster.invite(a, &name, world, 0)`):

```rust
    #[test]
    fn decline_clears_pending_and_notifies() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        let _ = roster.invite(1, "Bob", &world, 0);
        let effects = roster.decline(2);
        assert!(effects.iter().any(|e| matches!(
            e,
            PartyEffect::Notice { message } if message == "Bob declined the invite."
        )));
        let effects = roster.accept(2, &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "You have no pending party invite."));
    }

    #[test]
    fn invite_expires_after_ttl() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        let _ = roster.invite(1, "Bob", &world, 10);
        roster.expire_invites(10 + INVITE_TTL_TICKS);
        let effects = roster.accept(2, &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "You have no pending party invite."));
    }

    #[test]
    fn kick_removes_member_leader_only() {
        let world = world_with_players(3);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let _ = roster.invite(1, "Carol", &world, 0);
        let _ = roster.accept(3, &world);
        let effects = roster.kick(2, "Carol", &world);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "You are not the party leader."));
        let effects = roster.kick(1, "Carol", &world);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Carol was removed from the party.")));
        assert_eq!(roster.members_of(1).unwrap().len(), 2);
        assert!(roster.party_id(3).is_none());
    }

    #[test]
    fn promote_transfers_leader() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let effects = roster.promote(1, "Bob", &world);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Bob is now the leader.")));
        assert_eq!(roster.leader_of(1), Some(2));
        assert!(roster.set_loot_mode(1, super::loot::LootMode::NeedGreed) == false);
        assert!(roster.set_loot_mode(2, super::loot::LootMode::NeedGreed));
    }

    #[test]
    fn disband_clears_all() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let effects = roster.disband(1);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Update { members } if members.is_empty())));
        assert!(roster.party_id(1).is_none());
        assert!(roster.party_id(2).is_none());
    }

    #[test]
    fn ready_check_all_ready() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let _ = roster.ready_check(1, 0);
        let _ = roster.ready_respond(1, true, &world, &[1, 2]);
        let effects = roster.ready_respond(2, true, &world, &[1, 2]);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Everyone is ready.")));
    }
```

Update every existing `roster.invite(` call in this file to pass `0` as `now_tick`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib social::party --offline`

Expected: FAIL compiling (`decline` not found or invite arity).

- [ ] **Step 3: Implement**

Replace the pending map and `Party` / `invite` as specified. Locked implementation:

```rust
pub const INVITE_TTL_TICKS: u64 = 600;
pub const READY_CHECK_TTL_TICKS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Party,
    Raid,
}

pub struct PendingInvite {
    pub inviter: EntityId,
    pub expires_tick: u64,
}

pub struct ReadyCheck {
    pub party_id: u32,
    pub expires_tick: u64,
    pub responses: HashMap<EntityId, bool>,
}

// Party gains:
//   pub kind: GroupKind,
//   pub raid_groups: [Vec<EntityId>; 2],
// form_new_party sets kind: GroupKind::Party, raid_groups: [vec![], vec![]]

// pending: HashMap<EntityId, PendingInvite>
// ready: Option<ReadyCheck>
```

`invite` writes `PendingInvite { inviter, expires_tick: now_tick.saturating_add(INVITE_TTL_TICKS) }`. Keep existing error strings. If `pending` already has this invitee from a different inviter, replace and still emit the invited notice.

```rust
    pub fn expire_invites(&mut self, now_tick: u64) {
        self.pending.retain(|_, p| p.expires_tick > now_tick);
    }

    pub fn decline(&mut self, invitee: EntityId) -> Vec<PartyEffect> {
        let Some(pending) = self.pending.remove(&invitee) else {
            return vec![PartyEffect::Error {
                message: "You have no pending party invite.".into(),
            }];
        };
        vec![PartyEffect::Notice {
            message: format!(
                "{} declined the invite.",
                /* invitee name is not in roster; caller should pass world — use a stored name or generic */
                "The player"
            ),
        }]
    }
```

**Do not ship `"The player"`.** `decline` must take `world: &World` and use the existing `player_name` helper:

```rust
    pub fn decline(&mut self, invitee: EntityId, world: &World) -> Vec<PartyEffect> {
        let Some(_pending) = self.pending.remove(&invitee) else {
            return vec![PartyEffect::Error {
                message: "You have no pending party invite.".into(),
            }];
        };
        let name = player_name(world, invitee).unwrap_or_else(|| "Someone".into());
        vec![PartyEffect::Notice {
            message: format!("{name} declined the invite."),
        }]
    }
```

Tests call `roster.decline(2, &world)`. Update the test in Step 1 accordingly if you wrote `decline(2)` — **use `decline(invitee, world)`**.

```rust
    pub fn leader_of(&self, player: EntityId) -> Option<EntityId> {
        let pid = self.party_id(player)?;
        self.parties.get(&pid).map(|p| p.leader)
    }

    pub fn kick(&mut self, leader: EntityId, name: &str, world: &World) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(target) = find_player_by_name(world, name) else {
            return vec![PartyEffect::Error {
                message: format!("No player named '{name}'."),
            }];
        };
        if target == leader {
            return vec![PartyEffect::Error {
                message: "You cannot kick yourself.".into(),
            }];
        }
        if self.party_id(target) != self.party_id(leader) {
            return vec![PartyEffect::Error {
                message: "That player is not in your party.".into(),
            }];
        }
        let mut effects = self.leave(target);
        effects.insert(
            0,
            PartyEffect::Notice {
                message: format!("{name} was removed from the party."),
            },
        );
        effects
    }

    pub fn promote(&mut self, leader: EntityId, name: &str, world: &World) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(target) = find_player_by_name(world, name) else {
            return vec![PartyEffect::Error {
                message: format!("No player named '{name}'."),
            }];
        };
        if self.party_id(target) != Some(self.party_id(leader).unwrap_or(0)) {
            return vec![PartyEffect::Error {
                message: "That player is not in your party.".into(),
            }];
        }
        let pid = self.party_id(leader).unwrap();
        if let Some(party) = self.parties.get_mut(&pid) {
            party.leader = target;
        }
        vec![PartyEffect::Notice {
            message: format!("{name} is now the leader."),
        }]
    }

    pub fn disband(&mut self, leader: EntityId) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        if let Some(party) = self.parties.remove(&pid) {
            for m in party.members {
                self.membership.remove(&m);
            }
        }
        self.loot_modes.remove(&pid);
        if self.ready.as_ref().is_some_and(|r| r.party_id == pid) {
            self.ready = None;
        }
        vec![PartyEffect::Update {
            members: Vec::new(),
        }]
    }

    pub fn ready_check(&mut self, leader: EntityId, now_tick: u64) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        if self.ready.is_some() {
            return vec![PartyEffect::Error {
                message: "A ready check is already running.".into(),
            }];
        }
        self.ready = Some(ReadyCheck {
            party_id: pid,
            expires_tick: now_tick.saturating_add(READY_CHECK_TTL_TICKS),
            responses: HashMap::new(),
        });
        vec![PartyEffect::Notice {
            message: "Ready check started.".into(),
        }]
    }

    pub fn ready_respond(
        &mut self,
        player: EntityId,
        ready: bool,
        world: &World,
        connected: &[EntityId],
    ) -> Vec<PartyEffect> {
        let Some(pid) = self.party_id(player) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        let Some(check) = self.ready.as_mut() else {
            return vec![PartyEffect::Error {
                message: "There is no ready check.".into(),
            }];
        };
        if check.party_id != pid {
            return vec![PartyEffect::Error {
                message: "There is no ready check.".into(),
            }];
        }
        check.responses.insert(player, ready);
        let members = self.members_of(player).unwrap_or_default();
        let waiting = connected.iter().any(|m| members.contains(m) && !check.responses.contains_key(m));
        if !waiting {
            return self.finish_ready_check(world);
        }
        Vec::new()
    }

    fn finish_ready_check(&mut self) -> Vec<PartyEffect> {
        let Some(check) = self.ready.take() else {
            return Vec::new();
        };
        let Some(party) = self.parties.get(&check.party_id) else {
            return Vec::new();
        };
        let mut yes: Vec<EntityId> = Vec::new();
        let mut no: Vec<EntityId> = Vec::new();
        for m in &party.members {
            if check.responses.get(m).copied().unwrap_or(false) {
                yes.push(*m);
            } else {
                no.push(*m);
            }
        }
        if no.is_empty() {
            return vec![PartyEffect::Notice {
                message: "Everyone is ready.".into(),
            }];
        }
        // Names filled by a helper that looks up Identity — finish_ready_check
        // cannot see World. Keep ids out of the toast: store names at respond
        // time OR take world here.
        vec![PartyEffect::Notice {
            message: "Ready check complete.".into(),
        }]
    }
```

**Do not ship `"Ready check complete."` as the mixed summary.** Change `ready_respond` / `finish_ready_check` / `expire_ready_check` to take `world: &World` and format:

```rust
fn names(world: &World, ids: &[EntityId]) -> String {
    ids.iter()
        .filter_map(|id| player_name(world, *id))
        .collect::<Vec<_>>()
        .join(", ")
}
// Not ready toast:
format!(
    "Ready: {}. Not ready: {}.",
    names(world, &yes),
    names(world, &no)
)
```

Empty `Ready:` list is allowed (`Ready: . Not ready: Alice.` is wrong). If `yes` is empty use `Ready: none. Not ready: Alice.` — **spec says `Ready: {names}. Not ready: {names}.`**. Use empty string for names when a side is empty: `Ready: . Not ready: Alice.` is ugly. Lock this: if yes is empty, the string is `Ready: . Not ready: Alice.` **No.** Use `none` for an empty side so tests can lock:

`Ready: Alice. Not ready: Bob.` when mixed.  
`Everyone is ready.` when `no.is_empty()`.  
When TTL fires with no responses: `Ready: none. Not ready: Alice, Bob.`

```rust
    pub fn expire_ready_check(&mut self, now_tick: u64, world: &World) -> Vec<PartyEffect> {
        let Some(check) = &self.ready else {
            return Vec::new();
        };
        if check.expires_tick > now_tick {
            return Vec::new();
        }
        self.finish_ready_check(world)
    }
```

Add `kind: GroupKind::Party` and `raid_groups: [Vec::new(), Vec::new()]` to every `Party { ... }` construction (`form_new_party`).

Update `chat.rs` tests and `instances/mod.rs` / `quests.rs` tests that call `invite` to pass `0`.

Search: `rg "\.invite\(" crates/woc-sim`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --lib social::party --offline`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/social/party.rs crates/woc-sim/src/social/chat.rs crates/woc-sim/src/instances/mod.rs crates/woc-sim/src/quests.rs
git commit -m "feat(sim): party kick, promote, disband, invite TTL, ready check"
```

---

### Task 3: Classic-era `group_xp`

**Files:**
- Modify: `crates/woc-sim/src/social/party.rs` (`group_xp`)
- Modify: `crates/woc-sim/src/social/mod.rs`
- Modify: `crates/woc-sim/src/sim.rs` (kill_rewards loop)

**Interfaces:**
- Consumes: `kill_credit_share`, `collect_pending_mob_kills`
- Produces: `pub fn group_xp(mob_xp: u32, n: usize) -> u32`

- [ ] **Step 1: Write the failing test**

In `party.rs` tests:

```rust
    #[test]
    fn group_xp_classic_table() {
        assert_eq!(group_xp(100, 1), 100);
        assert_eq!(group_xp(100, 2), 75);
        assert_eq!(group_xp(100, 3), 66);
        assert_eq!(group_xp(100, 4), 62);
        assert_eq!(group_xp(100, 5), 60);
        assert_eq!(group_xp(100, 10), 30);
        assert_eq!(group_xp(50, 2), 37); // 50 * 15 / 20
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim --lib social::party::tests::group_xp_classic_table --offline`

Expected: FAIL compiling (`group_xp` not found).

- [ ] **Step 3: Implement**

```rust
pub fn group_xp(mob_xp: u32, n: usize) -> u32 {
    let n = n.clamp(1, 10);
    let bonus_n = n.min(5);
    let bonus_tenths: u64 = 10 + 5 * (bonus_n as u64 - 1);
    (mob_xp as u64 * bonus_tenths / (10 * n as u64)) as u32
}
```

In `sim.rs` kill_rewards, after building `recipients`:

```rust
            let share = crate::social::party::group_xp(reward.xp, recipients.len());
            for rid in recipients {
                if self.world.get::<Identity>(rid).map(|i| i.kind) == Some(EntityKind::Player) {
                    grant_xp(&mut self.world, rid, share, &mut self.events);
                    // on_mob_killed / on_boss_killed unchanged
```

Export from `social/mod.rs`:

```rust
pub use party::{
    group_xp, kill_credit_share, GroupKind, PartyEffect, PartyRoster, MAX_PARTY_SIZE,
    MIN_PARTY_SIZE, INVITE_TTL_TICKS, READY_CHECK_TTL_TICKS,
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim --lib social::party::tests::group_xp_classic_table --offline`

Expected: PASS. Then `cargo test -p woc-sim --offline` — existing solo wolf XP tests still pass because `n == 1`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/social/party.rs crates/woc-sim/src/social/mod.rs crates/woc-sim/src/sim.rs
git commit -m "feat(sim): split party kill XP with classic group bonus"
```

---

### Task 4: Sim API, expire hooks, snapshot roster, park keeps membership

**Files:**
- Modify: `crates/woc-sim/src/sim.rs`

**Interfaces:**
- Consumes: Task 2 methods, Task 1 snapshot fields
- Produces: `Sim::party_decline`, `party_kick`, `party_promote`, `party_disband`, `party_ready_check`, `party_ready_respond`; snapshot roster fill; `expire_*` at start of `tick_all`

- [ ] **Step 1: Write the failing tests**

In `sim.rs` tests, next to `party_invite_accept_leave_and_chat_roundtrip`:

```rust
    #[test]
    fn park_keeps_party_membership() {
        let mut sim = Sim::new(42);
        let a = sim.spawn_player("Alice", PlayerClass::Warrior).unwrap();
        let b = sim.spawn_player("Bob", PlayerClass::Mage).unwrap();
        let _ = sim.party_invite(a, "Bob");
        let _ = sim.party_accept(b);
        assert!(sim.parties.party_id(a).is_some());
        sim.park_player(b);
        assert_eq!(sim.party_members(a), Some(vec![a, b]));
        let snap = sim.snapshot_for_player(a);
        let bob = snap
            .party_members
            .iter()
            .find(|m| m.id == b)
            .expect("bob on roster");
        assert!(!bob.online);
        assert_eq!(snap.party_kind, "party");
        assert_eq!(snap.party_leader_id, Some(a));
    }

    #[test]
    fn snapshot_pending_invite_name() {
        let mut sim = Sim::new(42);
        let a = sim.spawn_player("Alice", PlayerClass::Warrior).unwrap();
        let b = sim.spawn_player("Bob", PlayerClass::Mage).unwrap();
        let _ = sim.party_invite(a, "Bob");
        let snap = sim.snapshot_for_player(b);
        assert_eq!(snap.pending_invite_from, "Alice");
    }
```

Use the real `spawn_player` signature already used in `party_invite_accept_leave_and_chat_roundtrip` (copy that test’s spawn pattern exactly; do not invent a different helper).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib sim::tests::park_keeps_party_membership --offline`

Expected: FAIL (`party_members` empty or `online` true).

- [ ] **Step 3: Implement**

After `refresh_daily_quests` in `tick_all`:

```rust
        self.parties.expire_invites(self.tick);
        let ready_effects = self.parties.expire_ready_check(self.tick, &self.world);
        self.events.extend(ready_effects.into_iter().filter_map(|e| match e {
            PartyEffect::Notice { message } => Some(woc_protocol::SimEvent::Toast { message }),
            _ => None,
        }));
```

Add methods next to `party_leave` (pass `self.tick` into `invite` / `ready_check`):

```rust
    pub fn party_decline(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.decline(player_id, &self.world))
    }
    pub fn party_kick(&mut self, player_id: EntityId, name: &str) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.kick(player_id, name, &self.world))
    }
    pub fn party_promote(&mut self, player_id: EntityId, name: &str) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.promote(player_id, name, &self.world))
    }
    pub fn party_disband(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.disband(player_id))
    }
    pub fn party_ready_check(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.ready_check(player_id, self.tick))
    }
    pub fn party_ready_respond(&mut self, player_id: EntityId, ready: bool) -> Vec<WsServerMsg> {
        let connected: Vec<EntityId> = self
            .parties
            .members_of(player_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|id| self.intents.contains_key(id))
            .collect();
        map_party_effects(self.parties.ready_respond(player_id, ready, &self.world, &connected))
    }
```

`party_invite` must pass `self.tick`:

```rust
        let effects = self.parties.invite(player_id, name, &self.world, self.tick);
```

In `snapshot_for_player`, replace `party_id: self.parties.party_id(player_id)` with:

```rust
            party_id: self.parties.party_id(player_id),
            party_leader_id: self.parties.leader_of(player_id),
            party_kind: self
                .parties
                .kind_of(player_id)
                .map(|k| match k {
                    crate::social::party::GroupKind::Party => "party".into(),
                    crate::social::party::GroupKind::Raid => "raid".into(),
                })
                .unwrap_or_default(),
            party_members: self.party_member_snapshots(player_id),
            pending_invite_from: self.parties.pending_inviter_name(player_id, &self.world),
            ready_check: self.parties.ready_snapshot(player_id, self.tick),
```

Add helpers on `PartyRoster`:

```rust
    pub fn kind_of(&self, player: EntityId) -> Option<GroupKind> {
        let pid = self.party_id(player)?;
        self.parties.get(&pid).map(|p| p.kind)
    }

    pub fn pending_inviter_name(&self, invitee: EntityId, world: &World) -> String {
        let Some(p) = self.pending.get(&invitee) else {
            return String::new();
        };
        player_name(world, p.inviter).unwrap_or_default()
    }

    pub fn raid_group_of(&self, player: EntityId) -> u8 {
        let Some(pid) = self.party_id(player) else {
            return 0;
        };
        let Some(party) = self.parties.get(&pid) else {
            return 0;
        };
        if party.kind != GroupKind::Raid {
            return 0;
        }
        if party.raid_groups[1].contains(&player) {
            1
        } else {
            0
        }
    }

    pub fn ready_snapshot(
        &self,
        player: EntityId,
        _now_tick: u64,
    ) -> Option<woc_protocol::ReadyCheckSnapshot> {
        let check = self.ready.as_ref()?;
        let pid = self.party_id(player)?;
        if check.party_id != pid {
            return None;
        }
        let total = self.members_of(player).map(|m| m.len() as u32).unwrap_or(0);
        let ready_count = check.responses.values().filter(|v| **v).count() as u32;
        Some(woc_protocol::ReadyCheckSnapshot {
            expires_tick: check.expires_tick,
            you_responded: check.responses.contains_key(&player),
            ready_count,
            total,
        })
    }
```

On `Sim`:

```rust
    fn party_member_snapshots(&self, player_id: EntityId) -> Vec<woc_protocol::PartyMemberSnapshot> {
        let Some(members) = self.parties.members_of(player_id) else {
            return Vec::new();
        };
        members
            .into_iter()
            .map(|id| {
                let ident = self.world.get::<Identity>(id);
                let hp = self.world.get::<Health>(id);
                let kit = self.world.get::<ClassKit>(id);
                woc_protocol::PartyMemberSnapshot {
                    id,
                    name: ident.map(|i| i.name.clone()).unwrap_or_default(),
                    class_id: kit
                        .map(|k| format!("{:?}", k.class).to_ascii_lowercase())
                        .unwrap_or_default(),
                    hp: hp.map(|h| h.hp).unwrap_or(0.0),
                    hp_max: hp.map(|h| h.hp_max).unwrap_or(0.0),
                    online: self.intents.contains_key(&id),
                    raid_group: self.parties.raid_group_of(id),
                }
            })
            .collect()
    }
```

**Class id:** do not `format!("{:?}", k.class)`. Read how `snapshot_for_player` already builds `class_id` (it uses the existing player class string). Copy that same conversion for each member (likely `kit.class.as_str()` or `woc_content` helper already in scope). Match local player `class_id` exactly.

Confirm `park_player` still does **not** call `on_despawn`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim --offline`

Expected: PASS. Fingerprint test still `3214741777866168171u64`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/sim.rs crates/woc-sim/src/social/party.rs
git commit -m "feat(sim): party snapshot roster, expire hooks, park-safe membership"
```

---

### Task 5: Server routes the new party verbs

**Files:**
- Modify: `crates/woc-server/src/game_ws.rs`

**Interfaces:**
- Consumes: `Sim::party_*` from Task 4, `WsClientMsg` from Task 1
- Produces: same `notices.send` pattern as `PartyInvite`

- [ ] **Step 1: Write the failing compile gate**

The crate will not compile until the match is exhaustive. Add arms. If `game_ws.rs` tests exist for party, add one that constructs `WsClientMsg::PartyKick`. Otherwise this task is compile + existing `park_keeps_player_for_resume`.

- [ ] **Step 2: Confirm compile fails without arms**

Run: `cargo test -p woc-server --offline`

Expected: FAIL if Task 1 landed and this match is non-exhaustive.

- [ ] **Step 3: Implement**

Copy the `PartyLeave` block for each new variant, calling the matching `realm.sim.party_*`. `PartyReadyRespond { ready }` passes `ready`. `ConvertToRaid` / `ConvertToParty` can call stubs that return `vec![WsServerMsg::Chat { channel: "system".into(), from: "Party".into(), text: "Not implemented.".into() }]` **only if Task 9 has not landed** — **do not stub**. Implement convert in Task 9; for this task add the match arms that call `realm.sim.convert_to_raid` / `convert_to_party` which you will add as empty-error wrappers in Task 4 **now** so the server compiles:

In `sim.rs` (if not already in Task 4):

```rust
    pub fn convert_to_raid(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.convert_to_raid(player_id))
    }
    pub fn convert_to_party(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.convert_to_party(player_id))
    }
```

In Task 2/4, `convert_to_raid` / `convert_to_party` may return `Error { message: "You need a full party of 5 to convert to a raid." }` until Task 9 fills the real logic. That is acceptable: the verb exists, tests for success land in Task 9.

Server match:

```rust
            WsClientMsg::PartyDecline => { /* party_decline */ }
            WsClientMsg::PartyKick { name } => { /* party_kick */ }
            WsClientMsg::PartyPromote { name } => { /* party_promote */ }
            WsClientMsg::PartyDisband => { /* party_disband */ }
            WsClientMsg::PartyReadyCheck => { /* party_ready_check */ }
            WsClientMsg::PartyReadyRespond { ready } => { /* party_ready_respond */ }
            WsClientMsg::ConvertToRaid => { /* convert_to_raid */ }
            WsClientMsg::ConvertToParty => { /* convert_to_party */ }
```

Each arm: lock realm, call sim, drop realm, `notices.send` JSON. Copy `PartyAccept` verbatim.

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace --exclude woc-client --offline`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-server/src/game_ws.rs crates/woc-sim/src/sim.rs crates/woc-sim/src/social/party.rs
git commit -m "feat(server): route party kick/promote/disband/ready/raid convert"
```

---

### Task 6: Bevy client — invite, prompts, frames, panel

**Files:**
- Modify: `crates/woc-client/src/main.rs`
- Modify: `crates/woc-client/src/input.rs`
- Modify: `crates/woc-client/src/hud.rs`
- Modify: `crates/woc-client/src/world_setup.rs`

**Interfaces:**
- Consumes: `TickSnapshot.party_members`, `pending_invite_from`, `ready_check`; `GameHost::send_party`
- Produces: playable 5-man keys from the spec §5.7

- [ ] **Step 1: Write the failing HUD unit test**

In `crates/woc-client/src/hud.rs` tests:

```rust
    #[test]
    fn party_frames_format_other_members() {
        let mut snap = TickSnapshot::default();
        snap.player_id = 1;
        snap.party_kind = "party".into();
        snap.party_leader_id = Some(1);
        snap.party_members = vec![
            woc_protocol::PartyMemberSnapshot {
                id: 1,
                name: "Alice".into(),
                class_id: "warrior".into(),
                hp: 100.0,
                hp_max: 100.0,
                online: true,
                raid_group: 0,
            },
            woc_protocol::PartyMemberSnapshot {
                id: 2,
                name: "Bob".into(),
                class_id: "mage".into(),
                hp: 40.0,
                hp_max: 80.0,
                online: false,
                raid_group: 0,
            },
        ];
        let text = party_frames_text(&snap);
        assert!(text.contains("Bob"));
        assert!(text.contains("40/80"));
        assert!(text.contains("AFK"));
        assert!(!text.contains("Alice"));
        let panel = party_panel_text(&snap);
        assert!(panel.contains("*"));
        assert!(panel.contains("[X] Leave"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-client party_frames_format_other_members --offline`

Expected: FAIL compiling (`party_frames_text` not found). `woc-client` tests that do not need GPU should already exist in `hud.rs`.

- [ ] **Step 3: Implement host + formatters + input + spawn**

`GameHost::send_party` in `main.rs`:

```rust
    pub(crate) fn send_party(&mut self, msg: WsClientMsg) {
        match self.play_mode {
            PlayMode::Offline => {
                if let Some(sim) = self.sim.as_mut() {
                    let pid = sim.player_id;
                    let outs = match &msg {
                        WsClientMsg::PartyInvite { name } => sim.party_invite(pid, name),
                        WsClientMsg::PartyAccept => sim.party_accept(pid),
                        WsClientMsg::PartyDecline => sim.party_decline(pid),
                        WsClientMsg::PartyLeave => sim.party_leave(pid),
                        WsClientMsg::PartyKick { name } => sim.party_kick(pid, name),
                        WsClientMsg::PartyPromote { name } => sim.party_promote(pid, name),
                        WsClientMsg::PartyDisband => sim.party_disband(pid),
                        WsClientMsg::PartyReadyCheck => sim.party_ready_check(pid),
                        WsClientMsg::PartyReadyRespond { ready } => {
                            sim.party_ready_respond(pid, *ready)
                        }
                        WsClientMsg::ConvertToRaid => sim.convert_to_raid(pid),
                        WsClientMsg::ConvertToParty => sim.convert_to_party(pid),
                        _ => Vec::new(),
                    };
                    for out in outs {
                        if let WsServerMsg::Chat { text, .. } = out {
                            self.recent_toasts.push((text, 4.0));
                        }
                    }
                }
            }
            PlayMode::Online => {
                if let Some(tx) = &self.to_net {
                    let _ = tx.send(msg);
                }
            }
        }
    }
```

`sim.player_id` is public on `Sim`. Import `WsServerMsg` in `main.rs` if needed.

HUD formatters (exact):

```rust
pub(crate) fn party_frames_text(snap: &TickSnapshot) -> String {
    let mut lines = Vec::new();
    if !snap.pending_invite_from.is_empty() {
        lines.push(format!(
            "{} invited you. O accept / P decline",
            snap.pending_invite_from
        ));
    }
    if let Some(rc) = &snap.ready_check {
        if !rc.you_responded {
            lines.push(format!(
                "Ready check {}/{}. O ready / P not ready",
                rc.ready_count, rc.total
            ));
        }
    }
    for m in &snap.party_members {
        if m.id == snap.player_id {
            continue;
        }
        let afk = if m.online { "" } else { " AFK" };
        lines.push(format!(
            "{} {} {:.0}/{:.0}{}",
            m.class_id, m.name, m.hp, m.hp_max, afk
        ));
    }
    lines.join("\n")
}

pub(crate) fn party_panel_text(snap: &TickSnapshot) -> String {
    let mut lines = vec!["Party".into()];
    for m in &snap.party_members {
        let star = if Some(m.id) == snap.party_leader_id {
            "*"
        } else {
            " "
        };
        let afk = if m.online { "" } else { " AFK" };
        lines.push(format!(
            "{star} {} {}{afk}",
            m.name, m.class_id
        ));
    }
    lines.push("[X] Leave  [Y] Promote  [-] Kick  [R] Ready  [Backspace] Disband  [=] Raid".into());
    lines.join("\n")
}
```

`UiFlags.show_party: bool` default false. Opening party closes map like bank does.

Spawn in `world_setup.rs` under the HP column: `HudPartyFrames` text (empty). Spawn a `HudPartyPanel` overlay `Visibility::Hidden` like `HudCharPanel` (right side, `left: Val::Px(12.0)`, `top: Val::Px(180.0)`, width 280). Sync visibility from `ui.show_party` in `hud.rs` update.

Input — click players. Replace the left-click mob loop with: consider `EntityKind::Player` (id != `snap.player_id`, `alive`) and `EntityKind::Mob` (`alive`). Pick nearest within 25 yd. If it is a player, set `intent.target_id` and **do not** set `intent.attack` / `local_auto_attack`. If it is a mob, keep current attack path.

Keys (after existing panel toggles; pending prompt wins):

```rust
    let pending = !host.snapshot.pending_invite_from.is_empty();
    let ready_prompt = host
        .snapshot
        .ready_check
        .as_ref()
        .is_some_and(|r| !r.you_responded);

    if pending && !ui.show_market && keys.just_pressed(KeyCode::KeyO) {
        host.send_party(WsClientMsg::PartyAccept);
    } else if ready_prompt && !ui.show_market && keys.just_pressed(KeyCode::KeyO) {
        host.send_party(WsClientMsg::PartyReadyRespond { ready: true });
    }

    if pending && !ui.show_mail && keys.just_pressed(KeyCode::KeyP) {
        host.send_party(WsClientMsg::PartyDecline);
    } else if ready_prompt && !ui.show_mail && keys.just_pressed(KeyCode::KeyP) {
        host.send_party(WsClientMsg::PartyReadyRespond { ready: false });
    } else if !pending && !ready_prompt && !ui.show_mail && keys.just_pressed(KeyCode::KeyP) {
        ui.show_party = !ui.show_party;
        if ui.show_party {
            ui.show_character = false;
            ui.show_map = false;
        }
    }

    if !ui.show_bank && keys.just_pressed(KeyCode::KeyG) {
        if let Some(tid) = host.snapshot.target_id {
            if let Some(e) = host.snapshot.entities.iter().find(|e| e.id == tid) {
                if e.kind == EntityKind::Player && e.id != host.snapshot.player_id {
                    host.send_party(WsClientMsg::PartyInvite {
                        name: e.name.clone(),
                    });
                }
            }
        }
    }

    if ui.show_party && keys.just_pressed(KeyCode::KeyX) {
        host.send_party(WsClientMsg::PartyLeave);
    }
    if ui.show_party && keys.just_pressed(KeyCode::KeyY) {
        if let Some(name) = targeted_other_member_name(&host.snapshot) {
            host.send_party(WsClientMsg::PartyPromote { name });
        }
    }
    if ui.show_party && keys.just_pressed(KeyCode::Minus) {
        if let Some(name) = targeted_other_member_name(&host.snapshot) {
            host.send_party(WsClientMsg::PartyKick { name });
        }
    }
    if ui.show_party && keys.just_pressed(KeyCode::KeyR) {
        host.send_party(WsClientMsg::PartyReadyCheck);
    }
    if ui.show_party && keys.just_pressed(KeyCode::Backspace) {
        host.send_party(WsClientMsg::PartyDisband);
    }
    if ui.show_party && keys.just_pressed(KeyCode::Equal) {
        if host.snapshot.party_kind == "raid" {
            host.send_party(WsClientMsg::ConvertToParty);
        } else {
            host.send_party(WsClientMsg::ConvertToRaid);
        }
    }
```

When `ui.show_party`, skip the hearth **R** branch (`if !ui.show_talents && !ui.show_party && KeyR`).

```rust
fn targeted_other_member_name(snap: &TickSnapshot) -> Option<String> {
    let tid = snap.target_id?;
    snap.party_members
        .iter()
        .find(|m| m.id == tid && m.id != snap.player_id)
        .map(|m| m.name.clone())
}
```

Update the help line in `world_setup.rs` to include `G invite · P party · O accept`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-client party_frames_format_other_members --offline && cargo check -p woc-client --offline && cargo test --workspace --exclude woc-client --offline`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/main.rs crates/woc-client/src/input.rs crates/woc-client/src/hud.rs crates/woc-client/src/world_setup.rs
git commit -m "feat(client): party invite keys, frames, and roster panel"
```

---

### Task 7: Minimap party blips

**Files:**
- Modify: `crates/woc-sim/src/map_view.rs`
- Modify: `crates/woc-client/src/map.rs`

**Interfaces:**
- Consumes: `TickSnapshot.party_members`
- Produces: `MapMarkerKind::Party` painted `[70, 140, 255, 255]` radius 3.5

- [ ] **Step 1: Write the failing test**

In `crates/woc-client/src/map.rs` tests (or add a small unit test next to existing marker tests). If `collect_dynamic_markers` is private, test via a `pub(crate)` helper or add the test in `map_view.rs` for the new enum arm compile.

Simplest compile+behavior test in `map.rs` `tests` module — if none can reach `collect_dynamic_markers`, add:

```rust
    #[test]
    fn party_member_uses_party_marker() {
        let mut snap = TickSnapshot::default();
        snap.player_id = 1;
        snap.party_members.push(woc_protocol::PartyMemberSnapshot {
            id: 2,
            name: "Bob".into(),
            class_id: "mage".into(),
            hp: 1.0,
            hp_max: 1.0,
            online: true,
            raid_group: 0,
        });
        snap.entities.push(EntitySnapshot {
            id: 2,
            kind: EntityKind::Player,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            hp: 1.0,
            hp_max: 1.0,
            level: 1,
            name: "Bob".into(),
            resource: 0.0,
            resource_max: 0.0,
            alive: true,
            template_id: None,
            on_ground: true,
            flying: false,
            swimming: false,
        });
        let region = woc_sim::map_view::MapRegion::around(0.0, 0.0, 50.0);
        let markers = super::collect_dynamic_markers(&snap, region, 1);
        assert!(markers.iter().any(|m| m.kind == MapMarkerKind::Party && m.label == "Bob"));
    }
```

Fill `EntitySnapshot` using the real field list from `crates/woc-protocol/src/lib.rs` (copy from an existing test if one constructs it). Make `collect_dynamic_markers` `pub(crate)` if it is private.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-client party_member_uses_party_marker --offline`

Expected: FAIL (`MapMarkerKind::Party` not found).

- [ ] **Step 3: Implement**

```rust
// map_view.rs enum:
    Party,
// paint arm:
            MapMarkerKind::Party => fill_disc(data, width, height, mx, my, 3.5, [70, 140, 255, 255]),
```

In `collect_dynamic_markers`, for `EntityKind::Player` other than self:

```rust
            EntityKind::Player => {
                let party = snap.party_members.iter().any(|m| m.id == entity.id);
                out.push(MapMarker {
                    x: entity.x,
                    z: entity.z,
                    kind: if party {
                        MapMarkerKind::Party
                    } else {
                        MapMarkerKind::Ally
                    },
                    label: entity.name.clone(),
                });
            }
```

Exhaustive matches on `MapMarkerKind` in `map.rs` legend: treat `Party` like `Ally` (no extra legend cap).

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-client party_member_uses_party_marker --offline && cargo test -p woc-sim --lib map_view --offline && cargo check -p woc-client --offline`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/map_view.rs crates/woc-client/src/map.rs
git commit -m "feat(client): blue minimap blips for party members"
```

---

### Task 8: Docs + version bump for `1.14.0`

**Files:**
- Modify: `VERSION.toml`, workspace `Cargo.toml` version, crate versions if they inherit
- Modify: `UPSTREAM.md` rewrite version + parity target
- Modify: `CHANGELOG.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`, `README.md`

**Interfaces:**
- Consumes: shipped behavior from Tasks 1–7
- Produces: rewrite `1.14.0` / `party-depth`

- [ ] **Step 1: Write the STATUS rows (fail if you skip them — this is the deliverable)**

Add a **Party depth (`party-depth`) — done** table at the top of `STATUS.md` with rows: invite G, accept/decline O/P, frames, kick/promote/disband, invite TTL, park-safe, group XP, ready check, protocol rev 9.

- [ ] **Step 2: Set versions**

`VERSION.toml`: `rewrite_version = "1.14.0"`, `parity_target = "party-depth"`.  
Workspace `package.version` = `"1.14.0"`.  
`UPSTREAM.md` rewrite version `1.14.0`, parity `party-depth`.

- [ ] **Step 3: CHANGELOG / ROADMAP / DEMO / README**

CHANGELOG new `## 1.14.0` section listing the spec DoD.

ROADMAP table: `1.14.0` (shipped) `party-depth`. Keep `1.15.0` as planned until Task 10.

DEMO: insert after step 5:

`5b. Two clients: target + **G** invite, **O** accept; party frames show HP; **P** panel **R** ready check; disconnect shows AFK; **X** leave.`

README controls: add `G invite · P party (O accept / X leave / Y promote / - kick / R ready) · party frames`.

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace --exclude woc-client --offline && cargo check -p woc-client --offline`

Expected: PASS. Any test that asserts rewrite version `1.13.0` or `PROTOCOL_REV == 8` must move to `1.14.0` / `9`.

Search: `rg "1\\.13\\.0"|rg "PROTOCOL_REV, 8"`

- [ ] **Step 5: Commit**

```bash
git add VERSION.toml Cargo.toml Cargo.lock UPSTREAM.md CHANGELOG.md docs README.md crates
git commit -m "docs: ship 1.14.0 party-depth"
```

---

### Task 9: Raid convert, cap 10, raid chat

**Files:**
- Modify: `crates/woc-sim/src/social/party.rs`
- Modify: `crates/woc-sim/src/social/chat.rs`
- Modify: `crates/woc-sim/src/sim.rs` (`MAX_REALM_PLAYERS`)
- Modify: `crates/woc-sim/src/social/mod.rs` (`MAX_RAID_SIZE`)

**Interfaces:**
- Consumes: `GroupKind`, `convert_to_*` stubs from Task 5
- Produces: real convert; `MAX_RAID_SIZE = 10`; `MAX_REALM_PLAYERS = 10`; `raid` chat channel

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn convert_full_party_to_raid_then_sixth() {
        let world = world_with_players(6);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        for other in 3..=5 {
            let name = world.get::<Identity>(other).unwrap().name.clone();
            let _ = roster.invite(1, &name, &world, 0);
            let _ = roster.accept(other, &world);
        }
        let effects = roster.convert_to_raid(1);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Converted to a raid.")));
        assert_eq!(roster.kind_of(1), Some(GroupKind::Raid));
        let _ = roster.invite(1, "Frank", &world, 0);
        let _ = roster.accept(6, &world);
        assert_eq!(roster.members_of(1).unwrap().len(), 6);
        assert_eq!(roster.raid_group_of(6), 1);
        let effects = roster.convert_to_party(1);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "Too many members to convert to a party."));
        let _ = roster.leave(6);
        let effects = roster.convert_to_party(1);
        assert!(effects.iter().any(|e| matches!(e, PartyEffect::Notice { message } if message == "Converted to a party.")));
    }

    #[test]
    fn convert_requires_five() {
        let world = world_with_players(2);
        let mut roster = PartyRoster::new();
        form_party(&mut roster, &world, 1, 2);
        let effects = roster.convert_to_raid(1);
        assert!(matches!(effects.as_slice(), [PartyEffect::Error { message }] if message == "You need a full party of 5 to convert to a raid."));
    }
```

Chat test:

```rust
    #[test]
    fn raid_channel_requires_raid() {
        let (roster, world) = duo();
        let effects = handle_chat(&roster, &world, 1, "raid", "hi");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { message }] if message == "You are not in a raid."));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim --lib social::party::tests::convert_full_party_to_raid_then_sixth --offline`

Expected: FAIL (`Converted to a raid.` not emitted).

- [ ] **Step 3: Implement**

```rust
pub const MAX_RAID_SIZE: usize = 10;

    fn cap_for(party: &Party) -> usize {
        match party.kind {
            GroupKind::Party => MAX_PARTY_SIZE,
            GroupKind::Raid => MAX_RAID_SIZE,
        }
    }

    pub fn convert_to_raid(&mut self, leader: EntityId) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        let Some(party) = self.parties.get_mut(&pid) else {
            return vec![PartyEffect::Error {
                message: "You are not in a party.".into(),
            }];
        };
        if party.kind == GroupKind::Raid {
            return vec![PartyEffect::Error {
                message: "Already a raid.".into(),
            }];
        }
        if party.members.len() != MAX_PARTY_SIZE {
            return vec![PartyEffect::Error {
                message: "You need a full party of 5 to convert to a raid.".into(),
            }];
        }
        party.kind = GroupKind::Raid;
        party.raid_groups[0] = party.members.clone();
        party.raid_groups[1].clear();
        vec![PartyEffect::Notice {
            message: "Converted to a raid.".into(),
        }]
    }

    pub fn convert_to_party(&mut self, leader: EntityId) -> Vec<PartyEffect> {
        if self.leader_of(leader) != Some(leader) {
            return vec![PartyEffect::Error {
                message: "You are not the party leader.".into(),
            }];
        }
        let Some(pid) = self.party_id(leader) else {
            return vec![PartyEffect::Error {
                message: "You are not in a raid.".into(),
            }];
        };
        let Some(party) = self.parties.get_mut(&pid) else {
            return vec![PartyEffect::Error {
                message: "You are not in a raid.".into(),
            }];
        };
        if party.kind != GroupKind::Raid {
            return vec![PartyEffect::Error {
                message: "You are not in a raid.".into(),
            }];
        }
        if party.members.len() > MAX_PARTY_SIZE {
            return vec![PartyEffect::Error {
                message: "Too many members to convert to a party.".into(),
            }];
        }
        party.kind = GroupKind::Party;
        party.raid_groups = [Vec::new(), Vec::new()];
        vec![PartyEffect::Notice {
            message: "Converted to a party.".into(),
        }]
    }
```

In `invite` / `accept`, replace `MAX_PARTY_SIZE` checks with `cap_for(party)` when the inviter already has a party. Full-raid toast stays `Your party is full.`

On accept into a raid, after `party.members.push(invitee)`:

```rust
            if party.kind == GroupKind::Raid {
                if party.raid_groups[0].len() < MAX_PARTY_SIZE {
                    party.raid_groups[0].push(invitee);
                } else {
                    party.raid_groups[1].push(invitee);
                }
            }
```

On `leave`, retain the id out of both `raid_groups`.

`MAX_REALM_PLAYERS`: change `8` to `10` in `sim.rs`. Search `MAX_REALM_PLAYERS` and spawn-cap tests.

`handle_chat` add:

```rust
        "raid" => {
            match roster.kind_of(speaker) {
                Some(GroupKind::Raid) => vec![ChatEffect::Message {
                    channel: "raid".into(),
                    from,
                    text: trimmed.to_string(),
                }],
                _ => vec![ChatEffect::Error {
                    message: "You are not in a raid.".into(),
                }],
            }
        }
```

Keep `party` channel as membership-only (broadcast caveat in the spec). Optionally later filter subgroup; **do not** add per-player routing.

Export `MAX_RAID_SIZE` from `social/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim --offline && cargo test --workspace --exclude woc-client --offline`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/social/party.rs crates/woc-sim/src/social/chat.rs crates/woc-sim/src/social/mod.rs crates/woc-sim/src/sim.rs
git commit -m "feat(sim): convert party to 10-player raid and raise realm cap"
```

---

### Task 10: Raid frames + ship `1.15.0` docs

**Files:**
- Modify: `crates/woc-client/src/hud.rs` (`party_frames_text` two columns when `party_kind == "raid"`)
- Modify: `VERSION.toml`, `Cargo.toml`, `UPSTREAM.md`, `CHANGELOG.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`, `README.md`

**Interfaces:**
- Consumes: `raid_group` on `PartyMemberSnapshot`
- Produces: rewrite `1.15.0` / `raid`

- [ ] **Step 1: Write the failing HUD test**

```rust
    #[test]
    fn raid_frames_group_two_on_second_column() {
        let mut snap = TickSnapshot::default();
        snap.player_id = 1;
        snap.party_kind = "raid".into();
        snap.party_members = (1..=6)
            .map(|id| woc_protocol::PartyMemberSnapshot {
                id,
                name: format!("P{id}"),
                class_id: "warrior".into(),
                hp: 10.0,
                hp_max: 10.0,
                online: true,
                raid_group: if id <= 5 { 0 } else { 1 },
            })
            .collect();
        let text = party_frames_text(&snap);
        assert!(text.contains("G2"));
        assert!(text.contains("P6"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-client raid_frames_group_two_on_second_column --offline`

Expected: FAIL (`G2` missing).

- [ ] **Step 3: Implement frames + docs**

When `snap.party_kind == "raid"`, prefix other members with `G1` / `G2` (`raid_group + 1`). Solo-column layout is fine (text HUD).

Bump `rewrite_version` to `1.15.0`, `parity_target = "raid"`. CHANGELOG `## 1.15.0`. ROADMAP mark `1.15.0` shipped. STATUS **Raid (`raid`) — done** table: convert, cap 10, raid chat, two-group frames, realm cap.

DEMO: `5c. Five players **=** convert to raid; invite a sixth; frames show G2; convert back fails until size ≤ 5.`

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace --exclude woc-client --offline && cargo test -p woc-client raid_frames_group_two_on_second_column --offline && cargo check -p woc-client --offline`

Expected: PASS. Search leftover `1.14.0` assertions that should now be `1.15.0` only in version/docs tests, not in party-depth behavior tests.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/hud.rs VERSION.toml Cargo.toml Cargo.lock UPSTREAM.md CHANGELOG.md docs README.md
git commit -m "feat: ship 1.15.0 raid convert, frames, and realm cap 10"
```

---

## Main-agent merge playbook

1. Land Tasks 1–8 as `1.14.0` (party-depth). Tag only when STATUS party-depth rows are `done` and workspace tests pass.
2. Land Tasks 9–10 as `1.15.0`. Do not convert dungeons into raid encounters in this program.
3. If `cursor/reputation-system-2d7e` (or another wave) takes `1.14.0` first, shift tags and leave protocol at whatever rev is current + 1 from 8.
