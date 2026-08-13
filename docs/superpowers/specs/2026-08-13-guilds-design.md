# 公会系统完善设计 — `1.16.0` / `guilds`

**Status:** Implemented. Ships as rewrite `1.16.0` / `guilds` (`1.14.0` reputation and `1.15.0` gear-more landed on `develop` after this spec was written).  
**Baseline:** rewrite `1.13.0` / parity `gear-slots` on `develop` (ECS `World` actor store).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `guilds`.  
**Protocol:** bump to rev **9**.

Party (shipped): `crates/woc-sim/src/social/party.rs`.  
Upstream guild verbs (server social, not sim): `server/social.ts` at the pin.  
Sim ECS (required): [`2026-08-13-sim-ecs-design.md`](2026-08-13-sim-ecs-design.md).

## 1. Goal

Rust 重写目前只有**小队**（2–5 人、按 `EntityId`、下线即散），没有公会。上游 0.31.0 已有完整公会：建会、邀请、三档职位、公会/官员频道、MOTD、花名册。本波把那条**可玩闭环**收进 `woc-sim`，并让离线 Bevy 与在线 `woc-server` 走同一套权威逻辑。

> 建会、邀人、升职、说话、下线再上还在会里。客户端只发命令，sim 裁决结果。

成功标准：`cargo test -p woc-sim` 覆盖建会到解散的全部动词；同一 `Rng` 种子与同一命令序列下花名册字节一致；两名在线角色重连后仍同会。

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Party | `Sim.parties: PartyRoster`；invite / accept / leave；杀敌共享；Need/Greed |
| Chat | `say` + `party`；`handle_chat` 只看 `PartyRoster` |
| Protocol | Rev **8**；`WsClientMsg::PartyInvite` / `PartyAccept` / `PartyLeave` / `Chat` |
| Persist | 角色 `CharacterSave`；领域经济 `RealmEconomy`（邮件 + AH JSON blob） |
| Durable id | `Durable.durable_id`；邮件用它做收件人键（离线 `local:{entity_id}`） |
| Park / resume | WS 关闭停泊实体；Hello 同 `character_id` 复用 `EntityId` |
| Client | 无公会面板；也未发送 `PartyInvite` / `Chat`（小队/聊天仅 sim + WS 通路） |
| `MAX_REALM_PLAYERS` | 8 |

诚实债务：

1. **公会是空的。** 框架规格把 guilds 列为 0.2 非目标；completion 只交付了 party。
2. **小队模型不能复用。** 小队按活实体 id，`on_despawn` 会退队。公会必须按角色 durable id，停泊/重连不得退会。
3. **上游把公会放在 server SocialDb，不进确定性 sim。** 重写的不变量是「一个 sim、多种宿主」；离线 Bevy 必须能建会。

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. 上游同构：server SocialDb only** | Postgres/memory 社交库；sim 不知道公会 | 在线能对齐上游；离线永远空；违反 one-sim | Reject |
| **B. 克隆 `PartyRoster`（`EntityId` 键）** | 最快；下线即散 | 公会失去持久性 | Reject |
| **C. `Sim.guilds: GuildRoster`，按 durable id，像邮件一样持久化（recommended）** | 离线+在线同一权威；停泊不退会；`RealmEconomy` 附加 JSON | 比 A 多一层 sim API；日历仍不能用墙钟 | **Adopt** |

公会是**领域级**状态（`AGENTS.md`：与 `Mailbox` / `AuctionHouse` / `PartyRoster` 同类），不是玩家列。不要新建 `GuildMember` 组件列，也不要把 `guild_id` 写进脂肪 `Entity`。

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.13.0** | `gear-slots` | Dual-wield / Finger2 / quality / MH enchant（shipped） |
| **1.14.0** | `reputation` | Hub factions（shipped after this spec was written） |
| **1.15.0** | `gear-more` | Extra slots / Hunter DW / OH enchant（shipped after this spec was written） |
| **1.16.0** | `guilds` | 建会、邀请、职位、公会/官员聊天、MOTD、花名册、持久化 |

