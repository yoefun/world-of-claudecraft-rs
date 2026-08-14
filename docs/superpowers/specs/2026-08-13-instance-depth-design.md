# 副本系统完善设计 — `1.22.0` / `dungeon-depth` + `1.23.0` / `delve-depth`

**Status:** Approved for implementation planning (cloud-agent deliverable 2026-08-14).  
**Baseline:** rewrite `1.21.0` / `mounts` on `develop` (ECS `World`; party/raid shipped).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal labels:** `dungeon-depth`, then `delve-depth`.  
**Protocol:** stays **10**. New snapshot fields use `#[serde(default)]`. Existing `InteractAction::{EnterDungeon,EnterDelve,AdvanceDelve,LeaveInstance}` are reused; no new WS verbs.

If another depth wave lands on `develop` before these tags, shift both numbers by one. Do not reuse a shipped label (`1.21.0` is `mounts`).

## 1. Goal

重写已经有一条**薄副本壳**：唯一 `dungeon#seq` 实例键、小队共享、Crypt / Barrow 的 trash+boss、3 房 Hollow delve、`InteractAction` 动词。它还不是可玩的副本系统。

本程序把副本做成诚实闭环：

1. **`1.22.0` dungeon-depth** — Bevy 客户端能在入口 **E** 进 5 人地下城、在入口 **E** 离开；隔离泄漏关掉；死亡释放回母区墓地；下线弹出到入口；宠物跟着进本。
2. **`1.23.0` delve-depth** — Hollow 不再抹掉大世界；独立 `{delve}#{seq}` 键；房间清完自动前进；入口挪开出生点。

Sim 仍是权威。Bevy 只发已有 `InteractAction` 并绘制 `TickSnapshot`。

> 把 STATUS 里「Dungeons / instances: done」和两个玩家在客户端里真正能做的事之间的缝补上。

## 2. Baseline (already shipped on `develop`)

| Piece | State |
| --- | --- |
| Content | `eastbrook_crypt` (L1, `crypt_warden`)；`mirefen_barrow` (L3, `barrow_hag`)；`eastbrook_hollow` 3 房 solo |
| Instance key | 地下城 `{id}#{seq}`；小队成员共享；delve 用裸 `def.id`（会撞） |
| Isolation | `InstanceAt` + `same_instance_space`；地下城进本保留大世界 |
| Delve enter | **despawn 全部** Mob/Npc/Loot，然后刷房间 |
| Boss | `spawn_boss_shell`（`template_id` 仍走 `spawn_mob_loot`，Crypt Cleaver 能掉） |
| Protocol | `EnterDungeon` / `EnterDelve` / `AdvanceDelve` / `LeaveInstance`；snapshot 只有 `zone_id` |
| Client | **从不发送**上述四个 action。**E** 只对 5 码内 Loot/尸体/NPC |
| Leave | 仅 `dungeon()` 查表；delve 的 `LeaveInstance` 恒失败；传送到**区域出生点**而非入口 |
| Death | `release_spirit` 永远落到 `eastbrook_graveyard` |
| Persist | `instance:` / `delve:` 区名被强制改成 `eastbrook`，坐标原样留下 |
| Pets | `create_pet` **不插入** `InstanceAt`；进本后宠物留在大世界，快照也看不见 |
| Snapshot leak | 不同 `InstanceAt` 的 **Player** 仍互相出现在 `entities` 里 |
| Map | 入口是 `MapMarkerKind::Portal` 紫点；无实体 Portal actor |
| Mount | 进本下马；本内拒绝上马（1.21.0） |
| Party / raid | `members_of` 已用于共享实例；无地下城人数硬顶 |
| Hollow 入口 | `(0, 0)`，距东溪出生 `(2, 4)` 约 4.5 码，落在 **E** 5 码圈内 |

### Honest remaining debt

