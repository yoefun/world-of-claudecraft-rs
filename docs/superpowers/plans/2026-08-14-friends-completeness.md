# Friends Completeness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining `1.22.0` / `friends` gaps so the shipped wave matches spec [`2026-08-14-friends-design.md`](../specs/2026-08-14-friends-design.md) and DoD §6 can actually run in CI.

**Architecture:** No new sim columns, no `CharacterSave.friends`, no protocol bump, no version bump. Keep `FriendRoster` on `Sim`. Extract two pure helpers (O-key priority, inbound chat kind) so the last untested host/client rules become unit tests. One sim-side refresh call on live `add` / `ignore`.

**Tech Stack:** Existing workspace crates. `cargo test -p woc-client` must compile after Task 1.

**Design spec:** [`docs/superpowers/specs/2026-08-14-friends-design.md`](../specs/2026-08-14-friends-design.md) (locked; this plan does not change it).

## Completeness score

Playable demo path (STATUS / DEMO step 20) works. Against the locked spec this wave is **not 100%**.

| Area | Score | Gap |
| --- | --- | --- |
| FriendRoster verbs + copy | 98% | Whisper unknown-name untested |
| Presence / park / persist | 100% | — |
| Protocol rev 11 | 100% | — |
| Spec §5.1 live refresh | 85% | `add` / `ignore` of an online character does not `refresh_entry` other books |
| Spec §5.6 **O** priority | 70% | Market-open **O** always buyout, even with a pending invite or ready prompt. Spec order is pending → ready → buyout → open friends |
| Spec §5.6 HUD | 90% | Panel exists; `friends_panel_text` has no unit test |
| Spec §8 whisper leak | 80% | Server branch looks right; no test that whisper is not `Broadcast` |
| DoD §6 in CI | 0% | `cargo test -p woc-client` does not compile (pre-existing missing fields), so `friend_enter_*` never runs |
| Fingerprint / ECS rules | 100% | — |

**In scope here:** the table above. **Out of scope:** spec §7 non-goals (bidirectional requests, ignore on say/party/raid, offline whisper mailbox, Real ID, nametag colors).

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- Do **not** bump `VERSION.toml`, workspace `version`, or `PROTOCOL_REV` (stay `1.22.0` / rev **11**).
- `woc-sim` / `woc-content` MUST NOT depend on Bevy, `bevy_ecs`, wgpu, axum, or tokio.
- Friends stay on `Sim.friends`. No `FriendList` component. No friends field on `CharacterSave`.
- Tick fingerprint must remain `3214741777866168171u64`. No new named tick phase.
- English copy stays locked in spec §5.8. Do not paraphrase.
- Whisper MUST NOT use `notices`. Ignore still filters whisper only.
- Author for commits: `yoefun <xinglinsky@outlook.com>`.
- Before claiming done: `cargo test --workspace --exclude woc-client`, `cargo test -p woc-client`, `cargo clippy --workspace --exclude woc-client -- -D warnings`, `cargo test -p woc-sim tick_phase_order_fingerprint_locked`.

## File map (modify)

| Path | Responsibility |
| --- | --- |
| `crates/woc-client/src/input.rs` | Extract `o_key_closed_action`; fix **O** order; keep existing `friend_enter_msg` tests |
| `crates/woc-client/src/hud.rs` | Fix `NpcSessionSnapshot` test literal; add `friends_panel_text` tests |
| `crates/woc-client/src/map.rs` | Add missing `mounted` on `EntitySnapshot` test literal |
| `crates/woc-sim/src/social/friends.rs` | `refresh_entry` after live add/ignore; whisper unknown-name test |
| `crates/woc-server/src/game_ws.rs` | Extract `inbound_chat_kind`; live/cold social-delete agreement test |

---

### Task 1: Unblock client tests and lock **O** priority

**Files:**
- Modify: `crates/woc-client/src/hud.rs` (`npc_session_help_mentions_auction_when_can_auction`)
- Modify: `crates/woc-client/src/map.rs` (`party_member_uses_party_marker`)
- Modify: `crates/woc-client/src/input.rs` (`handle_interact_keys` O-key + market **O** at ~940)

**Interfaces:**
- Consumes: `UiFlags.show_market` / `show_friends`, `TickSnapshot.pending_invite_from`, `TickSnapshot.ready_check`
- Produces: `o_key_closed_action(pending, ready_prompt, show_market) -> OKeyClosedAction`