`PROTOCOL_REV` → **9**（新 `WsClientMsg` 公会动词 + snapshot 花名册）。上游钉仍是 **0.31.0**。实现波打标 `1.16.0`。

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` 不依赖 Bevy / wgpu / axum / tokio。
- 客户端从不决定入会、职位、踢人、解散。
- 全部 sim RNG 走 mulberry32；公会动词**不抽随机**。
- **禁止墙钟。** 邀请超时用 tick，不用 `DateTime` / `std::time`。
- English-only 玩家可见字符串（toast / chat），文案锁死见 §5.8。
- 新 per-actor 状态才是 `World` 列。公会不是 per-actor 列。
- Tick 指纹保持 `3214741777866168171u64`。公会不新增 named phase。邀请过期在公会动词里惰性清理（传入 `Sim.tick`）。

```
durable character id
        │
        ▼
woc-sim GuildRoster  ── create / invite / rank / motd / chat
        │
        ▼
TickSnapshot.guild + guild_invite     protocol rev 9
        │
        ▼
RealmEconomy.guilds (serde default)   persist like mail
        │
        ▼
Bevy J-panel / WsClientMsg            display + commands only
```

### 5.1 `GuildRoster` (per-realm, on `Sim`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildRank {
    Leader,
    Officer,
    Member,
}

pub struct GuildMember {
    pub durable_id: String,
    pub name: String,
    pub class_id: String,
    pub level: u32,
    pub rank: GuildRank,
}

pub struct Guild {
    pub id: u32,
    pub name: String,
    pub motd: String,
    pub motd_set_by: String,
    pub members: Vec<GuildMember>,
}

pub struct PendingInvite {
    pub guild_id: u32,
    pub guild_name: String,
    pub from_name: String,
    pub expires_tick: u64,
}

pub struct GuildRoster {
    next_id: u32,
    guilds: HashMap<u32, Guild>,
    membership: HashMap<String, u32>,      // durable_id → guild id
    pending: HashMap<String, PendingInvite>, // invitee durable_id
}
```

键与邮件相同：`GuildRoster::member_key(world, player_id)` → `Durable.durable_id` 或 `local:{entity_id}`。

| Constant | Value | Notes |
| --- | --- | --- |
| `MAX_GUILD_MEMBERS` | **100** | 上游 `GUILD_MEMBER_LIMIT`；含离线成员 |
| `GUILD_INVITE_TTL_TICKS` | **1_200** | 60 s × 20 Hz；替代上游 60_000 ms 墙钟 |
| `GUILD_MOTD_MAX` | **240** | 上游同值；空串清空 |
| `GUILD_MESSAGE_MAX` | **200** | 公会/官员聊天 |
| `MIN_GUILD_NAME` / `MAX_GUILD_NAME` | **3 / 24** | 见 `validate_guild_name` |

`validate_guild_name(raw) -> Option<String>`：

1. `trim`。
2. 长度 3..=24。
3. `^[A-Za-z][A-Za-z ]*[A-Za-z]$`（字母 + 单空格；首尾必须是字母）。
4. 禁止连续空格。
5. 领域内**大小写不敏感**唯一；存储校验后的原大小写。

角色**最多加入一个公会**。

### 5.2 Verbs (sim-authoritative)

每个动词返回 `Vec<GuildEffect>`。宿主映到 `WsServerMsg::Chat`；花名册走每 tick 的 `TickSnapshot.guild`（不新增 `WsServerMsg::GuildUpdate`）。

```rust
pub enum GuildEffect {
    /// System toast to one live player.
    Notice { to: EntityId, message: String },
    /// System toast to every currently online member of `guild_id`.
    GuildNotice { guild_id: u32, message: String },
    Error { to: EntityId, message: String },
    /// Chat line. `officer_only` delivers only to Leader + Officer.
    Chat {
        guild_id: u32,
        channel: String, // "guild" | "officer"
        from: String,
        text: String,
        officer_only: bool,
    },
}
```

`woc-server` 把 `GuildNotice` / `Chat` 经 `player_tx` 发给对应在线成员，**不得**走全服 `notices` 广播（否则官员频道会漏给外人）。`Error` / 单人 `Notice` 只发给 `to`。离线宿主把发给本地 `player_id` 的消息推进 toast。