1. **Bevy 客户端进不了副本。** 协议和 sim 测试是绿的；`input.rs` 从不发 `EnterDungeon` / `LeaveInstance` / `EnterDelve` / `AdvanceDelve`。
2. **Delve 会拆掉整个 realm。** `enter_delve` 清掉所有 Mob/Npc/Loot；`try_advance_delve` 再清一次全图尸体。多人在线时不可接受。
3. **隔离对玩家泄漏。** `snapshot_includes_entity` 让大世界玩家看见本内玩家（坐标叠在同一 heightfield 上）。
4. **死亡/下线不知道母区。** 释放永远东溪墓地；读档把 Barrow 角色丢进东溪标签。
5. **Leave 不对称。** 地下城离开去区域出生；delve 无法 Leave；没有「站在入口再 E 出去」。
6. **宠物不进本。** 无 `InstanceAt`，进本后既打不到 trash 也不在快照里。
7. **Hollow 入口叠在出生点。** 出生即可误进 delve。

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. 客户端热键硬编码 dungeon id** | **U** 进 Crypt，无视位置 | 最快；客户端决定进本；可从世界任何地方传送 | Reject |
| **B. Dungeon Finder + 锁定 + 英雄难度 + 10 人团本** | 排队、CD、第二套 loot | 独立子系统；YAGNI；团本遭遇是 party-raid 明确非目标 | Reject |
| **C. 两波 sim 权威入口 + 隔离修补（recommended）** | `1.22.0` 让 5 人地下城可玩；`1.23.0` 按同一 `InstanceAt` 把 delve 做成独立实例 | 无新动词；additive snapshot；改现有测试把玩家放到入口 | **Adopt** |

不要为入口再做一个 `EntityKind::Portal` actor。入口是内容表上的坐标；客户端 **E** 在 5 码内对那点发已有 interact。不要把 `instance_id` 写进 `CharacterSave`——下线弹出，不持久化副本进度。

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.21.0** | `mounts` | Riding / V mount / instance dismount（shipped） |
| **1.22.0** | `dungeon-depth` | 入口 E 进出、隔离、母区墓地、宠物随行、下线弹出 |
| **1.23.0** | `delve-depth` | 独立 delve 键、不抹大世界、自动进房、入口搬家 |

`PROTOCOL_REV` 保持 **10**。新字段全部 `#[serde(default)]`。上游钉仍是 **0.31.0**。Tick 指纹保持 `3214741777866168171u64`。不新增 named phase。Delve 自动前进挂在 `tick_all` 击杀结算之后，与 `expire_invites` 同类（匿名钩子）。规划提交不改 `VERSION.toml`；实现波打标时再 bump。

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` 不依赖 Bevy / wgpu / axum / tokio。
- 客户端从不决定进本、共享实例、房间前进、掉落。
- 全部 sim RNG 走 mulberry32（本程序进本路径不抽随机）。
- 禁止墙钟。空本回收仍是「最后一名玩家离开 → despawn 本内非玩家」。
- English-only 玩家可见字符串，文案锁死见 §6.8。
- 新 per-actor 状态才是 `World` 列。本程序不新增列；复用 `InstanceAt`。宠物补插已有 `InstanceAt`。
- 不要把脂肪 `Entity` 请回来。

```
woc-content DUNGEONS / DELVES     entrance_x/z + min_level + zone_id
        │
        ▼
Bevy E (5 yd) ── InteractAction::EnterDungeon / EnterDelve / LeaveInstance
        │
        ▼
woc-sim instances + delves ── unique key, party share (dungeon only),
        │                     proximity, pet follow, parent-zone leave
        ▼
TickSnapshot.instance_id / instance_name / delve_room   (rev 10 additive)
        │
        ▼
HUD zone line paints instance name; map portal markers unchanged
```

地下城与 delve 共用 `InstanceAt.instance_id`。地下城键 `{dungeon}#{seq}`；delve 键 `{delve}#{seq}`。`dungeon_id_from_instance` 已按 `#` 切开，两边都能用。`LeaveInstance` 必须两边都能离开。

## 6. `1.22.0` / `dungeon-depth`

### 6.1 Constants

```rust
pub const INSTANCE_ENTER_RANGE: f32 = 5.0;
```

与客户端 **E** 对 NPC/尸体的 5 码圈相同。距离用 XZ 平面，不含 Y。

### 6.2 Protocol (additive, rev stays 10)

On `TickSnapshot` after `zone_id`:

```rust
    /// Unique instance key (`eastbrook_crypt#12`). Empty when overworld.
    #[serde(default)]
    pub instance_id: String,
    /// Content display name (`Eastbrook Crypt`). Empty when overworld.
    #[serde(default)]
    pub instance_name: String,
    /// 0-based delve room when inside a delve. `None` in dungeons / overworld.
    #[serde(default)]
    pub delve_room: Option<u32>,
