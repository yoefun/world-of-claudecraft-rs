# 好友系统完善设计 — `1.22.0` / `friends`

**Status:** Approved. Implementation plan: [`../plans/2026-08-14-friends.md`](../plans/2026-08-14-friends.md).  
**Baseline:** rewrite `1.21.0` / parity `mounts` on `develop` (ECS `World` actor store; protocol rev **10**).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `friends`.  
**Protocol:** bump to rev **11**.

Party (shipped): `crates/woc-sim/src/social/party.rs`.  
Guilds (shipped): `crates/woc-sim/src/social/guild.rs`.  
Mail directory (shipped): `crates/woc-sim/src/mail.rs` `CharacterDirectory`.  
Sim ECS (required): [`2026-08-13-sim-ecs-design.md`](2026-08-13-sim-ecs-design.md).

公会波次把 Friends / ignore / block 列为明确非目标（[`2026-08-13-guilds-design.md`](2026-08-13-guilds-design.md) §7）。本波补上那条**可玩社交闭环**。

## 1. Goal

Rust 重写已有小队、团队、公会和密语以外的公开频道，但**没有好友簿、忽略列表、密语**。Bevy 客户端无法把一个角色记下来、看对方是否在线、或只把一句话发给他。

本波把经典 1.12 风格的好友+忽略收进 `woc-sim`：按 durable 角色 id 记账，离线 Bevy 与在线 `woc-server` 走同一套权威逻辑。客户端只发命令。

> 加好友、看在线、密语、忽略、删号清名单、重连名单还在。客户端从不决定谁在谁的列表上。

成功标准：`cargo test -p woc-sim` 覆盖加/删/忽略/密语/在线判定；`RealmEconomy` roundtrip 后重连仍看到同一份名单；两名在线角色可以互相密语，被忽略的一方收不到。

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Party | `Sim.parties: PartyRoster`；按 `EntityId`；停泊不退队；无好友 |
| Guild | `Sim.guilds: GuildRoster`；按 durable id；`RealmEconomy.guilds` |
| Chat | `say` / `party` / `raid` / `guild` / `officer`；**无 `whisper`**；`say`/`party`/`raid` 仍走全服 `notices` |
| Directory | `Sim.directory: CharacterDirectory`；大小写不敏感名字 → durable key（邮件离线投递） |
| Protocol | Rev **10**；`Chat { channel, text }` 无收件人字段 |
| Persist | `RealmEconomy` JSON blob（mail / market / guilds）；无好友字段 |
| Park / resume | WS 关闭停泊；Hello 同 `character_id` 恢复；`intents` 有无 = 是否连着 |
| Client | **J** 公会；**P** 小队；**O** 仅在有邀请/就位/AH 一口价时有意义，**没有社交面板** |
| `MAX_REALM_PLAYERS` | 10 |
| Tick fingerprint | `3214741777866168171u64` |

诚实债务：

1. **好友是空的。** 公会规格写明 Friends / ignore / block 是另一套社交，本波之前从未开工。
2. **不能密语。** `handle_chat` 没有 `whisper`；在线宿主把非公会聊天广播给全服。
3. **忽略不存在。** 没有按人屏蔽密语的权威状态。
4. **不能把离线角色记进名单。** 公会邀请要求目标当前在线；邮件已经能靠 `CharacterDirectory` 投给从未进过本进程的角色。好友必须走目录，不能克隆公会邀请。

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. 上游同构：server SocialDb only** | Postgres/memory 社交库；sim 不知道好友 | 在线能对齐上游 `server/social.ts`；离线永远空；违反 one-sim | Reject |
| **B. 玩家列 `Friends` / `Ignore`** | 新 `World` 列；随 `CharacterSave` 落盘 | 社交图是领域级，不是战斗/背包；双写；删号要扫全表角色行 | Reject |
| **C. `Sim.friends: FriendRoster`，按 durable id，像公会/邮件一样持久化（recommended）** | 离线+在线同一权威；停泊不丢名单；`RealmEconomy` 附加 JSON | 比 B 多一层 sim API；密语必须定向投递 | **Adopt** |

好友是**领域级**状态（`AGENTS.md`：与 `Mailbox` / `AuctionHouse` / `PartyRoster` / `GuildRoster` 同类），不是玩家列。不要新建 `FriendList` 组件，也不要把好友 id 写进脂肪 `Entity` 或 `CharacterSave`。

