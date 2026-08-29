# 制造系统 ECS 接入

日期：2026-08-13  
仓库：`world-of-claudecraft-rs`  
范围：把 `woc-manufacturing` 的 v1 规则接到 `develop` 上已有的 `woc-sim` ECS，而不是把原型 crate 改写成第二套世界循环。

## 1. 目标

制造成为 Eastbrook 的正式玩法：采集、剥皮、七条制造专业、附魔走 `ecs::World`、`woc-content` 表和 `woc-protocol` 动作。同一套 `Rng` 种子下，两次会话的背包、技能与产出一致。

成功标准：`cargo test -p woc-content --lib`、`cargo test -p woc-protocol --lib`、`cargo test -p woc-sim --lib`、`cargo test -p woc-manufacturing` 全绿。

## 2. 非目标

- 不把 `ProfessionSession` 搬进 Bevy，也不让 `woc-sim` 依赖 `woc-manufacturing` 的平行背包 / `ItemId` 枚举。
- 不删 `woc-manufacturing`：它仍是 typed 规则与经济不变量的 oracle。
- 不新增手腕装备槽；护腕附魔表保留，v1 没有可附魔护腕。
- 不改客户端制造 HUD（协议先落地；拒绝原因用稳定 id）。

## 3. 架构

```
woc-content     专业 / 配方 / 节点 / 工作台 / 附魔 / 物品表（string id）
woc-protocol    InteractAction + SimEvent::ProfessionDenied
woc-sim         ECS 组件 + professions 系统（规则的 live 实现）
woc-manufacturing   保留；不接入 tick loop
```

活数据：

| 状态 | 组件 / 字段 |
|------|-------------|
| 技能 | `Progress.professions`（string id → rank） |
| 精工标记 | `Progress.last_masterwork` |
| 背包 / 金币 | `Bags` / `Progress.copper` |
| 位置 | `Transform` |
| 采集节点冷却 | `GatherNodeState.ready_tick` |
| 可剥皮尸体 | `Skinnable` |
| 制造施法 | `ProfessionCast`（与战斗 `Combat.cast` 分开） |
| 装备附魔 | `InvStack.enchant_id` |

内容 id：锻造继续用现有 `blacksmithing`（对应原型 `Forging`）。采集上限 100，制造上限 125。

## 4. 规则（与制造规格一致）

- 已知配方没有 skillReq 准入；技能只影响涨幅与精工。采集用工具门，不用技能门。
- 采集成功恰好 2 次 RNG；制造成功 1 次精工抽取；分解 0 次。拒绝路径 0 次抽取。
- 工作台半径 20。野外配方 `station = None`。
- 金币税：`item_level_budget * 2` 铜。
- 配方经济：`reagent_unit_value`（`vendor_buy > 0` 则买价，否则卖价）× 数量 > 产物 `vendor_sell` × 数量。
- 采集物 `vendor_buy = 4 * vendor_sell`，且不得出现在 NPC 货物表。
- 拒绝原因是 `ProfessionDeny` 枚举（snake_case 上线），sim 不发英文 profession toast。
- 采集 / 制造 / 剥皮 / 附魔经 `handle_interact` 进入 `ProfessionCast`，在 tick 到期时结算。`gather_content` / `craft` 仍是可单测的立即结算路径。

## 5. 协议

新增动作：`Skin { corpse_id }`、`Disenchant { bag_slot }`、`ApplyEnchant { bag_slot, enchant_id, confirm }`。

新增事件：`ProfessionDenied { player, reason }`。

## 6. Tick

在 `loot_pickup` 之后、`build_snapshot` 之前增加 `profession_casts` 阶段。时长按 20 Hz 向上取整，与制造规格相同。