```

旧 JSON 缺这些键必须反序列化成空 / `None`。不新增 `WsClientMsg`。`InteractAction` 四个副本变体保持原样。

### 6.3 Enter / leave (dungeon)

`enter_dungeon` 增加距离门（现有测试必须先把玩家放到 `def.entrance_x/z`）：

1. 内容 id 存在、是玩家、`level >= min_level`、当前 `instance_id` 为空。
2. XZ 距 `def.entrance_x/z` ≤ `INSTANCE_ENTER_RANGE`。
3. 否则 toast 后 `return false`（见 §6.8）。不创建实例。
4. 成功路径保持现状：小队已有同 dungeon 的活实例则加入，否则 `{id}#{seq}`；缺活 boss 刷 shell；缺活 trash 刷 pack；传送到入口地面；`zone_id = instance:{dungeon}`；下马；清战斗/vendor/threat。
5. 调用 `follow_owner_into_instance(world, player_id)`：找到 `Owner.owner_id == player` 的宠物，复制玩家的 `InstanceAt` 与 `Identity.zone_id`，把宠物 `Transform` 放到玩家身旁（现有 `SUMMON_OFFSET` 即可）。若宠物没有 `InstanceAt` 列，先 `insert` 默认再写。
6. Toast `"Entered {def.name}."`。

`leave_instance` 改为统一离开（地下城 + 1.23 的 delve abort）：

1. 读玩家 `InstanceAt.instance_id`。没有则 `false`。
2. 用 `dungeon_id_from_instance` 得到 content id。若是 dungeon：`load_overworld_zone_at(..., def.zone_id, def.entrance_x, def.entrance_z)`（**入口**，不是区域出生点）。若是 delve：同样落到 `DelveDef.zone_id` + 入口（1.22 若还不识别 delve，先只处理 dungeon；1.23 接上）。
3. 玩家已不在该 key 内之后，若没有其他 **Player** 仍持有该 key，despawn 该 key 上所有非玩家（含宠物？不：先把本玩家宠物带出去。其他玩家的宠物跟他们走）。
4. `follow_owner_into_instance` 再次同步宠物到大世界（`instance_id = None`）。
5. Toast `"Left the instance."`。现有 `InstanceLeft` 事件保留。

Hearth 已走 `load_overworld_zone_at` 清 `InstanceAt`。保持。离开后必须再 `follow_owner_into_instance`；在 `load_overworld_zone_at` 末尾调用，这样 hearth / leave / persist 共用。

### 6.4 Snapshot isolation

`snapshot_includes_entity` 改为：

```rust
match (viewer_instance, entity_instance) {
    (None, None) => true,
    (Some(a), Some(b)) => a == b,
    _ => false,
}
```

跨本的玩家不再出现在 `entities` 里。小队框继续用 `party_members`（不受 AOI/实例过滤）。宠物必须带 `InstanceAt` 才能出现在本内快照。

`TickSnapshot` 填充：

- `instance_id`：玩家 key，否则 `""`
- `instance_name`：`dungeon(id).name` 或 `delve(id).name`，否则 `""`
- `delve_room`：`InstanceAt.delve_room`

### 6.5 Death / spirit

`release_spirit` 不再写死东溪：

1. 若玩家在副本：记下 content id，先走 leave 语义（弹出到母区入口、可能回收空本），再落到**母区**墓地。
2. 母区 = `DungeonDef.zone_id` / `DelveDef.zone_id` / 否则 `Identity.zone_id` 的 canonical。
3. `graveyard_for_zone(parent)`，找不到再用 `eastbrook_graveyard`。
4. Toast 仍是 `"You return to life at {}."`（现有格式，`id` 下划线转空格）。

尸体标记清掉。玩家活着出现在母区墓地，可以跑回入口 **E** 再进。若小队队友还在本里，`find_party_instance` 会让他进**同一个** key。

### 6.6 Persist

`apply_player_state` 遇到 `zone_id` 以 `instance:` 或 `delve:` 开头时：

1. 解析 content id。
2. `zone_id` 改为 def 的母区 tag（`eastbrook` / `mirefen` / …），**不是**一律东溪。
3. `pos_x/pos_z` 改为该 def 的 `entrance_x/z`。
4. `InstanceAt` 清空（已做）。

`export_player_state` 仍导出当时的 `Identity.zone_id`。弹出发生在 **load**，这样崩溃前的存档也能安全读回。不把 `instance_id` 写入 `CharacterSave`。

### 6.7 Client

`handle_interact_keys` 在 **E** 且没有更近的 loot/尸体时（loot 仍优先）：