子选择（在 C 之内）：

| Sub-approach | What it does | Verdict |
| --- | --- | --- |
| **C1. 双向申请（像公会邀请）** | 在线 TTL、pending 不落盘 | 重启作废；离线加好友失败；10 人领域里申请摩擦大于收益 | Reject |
| **C2. 单向添加，经 `CharacterDirectory`（recommended）** | 对齐原版 1.12 好友簿 + 邮件目录；立即持久 | 被加的人不会收到申请，可用忽略对抗 | **Adopt** |

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.21.0** | `mounts` | Riding ranks / **V** mount（shipped） |
| **1.22.0** | `friends` | 好友簿、忽略、密语、在线/离线、持久化、Bevy **O** 面板 |

`PROTOCOL_REV` → **11**（新 `WsClientMsg` 好友动词 + `Chat.target` + snapshot 名单）。上游钉仍是 **0.31.0**。实现波打标 `1.22.0`。规划提交**不**改 `VERSION.toml` / workspace version。

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` 不依赖 Bevy / wgpu / axum / tokio。
- 客户端从不决定加好友、忽略、密语能否送达。
- 全部 sim RNG 走 mulberry32；好友动词**不抽随机**。
- **禁止墙钟。** 无邀请 TTL（单向添加）。在线状态用 `Sim.intents`，不用 `DateTime`。
- English-only 玩家可见字符串（toast / chat），文案锁死见 §5.8。
- 新 per-actor 状态才是 `World` 列。好友不是 per-actor 列。
- Tick 指纹保持 `3214741777866168171u64`。好友不新增 named phase。在线/离线提示挂在 `spawn` / `resume` / `park`，不进 `tick_all`。

```
durable character id
        │
        ▼
woc-sim FriendRoster  ── add / remove / ignore / whisper
        │
        ▼
TickSnapshot.friends + ignored     protocol rev 11
        │
        ▼
RealmEconomy.social (serde default) persist like guilds
        │
        ▼
Bevy O-panel / WsClientMsg         display + commands only
```

密语与公会聊天一样：**不得**走 `notices` 全服广播。宿主按 `SocialDelivery` 投递给单个 `player_tx`。

### 5.1 `FriendRoster` (per-realm, on `Sim`)

```rust
pub struct FriendEntry {
    pub durable_id: String,
    pub name: String,
    pub class_id: String,
    pub level: u32,
}

pub struct SocialBook {
    pub owner_durable: String,
    pub friends: Vec<FriendEntry>,
    pub ignored: Vec<FriendEntry>,
}

pub struct FriendRoster {
    books: HashMap<String, SocialBook>, // owner durable → book
}
```

键与邮件/公会相同：`FriendRoster::owner_key(world, player_id)` → `Mailbox::mailbox_key`（`Durable.durable_id` 或 `local:{entity_id}`）。

| Constant | Value | Notes |
| --- | --- | --- |
| `MAX_FRIENDS` | **50** | 原版好友上限；含离线 |
| `MAX_IGNORE` | **50** | 原版忽略上限 |
| `WHISPER_MAX` | **200** | 与 `GUILD_MESSAGE_MAX` 相同 |

查找目标名字：先 `CharacterDirectory::lookup`（大小写不敏感），再扫当前 `World` 里带 `ClassKit` 的 `Identity.name` 精确匹配。目录未注册且当前不在线 → `"No player named '{name}'."`。

缓存的 `name` / `class_id` / `level` 在该角色每次成功 `spawn` / `resume` / `add` / `ignore` 时从 `Identity` + `ClassKit` + `Health` 刷新（对当前在线的那一侧）。离线条目保留上次见到的投影。

角色可以出现在**多人**的好友/忽略名单上。单向：Alice 加 Bob 不自动让 Bob 加 Alice。

### 5.2 Verbs (sim-authoritative)

每个动词返回 `Vec<SocialEffect>`。宿主映到 `WsServerMsg::Chat`。名单走每 tick 的 `TickSnapshot.friends` / `ignored`（不新增 `WsServerMsg::FriendUpdate`）。

```rust
pub enum SocialEffect {
    /// System toast to one live player.
    Notice { to: EntityId, message: String },
    Error { to: EntityId, message: String },
    /// Targeted chat line (whisper echo or inbound whisper).
    Chat {
        to: EntityId,
        channel: String, // "whisper" | "system"
        from: String,
        text: String,
    },
}

