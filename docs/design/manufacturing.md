# 制造系统

v1 设计规格：[`../superpowers/specs/2026-08-13-manufacturing-system-design.md`](../superpowers/specs/2026-08-13-manufacturing-system-design.md)

ECS 接入规格：[`../superpowers/specs/2026-08-13-manufacturing-ecs-design.md`](../superpowers/specs/2026-08-13-manufacturing-ecs-design.md)

实现计划：[`../superpowers/plans/2026-08-13-manufacturing-system.md`](../superpowers/plans/2026-08-13-manufacturing-system.md) · [`../superpowers/plans/2026-08-13-manufacturing-ecs.md`](../superpowers/plans/2026-08-13-manufacturing-ecs.md)

## Live 路径（`woc-sim` ECS）

正式玩法走 `woc-sim` 的 `professions` 模块，数据在 `woc-content`，动作/拒绝原因在 `woc-protocol`。

| 状态 | 存放 |
|------|------|
| 技能 | `Progress.professions`（string id，锻造为 `blacksmithing`） |
| 背包 / 金币 / 位置 | `Bags` / `Progress.copper` / `Transform` |
| 采集冷却 | `GatherNodeState` |
| 可剥皮尸体 | `Skinnable` |
| 制造施法 | `ProfessionCast`（与战斗施法分开） |
| 拒绝 | `SimEvent::ProfessionDenied { reason: ProfessionDeny }` |

`handle_interact` 开始施法；`tick_profession_casts` 在 `profession_casts` tick 阶段结算。`gather_content` / `craft` 仍是可单测的立即结算路径。

```sh
cargo test -p woc-sim --lib professions
```

## Oracle crate（`woc-manufacturing`）

typed `ItemId` / `ProfessionSession` 原型仍保留，用于规则与经济不变量的独立回归。它不接入 `Sim` tick loop，也不被 `woc-sim` 依赖。

```sh
cargo test -p woc-manufacturing
```