1. 若 `snapshot.instance_id` 非空：玩家 XZ 距**当前**地下城/delve 入口 ≤ 5 → `LeaveInstance`。否则继续 NPC。
2. 否则在 `woc_content::DUNGEONS` 里找：`canonical_zone` 匹配玩家 `zone_id` 且距入口 ≤ 5 → `EnterDungeon { dungeon_id }`。
3. 1.22 不发 `EnterDelve`（留给 1.23，避免 Hollow 入口还在出生点时误进）。
4. 都没有再走现有 NPC Talk。

HUD `zone_name`：若 `instance_name` 非空，显示 `instance_name`（Crypt 显示 `Eastbrook Crypt` 而不是 `instance:eastbrook_crypt`）。Delve 1.23 可附带 `Room N`。

Help 行补：`E dungeon enter/leave`。

`GameHost::interact` 已能离线调 sim、在线发 `WsClientMsg::Interact`。Server 已把四个 action 路由到 host。这两层 1.22 **不改**。

### 6.8 Locked strings

| When | Toast |
| --- | --- |
| 成功进入 | `Entered {name}.` |
| 成功离开 | `Left the instance.` |
| 距入口 > 5 | `You must be closer to the entrance.` |
| 等级不足 | `You must be level {min} to enter {name}.` |
| 已在任意副本 | `You are already in an instance.` |
| 未知 id（作弊/过期客户端） | `There is no such instance.` |
| 非玩家 / 死了 | 静默 `false`（现有 interact 对死人的习惯） |

`{name}` 是 `DungeonDef.name` / `DelveDef.name`。`{min}` 是十进制等级。

### 6.9 `create_pet`

`create_pet` 插入 `InstanceAt::default()`，然后立刻从主人复制 `instance_id` / `delve_room` / `zone_id`。本内召唤的宠物生在本内。

### 6.10 Definition of done (`1.22.0`)

1. 单机：走到 Crypt 入口 **E** → 进本、看见 trash+boss、入口再 **E** → 出现在 Crypt 大世界入口，不是东溪出生点。
2. 两人组队：A 进 Crypt，B 在入口 **E** 进入**同一** `eastbrook_crypt#N`；只有一只 Warden。
3. 大世界玩家的 `entities` 不含本内玩家；小队框仍列出本内队友。
4. 猎人进本：宠物出现在本内快照并能打 trash。
5. 本内死亡释放 → 东溪墓地（Crypt）或 Mirefen 墓地（Barrow）；可再进队友还在的实例。
6. 存档 `zone_id=instance:mirefen_barrow` 读回 → `mirefen` + Barrow 入口坐标。
7. 旧 snapshot JSON 缺 `instance_id` 仍能反序列化。`PROTOCOL_REV == 10`。指纹不变。
8. `cargo test --workspace --exclude woc-client` 绿；`cargo check -p woc-client` 绿。

## 7. `1.23.0` / `delve-depth`

Depends on `1.22.0`。同一 protocol rev 10（`delve_room` 已在 1.22 预留）。

### 7.1 Isolation

`enter_delve`：

- **禁止**全图 despawn。
- 等级 / 已在副本 / 空 rooms / 坏 mob 模板：与现在相同，加上 **入口 5 码**门（toast 同 §6.8）。
- 实例键改为 `{delve_id}#{seq}`，**不**走 `find_party_instance`。每人独立 Hollow。
- `InstanceAt.instance_id = key`，`delve_room = Some(0)`，`zone_id = delve:{id}`。
- 只刷房间 mob，打上同一 key。
- 宠物跟随（1.22 helper）。
- Toast `Entered {name}.`

`try_advance_delve`：

- 用完整 key 判断本房间是否还有活 mob（不要再用裸 `def.id`）。
- 清房间时只 despawn **同一 key** 的 Mob/Loot，绝不碰大世界。
- 中间房：传送玩家到 `entrance + (0, room*10)`（现有偏移），刷下一房。
- 末房：发奖励（现有铜币+物品；背包满 toast 保留），然后 `leave_instance` 语义回母区入口，**不**走区域出生点。`DelveCompleted` + `InstanceLeft` 保留。

`leave_instance` 对 delve key：无奖励 abort，回入口。Hearth / 死亡释放走同一条。

### 7.2 Auto-advance

在 `tick_all` 击杀结算（phase 6 之后、`pvp_and_market` 之前）对每个带 `delve_room` 的玩家调 `try_advance_delve`。房间清完的当 tick 就前进，客户端不必再按 **E**。`AdvanceDelve` interact 保留为同函数的手动入口（测试与旧客户端仍可用），不是新协议。

不新增 `TICK_PHASES` 名字。指纹不变。