pub enum SocialDelivery {
    To {
        player: EntityId,
        msg: woc_protocol::WsServerMsg,
    },
}
```

`woc-server` 把每条 `SocialDelivery` 经 `player_tx` 发给对应在线成员，**不得**走全服 `notices`。离线宿主把发给本地 `player_id` 的消息推进 toast。

| Verb | Who | Rule |
| --- | --- | --- |
| `add(name)` | 任何玩家 | 经目录或在线 `Identity` 解析。不能加自己。已是好友 → error。你忽略对方 → error（先 unignore）。对方忽略你 → 仍允许加入你的名单（单向通讯录），但密语会被拒。满员 → error。目标必须能解析到 durable key。 |
| `remove(name)` | 任何玩家 | 不在名单 → error。从 `friends` 删除。忽略名单不动。 |
| `ignore(name)` | 任何玩家 | 解析同 `add`。不能忽略自己。已忽略 → error。若在好友名单则先移除再忽略。满员 → error。 |
| `unignore(name)` | 任何玩家 | 不在忽略名单 → error。 |
| `whisper(name, text)` | 任何玩家 | 见 §5.3。 |

`add` / `ignore` **不**要求目标当前在线（对齐邮件，不对齐公会邀请）。

**在线判定（好友面板，不是公会）：** `online == Sim.intents.contains_key(entity)`。停泊（有 `ClassKit`、无 intent）显示离线，因为密语送不出去。公会花名册仍用 `ClassKit` 存在性，本波不改公会。

**Park / resume：** 停泊不得改好友/忽略名单。`park_player` / 成功 `resume_player` / 新 `spawn_player` 之后调用 `presence(player_id, online)`：向**当前有 intent** 且把该角色列在好友里的玩家发 Notice（§5.8）。停泊的好友收不到提示（没有 `player_tx`）。

角色 **REST 删除**（`DELETE /api/characters/{id}`）必须 `remove_character(durable_id)`：从**所有** `SocialBook` 的 `friends` 与 `ignored` 里删掉该 durable，并删掉该 durable 自己的 book。`woc-server` 在现有 `on_character_deleted` 里一并调用（公会逻辑保留）。

### 5.3 Whisper

密语是独立动词，**不**塞进 `handle_chat` 的 `say`/`party`/`raid`/`guild`/`officer` 匹配（避免改签名导致无谓 churn）。`Sim::whisper` 走 `FriendRoster::whisper`。

规则（按顺序）：

1. `trim` 文本。空 → `"Chat message is empty."`
2. 长度 > 200 → `"Chat message is too long."`
3. 解析目标名字。失败 → `"No player named '{name}'."`
4. 目标 durable == 自己 → `"You cannot whisper yourself."`
5. 你忽略对方 → `"You are ignoring {name}."`
6. 对方忽略你 → `"{name} is ignoring you."`
7. 目标当前没有 intent 槽（未连接，含停泊）→ `"{name} is not online."`
8. 成功：两条 `SocialEffect::Chat`，channel `"whisper"`：
   - 发给目标：`from` = 说话者名字，`text` = 原文。
   - 发给自己：`from` = `To {name}`，`text` = 原文。

密语**不**要求双方是好友。忽略是唯一屏蔽。密语**不**落盘、不离线留言（那是邮件）。

现有 `Chat { channel, text }` 增加加法字段：

```rust
Chat {
    channel: String,
    text: String,
    #[serde(default)]
    target: String,
}
```

`channel == "whisper"` 时 `target` 是收件人名字。旧 JSON 无 `target` 时反序列化为 `""`；非 whisper 频道忽略该字段。在线宿主：`whisper` 走 `sim.whisper` + `player_tx`；其它频道保持现状（公会定向，其余 `notices`）。

本波**不**让忽略过滤 `say` / `party` / `raid`。那些频道已经全服广播（公会规格留下的脚手架债）。忽略只挡密语。

### 5.4 Protocol rev 11

`TickSnapshot` 加法字段（`#[serde(default)]`）：

```rust
pub struct FriendSnapshot {
    pub name: String,
    pub class_id: String,
    pub level: u32,
    pub online: bool,
    pub zone_id: String, // empty when offline
}

pub struct IgnoredSnapshot {
    pub name: String,
}

// on TickSnapshot:
pub friends: Vec<FriendSnapshot>,
pub ignored: Vec<IgnoredSnapshot>,
```