Spec §5.6 order when the friends panel is **not** open (friends-open still types `o` via the existing early return):

1. Pending party invite → `PartyAccept`
2. Unanswered ready check → `ReadyRespond { ready: true }`
3. Auction house open → market buyout
4. Else open friends (and close guild)

Today steps 1–2 are skipped when `show_market` is true, so buyout always wins. That is the bug.

- [ ] **Step 1: Fix the two missing test literals so the crate compiles**

In `crates/woc-client/src/hud.rs` `npc_session_help_mentions_auction_when_can_auction`, add `train_riding: false` before `can_auction` (same field order as the repair test):

```rust
            buyback: vec![],
            train_riding: false,
            can_auction: true,
```

In `crates/woc-client/src/map.rs` `party_member_uses_party_marker`, add the mounts field:

```rust
            swimming: false,
            mounted: None,
        });
```

- [ ] **Step 2: Write the failing O-key tests**

Add to `crates/woc-client/src/input.rs` tests (next to `friend_enter_parses_add_and_whisper`):

```rust
    #[test]
    fn o_key_priority_matches_spec() {
        use OKeyClosedAction::*;
        assert_eq!(
            o_key_closed_action(true, false, true),
            PartyAccept,
            "pending invite beats market buyout"
        );
        assert_eq!(
            o_key_closed_action(false, true, true),
            ReadyRespond,
            "ready check beats market buyout"
        );
        assert_eq!(o_key_closed_action(false, false, true), MarketBuyout);
        assert_eq!(o_key_closed_action(false, false, false), OpenFriends);
        assert_eq!(o_key_closed_action(true, true, false), PartyAccept);
    }
```

- [ ] **Step 3: Run tests — compile must start working; O-key test must fail**

Run:

```bash
cargo test -p woc-client --lib input::tests::o_key_priority_matches_spec -- --exact
```

Expected: FAIL with `cannot find type OKeyClosedAction` (or `cannot find function o_key_closed_action`). If the crate still fails to compile on a missing struct field, fix that literal before continuing.

- [ ] **Step 4: Implement the helper and wire it**

Add above `handle_interact_keys` in `crates/woc-client/src/input.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OKeyClosedAction {
    PartyAccept,
    ReadyRespond,
    MarketBuyout,
    OpenFriends,
}

fn o_key_closed_action(pending: bool, ready_prompt: bool, show_market: bool) -> OKeyClosedAction {
    if pending {
        OKeyClosedAction::PartyAccept
    } else if ready_prompt {
        OKeyClosedAction::ReadyRespond
    } else if show_market {
        OKeyClosedAction::MarketBuyout
    } else {
        OKeyClosedAction::OpenFriends
    }
}
```

Replace the pending / ready / open-friends **O** chain (~550–570) **and** the later `if ui.show_market && keys.just_pressed(KeyCode::KeyO)` buyout (~940) with one dispatch after `pending` / `ready_prompt` are computed. Keep the friends-panel and guild-panel early returns **above** this so an open O-panel still types `o`.

```rust
    if keys.just_pressed(KeyCode::KeyO) {
        match o_key_closed_action(pending, ready_prompt, ui.show_market) {
            OKeyClosedAction::PartyAccept => {
                host.send_party(WsClientMsg::PartyAccept);
            }
            OKeyClosedAction::ReadyRespond => {
                host.send_party(WsClientMsg::PartyReadyRespond { ready: true });
            }
            OKeyClosedAction::MarketBuyout => {
                if let Some(listing) = host
                    .snapshot
                    .market
                    .iter()
                    .find(|l| !l.mine && l.price <= host.snapshot.progress.copper)
                    .cloned()
                {
                    host.interact(
                        player_id,
                        InteractAction::MarketBuy {
                            listing_id: listing.id,
                        },
                    );
                    host.recent_toasts.push((
                        format!("Buying listing #{} for {}c.", listing.id, listing.price),
                        2.0,
                    ));
                } else {
                    host.recent_toasts
                        .push(("No affordable market listings.".into(), 2.0));
                }
            }
            OKeyClosedAction::OpenFriends => {
                ui.show_friends = true;
                ui.show_guild = false;
                ui.guild_compose.clear();
                ui.show_character = false;
                ui.show_map = false;
                ui.show_market = false;
            }
        }
    }
```