### 7.3 Move Hollow entrance

`eastbrook_hollow.entrance_x/z`：`(0, 0)` → **`(8.0, -6.0)`**。

距出生 `(2, 4)` 的 XZ ≈ 11.7 > 5，避免误进。地图紫点跟着内容表走。不改 Crypt `(-8, 0)`。

### 7.4 Client

同一 **E** 分发器补第三支：大世界、距某 `DELVES` 入口 ≤ 5 → `EnterDelve { delve_id }`。本内且 `delve_room` 为 Some 时，入口 **E** 仍是 `LeaveInstance`（abort）。前进不靠按键。

HUD：`Eastbrook Hollow — Room 1`（`delve_room + 1`，1-based 给玩家看）。

### 7.5 Definition of done (`1.23.0`)

1. 进 Hollow **不**删除东溪狼 / NPC；第二名玩家可继续在东溪打怪。
2. 两名玩家各进 Hollow → 两个不同 `{seq}`，互不可见。
3. 清三房自动前进并在末房发 `eastbrook_greaves` + 75c，出现在 `(8, -6)` 东溪。
4. 第二房中途 Leave / Hearth / 释放：无奖励，入口落地，大世界完整。
5. 出生点 **E** 不再进 Hollow；走到 `(8, -6)` 才进。
6. Workspace tests + client check 绿。指纹不变。`PROTOCOL_REV == 10`。

## 8. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Dungeon Finder / LFG / 职责排队 | party-raid 已列为非目标 |
| 锁定、英雄/普通、钥石 | 内容与节奏系统，不是进出闭环 |
| 10 人团本遭遇（Nythraxis） | party-raid §7；本程序不改 boss 表 |
| 独立地下城 heightfield / 室内网格 | 继续用同一 strip + `InstanceAt` |
| `EntityKind::Portal` actor | 入口是内容坐标；E 对点 |
| 把 `instance_id` 写入角色存档 | 下线弹出；崩溃读档也安全 |
| 空本 tick 超时 | 最后玩家离开已经 despawn |
| 重置命令 / 重置石 | 全员离开即重置 |
| 地下城人数硬顶 5 | 团队可进现有本；不在本波改平衡 |
| 新地下城 / 第二 delve | Crypt + Barrow + Hollow 足够验证闭环 |
| 浏览器 / Electron / i18n / 脂肪 Entity | 全局非目标 |

## 9. Error handling

所有失败 toast 见 §6.8。未知 `dungeon_id` / `delve_id` → `There is no such instance.`。死人 interact 静默。距离门在 **sim** 里执行，客户端过滤只是少发无效包。旧客户端从不发 Enter*，进不了本，但 rev 10 仍兼容（不会被 version gate 踢）。

## 10. Testing

**Protocol:** 缺 `instance_id` / `instance_name` / `delve_room` 的旧 `TickSnapshot` JSON 默认空；`PROTOCOL_REV` 仍为 10。

**Dungeon sim:** 现有进本测试先把 `Transform` 设到入口。新增：距离拒绝；leave 落在入口；两人隔离（大世界看不见本内 mob）；宠物 `InstanceAt` 跟随；Barrow 死亡释放用 `mirefen_graveyard`；persist 把 `instance:mirefen_barrow` 读回 mirefen 入口。

**Delve sim:** 进 Hollow 后东溪 `young_wolf` 仍活着；两玩家两个 key；前进不删大世界 loot；leave abort 无 greaves；自动前进（tick_all 清房后 `delve_room` +1）。

**Client:** `quest_interact_actions` 风格的单元测试：给定 snapshot 位置与 `instance_id`，**E** 选择 `EnterDungeon` / `LeaveInstance` / `EnterDelve` / NPC。`cargo check -p woc-client`。

## 11. Implementation notes

- 规划提交只加 spec / plan / ROADMAP / STATUS 指针。不 bump `VERSION.toml`。
- 实现时 `1.22.0` 与 `1.23.0` 可同一 PR 连续落地，或按波拆 PR；打标前跑 `cargo test --workspace --exclude woc-client` 与 `cargo check -p woc-client`。
- `enter_dungeon` 的距离门会打破「在出生点直接 enter」的旧测试——实现 Task 必须改测试坐标，这是预期，不是回归。
- Boss shell 继续用 `template_id` 走 `spawn_mob_loot`。不要在本程序改 loot 表。
- `load_overworld_zone_at` 末尾同步宠物，避免 hearth 把玩家送走、宠物留在本内被 despawn。
