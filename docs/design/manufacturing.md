# 制造系统

v1 设计规格：[`../superpowers/specs/2026-08-13-manufacturing-system-design.md`](../superpowers/specs/2026-08-13-manufacturing-system-design.md)

实现计划：[`../superpowers/plans/2026-08-13-manufacturing-system.md`](../superpowers/plans/2026-08-13-manufacturing-system.md)

## 已实现的公开面（`woc-sim`）

### 专业

十个 `ProfessionId`，技能存于 `ProfessionSkills`（`[u16; 10]`，按 `ProfessionId::ALL` 顺序）：

| `ProfessionId` | 类别 | 技能上限 |
|----------------|------|----------|
| Mining | 采集 | 100 |
| Herbalism | 采集 | 100 |
| Skinning | 采集 | 100 |
| Forging | 制造 | 125 |
| Leatherworking | 制造 | 125 |
| Tailoring | 制造 | 125 |
| Jewelcrafting | 制造 | 125 |
| Enchanting | 制造 | 125 |
| Engineering | 制造 | 125 |
| Alchemy | 制造 | 125 |

### 工作台

`StationType`：`Forge`、`Tannery`、`Loom`、`JewelersBench`、`Apothecary`、`Toolworks`。半径 20 世界单位。

### 经济

每条配方满足 `input_value > output_value`（`reagent_unit_value` × 数量 对 `sell_value × result_count`）。采集物不进 NPC 货物表。

实现偏差：`CopperGrenade` 的 `sell_value` 为 **8**（非计划文档草稿中的 10），以满足手榴弹配方金负约束。

### 测试

```sh
cargo test -p woc-sim
```

覆盖采集、剥皮、制造、附魔、经济不变量与 `ProfessionSession` 确定性回放。