| Verb | Who | Rule |
| --- | --- | --- |
| `create(name)` | 无会角色 | 原子：建会并把自己设为 Leader。重名 / 已在会 → error |
| `invite(target_name)` | Officer 或 Leader | 目标必须是领域内**在线**玩家（`ClassKit` + 精确 `Identity.name`）。已在会 / 已有未过期邀请 / 满员 → error。成员不能邀 |
| `accept` | 被邀人 | 过期或会已不存在 → error。入会 rank = `Member` |
| `decline` | 被邀人 | 静默丢掉 pending（上游 `guildDecline`） |
| `leave` | 会员 | Leader 且还有其他人 → 拒绝（必须转让或解散）。最后一人离开 → 删会 |
| `kick(name)` | Officer+ | 不能踢自己（用 leave）。不能踢 Leader。Officer 不能踢 Officer。只踢本会 |
| `set_rank(name, Officer \| Member)` | Leader only | 不能 `set_rank(..., Leader)`（必须 `transfer_leader`） |
| `transfer_leader(name)` | Leader | 目标升 Leader，自己降 Officer |
| `disband` | Leader | 删除公会，所有成员变无会 |
| `set_motd(text)` | Officer+ | clamp 到 240；`''` 清空；`motd_set_by` = 操作者名字 |
| `chat("guild" \| "officer", text)` | 会员；官员频道还要 Officer+ | trim；空 / 超 200 → error。官员频道只投递给 Leader+Officer |

`invite` / `accept` 对照 `Sim.tick`：`expires_tick = tick + GUILD_INVITE_TTL_TICKS`。任何公会动词开头先丢掉 `expires_tick <= tick` 的 pending。

在线状态**不存盘**：snapshot 时用 `World` 里是否存在对应 `Durable` 来标 `online`。缓存的 `name` / `class_id` / `level` 在该角色每次 `create`/`accept`/成功 Hello 注入时从 `Identity` + `ClassKit` + `Health` 刷新。

**Park / resume：** 停泊不得调用 `leave`。`PartyRoster::on_despawn` 仍退队；公会没有对等的 park 钩子。角色 **REST 删除**（`DELETE /api/characters/{id}`）必须 `remove_member(character_id)`：若删的是 Leader 且还有人，先把花名册第一名升为 Leader 再移除；若是最后一人则删会。`woc-server` 在 persist 删除之后调用 `game_ws::on_character_deleted`；领域尚未起来时只改即将写出的 `RealmEconomy` 里的 guilds 字段（删掉该 durable 成员）。

### 5.3 Chat

扩展 `handle_chat`：签名增加 `&GuildRoster`。频道：

| Channel | Gate |
| --- | --- |
| `say` | 不变 |
| `party` | 仍要 `PartyRoster` 成员 |
| `guild` | 要公会成员 |
| `officer` | Leader 或 Officer |

`handle_chat` 对 `guild` / `officer` 返回一条 `ChatEffect::Message`。`Sim::chat` 把它升成带 `guild_id` + `officer_only` 的 `GuildEffect::Chat`（或等价的 `WsServerMsg` 列表）。在线宿主只投递给该会在线成员（官员频道再滤职位）。`say` / `party` 仍走现有 `notices` 广播（小队聊天全服可见是已有脚手架，本波不修）。

本波**不做**屏蔽/忽略列表（上游 friends/blocks 是另一套社交）。

### 5.4 Protocol rev 9

`TickSnapshot` 加法字段（`#[serde(default)]`）：

```rust
pub struct GuildMemberSnapshot {
    pub name: String,
    pub class_id: String,
    pub level: u32,
    pub rank: String,          // "leader" | "officer" | "member"
    pub online: bool,
}

pub struct GuildSnapshot {
    pub id: u32,
    pub name: String,
    pub rank: String,          // viewer's rank
    pub motd: String,
    pub motd_set_by: String,
    pub members: Vec<GuildMemberSnapshot>,
}

pub struct GuildInviteSnapshot {
    pub from_name: String,
    pub guild_name: String,
}

// on TickSnapshot:
pub guild: Option<GuildSnapshot>,
pub guild_invite: Option<GuildInviteSnapshot>,
```

`WsClientMsg` 新变体（与 `PartyInvite` 同级，不当 `InteractAction`）：

```text
GuildCreate { name: String }
GuildInvite { name: String }
GuildAccept
GuildDecline
GuildLeave
GuildKick { name: String }
GuildSetRank { name: String, rank: String }   // "officer" | "member"
GuildTransferLeader { name: String }
GuildDisband
GuildSetMotd { text: String }
```

现有 `Chat { channel, text }` 承载 `guild` / `officer`。`PROTOCOL_REV = 9`。

