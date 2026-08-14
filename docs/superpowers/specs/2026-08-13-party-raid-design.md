# Party depth + raid — group system completion

**Status:** Approved for implementation planning (cloud-agent deliverable 2026-08-13).  
**Rewrite targets:** `1.14.0` / `party-depth`, then `1.15.0` / `raid`.  
**Upstream pin:** unchanged (`0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Protocol:** bump to rev **9**.

If another depth wave lands on `develop` before these tags, shift both numbers by one. Do not reuse a shipped label (`1.13.0` is `gear-slots`).

## 1. Goal

The rewrite already has a **thin party slice**: invite / accept / leave, size 2–5, 40 yd kill credit, Need/Greed, party chat, party-shared instances, quest share, priest heals. It is not a playable group system.

This program makes grouping honest:

1. **`1.14.0` party-depth** — every verb a 5-man needs, a roster the client can see, XP that splits, membership that survives park/resume.
2. **`1.15.0` raid** — convert a full party into a 10-player raid of two groups (upstream parity), bump the realm cap to 10, raid chat + frames.

Sim remains the authority. The Bevy client only sends verbs and paints `TickSnapshot`.

> Close the gap between “Party + chat: done” in STATUS and what two players can actually do in the Bevy client.

## 2. Baseline (already shipped on `develop`)

| Piece | State |
| --- | --- |
| `PartyRoster` | Invite by exact name, accept, leave; dissolve below 2; cap 5 |
| Leader | Stored; used only for loot mode + succession on leave |
| Kill credit | Full `xp_value` granted to every in-range mate (no split) |
| Protocol | `PartyInvite` / `PartyAccept` / `PartyLeave` / `PartyUpdate`; snapshot `party_id` + `loot_mode` |
| Client | Loot mode **[** / **]** only. `PartyUpdate` is ignored. No invite/accept/leave keys. No roster HUD |
| Click target | Left-click acquires **mobs** only |
| Park / resume | Entity stays; `despawn_player` (not park) calls `on_despawn` → leave |
| Realm cap | `MAX_REALM_PLAYERS = 8` |
| Chat | `say`, `party` (membership check only; server broadcasts notices realm-wide) |
| Instances / quests / heals | Already party-aware via `members_of` |

### Honest remaining debt

1. **The Bevy client cannot form a party.** Protocol and sim tests pass; `input.rs` never sends `PartyInvite` / `PartyAccept` / `PartyLeave`.
2. **No decline, kick, promote, disband, or invite TTL.** One pending slot per invitee; it never expires; anyone can overwrite it.
3. **Snapshot is an id, not a roster.** HUD cannot paint HP/names without scanning `entities`, and parked mates may be outside AOI.
4. **XP is duplicated, not split.** A 5-man farm yields 5× solo XP.
5. **No raid.** Upstream parties convert to a 10-player raid of two groups. Rewrite hard-caps at 5 and the realm at 8.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Client-only roster + slash commands** | Paint `entities` as a party; fake invite in HUD | Fast chrome; client would decide membership | Reject |
| **B. Guilds + Dungeon Finder in the same wave** | Charters, ranks, LFG queue | Independent subsystems; blows YAGNI | Reject (later programs) |
| **C. Two-wave sim verbs + snapshot roster (recommended)** | `1.14.0` completes 5-man; `1.15.0` adds raid on the same `PartyRoster` | Protocol rev 9; realm cap 10 in the second tag | **Adopt** |

## 4. Architecture

Unchanged invariants: no fat `Entity`; no Bevy group components; mulberry32 / tick phase names untouched; English-only strings. `PartyRoster` stays a **per-realm `Sim` field** (`AGENTS.md`). Invite TTL and ready-check expiry hook at the start of `tick_all` next to `refresh_daily_quests` — **not** a new named phase. Fingerprint stays `3214741777866168171u64`.

```
woc-protocol rev 9     Party* verbs + TickSnapshot roster fields
        │
        ▼
woc-sim social/party.rs ── invite/decline/kick/promote/disband/ready/convert
        │                 PartyRoster on Sim (not a World column)
        ▼
woc-sim sim.rs          ── group_xp in kill_rewards; expire pending; snapshot
        ▼
woc-server game_ws.rs   ── route new WsClientMsg (same notices broadcast)
        ▼
Bevy HUD/input/map      ── G invite, O/P prompts, P roster, frames, blue blips
```

Raid is the same `Party` row with `kind: GroupKind::Raid` and two subgroups. Loot, kill credit, quest share, and instance share keep calling `members_of` (the full raid list).

## 5. `1.14.0` / `party-depth`

### 5.1 Constants

```rust
pub const MAX_PARTY_SIZE: usize = 5;
pub const MIN_PARTY_SIZE: usize = 2;
pub const PARTY_CREDIT_RANGE: f32 = 40.0;
pub const INVITE_TTL_TICKS: u64 = 600;      // 30 s at 20 Hz
pub const READY_CHECK_TTL_TICKS: u64 = 300; // 15 s
```

### 5.2 Roster types

```rust
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

pub struct Party {
    pub id: u32,
    pub leader: EntityId,
    pub members: Vec<EntityId>,
    pub kind: GroupKind,                 // Party in 1.14.0
    pub raid_groups: [Vec<EntityId>; 2], // empty when Party
}

pub struct PartyRoster {
    next_id: u32,
    parties: HashMap<u32, Party>,
    membership: HashMap<EntityId, u32>,
    pending: HashMap<EntityId, PendingInvite>,
    loot_modes: HashMap<u32, LootMode>,
    ready: Option<ReadyCheck>,
}
```

`invite` takes `now_tick: u64` and writes `expires_tick = now_tick + INVITE_TTL_TICKS`. A second invite to the same invitee **replaces** the pending row (toast `Invite replaced.`). Inviting someone already pending from you is a no-op notice `Invite pending.`

`expire_invites(now_tick)` and `expire_ready_check(now_tick, world)` run at the start of `tick_all` after dailies.

### 5.3 Verbs (sim)

All return `Vec<PartyEffect>`. Unknown / missing players → `Error` toasts below. Leader-only verbs fail with `You are not the party leader.`

| Method | Who | Success | Fail toasts (exact) |
| --- | --- | --- | --- |
| `invite(inviter, name, world, now_tick)` | any member, or solo | pending + `Notice` `{inviter} invited {invitee} to a party.` | `You are not in the realm.` / `No player named '{name}'.` / `You cannot invite yourself.` / `{name} is already in a party.` / `Your party is full.` |
| `decline(invitee, world)` | invitee | drop pending; notice `{name} declined the invite.` | `You have no pending party invite.` |
| `accept(invitee, world)` | invitee | join / form; `Update` | existing toasts. After TTL, `expire_invites` drops the row, so accept uses `You have no pending party invite.` (no separate expired toast) |
| `leave(player)` | member | remove; dissolve if `< 2`; new leader = `members[0]` if leader left | `You are not in a party.` |
| `kick(leader, name, world)` | leader | same as leave for the target; notice `{name} was removed from the party.` | `No player named '{name}'.` / `That player is not in your party.` / `You cannot kick yourself.` |
| `promote(leader, name, world)` | leader | `party.leader = target`; notice `{name} is now the leader.` | `That player is not in your party.` |
| `disband(leader)` | leader | clear all members + pending + ready for that id; `Update { members: [] }` | `You are not the party leader.` |
| `ready_check(leader, now_tick)` | leader | start `ReadyCheck`; members unanswered | `A ready check is already running.` / `You are not in a party.` |
| `ready_respond(player, ready, world, connected)` | member during check | record; if every id in `connected` (intent slots) has answered, emit summary and clear. Parked members are omitted from `connected` and only appear as not-ready on TTL | `There is no ready check.` |

Ready-check summary notice (exact):

- All true: `Everyone is ready.`
- Else: `Ready: {names}. Not ready: {names}.` (comma-space; missing responses count as not ready after TTL).

Parked members (no intent slot) are still in `members`. They do not block ready-check completion: unanswered parked players count as not ready immediately in the TTL summary, but the check still waits the full TTL unless every **connected** member (`intents.contains`) has answered.

### 5.4 Park vs despawn

- `park_player` — **must not** call `on_despawn`. Membership stays. Snapshot `online: false`.
- `despawn_player` — still `on_despawn` → leave (character gone from the realm).
- Realm restart: roster is RAM-only. Out of scope to persist parties.

### 5.5 Classic-era XP split

Replace “each recipient gets full `reward.xp`” with `group_xp(reward.xp, n)` where `n` is the number of **in-range** recipients (killer + `kill_credit_share` mates), clamped `1..=10`:

```rust
pub fn group_xp(mob_xp: u32, n: usize) -> u32 {
    let n = n.clamp(1, 10);
    let bonus_n = n.min(5);
    let bonus_tenths: u64 = 10 + 5 * (bonus_n as u64 - 1); // 10,15,20,25,30
    (mob_xp as u64 * bonus_tenths / (10 * n as u64)) as u32
}
```

Locked examples (`mob_xp = 100`): n=1 → 100; n=2 → 75; n=3 → 66; n=4 → 62; n=5 → 60; n=10 → 30.

Quest kill credit and deeds still fire **once per in-range member** (unchanged). Only the XP integer changes. Loot spawn is unchanged (one pile / Need-Greed as today).

### 5.6 Protocol rev 9

```rust
// TickSnapshot (all #[serde(default)])
pub party_leader_id: Option<EntityId>,
pub party_kind: String,                     // "" | "party" | "raid"
pub party_members: Vec<PartyMemberSnapshot>,
pub pending_invite_from: String,            // inviter name, or ""
pub ready_check: Option<ReadyCheckSnapshot>,

pub struct PartyMemberSnapshot {
    pub id: EntityId,
    pub name: String,
    pub class_id: String,
    pub hp: f32,
    pub hp_max: f32,
    pub online: bool,                       // has an intent slot
    pub raid_group: u8,                     // 0 in party-depth; 0 or 1 in raid
}

pub struct ReadyCheckSnapshot {
    pub expires_tick: u64,
    pub you_responded: bool,
    pub ready_count: u32,
    pub total: u32,
}

// WsClientMsg (new variants)
PartyDecline,
PartyKick { name: String },
PartyPromote { name: String },
PartyDisband,
PartyReadyCheck,
PartyReadyRespond { ready: bool },
// 1.15.0:
ConvertToRaid,
ConvertToParty,

// WsServerMsg: keep PartyUpdate { members }. Client may ignore it.
// Snapshot is the roster source of truth.
```

Old `{"type":"party_invite","name":"Bob"}` still deserializes. `PROTOCOL_REV = 9`. Online version gate (shipped `1.4.0`) kicks stale clients.

`TickSnapshot::default` and every struct literal of `TickSnapshot` in tests must include the new fields (or the compile will fail — that is the point of the rev).

### 5.7 Client (`1.14.0`)

Add `GameHost::send_party(msg: WsClientMsg)`: offline calls `Sim::party_*`; online sends on `to_net`.

| Input | When | Action |
| --- | --- | --- |
| Left-click | nearest alive **player** within 25 yd (not self) is closer than any mob | set `target_id` only — **do not** set `attack` |
| Left-click | nearest alive **mob** | existing acquire + attack |
| **G** | bank closed; target is another player | `PartyInvite` with that `Identity.name` |
| **O** | market closed; `pending_invite_from` non-empty | `PartyAccept` |
| **P** | mail closed; `pending_invite_from` non-empty | `PartyDecline` |
| **O** / **P** | market/mail closed; `ready_check` present and `you_responded == false` | `PartyReadyRespond { ready: true/false }` (takes priority over invite O/P if both somehow set — they cannot be) |
| **P** | mail closed; no pending invite; no unanswered ready check | toggle party roster panel |
| **X** | party panel open | `PartyLeave` |
| **Y** | party panel open; leader; target is a member | `PartyPromote` |
| **Minus** | party panel open; leader; target is a member | `PartyKick` |
| **R** | party panel open; leader | `PartyReadyCheck` (steals hearth **R** while the panel is open) |
| **Backspace** | party panel open; leader | `PartyDisband` |

HUD:

- Top-left under HP: party frames for **other** members (name, class prefix, `hp/hp_max` bar, `AFK` when `!online`). Local player is not duplicated.
- Pending invite line: `{name} invited you. O accept / P decline`.
- Ready check line: `Ready check {ready_count}/{total}. O ready / P not ready`.
- Party panel (toggle **P**): member list with `*` on the leader; hints for X/Y/Minus/R/Backspace.

Minimap / world map: if `entity.id` is in `party_members`, use new `MapMarkerKind::Party` (blue blip). Other players stay `Ally`.

### 5.8 Definition of done (`1.14.0`)

1. Two Bevy clients: target + **G** / **O** forms a party; frames show HP; **X** on the panel dissolves a duo.
2. Leader kick / promote / disband unit tests + toasts.
3. Invite expires after 600 ticks; accept after expiry toasts `You have no pending party invite.`
4. Park then resume: same `party_id`, `online` flips false → true; mate still listed.
5. `group_xp` tests lock the table in §5.5; a 2-man wolf kill grants 75% each, not 100% each.
6. Ready check summary toast matches §5.3.
7. Protocol tests: `PROTOCOL_REV == 9`; old snapshots without roster fields deserialize to defaults.
8. `cargo test --workspace --exclude woc-client` green; `cargo check -p woc-client` green.
9. Tick fingerprint unchanged.

## 6. `1.15.0` / `raid`

Depends on `1.14.0`. Same protocol rev **9** (raid verbs already reserved). No further rev unless a field was forgotten.

### 6.1 Convert

```rust
pub const MAX_RAID_SIZE: usize = 10;

pub fn convert_to_raid(&mut self, leader: EntityId) -> Vec<PartyEffect>
pub fn convert_to_party(&mut self, leader: EntityId) -> Vec<PartyEffect>
```

- `convert_to_raid`: leader, `kind == Party`, `members.len() == MAX_PARTY_SIZE`. Sets `kind = Raid`, `raid_groups[0] = members.clone()`, `raid_groups[1] = vec![]`. Notice `Converted to a raid.`
- Fail: `You need a full party of 5 to convert to a raid.` / not leader / already raid (`Already a raid.`)
- `convert_to_party`: leader, `kind == Raid`, `members.len() <= MAX_PARTY_SIZE`. Sets `kind = Party`, clears `raid_groups`. Notice `Converted to a party.`
- Fail: `Too many members to convert to a party.` (if `> 5`) / `You are not in a raid.`

Invites into a raid use `MAX_RAID_SIZE`. New members fill group 0 until 5, then group 1. `leave` / `kick` compact the member’s subgroup; if total `< 2`, dissolve as today.

Client: party panel **Equals** (`KeyCode::Equal`) — convert to raid when size is 5; convert to party when already a raid and size ≤ 5.

### 6.2 Realm cap

`MAX_REALM_PLAYERS`: **8 → 10**. Tests that hard-code 8 must move to 10. AOI / park behavior unchanged.

### 6.3 Chat

| Channel | Who hears it (host still broadcasts; **receivers filter** by membership in `handle_chat` by returning one `Message` that `game_ws` sends to `notices` — same as today). |
| --- | --- |
| `party` | If `kind == Party`: all members. If `kind == Raid`: only mates in the speaker’s `raid_groups[i]`. Error `You are not in a party.` when ungrouped. |
| `raid` | All raid members. Error `You are not in a raid.` when `kind != Raid`. |
| `say` | Unchanged |

1.14.0 has no raid channel. Adding `raid` in 1.15.0 is a `handle_chat` match arm.

**Delivery:** today’s `notices` broadcast means every connected client sees every `Chat` message. That is acceptable for a 10-player realm (same as current party chat). Do not add per-player routing in this program.

### 6.4 Frames + snapshot

`PartyMemberSnapshot.raid_group` is 0 or 1. HUD paints two columns when `party_kind == "raid"`. Minimap still uses `MapMarkerKind::Party`.

### 6.5 Definition of done (`1.15.0`)

1. Five players convert; sixth invite succeeds; 11th invite toasts `Your party is full.` (reuse that string for raid cap).
2. Convert back works at size ≤ 5 and fails at 6+.
3. `raid` chat errors when not in a raid; `party` chat in a raid only conceptually groups by subgroup (see §6.3 broadcast caveat).
4. `MAX_REALM_PLAYERS == 10`; two extra players can Hello.
5. `group_xp(100, 10) == 30`.
6. Workspace tests + client check green. Fingerprint unchanged.
7. No new dungeon/raid encounter (Nythraxis-class content is a later program).

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Guilds (charter, ranks, guild chat, officer chat) | Independent persist-backed org; own program |
| Dungeon Finder / LFG / role queue | Upstream social extra; not required to *be* in a group |
| Friends / ignore / whispers UI | Separate social list |
| Right-click context menus | Bevy uses keys; **G** on target is the invite |
| Persist parties across realm restart | RAM roster; park/resume is the disconnect story |
| 25/40-man, raid markers, ready-check UI chrome beyond the hint line | YAGNI |
| Per-player chat routing | Realm ≤ 10; existing notices broadcast stays |
| Heroic / 10-man raid encounter | Content, not the container |
| Wall-clock invite timers | Sim tick TTL only |
| Reintroducing a fat `Entity` | `AGENTS.md` |

## 8. Error handling

English-only toasts, exact strings in §5.3 and §6.1. Unknown `WsClientMsg` names on an old server cannot happen after the rev-9 gate. Empty kick/promote names → `No player named ''.` (same `find_player_by_name` miss path). Client never invents membership.

## 9. Testing

**Protocol:** rev 9 constant; new WS variants roundtrip; omitted snapshot roster fields default empty; old `party_invite` JSON still parses.

**Sim (`party.rs`):** keep existing form/leave/cap tests; add decline, kick, promote, disband, TTL, park-keeps-membership (via `Sim::park_player` in `sim.rs` tests), ready check all-ready and timeout, convert 5→raid, convert back, 11th invite fails.

**XP:** `group_xp` table; integration: two players in range, wolf `xp_value` 50 → each gains `group_xp(50, 2)`.

**Chat:** `raid` channel rejected before convert; after convert, `handle_chat(..., "raid", ...)` returns `Message`.

**Client:** `cargo check -p woc-client` only in CI (no GPU). Manual demo in `docs/parity/DEMO.md`.

## 10. Risks

| Risk | Mitigation |
| --- | --- |
| XP split nerfs existing solo tests | Solo `n == 1` stays 100%; only grouped kills change |
| AOI hides mates so frames go empty | Roster lives on `TickSnapshot.party_members`, filled from `World` even if the entity is omitted from `entities` |
| Park accidentally leaves | Unit test `park_player` then `party_id` still `Some`; do not call `on_despawn` from park |
| Realm cap 10 surprises online-hard tests | Change is isolated to `1.15.0`; search `MAX_REALM_PLAYERS` and `== 8` in spawn tests |
| `PartyUpdate` vs snapshot drift | Snapshot is source of truth; keep `PartyUpdate` for existing sim tests |

## 11. Success demo (human)

**1.14.0:** two online clients in Eastbrook. Alice left-clicks Bob, **G**. Bob sees the invite line, **O**. Both see a party frame. They kill a wolf standing together — each gains split XP. Alice **P** (panel) **R** — ready check; both **O** — `Everyone is ready.` Alice parks (disconnect) — Bob’s frame shows `AFK`. Alice resumes — frame live. Bob **X** — party dissolves.

**1.15.0:** five players, leader **Equals** — `Converted to a raid.` Sixth player invited. Frames show two columns once group 1 is non-empty. Leader **Equals** with 6 members toasts `Too many members to convert to a party.`

When `1.14.0` DoD is green, tag `1.14.0`. Tag `1.15.0` only after raid DoD.