好友按名字排序。在线的排在离线前面。忽略按名字排序。`zone_id` 仅在目标有 intent 时从 `Identity.zone_id` 读取，否则 `""`。

`WsClientMsg` 新变体（与 `GuildInvite` 同级）：

```text
FriendAdd { name: String }
FriendRemove { name: String }
FriendIgnore { name: String }
FriendUnignore { name: String }
```

现有 `Chat` 承载密语（`channel: "whisper"`, `target: name`）。`PROTOCOL_REV = 11`。

### 5.5 Persist

`RealmEconomy` 加法（旧 JSON 缺字段 → 空）：

```rust
pub struct SocialEntryDto {
    pub durable_id: String,
    pub name: String,
    pub class_id: String,
    pub level: u32,
}

pub struct SocialBookDto {
    pub owner_durable: String,
    pub friends: Vec<SocialEntryDto>,
    pub ignored: Vec<SocialEntryDto>,
}

RealmEconomy {
    // existing mail/market/guilds/ids…
    #[serde(default)]
    pub social: Vec<SocialBookDto>,
}
```

不存 `online` / `zone_id`。空 book（好友与忽略都空）export 时省略。

`bridge::{apply_economy_to_sim, export_economy_from_sim}` 读写 `sim.friends`。`economy_dirty` 在任何改变名单的动词后置位（add/remove/ignore/unignore）。密语**不**置脏。不新增 migration 文件：`002_realm_economy` 已是 JSON blob。

名单**不**写入 `CharacterSave`。`FriendRoster` 是唯一来源。

### 5.6 Client (Bevy)

**O** 切换好友面板，但以下优先：

1. 有 pending 小队邀请且面板未打开 → 仍 **O** accept（现有行为）。
2. 有未回应的 ready check 且面板未打开 → 仍 **O** ready。
3. 拍卖行打开 → 仍 **O** buyout。
4. 否则 toggle `show_friends`。打开时关掉 `show_guild`（两个 compose 互斥）。银行打开时 **O** 仍不抢铜币（银行不用 O）。

面板是 `ChromePanelKind::Friends` 文本 HUD，不是 DESIGN.md 社交页。

`UiFlags.friend_compose: String`：面板打开时 A–Z / Space / `/` / Backspace 编辑（复用公会 compose 键表）。`typing` 包含 `show_friends`。

| Input | Action |
| --- | --- |
| **Esc** | 关面板 |
| **Enter** | 解析 compose，见下 |
| `/add Name` | `FriendAdd` |
| `/remove Name` | `FriendRemove` |
| `/ignore Name` | `FriendIgnore` |
| `/unignore Name` | `FriendUnignore` |
| `/w Name text` 或 `/whisper Name text` | `Chat { channel: "whisper", target: Name, text }` |
| `/add` 且当前目标是其他玩家 | `FriendAdd` 用目标 `EntitySnapshot.name` |
| `/ignore` 且当前目标是其他玩家 | `FriendIgnore` 用目标名 |

花名册：先 `Friends` 段（在线名后 `*`，其后 class / level / zone），再 `Ignored` 段。空名单显示 `No friends yet. /add Name`。

`GameHost` 增加 `social_msg(WsClientMsg)`：离线直接调 `Sim` 方法；在线 `to_net.send`。密语与好友动词都走这条（与 `guild_msg` 相同的 toast 路径）。

### 5.7 Server

`game_ws.rs` 为每个新 `WsClientMsg` 调对应 `Sim` 方法，按 `SocialDelivery` 定向发送（克隆 `run_guild_op` 为 `run_social_op`，但密语**不**设 `economy_dirty`）。`Chat` 在 `channel` 为 `whisper` 时走 `sim.whisper`，不得 `notices.send`。

`on_character_deleted` 在现有公会清理之后调用 `sim.friends.remove_character`；领域未起来时改即将写出的 `RealmEconomy.social`。

离线 `WorldHost` 不强制 WS；`Sim` 方法供 `GameHost` 直调。

Hello / spawn 成功后 `refresh_entry` 所有包含该 durable 的 book，并 `presence(..., true)`。`park_player` 末尾 `presence(..., false)`。

### 5.8 Locked English copy