Copy the exact `else` toast string from the current market **O** block (do not paraphrase). Delete the old duplicated **O** branches so **O** cannot fire twice in one frame.

`player_id` is bound later in `handle_interact_keys` (`let player_id = host.snapshot.player_id` around line 626). Either:

1. Move that binding above the **O** dispatch, then put the `match` where the pending/ready/open-friends chain is today and **delete** the old market **O** block at ~940, or
2. Keep computing `pending` / `ready_prompt` where they are, but run the `match` at the current buyout site (~940) after `player_id` exists, and **delete** the early **O** chain so **O** cannot fire twice.

Do not invent a second `player_id` source. Copy the buyout `else` toast exactly: `"No affordable market listings."`

- [ ] **Step 5: Run client tests**

Run:

```bash
cargo test -p woc-client
```

Expected: PASS, including `o_key_priority_matches_spec` and `friend_enter_parses_add_and_whisper`.

- [ ] **Step 6: Commit**

```bash
git add crates/woc-client/src/input.rs crates/woc-client/src/hud.rs crates/woc-client/src/map.rs
git commit -m "fix(client): O-key friends priority and compile client tests"
```

---

### Task 2: Friends panel text tests

**Files:**
- Modify: `crates/woc-client/src/hud.rs` (`friends_panel_text`, `#[cfg(test)]`)

**Interfaces:**
- Consumes: `TickSnapshot.friends` / `ignored`, `UiFlags.friend_compose`
- Produces: unchanged `friends_panel_text` string contract from spec §5.6

- [ ] **Step 1: Write the failing tests**

Add in `crates/woc-client/src/hud.rs` tests:

```rust
    #[test]
    fn friends_panel_empty_shows_add_hint() {
        let text = friends_panel_text(&TickSnapshot::default(), "");
        assert!(text.contains("No friends yet. /add Name"));
        assert!(text.contains("> _"));
    }

    #[test]
    fn friends_panel_lists_online_star_and_ignored() {
        let mut snap = TickSnapshot::default();
        snap.friends.push(woc_protocol::FriendSnapshot {
            name: "Bob".into(),
            class_id: "mage".into(),
            level: 8,
            online: true,
            zone_id: "eastbrook".into(),
        });
        snap.friends.push(woc_protocol::FriendSnapshot {
            name: "Carol".into(),
            class_id: "rogue".into(),
            level: 3,
            online: false,
            zone_id: String::new(),
        });
        snap.ignored.push(woc_protocol::IgnoredSnapshot {
            name: "Dave".into(),
        });
        let text = friends_panel_text(&snap, "/w Bob hi");
        assert!(text.contains("*Bob  mage  8  eastbrook"));
        assert!(text.contains(" Carol  rogue  3"));
        assert!(text.contains("Ignored"));
        assert!(text.contains(" Dave"));
        assert!(text.contains("> /w Bob hi_"));
        assert!(!text.contains("No friends yet"));
    }
```

If `FriendSnapshot` / `IgnoredSnapshot` need `..Default::default()`, use that instead of listing fields — match the structs in `crates/woc-protocol/src/lib.rs`.

- [ ] **Step 2: Run tests to verify they fail or already pass**

Run:

```bash
cargo test -p woc-client --lib hud::tests::friends_panel_empty_shows_add_hint hud::tests::friends_panel_lists_online_star_and_ignored
```

Expected: FAIL only if the HUD format drifted from spec. If they PASS on the current `friends_panel_text`, do not restyle the panel — keep the tests as the lock.

- [ ] **Step 3: Fix `friends_panel_text` only if a test failed**

Keep:

- Header `Friends  [O]`
- Empty both lists → `No friends yet. /add Name`
- Online marker `*` immediately before the name; offline uses a space
- Class, level, then zone only when `online && !zone_id.is_empty()`
- `Ignored` section only when the ignore list is non-empty
- Compose line `> {compose}_`

Do not add command-hint lines the spec does not list.

- [ ] **Step 4: Re-run**

```bash
cargo test -p woc-client --lib hud::tests::friends_panel_empty_shows_add_hint hud::tests::friends_panel_lists_online_star_and_ignored
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/hud.rs
git commit -m "test(client): lock friends O-panel roster text"
```

---

### Task 3: Live add/ignore refreshes every book; whisper unknown name

