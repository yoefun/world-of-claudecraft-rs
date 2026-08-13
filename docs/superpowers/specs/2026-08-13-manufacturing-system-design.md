# 制造系统（Professions）设计规格

日期：2026-08-13  
仓库：`world-of-claudecraft-rs`（Rust 重写）  
范围：v1 可玩制造闭环——采集、锻造、剥皮、制皮、附魔、工程学、炼金

本规格把 TypeScript 版 [World of ClaudeCraft](https://github.com/levy-street/world-of-claudecraft) 已落地的 Professions 2.0 模型，收成一份可在空仓库里从零实现的 Rust 设计。v1 只做用户点名的七条生产线；烹饪、伐木、钓鱼、裁缝、珠宝、铭文、职业原型（archetype）与委托板明确延后。

---

## 1. 目标

在确定性模拟核（`woc-sim`）里实现一套**服务器权威、数据即代码、可单测**的制造经济：

1. 野外节点采集（采矿、采药）产出矿石与草药。
2. 尸体剥皮产出兽皮。
3. 城镇工作台把原料做成装备、药剂、装置。
4. 附魔把装备分解为奥术材料，再把加成写回具体物品实例。
5. 任何配方的 NPC 回收价严格低于原料价值，避免制造成为印钞机。

成功标准：`cargo test -p woc-sim` 覆盖采集、剥皮、五条制造专业与经济不变量；给定同一 `Rng` 种子，两次完整制造会话的背包、技能与产出字节一致。

---

## 2. 非目标（v1 不做）

- 十职业环、相邻双专业 combo、archetype / Jack of All Trades。
- 委托订单板、Maker's Bond、邮件、世界拍卖。
- 钓鱼、伐木、烹饪、裁缝、铭文、珠宝。
- 客户端 HUD、3D 场景、网络同步（只预留命令/事件形状）。
- 移动工作台、工具附魔充能、GM 恢复路径。

这些能力的接口用 `ProfessionId` 枚举与 `RecipeDef.station` 预留，不在 v1 写死 10 槽数组。

---

## 3. 系统模型

### 3.1 专业集合

| 中文 | `ProfessionId` | 类别 | 技能上限 | 工作台 | 输入 | 输出 |
|------|----------------|------|----------|--------|------|------|
| 采矿 | `Mining` | 采集 | 100 | 无（节点） | 矿脉 | 矿石、粗石 |
| 草药学 | `Herbalism` | 采集 | 100 | 无（节点） | 药草 | 草药 |
| 剥皮 | `Skinning` | 采集 | 100 | 无（尸体） | 带皮尸体 | 兽皮 |
| 锻造 | `Forging` | 制造 | 125 | `Forge` | 矿石/锭、助熔剂 | 武器、锁甲/板甲、锭 |
| 制皮 | `Leatherworking` | 制造 | 125 | `Tannery` | 兽皮、筋、线 | 皮甲 |
| 炼金 | `Alchemy` | 制造 | 125 | `Apothecary` | 草药、空瓶 | 药剂、合剂 |
| 工程学 | `Engineering` | 制造 | 125 | `Toolworks` | 锭、粗石、螺栓 | 装置、炸药、工具 |
| 附魔 | `Enchanting` | 制造 | 125 | 无（任意地点） | 分解材料 | 装备实例上的加成 |

采集与制造技能彼此独立、只增不减。涨一种专业从不抽另一种。达到 `max_skill` 后动作仍成功，只是不再涨技能。

v1 **不**限制每人只能学两门主专业。所有专业对所有角色开放，用施法时间、材料与技能灰化做节流。日后若要 WoW 式栏位，只加一层准入，不改技能计数器。

### 3.2 技能档

`TIER_SKILL_STEP = 25`。`tier_for_skill(s) = min(5, s / 25)`，得到 0..=5。

对 `skill_req = R` 的节点或配方：

| 玩家技能相对 R | 颜色 | 技能涨幅 |
|----------------|------|----------|
| `current < R` | 红（仍允许做；采集另受工具档限制） | 2 |
| `R .. R+24` | 橙 | 2 |
| `R+25 .. R+49` | 黄 | 1 |
| `R+50 .. R+74` | 绿 | 1 |
| `>= R+75` 或已达上限 | 灰 | 0 |

**已知配方没有 skillReq 准入门槛**（沿用原版锁定裁决）：技能不够仍可做，只是产出不触发精工，且高于天花板的配方不涨技能。采集节点则用**工具档**卡「能不能挖」，不是用技能卡。

### 3.3 施法节奏

制造与采集都是施法，不是瞬发。

| 动作 | 时长 |
|------|------|
| 野外采集基础 | 2.5 s |
| 每高出节点一档的工具 | −0.4 s |
| 每高出一档熟练度带 | −0.15 s |
| 采集下限 | 1.5 s |
| 配方 `skill_req` 0 / 25 / 50 / 75 / 100+ | 1.75 / 2.5 / 3.0 / 3.5 / 4.0 s |
| 施法时长夹紧 | 1.5 ..= 5.0 s |
| 分解 / 附魔 / 拆解 | 1.5 s |
| 单次开始制造的批量上限 | 50 |

Tick 率：20 Hz（`TICK_HZ = 20`）。时长换算为 tick 后向上取整。

---

## 4. 架构

```
crates/woc-sim/          确定性游戏核。禁止 I/O、时间、线程、hash 随机。
  src/rng.rs             Rng trait；XorShift64 实现；测试用脚本序列。
  src/item.rs            ItemId、ItemDef、sell/buy 价值、装备槽。
  src/inventory.rs       可堆叠背包；实例物品占独立格。
  src/professions/
    types.rs             ProfessionId、RecipeDef、DenyReason。
    skill.rs             独立计数器、档位、涨幅。
    gathering.rs         节点采集：准入、施法、两抽结算。
    skinning.rs          尸体剥皮：标签门、一次一尸。
    crafting.rs          制造准入、施法完成、全有或全无扣材料。
    masterwork.rs        单一精工触发。
    stations.rs          工作台类型与距离门。
    tools.rs             采集工具档与「必须持有对应工具」。
    enchanting.rs        分解、上附魔、替换确认。
    duration.rs          纯函数时长表。
  src/content/           静态表：物品、节点、配方、附魔、工作台。
```

随机数**只**允许通过 `Rng`。采集成功恰好 2 次抽取（稀有度、稀有事件）；拒绝抽取 0 次。制造成功恰好 1 次精工抽取；拒绝 0 次。分解不抽随机（材料由品质表决定）。测试用 `ScriptedRng` 按队列吐 u32。

宿主（日后的服务器）只做：把玩家命令送进 sim、把 `SimEvent` 映到网络。v1 用 `ProfessionSession` 作为可单测的门面，不引入完整世界循环。

---

## 5. 数据流

### 5.1 采集节点

```
start_gather(player, node_id)
  → 工具门、距离门、节点冷却门、背包门
  → 开始 Gather 施法（不抽随机）
complete_gather(player, rng)
  → 再验距离 / 冷却 / 容量（不再验工具，沿用原版：开工时的工具在收工时有效）
  → draw1: 材料稀有度
  → draw2: 稀有事件（5× 产量、必签名；v1 签名简化为「精制品」堆叠，不做制作者印章）
  → 按工具档决定普通 / fine_ 等级
  → 写入背包、涨采矿或草药学、启动该玩家对该节点的冷却
```

节点刷新是**按观察者**的：玩家 A 采完，玩家 B 仍看见同一节点（v1 用 `HashMap<(PlayerId, NodeId), ready_tick>` 表达；单人测试里等于全局冷却）。

矿脉需要镐，药草需要镰刀。徒手不能采集节点。工具必须在背包或装备栏，档位覆盖节点 `tier`。

### 5.2 剥皮

```
start_skin(player, corpse_id)
  → 尸体存在、带 hide 族标签、未被剥过、距离、剥皮刀档位、背包
  → 开始 Skin 施法
complete_skin(player, rng)
  → 再验尸体仍在且未剥
  → draw1: 兽皮稀有度
  → draw2: 完美兽皮事件
  → 标记尸体已剥（一次一尸）
  → 涨剥皮技能
```

剥皮是独立采集专业，不是「任何采集工具都能割尸体」。无剥皮刀则拒绝。无 hide 标签的尸体拒绝，且**不**把尸体标成已剥。

v1 生物材料只接线 `hide`。利爪、毒囊、丝绸等延后，避免和「剥皮专业」抢身份。

### 5.3 制造

```
start_craft(player, recipe_id, count)
  → 已知配方、材料够（允许 fine_ 向下替代普通）、金币手续费够、工作台距离、背包空位、count∈[1,50]
  → 开始 Craft 施法
complete_craft(player, rng)
  → 全有或全无扣试剂与金币
  → 确定性产出（定义品质）
  → 一次精工抽取：成功则品质 +1 档
  → 涨对应专业技能
  → 若批量 >1，按件循环；中途材料或空位不足则停，已做出的保留
```

工作台半径 20 世界单位。熔炼铜锭等「手上配方」`station = None`，可在野外做。武器、护甲、药剂、装置绑定对应工作台。

手续费：`CRAFT_GOLD_SINK_COPPER_PER_BUDGET = 2`，即 `2 * item_level_budget` 铜币，成功时收取。

### 5.4 附魔

附魔三条路径，全部 1.5 s 施法，无工作台：

1. **分解**装备实例 → 按品质给奥术尘 / 精华 / 碎片。绿：尘；蓝：尘+精华；紫：碎片。绑定装备可分解。分解摧毁原物。
2. **上附魔**到指定实例：槽位必须匹配；已有附魔必须 `confirm_replace = true`，旧附魔销毁且不退材料；同一 id 再上一次拒绝 `SameEnchant`。
3. v1 不做拆解（salvage）与工具符文。

附魔技能用分解与上附魔涨。配方表（`EnchantDef`）v1 全员已知，无学习步骤。

---

## 6. 精工

`masterwork_proc_chance(player_skill, recipe_skill_req) -> u8`：

- 基础 3%（技能档与配方档持平）。
- 每高出配方一档 +1%。
- 上限 15%。
- v1 不加「签名试剂」与「专精」修正。

触发后品质 +1 档（Common→Uncommon→Rare→Epic，不到传奇）。精工只升不降。拒绝制造不抽。

---

## 7. 经济不变量

对每一条 `RecipeDef` 与每一条可制造的 `EnchantDef`：

```
input_value = Σ reagent_unit_value(item) * count
output_value = sell_value(result) * result_count
input_value > output_value
```

`reagent_unit_value`：若 `buy_value` 有限且 >0 则用买价，否则用卖价。采集物与剥皮物**禁止**出现在任何 NPC 货物表。采集物定价约定：`buy_value = 4 * sell_value`，使配方经济式稳定。

测试 `recipe_economy` 的例外列表必须为空。新配方先过这条再合入。

NPC 只做金汇：训练费（v1 暂无训练，所有 v1 配方祖父为已知）、制造手续费、工具与空瓶等加工品售卖。NPC 永不出售矿石、草药、兽皮。

---

## 8. 各专业 v1 内容

内容以「东溪谷」起步区为唯一地带。名字用通用奇幻词，避开其他 IP 的独特造币。

### 8.1 采集（采矿 + 草药学）

节点类型：`Ore`、`Herb`。起步区每种 6 个，tier 1，`skill_req = 0`，刷新 60 s。

| 节点 | 普通产出 | 精制产出（镐/镰严格高于材料带） | 稀有事件 |
|------|----------|----------------------------------|----------|
| 铜矿脉 | `CopperOre` ×1–2，`CoarseStone` ×0–1 | `FineCopperOre` | 富矿脉：5× 且给精制 |
| 银叶丛 | `Silverleaf` ×1–2 | `FineSilverleaf` | 月下开花：5× 且给精制 |
| 地根丛 | `Earthroot` ×1 | `FineEarthroot` | 同上 |

工具：`CopperPick`（档 1）、`CopperSickle`（档 1）。徒手不能挖。

### 8.2 剥皮

起步区可剥生物带 `hide` 标签（野猪、狼）。产出 `LightLeather` ×1–2；稀有事件给 `FineLightLeather`。工具：`SkinningKnife` 档 1。

### 8.3 锻造

工作台：`Forge`。手上配方：熔炼。

| 配方 | 试剂 | 产物 | skill_req | 工作台 |
|------|------|------|-----------|--------|
| `smelt_copper` | CopperOre×2 | CopperBar×1 | 0 | 无 |
| `copper_shortsword` | CopperBar×3, SmithingFlux×2 | CopperShortsword×1 | 0 | Forge |
| `copper_chain_vest` | CopperBar×5, SmithingFlux×3 | CopperChainVest×1 | 0 | Forge |
| `copper_pick` | CopperBar×3, CoarseStone×2 | CopperPick×1 | 0 | Forge |

助熔剂 `SmithingFlux` 由东溪铁匠 NPC 出售（加工品，非采集物）。

### 8.4 制皮

工作台：`Tannery`。

| 配方 | 试剂 | 产物 | skill_req | 工作台 |
|------|------|------|-----------|--------|
| `cure_light_leather` | LightLeather×1 | CuredLightLeather×1 | 0 | 无 |
| `light_leather_jerkin` | CuredLightLeather×4, SpoolOfThread×2 | LightLeatherJerkin×1 | 0 | Tannery |
| `light_leather_belt` | CuredLightLeather×2, SpoolOfThread×1 | LightLeatherBelt×1 | 0 | Tannery |

线卷由杂货 NPC 出售。

### 8.5 炼金

工作台：`Apothecary`。空瓶由炼金供应商出售。

| 配方 | 试剂 | 产物 | skill_req | 工作台 |
|------|------|------|-----------|--------|
| `minor_healing_potion` | Silverleaf×2, EmptyVial×1 | MinorHealingPotion×1 | 0 | Apothecary |
| `elixir_of_minor_strength` | Earthroot×2, EmptyVial×1 | ElixirOfMinorStrength×1 | 0 | Apothecary |

药剂效果 v1 只记录在 `ItemDef.use_effect` 枚举上（治疗 40 HP / 力量 +4 持续 300 s），完整战斗 buff 循环不在本规格。

### 8.6 工程学

工作台：`Toolworks`。

| 配方 | 试剂 | 产物 | skill_req | 工作台 |
|------|------|------|-----------|--------|
| `rough_blasting_powder` | CoarseStone×2 | RoughBlastingPowder×1 | 0 | 无 |
| `copper_bolt` | CopperBar×1 | CopperBolt×2 | 0 | Toolworks |
| `copper_grenade` | CopperBar×1, RoughBlastingPowder×2, CopperBolt×1 | CopperGrenade×2 | 0 | Toolworks |

手榴弹 v1 是可消耗物品，`use_effect = Grenade { damage: 25, radius: 5 }`，投掷结算不在本规格。

### 8.7 附魔

分解表（无随机）：

| 被分解物品品质 | 产出 |
|----------------|------|
| Common | ArcaneDust×1 |
| Uncommon | ArcaneDust×2 |
| Rare | ArcaneDust×2, ArcaneEssence×1 |
| Epic | ArcaneShard×1 |

v1 可学附魔（全员已知）：

| id | 槽 | 试剂 | 加成 |
|----|----|------|------|
| `enchant_bracer_minor_health` | Wrist | ArcaneDust×2 | sta +2 |
| `enchant_weapon_minor_might` | MainHand | ArcaneDust×5 | str +2 |
| `enchant_chest_minor_stamina` | Chest | ArcaneDust×3, ArcaneEssence×1 | sta +3 |

加成写进 `ItemInstance.enchant`，战斗核日后读取。同一实例同时只有一条附魔。

---

## 9. 错误处理

所有拒绝都是稳定 `DenyReason` 枚举，**禁止**在 sim 里拼英文句子。宿主负责本地化。

采集：`OutOfRange`、`NodeNotReady`、`MissingTool`、`ToolTierTooLow`、`InventoryFull`、`UnknownNode`、`Busy`。  
剥皮：`CorpseGone`、`NothingToSkin`、`AlreadySkinned`、`MissingKnife`、`ToolTierTooLow`、`OutOfRange`、`InventoryFull`、`Busy`。  
制造：`UnknownRecipe`、`MissingReagents`、`InsufficientGold`、`StationRequired`、`InventoryFull`、`Busy`、`InvalidCount`。  
附魔：`UnknownEnchant`、`WrongSlot`、`AlreadyEnchanted`、`SameEnchant`、`MissingReagents`、`NotInstanced`、`Busy`。

重复拒绝每次仍发射事件（带同一 reason），由宿主决定是否抑制 toast。

---

## 10. 测试策略

| 套件 | 钉死的行为 |
|------|------------|
| `skill.rs` | 独立累加、封顶停涨、档位表 |
| `gathering.rs` | 成功 2 抽、拒绝 0 抽、徒手失败、按玩家冷却、fine 门（工具必须严格高于材料带） |
| `skinning.rs` | 无皮标签不剥且不占尸、一尸一次、刀档 |
| `crafting.rs` | 材料不足不扣、工作台距离、批量中途停、手续费 |
| `masterwork.rs` | 几率公式、品质只升 |
| `enchanting.rs` | 分解表、未确认替换、同 id 拒绝 |
| `recipe_economy.rs` | 全部配方 `input > output`，例外列表为空 |
| `determinism.rs` | 同一种子两趟会话背包与技能一致 |
| `duration.rs` | 夹紧在 1.5..=5.0 |

内容表用 `#[cfg(test)]` 遍历宏保证每个 `ItemId` 都有定义、每个配方试剂都存在。

---

## 11. 与原版的刻意差异

1. **锻造合一**：原版把武器匠与护甲匠拆成环上两职。v1 用户点名「锻造」，合成一职，工作台仍是 `Forge`。日后拆分只需把 `profession` 字段改掉，配方表不用重写产物。
2. **剥皮升格为专业**：原版尸体收割是通用交互。v1 把兽皮收割收进剥皮技能与剥皮刀。
3. **采集只开矿与草**：伐木、钓鱼延后，避免 v1 节点表膨胀。
4. **无职业原型天花板**：v1 人人可把每职练到上限。精工只跟技能档走。
5. **签名简化**：v1 精制材料是普通可堆叠物品，不做制作者印章与实例合并规则。这是最大的范围削减；背包模型因此保持简单。

---

## 12. 风险

- 空仓库没有物品、战斗、玩家实体。制造系统必须自带最小 `ItemDef` / `Inventory` / `PlayerId`，日后并入完整 sim 时这些类型是迁移边界。
- 经济式依赖买价 4× 卖价。若日后改采集物价格，必须先跑 `recipe_economy`。
- 批量制造若在精工抽取上用同一 rng 流，测试必须按件固定抽取次数，否则改批量会带动后续会话。

---

## 13. 实施顺序

见 `docs/superpowers/plans/2026-08-13-manufacturing-system.md`。顺序锁定为：工作区与物品核 → 技能 → 采集 → 剥皮 → 制造引擎与工作台 → 锻造 → 制皮 → 炼金 → 工程学 → 附魔 → 经济与确定性金样。每一截结束时 `cargo test -p woc-sim` 为绿，并可单独玩通该专业。