### 5.5 Persist

`RealmEconomy` 加法（旧 JSON 缺字段 → 空）：

```rust
pub struct GuildMemberDto { durable_id, name, class_id, level, rank }
pub struct GuildDto { id, name, motd, motd_set_by, members: Vec<GuildMemberDto> }

RealmEconomy {
    // existing mail/market/ids…
    #[serde(default)]
    pub guilds: Vec<GuildDto>,
    #[serde(default = "default_next_id")]
    pub next_guild_id: u32,
}
```

Pending 邀请**不**落盘（重启即作废，与上游进程内 `pendingGuildInvites` 一致）。

`bridge::{apply_economy_to_sim, export_economy_from_sim}` 读写 `sim.guilds`。`economy_dirty` 在任何公会动词后置位（与 Interact 相同）。不新增 migration 文件：`002_realm_economy` 已是 JSON blob。

Membership **不**写入 `CharacterSave`。`GuildRoster` 是唯一来源，避免与角色行双写漂移。

### 5.6 Client (Bevy)

**J** 切换公会面板（银行打开时 **J** 仍是存铜；`show_guild` 与 `show_bank` 互斥）。面板是 `ChromePanelKind::Guild` 文本 HUD，不是 DESIGN.md 社交页。

`UiFlags.guild_compose: String`：面板打开时 A–Z / Space / Backspace 编辑。

| State | Keys |
| --- | --- |
| 无会、无邀请 | 输入名字，**Enter** → `GuildCreate` |
| 有 pending 邀请 | 面板显示 `from invited you to <name>.`；**Enter** accept，**X** decline |
| 在会 | **Enter** 把 compose 当公会聊天发出；前缀 `/o `（含空格）走官员频道 |
| 在会、目标是玩家 | **V** `GuildInvite`（用目标 `EntitySnapshot.name`） |
| 在会 | **Q** `GuildLeave` |
| Officer+、目标是玩家 | **K** `GuildKick` |
| Leader、目标是玩家 | **P** `GuildSetRank officer`；**O** `GuildSetRank member`；**T** `GuildTransferLeader`（面板打开时 T 不召唤宠物） |
| Leader | **D** `GuildDisband` |
| Officer+ | compose 以 `/motd ` 开头并 Enter → `GuildSetMotd`（其余文本；空则清空） |

`GameHost` 增加 `guild_msg(WsClientMsg)`：离线直接调 `Sim` 方法；在线 `to_net.send`。

花名册按职位排序（leader、officer、member），同职按名字。在线名后标 `*`。MOTD 非空时钉在顶部。

### 5.7 Server

`game_ws.rs` 为每个新 `WsClientMsg` 调对应 `Sim` 方法，广播返回的 `WsServerMsg`（与 `PartyInvite` 相同的 `notices` 通道）。`Chat` 已存在；`sim.chat` 改为同时看公会。公会动词后 `economy_dirty = true`。

离线 `WorldHost` 不强制 WS；`Sim` 方法供 `GameHost` 直调。

### 5.8 Locked English copy

| Situation | Message |
| --- | --- |
| Bad name | `Guild names are 3-24 letters (spaces allowed).` |
| Name taken | `A guild named '{name}' already exists.` |
| Already in guild (self) | `You are already in a guild.` |
| Founded | `You found the guild <{name}>! You are its Guild Master.` |
| Not in guild | `You are not in a guild.` |
| Member cannot invite | `Only officers and the Guild Master may invite.` |
| Invite self | `You cannot invite yourself.` |
| Target already in a guild | `{name} is already in a guild.` |
| Target has pending | `{name} already has a pending guild invitation.` |
| Guild full | `Your guild is full.` / `That guild is full.` |
| Invited (inviter) | `You have invited {name} to the guild.` |
| Invite expired | `The guild invitation has expired.` |
| Joined broadcast | `{name} has joined the guild.` |
| Leader leave blocked | `As Guild Master you must promote a new leader or disband the guild before leaving.` |
| Left | `You have left <{name}>.` |
| Left + last member | `You have left <{name}>. The guild has disbanded.` |
| Left broadcast | `{name} has left the guild.` |
| Transfer | `{name} is now the Guild Master of <{guild}>.` |
| Not leader (disband/rank/transfer) | `Only the Guild Master may disband the guild.` / `Only the Guild Master may change ranks.` / `Only the Guild Master may promote a new leader.` |
| Disbanded | `<{name}> has been disbanded.` |
| Kick not allowed (member) | `Only officers and the Guild Master may remove members.` |
| Kick self | `Use Leave Guild to remove yourself.` |
| Kick leader | `You cannot remove the Guild Master.` |
| Kick officer as officer | `Only the Guild Master may remove an officer.` |
| Kicked target | `You have been removed from <{name}>.` |
| Kick broadcast | `{name} has been removed from the guild by {actor}.` |
| Rank already | `{name} is already {label}.` |
| Rank broadcast | `{name} is now {label}.` |
| Set-rank used leader | `Use a guild transfer to hand over leadership.` |
| Officer chat denied | `Only officers and the Guild Master can use officer chat.` |
| Empty chat | `Chat message is empty.` |
| Chat too long | `Chat message is too long.` |
| Unknown channel | `Unknown chat channel '{ch}'.` |
| No such player | `No player named '{name}'.` |
| Not in your guild | `{name} is not in your guild.` |