**Files:**
- Modify: `crates/woc-sim/src/social/friends.rs` (`add`, `ignore`, tests)

**Interfaces:**
- Consumes: existing `FriendRoster::refresh_entry`, `Resolved.entity_id`
- Produces: after a **successful** `add` or `ignore`, if the target is currently in the world, `refresh_entry(world, &target.durable)` so every book that already lists that durable gets live `name` / `class_id` / `level` (spec §5.1)

- [ ] **Step 1: Write the failing tests**

Append in `crates/woc-sim/src/social/friends.rs` tests:

```rust
    #[test]
    fn add_and_ignore_refresh_other_books_when_target_is_live() {
        let mut world = world_with_players(1);
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob-durable");
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "bob", &world, &dir);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        world
            .get_mut::<crate::ecs::components::Durable>(2)
            .unwrap()
            .durable_id = Some("bob-durable".into());
        world.get_mut::<Health>(2).unwrap().level = 9;
        crate::ecs::spawn::create_player(&mut world, 3, "Carol", PlayerClass::Rogue, 2.0, 0.0);
        let _ = roster.ignore(3, "Bob", &world, &dir);
        let alice = &roster
            .book_of(&FriendRoster::owner_key(&world, 1))
            .unwrap()
            .friends[0];
        assert_eq!(alice.name, "Bob");
        assert_eq!(alice.class_id, "mage");
        assert_eq!(alice.level, 9);
    }

    #[test]
    fn whisper_unknown_name() {
        let world = world_with_players(1);
        let dir = dir_from_world(&world);
        let roster = FriendRoster::new();
        assert!(roster
            .whisper(1, "Zed", "hi", &world, &dir, &intents_for(&[1]))
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "No player named 'Zed'.")
            }));
    }
```

`whisper_unknown_name` should already pass. `add_and_ignore_refresh_other_books_when_target_is_live` should fail with Alice still at `"bob"` / `""` / `1`.

- [ ] **Step 2: Run the refresh test and confirm it fails**

Run:

```bash
cargo test -p woc-sim --lib social::friends::tests::add_and_ignore_refresh_other_books_when_target_is_live -- --exact
```

Expected: FAIL `left: "bob"` (or level `1`).

- [ ] **Step 3: Call `refresh_entry` after a successful live add/ignore**

At the end of `add`, after `book.friends.push(...)` and before the notice `vec!`:

```rust
        if target.entity_id.is_some() {
            self.refresh_entry(world, &target.durable);
        }
```

Same after `book.ignored.push(...)` in `ignore`.

Do **not** refresh on error paths (self, duplicate, full, unignore-required). Do **not** refresh on `remove` / `unignore` (those drop rows). `presence(..., true)` already refreshes on spawn/resume — leave that.

- [ ] **Step 4: Run friends tests**

Run:

```bash
cargo test -p woc-sim --lib social::friends::tests
```

Expected: PASS (including the new two).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/social/friends.rs
git commit -m "fix(sim): refresh friend cache on live add/ignore"
```

---

### Task 4: Whisper is not a notices broadcast; live/cold delete agree

**Files:**
- Modify: `crates/woc-server/src/game_ws.rs` (`WsClientMsg::Chat` arm, tests)

**Interfaces:**
- Consumes: `channel: &str` from `WsClientMsg::Chat`
- Produces: `inbound_chat_kind(channel) -> InboundChatKind` used by the existing Chat match

- [ ] **Step 1: Write the failing tests**

Add next to `remove_social_from_economy_sweeps_books` in `crates/woc-server/src/game_ws.rs`:

```rust
    #[test]
    fn whisper_channel_is_not_a_notices_broadcast() {
        assert_eq!(inbound_chat_kind("whisper"), InboundChatKind::Whisper);
        assert_eq!(inbound_chat_kind("WHISPER"), InboundChatKind::Whisper);
        assert_eq!(inbound_chat_kind("guild"), InboundChatKind::Guild);
        assert_eq!(inbound_chat_kind("officer"), InboundChatKind::Guild);
        assert_eq!(inbound_chat_kind("say"), InboundChatKind::Broadcast);
        assert_eq!(inbound_chat_kind("party"), InboundChatKind::Broadcast);
        assert_eq!(inbound_chat_kind("raid"), InboundChatKind::Broadcast);
    }

    #[test]
    fn cold_and_live_social_removal_agree() {
        let mut sim = Sim::new_empty_eastbrook();
        let a = sim.spawn_player("Alice", PlayerClass::Warrior).unwrap();
        let b = sim.spawn_player("Bob", PlayerClass::Mage).unwrap();
        for (id, durable) in [(a, "char-alice"), (b, "char-bob")] {
            sim.world
                .get_mut::<woc_sim::ecs::components::Durable>(id)
                .unwrap()
                .durable_id = Some(durable.into());
        }
        sim.directory.register("Alice", "char-alice");
        sim.directory.register("Bob", "char-bob");
        let _ = sim.friend_add(a, "Bob");
        let _ = sim.friend_ignore(b, "Alice");
        let mut cold = export_economy_from_sim(&sim);
        remove_social_from_economy(&mut cold, "char-bob");
        sim.friends.remove_character("char-bob");
        let live = export_economy_from_sim(&sim);
        assert_eq!(live.social, cold.social);
        assert!(live.social.iter().all(|book| book.owner_durable != "char-bob"));
        assert!(live.social.iter().all(|book| {
            book.friends.iter().all(|e| e.durable_id != "char-bob")
                && book.ignored.iter().all(|e| e.durable_id != "char-bob")
        }));
    }