| Situation | Message |
| --- | --- |
| No such player | `No player named '{name}'.` |
| Add self | `You cannot add yourself.` |
| Already friends | `{name} is already on your friends list.` |
| Friends full | `Your friends list is full.` |
| Added | `{name} has been added to your friends list.` |
| Not on friends | `{name} is not on your friends list.` |
| Removed | `{name} has been removed from your friends list.` |
| Ignore self | `You cannot ignore yourself.` |
| Already ignored | `{name} is already on your ignore list.` |
| Ignore full | `Your ignore list is full.` |
| Ignored | `{name} is now being ignored.` |
| Add while ignored (you ignore them) | `Unignore {name} before adding them as a friend.` |
| Not ignored | `{name} is not on your ignore list.` |
| Unignored | `{name} is no longer being ignored.` |
| Whisper self | `You cannot whisper yourself.` |
| You ignore target | `You are ignoring {name}.` |
| Target ignores you | `{name} is ignoring you.` |
| Target offline | `{name} is not online.` |
| Empty chat | `Chat message is empty.` |
| Chat too long | `Chat message is too long.` |
| Friend came online | `{name} has come online.` |
| Friend went offline | `{name} has gone offline.` |

## 6. Definition of done

1. `FriendRoster` 单测：add（在线名、目录名）、不能加自己、重复、满员、remove、ignore 会踢出好友、unignore、删号清扫。
2. `whisper`：空/过长、离线、自密、你忽略、被忽略、成功双向 Chat。
3. `TickSnapshot.friends` / `ignored` 有默认；`PROTOCOL_REV == 11`；旧 `Chat` JSON 无 `target` 仍能反序列化。
4. `RealmEconomy.social` roundtrip；apply/export 后重连角色仍有同一份名单。
5. 停泊/恢复不改名单；停泊显示离线；有 intent 显示在线。
6. Bevy **O** 面板：`/add` `/w` `/ignore`；Esc 关闭；小队邀请时 **O** 仍是 accept。
7. `docs/parity/STATUS.md` + `DEMO.md` 有 `1.22.0` 好友步骤。
8. 指纹测试仍为 `3214741777866168171`。无新 tick phase。无脂肪 `Entity`。

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| 双向好友申请 / 待确认 | 见 §3 C1；目录单向添加更适合离线角色 |
| 好友备注 / 分组 / 最近玩家 | HUD 文本面板塞不下；YAGNI |
| Real ID / BattleTag / 跨角色账号好友 | 上游账号壳；本波按角色 durable |
| 忽略过滤 say / party / raid | 那些频道已全服广播；本波只挡密语 |
| 离线密语留言 | 邮件已经做离线投递 |
| Who 列表 / 附近玩家搜索 | 独立社交查询 |
| 名牌显示好友色 | 可后加 additive snapshot |
| 把好友放进 Bevy ECS 或 `CharacterSave` | 双写；违反 ECS 规则 |
| 公会银行 / 日历 | 仍按公会非目标 |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| 按 `EntityId` 存名单，重连丢失 | 只按 durable id；测试 park/resume + persist |
| 密语走 `notices` 泄漏给全服 | `SocialDelivery` + `player_tx`；单测 + 宿主路由测试 |
| **O** 抢走小队 accept | pending / ready / market 优先；面板打开才 compose |
| 停泊被标成在线，密语失败却显示 `*` | 好友 `online` 看 `intents`，不看 `ClassKit` |
| `CharacterSave.friends` 与花名册不一致 | 不写角色行 |
| Protocol 10 客户端撞上新变体 | 升 rev 11；title 已有 version gate |
| 新 tick phase 改指纹 | 在线提示只挂 spawn/park |

## 9. Success demo (human)

两名在线客户端，同一 `woc-server`：

1. Alice **O**，输入 `/add Bob` Enter → toast `Bob has been added to your friends list.` 面板 Bob 后有 `*`。
2. Alice `/w Bob pull west` Enter → Bob 收到 `[whisper] Alice: pull west`；Alice 看到 `[whisper] To Bob: pull west`。
3. Bob **O** `/ignore Alice` Enter → Alice 再密语得到 `Bob is ignoring you.`
4. Bob `/unignore Alice`，`/add Alice`。两人 Alt-F4 再登入：好友名单仍在；对方上线时收到 `{name} has come online.`
5. 删除 Bob 的角色：Alice 名单不再有 Bob。

Footer：`WoC-rs 1.22.0 · upstream 0.31.0`。