`{label}` is `Guild Master` / `Officer` / `Member`.

## 6. Definition of done

1. `GuildRoster` 单测：create、重名、invite/accept/decline、过期、满员、leave/最后一人解散、Leader 不能丢下别人离开、kick 权限、set_rank、transfer、disband、MOTD。
2. `handle_chat`：`guild` 要成员；`officer` 要职位；非成员 error。
3. `TickSnapshot.guild` / `guild_invite` 有默认；`PROTOCOL_REV == 9`。
4. `RealmEconomy.guilds` roundtrip；apply/export 后重连角色仍在会。
5. 停泊/恢复不退会；小队仍在 despawn 时解散。
6. Bevy **J** 面板：无会可建会；有邀请可接受；在会可见花名册与 MOTD；**V** 邀请当前目标。
7. `docs/parity/STATUS.md` + `DEMO.md` 有 1.16.0 公会步骤。
8. 指纹测试仍为 `3214741777866168171`。无新 tick phase。无脂肪 `Entity`。

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| 公会银行 / 金库 / 物品页 | 上游 0.34，钉在 0.31.0 之后 |
| 公会日历 | UTC 日，墙钟，违反 sim 不变量 |
| Friends / ignore / block | 另一套社交；本波只做公会 |
| 公会排行榜 / Vale Cup 会旗 | 上游 server-only 展示 |
| `guildsFounded` deed | 事迹扩展另波 |
| 会章物品 / 签名人数 | 0.31.0 `guildCreate` 只校验名字 |
| 自定义职位超过三档 | 上游固定 `leader \| officer \| member` |
| 公会增益 / 声望 / 会阶 | 无玩法挂钩 |
| 管理后台 / 报名审核 | 产品壳 |
| 名牌显示 `<Guild>` | 可后加 additive snapshot |
| 把公会放进 Bevy ECS 或 `CharacterSave` | 双写；违反 ECS 规则 |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| 按 `EntityId` 存会籍，重连丢会 | 只按 durable id；测试 park/resume |
| 邀请用墙钟 | 只用 `Sim.tick` + 1200 |
| 新 tick phase 改指纹 | 惰性过期，不改 `tick_all` |
| `CharacterSave.guild_id` 与花名册不一致 | 不写角色行 |
| 客户端无输入框导致无法建会 | J 面板 compose buffer |
| Protocol 8 客户端撞上新变体 | 升 rev 9；title 已有 version gate |

## 9. Success demo (human)

两名在线客户端，同一 `woc-server`：

1. Alice **J**，输入 `Vale Watch`，Enter → toast 建会。
2. Alice 选中 Bob，**V** → Bob 面板显示邀请；Bob Enter 入会。
3. 两人 **J** 看到对方；Alice `/motd Kill wolves at dusk` Enter。
4. Alice 输入 `pulling west` Enter → Bob 收到 `[guild] Alice: pulling west`。
5. Alice **P** 把 Bob 升军官；Bob `/o ready` → 仅官员可见。
6. 两人均 Alt-F4 再登入：仍在 `<Vale Watch>`，MOTD 仍在。
7. Alice **T** 把会交给 Bob，**Q** 离会；Bob **D** 解散。

Footer：`WoC-rs 1.16.0 · upstream 0.31.0`。