```

Reuse the same `PlayerClass` / `export_economy_from_sim` imports the guild cold/live test already uses in this module.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p woc-server --lib whisper_channel_is_not_a_notices_broadcast cold_and_live_social_removal_agree
```

Expected: FAIL `cannot find type InboundChatKind` (delete-agreement may already pass).

- [ ] **Step 3: Extract `inbound_chat_kind` and use it in the Chat arm**

Place near `run_social_op`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InboundChatKind {
    Whisper,
    Guild,
    Broadcast,
}

fn inbound_chat_kind(channel: &str) -> InboundChatKind {
    if channel.eq_ignore_ascii_case("whisper") {
        InboundChatKind::Whisper
    } else if channel.eq_ignore_ascii_case("guild") || channel.eq_ignore_ascii_case("officer") {
        InboundChatKind::Guild
    } else {
        InboundChatKind::Broadcast
    }
}
```

Replace the nested `if channel.eq_ignore_ascii_case("whisper")` / guild / else in the `WsClientMsg::Chat` arm with `match inbound_chat_kind(&channel)`. Keep:

- `Whisper` → `run_social_op(&shared, false, |sim| sim.whisper(...))` (dirty stays **false**)
- `Guild` → existing `guild_chat` + `send_to_players`
- `Broadcast` → existing `sim.chat` + `notices.send`

Do not send whisper through `notices`.

- [ ] **Step 4: Run server + fingerprint**

Run:

```bash
cargo test -p woc-server --lib whisper_channel_is_not_a_notices_broadcast cold_and_live_social_removal_agree
cargo test -p woc-sim tick_phase_order_fingerprint_locked
```

Expected: PASS; fingerprint still `3214741777866168171`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-server/src/game_ws.rs
git commit -m "test(server): whisper is not notices; social delete agrees"
```

---

### Task 5: Workspace verify (no version bump)

**Files:** none unless a test failed.

- [ ] **Step 1: Run the full gate**

```bash
cargo clippy --workspace --exclude woc-client -- -D warnings
cargo test --workspace --exclude woc-client
cargo test -p woc-client
cargo check -p woc-client
cargo test -p woc-sim tick_phase_order_fingerprint_locked
```

Expected: all PASS. Fingerprint `3214741777866168171`. No `FriendList` component. `CharacterSave` still has no friends field.

- [ ] **Step 2: Commit only if Step 1 forced a fix**

Do not add CHANGELOG / VERSION / STATUS edits. This wave is still `1.22.0` / `friends`.

---

## Self-review

1. **Spec coverage:** §5.1 refresh → Task 3. §5.6 O order + HUD → Tasks 1–2. §5.7 / §8 whisper routing and delete sweep → Task 4. DoD §6 CI → Task 1 compile + Task 2. §7 non-goals not implemented.
2. **Placeholders:** none. Helpers and tests are fully specified.
3. **Types:** `OKeyClosedAction` and `InboundChatKind` names are used consistently inside their tasks.

## First executable dispatch

Start with Task 1. Client tests compiling is the gate for Task 2. Tasks 3 and 4 are independent of the client and may run after Task 1 lands.
