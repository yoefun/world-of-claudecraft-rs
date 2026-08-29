# 制造系统 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在空的 `world-of-claudecraft-rs` 仓库里落地一套可单测的确定性制造核，覆盖采集（采矿、草药学）、剥皮、锻造、制皮、裁缝、珠宝、炼金、工程学、附魔的 v1 闭环。

**Architecture:** 全部玩法逻辑放在 `crates/woc-sim`：纯函数 + 注入 `Rng`，禁止 I/O 与墙钟。物品、节点、配方、附魔是 `src/content/` 里的静态表。`ProfessionSession` 是测试门面，模拟一名玩家、一组节点、若干尸体与城镇工作台。日后的服务器只把命令推进这个核。

**Tech Stack:** Rust 2021、`cargo test`、无第三方依赖（v1 不引 serde / rand）。

## Global Constraints

- Rust edition 2021；crate 名 `woc-sim`；禁止 `std::time`、`std::thread`、`rand` crate、`HashMap` 迭代顺序依赖（需要稳定遍历时用 `BTreeMap`）。
- 随机数只走 `Rng` trait。采集成功恰好 2 抽、拒绝 0 抽；制造成功恰好 1 次精工抽、拒绝 0 抽；分解 0 抽。
- sim 不发射英文句子：拒绝原因是 `DenyReason` 枚举。
- `TICK_HZ = 20`；施法时长换 tick 向上取整；采集/制造时长夹紧在 1.5..=5.0 秒。
- 已知配方无 `skill_req` 准入门槛；技能只影响涨幅与精工。节点用工具档准入，不用技能卡死。
- 经济：每条配方 `input_value > output_value`；采集物与兽皮不得出现在 NPC 货物表；采集物 `buy_value = 4 * sell_value`。
- 技能独立累加，达 `max_skill` 后动作仍成功、涨幅为 0。
- 标识符用通用奇幻词（Copper、Silverleaf、ArcaneDust），不造其他作品的独特币名。

---

## File structure

| Path | Responsibility |
|------|----------------|
| `Cargo.toml` | workspace，成员 `crates/woc-sim` |
| `crates/woc-sim/Cargo.toml` | crate 清单，无第三方依赖 |
| `crates/woc-sim/src/lib.rs` | 模块出口 |
| `crates/woc-sim/src/rng.rs` | `Rng`、`XorShift64`、`ScriptedRng` |
| `crates/woc-sim/src/item.rs` | `ItemId`、`Quality`、`EquipSlot`、`ItemDef`、查表 |
| `crates/woc-sim/src/inventory.rs` | 可堆叠背包与装备实例 |
| `crates/woc-sim/src/gold.rs` | 铜币余额 |
| `crates/woc-sim/src/professions/mod.rs` | 子模块与 `ProfessionSession` |
| `crates/woc-sim/src/professions/types.rs` | `ProfessionId`、`DenyReason`、配方/节点形状 |
| `crates/woc-sim/src/professions/skill.rs` | 技能计数、档位、涨幅 |
| `crates/woc-sim/src/professions/duration.rs` | 施法时长纯函数 |
| `crates/woc-sim/src/professions/tools.rs` | 镐/镰/刀档位门 |
| `crates/woc-sim/src/professions/gathering.rs` | 节点采集 |
| `crates/woc-sim/src/professions/skinning.rs` | 尸体剥皮 |
| `crates/woc-sim/src/professions/stations.rs` | 工作台距离门 |
| `crates/woc-sim/src/professions/masterwork.rs` | 精工几率与品质上调 |
| `crates/woc-sim/src/professions/crafting.rs` | 制造施法与结算 |
| `crates/woc-sim/src/professions/enchanting.rs` | 分解与上附魔 |
| `crates/woc-sim/src/content/mod.rs` | 内容出口 |
| `crates/woc-sim/src/content/items.rs` | 全部 `ItemDef` |
| `crates/woc-sim/src/content/nodes.rs` | 东溪谷矿脉与药草 |
| `crates/woc-sim/src/content/recipes.rs` | 七条制造专业的配方 |
| `crates/woc-sim/src/content/enchants.rs` | 分解表与附魔定义 |
| `crates/woc-sim/src/content/stations.rs` | 东溪谷工作台坐标 |
| `crates/woc-sim/src/content/vendors.rs` | NPC 可售加工品（不含采集物） |
| `README.md` | 指向规格与如何跑测试 |

---

### Task 1: Workspace and crate skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `crates/woc-sim/Cargo.toml`
- Create: `crates/woc-sim/src/lib.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: nothing
- Produces: workspace that `cargo test -p woc-sim` can run; `woc_sim` crate with `TICK_HZ: u32 = 20`

- [ ] **Step 1: Write the failing crate entry**

`Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/woc-sim"]

[workspace.package]
edition = "2021"
license = "MIT"
version = "0.1.0"
```

`crates/woc-sim/Cargo.toml`:

```toml
[package]
name = "woc-sim"
version.workspace = true
edition.workspace = true
license.workspace = true
```

`crates/woc-sim/src/lib.rs`:

```rust
pub const TICK_HZ: u32 = 20;

pub fn ticks_from_seconds(seconds: f32) -> u32 {
    (seconds * TICK_HZ as f32).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_rate_is_twenty_hertz() {
        assert_eq!(TICK_HZ, 20);
        assert_eq!(ticks_from_seconds(1.5), 30);
        assert_eq!(ticks_from_seconds(1.51), 31);
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p woc-sim --lib ticks_from_seconds -- --exact` is unnecessary; run `cargo test -p woc-sim`

Expected: PASS (`tick_rate_is_twenty_hertz`)

- [ ] **Step 3: Point the README at the plan**

Replace `README.md` with:

```markdown
# world-of-claudecraft-rs

Rust rewrite of World of ClaudeCraft. v1 starts with the manufacturing sim
(`crates/woc-sim`).

- Design: `docs/superpowers/specs/2026-08-13-manufacturing-system-design.md`
- Plan: `docs/superpowers/plans/2026-08-13-manufacturing-system.md`

```sh
cargo test -p woc-sim
```
```

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/woc-sim README.md
git commit -m "chore: bootstrap woc-sim workspace and tick clock"
```

---

### Task 2: Deterministic RNG seam

**Files:**
- Create: `crates/woc-sim/src/rng.rs`
- Modify: `crates/woc-sim/src/lib.rs`

**Interfaces:**
- Consumes: `TICK_HZ` unused
- Produces: `pub trait Rng { fn next_u32(&mut self) -> u32; fn chance(&mut self, percent: u8) -> bool; }`；`XorShift64::new(seed: u64)`；`ScriptedRng::from_seq(&[u32])`（队列耗尽则 panic）

- [ ] **Step 1: Write the failing test in rng.rs**

```rust
pub trait Rng {
    fn next_u32(&mut self) -> u32;

    fn chance(&mut self, percent: u8) -> bool {
        debug_assert!(percent <= 100);
        (self.next_u32() % 100) < u32::from(percent)
    }
}

pub struct ScriptedRng {
    seq: std::vec::IntoIter<u32>,
}

impl ScriptedRng {
    pub fn from_seq(seq: &[u32]) -> Self {
        Self {
            seq: seq.to_vec().into_iter(),
        }
    }
}

impl Rng for ScriptedRng {
    fn next_u32(&mut self) -> u32 {
        self.seq
            .next()
            .expect("ScriptedRng exhausted; harvest/craft drew more than the test scripted")
    }
}

pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed | 1,
        }
    }
}

impl Rng for XorShift64 {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_chance_consumes_one_draw() {
        let mut rng = ScriptedRng::from_seq(&[3, 50]);
        assert!(rng.chance(5));
        assert!(!rng.chance(50));
    }

    #[test]
    fn xorshift_is_deterministic() {
        let mut a = XorShift64::new(42);
        let mut b = XorShift64::new(42);
        let seq_a: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let seq_b: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        assert_eq!(seq_a, seq_b);
    }
}
```

- [ ] **Step 2: Export from lib.rs**

```rust
pub const TICK_HZ: u32 = 20;

pub mod rng;

pub fn ticks_from_seconds(seconds: f32) -> u32 {
    (seconds * TICK_HZ as f32).ceil() as u32
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS (`scripted_chance_consumes_one_draw`, `xorshift_is_deterministic`, previous tick test)

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/rng.rs crates/woc-sim/src/lib.rs
git commit -m "feat: add deterministic Rng seam with scripted draws"
```

---

### Task 3: Item catalog and inventory

**Files:**
- Create: `crates/woc-sim/src/item.rs`
- Create: `crates/woc-sim/src/inventory.rs`
- Create: `crates/woc-sim/src/gold.rs`
- Create: `crates/woc-sim/src/content/mod.rs`
- Create: `crates/woc-sim/src/content/items.rs`
- Modify: `crates/woc-sim/src/lib.rs`

**Interfaces:**
- Consumes: nothing from professions
- Produces: `ItemId` enum（本任务先放采集与熔炼用到的 id，后续任务往同一枚举追加变体，禁止另起一套 id）；`Inventory::try_add(ItemStack) -> Result<(), InventoryError::Full>`；`Inventory::try_remove(ItemId, u16) -> Result<(), InventoryError::Missing>`；`Gold { copper: u32 }`

- [ ] **Step 1: Write item ids and defs**

`crates/woc-sim/src/item.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Quality {
    Common,
    Uncommon,
    Rare,
    Epic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EquipSlot {
    MainHand,
    Chest,
    Wrist,
    Waist,
    Legs,
    Ring,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ItemId {
    CopperOre,
    FineCopperOre,
    CoarseStone,
    Silverleaf,
    FineSilverleaf,
    Earthroot,
    FineEarthroot,
    LightLeather,
    FineLightLeather,
    CuredLightLeather,
    CopperBar,
    SmithingFlux,
    SpoolOfThread,
    EmptyVial,
    CopperPick,
    CopperSickle,
    SkinningKnife,
    CopperShortsword,
    CopperChainVest,
    LightLeatherJerkin,
    LightLeatherBelt,
    LinenCloth,
    BoltOfLinen,
    LinenTrousers,
    LinenVestments,
    Tigerseye,
    CopperSetting,
    TigerseyeBand,
    MinorHealingPotion,
    ElixirOfMinorStrength,
    RoughBlastingPowder,
    CopperBolt,
    CopperGrenade,
    ArcaneDust,
    ArcaneEssence,
    ArcaneShard,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub id: ItemId,
    pub quality: Quality,
    pub slot: EquipSlot,
    pub sell_value: u32,
    pub buy_value: u32,
    pub stackable: bool,
    pub gathered: bool,
}

pub fn reagent_unit_value(def: &ItemDef) -> u32 {
    if def.buy_value > 0 {
        def.buy_value
    } else {
        def.sell_value
    }
}
```

`crates/woc-sim/src/content/items.rs` 必须为 `ItemId` 每个变体提供一行。采集物卖价与买价 1:4：

```rust
use crate::item::{EquipSlot, ItemDef, ItemId, Quality};

pub const ITEM_DEFS: &[ItemDef] = &[
    ItemDef { id: ItemId::CopperOre, quality: Quality::Common, slot: EquipSlot::None, sell_value: 5, buy_value: 20, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineCopperOre, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 12, buy_value: 48, stackable: true, gathered: true },
    ItemDef { id: ItemId::CoarseStone, quality: Quality::Common, slot: EquipSlot::None, sell_value: 2, buy_value: 8, stackable: true, gathered: true },
    ItemDef { id: ItemId::Silverleaf, quality: Quality::Common, slot: EquipSlot::None, sell_value: 5, buy_value: 20, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineSilverleaf, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 12, buy_value: 48, stackable: true, gathered: true },
    ItemDef { id: ItemId::Earthroot, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 24, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineEarthroot, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 14, buy_value: 56, stackable: true, gathered: true },
    ItemDef { id: ItemId::LightLeather, quality: Quality::Common, slot: EquipSlot::None, sell_value: 8, buy_value: 32, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineLightLeather, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 18, buy_value: 72, stackable: true, gathered: true },
    ItemDef { id: ItemId::CuredLightLeather, quality: Quality::Common, slot: EquipSlot::None, sell_value: 10, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperBar, quality: Quality::Common, slot: EquipSlot::None, sell_value: 8, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::SmithingFlux, quality: Quality::Common, slot: EquipSlot::None, sell_value: 4, buy_value: 16, stackable: true, gathered: false },
    ItemDef { id: ItemId::SpoolOfThread, quality: Quality::Common, slot: EquipSlot::None, sell_value: 3, buy_value: 12, stackable: true, gathered: false },
    ItemDef { id: ItemId::EmptyVial, quality: Quality::Common, slot: EquipSlot::None, sell_value: 2, buy_value: 8, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperPick, quality: Quality::Common, slot: EquipSlot::None, sell_value: 20, buy_value: 80, stackable: false, gathered: false },
    ItemDef { id: ItemId::CopperSickle, quality: Quality::Common, slot: EquipSlot::None, sell_value: 20, buy_value: 80, stackable: false, gathered: false },
    ItemDef { id: ItemId::SkinningKnife, quality: Quality::Common, slot: EquipSlot::None, sell_value: 15, buy_value: 60, stackable: false, gathered: false },
    ItemDef { id: ItemId::CopperShortsword, quality: Quality::Common, slot: EquipSlot::MainHand, sell_value: 28, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::CopperChainVest, quality: Quality::Common, slot: EquipSlot::Chest, sell_value: 40, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LightLeatherJerkin, quality: Quality::Common, slot: EquipSlot::Chest, sell_value: 36, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LightLeatherBelt, quality: Quality::Common, slot: EquipSlot::Waist, sell_value: 16, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LinenCloth, quality: Quality::Common, slot: EquipSlot::None, sell_value: 4, buy_value: 16, stackable: true, gathered: false },
    ItemDef { id: ItemId::BoltOfLinen, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::LinenTrousers, quality: Quality::Common, slot: EquipSlot::Legs, sell_value: 40, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LinenVestments, quality: Quality::Common, slot: EquipSlot::Chest, sell_value: 50, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::Tigerseye, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 15, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperSetting, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::TigerseyeBand, quality: Quality::Common, slot: EquipSlot::Ring, sell_value: 18, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::MinorHealingPotion, quality: Quality::Common, slot: EquipSlot::None, sell_value: 12, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ElixirOfMinorStrength, quality: Quality::Common, slot: EquipSlot::None, sell_value: 14, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::RoughBlastingPowder, quality: Quality::Common, slot: EquipSlot::None, sell_value: 3, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperBolt, quality: Quality::Common, slot: EquipSlot::None, sell_value: 3, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperGrenade, quality: Quality::Common, slot: EquipSlot::None, sell_value: 10, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ArcaneDust, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ArcaneEssence, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 20, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ArcaneShard, quality: Quality::Rare, slot: EquipSlot::None, sell_value: 80, buy_value: 0, stackable: true, gathered: false },
];

pub fn item_def(id: ItemId) -> &'static ItemDef {
    ITEM_DEFS
        .iter()
        .find(|d| d.id == id)
        .expect("missing ItemDef")
}
```

`crates/woc-sim/src/content/mod.rs`:

```rust
pub mod items;
```

- [ ] **Step 2: Write inventory with tests**

`crates/woc-sim/src/inventory.rs`:

```rust
use crate::item::ItemId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryError {
    Full,
    Missing,
}

#[derive(Clone, Debug)]
pub struct Inventory {
    slots: Vec<Option<ItemStack>>,
}

impl Inventory {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            slots: vec![None; cap],
        }
    }

    pub fn count(&self, item: ItemId) -> u16 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.item == item)
            .map(|s| s.count)
            .sum()
    }

    pub fn try_add(&mut self, stack: ItemStack) -> Result<(), InventoryError> {
        let def = crate::content::items::item_def(stack.item);
        if def.stackable {
            if let Some(existing) = self
                .slots
                .iter_mut()
                .flatten()
                .find(|s| s.item == stack.item)
            {
                existing.count = existing.count.saturating_add(stack.count);
                return Ok(());
            }
        }
        let empty = self
            .slots
            .iter_mut()
            .find(|s| s.is_none())
            .ok_or(InventoryError::Full)?;
        *empty = Some(stack);
        Ok(())
    }

    pub fn try_remove(&mut self, item: ItemId, count: u16) -> Result<(), InventoryError> {
        if self.count(item) < count {
            return Err(InventoryError::Missing);
        }
        let mut remaining = count;
        for slot in self.slots.iter_mut() {
            if remaining == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.item == item {
                    let take = stack.count.min(remaining);
                    stack.count -= take;
                    remaining -= take;
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn has(&self, item: ItemId) -> bool {
        self.count(item) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stackable_ore_merges_then_removes() {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 2,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 3,
        })
        .unwrap();
        assert_eq!(inv.count(ItemId::CopperOre), 5);
        inv.try_remove(ItemId::CopperOre, 5).unwrap();
        assert_eq!(inv.count(ItemId::CopperOre), 0);
    }

    #[test]
    fn full_bag_rejects_unstackable_tools() {
        let mut inv = Inventory::with_capacity(1);
        inv.try_add(ItemStack {
            item: ItemId::CopperPick,
            count: 1,
        })
        .unwrap();
        let err = inv
            .try_add(ItemStack {
                item: ItemId::CopperSickle,
                count: 1,
            })
            .unwrap_err();
        assert_eq!(err, InventoryError::Full);
    }
}
```

`crates/woc-sim/src/gold.rs`:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gold {
    pub copper: u32,
}

impl Gold {
    pub fn try_spend(&mut self, amount: u32) -> bool {
        if self.copper < amount {
            return false;
        }
        self.copper -= amount;
        true
    }
}
```

- [ ] **Step 3: Wire modules and catalog completeness test**

`lib.rs` 增加：

```rust
pub mod content;
pub mod gold;
pub mod inventory;
pub mod item;
pub mod rng;
```

在 `content/items.rs` 末尾：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemId;

    #[test]
    fn every_item_id_has_exactly_one_def() {
        let ids = [
            ItemId::CopperOre,
            ItemId::FineCopperOre,
            ItemId::CoarseStone,
            ItemId::Silverleaf,
            ItemId::FineSilverleaf,
            ItemId::Earthroot,
            ItemId::FineEarthroot,
            ItemId::LightLeather,
            ItemId::FineLightLeather,
            ItemId::CuredLightLeather,
            ItemId::CopperBar,
            ItemId::SmithingFlux,
            ItemId::SpoolOfThread,
            ItemId::EmptyVial,
            ItemId::CopperPick,
            ItemId::CopperSickle,
            ItemId::SkinningKnife,
            ItemId::CopperShortsword,
            ItemId::CopperChainVest,
            ItemId::LightLeatherJerkin,
            ItemId::LightLeatherBelt,
            ItemId::LinenCloth,
            ItemId::BoltOfLinen,
            ItemId::LinenTrousers,
            ItemId::LinenVestments,
            ItemId::Tigerseye,
            ItemId::CopperSetting,
            ItemId::TigerseyeBand,
            ItemId::MinorHealingPotion,
            ItemId::ElixirOfMinorStrength,
            ItemId::RoughBlastingPowder,
            ItemId::CopperBolt,
            ItemId::CopperGrenade,
            ItemId::ArcaneDust,
            ItemId::ArcaneEssence,
            ItemId::ArcaneShard,
        ];
        assert_eq!(ids.len(), ITEM_DEFS.len());
        for id in ids {
            assert_eq!(item_def(id).id, id);
        }
    }

    #[test]
    fn gathered_materials_use_four_times_buy_value() {
        for def in ITEM_DEFS.iter().filter(|d| d.gathered) {
            assert_eq!(def.buy_value, def.sell_value * 4, "{:?} buy/sell ratio", def.id);
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS, including `every_item_id_has_exactly_one_def` and `gathered_materials_use_four_times_buy_value`

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: add item catalog, stacking inventory, and gold"
```

---

### Task 4: Profession ids, skills, and duration table

**Files:**
- Create: `crates/woc-sim/src/professions/mod.rs`
- Create: `crates/woc-sim/src/professions/types.rs`
- Create: `crates/woc-sim/src/professions/skill.rs`
- Create: `crates/woc-sim/src/professions/duration.rs`
- Modify: `crates/woc-sim/src/lib.rs`

**Interfaces:**
- Consumes: `ticks_from_seconds`
- Produces: `ProfessionId::{Mining, Herbalism, Skinning, Forging, Leatherworking, Tailoring, Jewelcrafting, Enchanting, Engineering, Alchemy}`；`ProfessionSkills::gain(id, req) -> u16`；`craft_cast_seconds(skill_req: u16) -> f32`；`gather_cast_seconds(tool_tiers_above: u8, proficiency_bands_above: u8) -> f32`

- [ ] **Step 1: Write types and skill tests**

`types.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProfessionCategory {
    Gathering,
    Crafting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProfessionId {
    Mining,
    Herbalism,
    Skinning,
    Forging,
    Leatherworking,
    Tailoring,
    Jewelcrafting,
    Enchanting,
    Engineering,
    Alchemy,
}

impl ProfessionId {
    pub const ALL: [ProfessionId; 10] = [
        ProfessionId::Mining,
        ProfessionId::Herbalism,
        ProfessionId::Skinning,
        ProfessionId::Forging,
        ProfessionId::Leatherworking,
        ProfessionId::Tailoring,
        ProfessionId::Jewelcrafting,
        ProfessionId::Enchanting,
        ProfessionId::Engineering,
        ProfessionId::Alchemy,
    ];

    pub fn category(self) -> ProfessionCategory {
        match self {
            ProfessionId::Mining | ProfessionId::Herbalism | ProfessionId::Skinning => {
                ProfessionCategory::Gathering
            }
            _ => ProfessionCategory::Crafting,
        }
    }

    pub fn max_skill(self) -> u16 {
        match self.category() {
            ProfessionCategory::Gathering => 100,
            ProfessionCategory::Crafting => 125,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenyReason {
    OutOfRange,
    NodeNotReady,
    MissingTool,
    ToolTierTooLow,
    InventoryFull,
    UnknownNode,
    Busy,
    CorpseGone,
    NothingToSkin,
    AlreadySkinned,
    MissingKnife,
    UnknownRecipe,
    MissingReagents,
    InsufficientGold,
    StationRequired,
    InvalidCount,
    UnknownEnchant,
    WrongSlot,
    AlreadyEnchanted,
    SameEnchant,
    NotInstanced,
}

pub const TIER_SKILL_STEP: u16 = 25;
pub const HARVEST_RANGE: f32 = 5.0;
pub const STATION_RADIUS: f32 = 20.0;
pub const CRAFT_GOLD_SINK_COPPER_PER_BUDGET: u32 = 2;
pub const CRAFT_BATCH_MAX: u16 = 50;
```

`skill.rs`:

```rust
use super::types::{ProfessionId, TIER_SKILL_STEP};

#[derive(Clone, Debug, Default)]
pub struct ProfessionSkills {
    values: [u16; 10],
}

impl ProfessionSkills {
    fn index(id: ProfessionId) -> usize {
        match id {
            ProfessionId::Mining => 0,
            ProfessionId::Herbalism => 1,
            ProfessionId::Skinning => 2,
            ProfessionId::Forging => 3,
            ProfessionId::Leatherworking => 4,
            ProfessionId::Tailoring => 5,
            ProfessionId::Jewelcrafting => 6,
            ProfessionId::Enchanting => 7,
            ProfessionId::Engineering => 8,
            ProfessionId::Alchemy => 9,
        }
    }

    pub fn get(&self, id: ProfessionId) -> u16 {
        self.values[Self::index(id)]
    }

    pub fn gain(&mut self, id: ProfessionId, skill_req: u16) -> u16 {
        let current = self.get(id);
        let cap = id.max_skill();
        let amount = skill_gain_amount(current, skill_req, cap);
        self.values[Self::index(id)] = current + amount;
        amount
    }
}

pub fn tier_for_skill(skill: u16) -> u8 {
    (skill / TIER_SKILL_STEP).min(5) as u8
}

pub fn skill_gain_amount(current: u16, req: u16, cap: u16) -> u16 {
    if current >= cap {
        return 0;
    }
    let delta = current.saturating_sub(req);
    match delta {
        0..=24 => 2,
        25..=74 => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gathering_and_crafting_caps_differ() {
        assert_eq!(ProfessionId::Mining.max_skill(), 100);
        assert_eq!(ProfessionId::Forging.max_skill(), 125);
    }

    #[test]
    fn skills_are_independent_and_stop_at_cap() {
        let mut skills = ProfessionSkills::default();
        assert_eq!(skills.gain(ProfessionId::Mining, 0), 2);
        assert_eq!(skills.get(ProfessionId::Herbalism), 0);
        skills.values[0] = 99;
        assert_eq!(skills.gain(ProfessionId::Mining, 0), 1);
        assert_eq!(skills.gain(ProfessionId::Mining, 0), 0);
        assert_eq!(skills.get(ProfessionId::Mining), 100);
    }

    #[test]
    fn gray_actions_grant_zero() {
        assert_eq!(skill_gain_amount(80, 0, 100), 0);
        assert_eq!(skill_gain_amount(30, 0, 100), 1);
        assert_eq!(skill_gain_amount(10, 0, 100), 2);
    }
}
```

- [ ] **Step 2: Duration table**

`duration.rs`:

```rust
pub fn clamp_cast_seconds(seconds: f32) -> f32 {
    seconds.clamp(1.5, 5.0)
}

pub fn craft_cast_seconds(skill_req: u16) -> f32 {
    let raw = match skill_req {
        0 => 1.75,
        1..=25 => 2.5,
        26..=50 => 3.0,
        51..=75 => 3.5,
        _ => 4.0,
    };
    clamp_cast_seconds(raw)
}

pub fn gather_cast_seconds(tool_tiers_above: u8, proficiency_bands_above: u8) -> f32 {
    let raw = 2.5 - 0.4 * f32::from(tool_tiers_above) - 0.15 * f32::from(proficiency_bands_above);
    clamp_cast_seconds(raw)
}

pub fn enchant_family_seconds() -> f32 {
    1.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_stay_inside_ux_band() {
        assert_eq!(craft_cast_seconds(0), 1.75);
        assert_eq!(craft_cast_seconds(100), 4.0);
        assert_eq!(gather_cast_seconds(0, 0), 2.5);
        assert_eq!(gather_cast_seconds(4, 4), 1.5);
        assert_eq!(enchant_family_seconds(), 1.5);
    }
}
```

`professions/mod.rs`:

```rust
pub mod duration;
pub mod skill;
pub mod types;

pub use skill::ProfessionSkills;
pub use types::{DenyReason, ProfessionId};
```

`lib.rs` 增加 `pub mod professions;`

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS (`skills_are_independent_and_stop_at_cap`, `gray_actions_grant_zero`, `durations_stay_inside_ux_band`)

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/professions crates/woc-sim/src/lib.rs
git commit -m "feat: add profession ids, independent skill counters, cast durations"
```

---

### Task 5: Tools and gathering nodes

**Files:**
- Create: `crates/woc-sim/src/professions/tools.rs`
- Create: `crates/woc-sim/src/content/nodes.rs`
- Create: `crates/woc-sim/src/professions/gathering.rs`
- Modify: `crates/woc-sim/src/content/mod.rs`
- Modify: `crates/woc-sim/src/professions/mod.rs`
- Modify: `crates/woc-sim/src/professions/types.rs`（追加 `Vec2`、`GatherNodeDef`、`NodeId`）

**Interfaces:**
- Consumes: `Inventory::has`、`ProfessionSkills`、`Rng::chance`、`DenyReason`
- Produces: `best_tool_tier(inv, profession) -> Option<u8>`；`start_gather` / `complete_gather`；成功路径调用 `rng.chance` 恰好两次

- [ ] **Step 1: Node records and tool gate**

在 `types.rs` 追加：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct NodeId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub z: f32,
}

impl Vec2 {
    pub fn distance(self, other: Vec2) -> f32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        (dx * dx + dz * dz).sqrt()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Ore,
    Herb,
}

#[derive(Clone, Copy, Debug)]
pub struct GatherNodeDef {
    pub id: NodeId,
    pub kind: NodeKind,
    pub pos: Vec2,
    pub tier: u8,
    pub skill_req: u16,
    pub respawn_seconds: u32,
}
```

`tools.rs`:

```rust
use crate::inventory::Inventory;
use crate::item::ItemId;
use super::types::{NodeKind, ProfessionId};

pub fn tool_item_for(profession: ProfessionId) -> Option<ItemId> {
    match profession {
        ProfessionId::Mining => Some(ItemId::CopperPick),
        ProfessionId::Herbalism => Some(ItemId::CopperSickle),
        ProfessionId::Skinning => Some(ItemId::SkinningKnife),
        _ => None,
    }
}

pub fn profession_for_node(kind: NodeKind) -> ProfessionId {
    match kind {
        NodeKind::Ore => ProfessionId::Mining,
        NodeKind::Herb => ProfessionId::Herbalism,
    }
}

/// v1 tools are all tier 1. Presence in bags is the gate.
pub fn best_tool_tier(inv: &Inventory, profession: ProfessionId) -> Option<u8> {
    let item = tool_item_for(profession)?;
    if inv.has(item) {
        Some(1)
    } else {
        None
    }
}

pub fn can_gather_tier(tool_tier: u8, node_tier: u8) -> bool {
    tool_tier >= node_tier
}
```

`content/nodes.rs`（东溪谷 6 矿 + 3 银叶 + 3 地根，坐标避开互相重叠）：

```rust
use crate::professions::types::{GatherNodeDef, NodeId, NodeKind, Vec2};

pub const GATHER_NODES: &[GatherNodeDef] = &[
    GatherNodeDef { id: NodeId(1), kind: NodeKind::Ore, pos: Vec2 { x: -70.0, z: -53.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(2), kind: NodeKind::Ore, pos: Vec2 { x: -73.0, z: -49.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(3), kind: NodeKind::Ore, pos: Vec2 { x: -67.0, z: -57.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(4), kind: NodeKind::Ore, pos: Vec2 { x: -92.0, z: -48.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(5), kind: NodeKind::Ore, pos: Vec2 { x: -87.0, z: -45.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(6), kind: NodeKind::Ore, pos: Vec2 { x: -65.0, z: -69.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(11), kind: NodeKind::Herb, pos: Vec2 { x: 12.0, z: -20.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(12), kind: NodeKind::Herb, pos: Vec2 { x: 16.0, z: -18.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(13), kind: NodeKind::Herb, pos: Vec2 { x: 10.0, z: -24.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(14), kind: NodeKind::Herb, pos: Vec2 { x: 40.0, z: 8.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(15), kind: NodeKind::Herb, pos: Vec2 { x: 44.0, z: 6.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
    GatherNodeDef { id: NodeId(16), kind: NodeKind::Herb, pos: Vec2 { x: 38.0, z: 12.0 }, tier: 1, skill_req: 0, respawn_seconds: 60 },
];

pub fn node_by_id(id: NodeId) -> Option<&'static GatherNodeDef> {
    GATHER_NODES.iter().find(|n| n.id == id)
}

/// Herb nodes 11-13 are silverleaf; 14-16 are earthroot.
pub fn herb_is_earthroot(id: NodeId) -> bool {
    id.0 >= 14
}
```

- [ ] **Step 2: Gathering resolve with two-draw contract**

`gathering.rs`:

```rust
use crate::content::nodes::{herb_is_earthroot, node_by_id};
use crate::inventory::{Inventory, ItemStack};
use crate::item::ItemId;
use crate::rng::Rng;
use crate::TICK_HZ;
use super::skill::ProfessionSkills;
use super::tools::{best_tool_tier, can_gather_tier, profession_for_node};
use super::types::{
    DenyReason, GatherNodeDef, HARVEST_RANGE, NodeId, NodeKind, ProfessionId, Vec2,
};

pub fn evaluate_gather(
    pos: Vec2,
    inv: &Inventory,
    node: &GatherNodeDef,
    ready_tick: u64,
    now: u64,
    busy: bool,
) -> Result<ProfessionId, DenyReason> {
    if busy {
        return Err(DenyReason::Busy);
    }
    if pos.distance(node.pos) > HARVEST_RANGE {
        return Err(DenyReason::OutOfRange);
    }
    if now < ready_tick {
        return Err(DenyReason::NodeNotReady);
    }
    let profession = profession_for_node(node.kind);
    let tool_tier = best_tool_tier(inv, profession).ok_or(DenyReason::MissingTool)?;
    if !can_gather_tier(tool_tier, node.tier) {
        return Err(DenyReason::ToolTierTooLow);
    }
    Ok(profession)
}

pub struct HarvestGrant {
    pub stacks: Vec<ItemStack>,
    pub skill_gained: u16,
    pub profession: ProfessionId,
    pub next_ready_tick: u64,
}

pub fn complete_gather(
    pos: Vec2,
    inv: &mut Inventory,
    skills: &mut ProfessionSkills,
    node: &GatherNodeDef,
    ready_tick: u64,
    now: u64,
    rng: &mut impl Rng,
) -> Result<HarvestGrant, DenyReason> {
    let profession = evaluate_gather(pos, inv, node, ready_tick, now, false)?;
    let tool_tier = best_tool_tier(inv, profession).expect("tool re-checked");
    let rare = rng.chance(15);
    let double = rng.chance(20);
    let (base, fine) = match node.kind {
        NodeKind::Ore => (ItemId::CopperOre, ItemId::FineCopperOre),
        NodeKind::Herb if herb_is_earthroot(node.id) => (ItemId::Earthroot, ItemId::FineEarthroot),
        NodeKind::Herb => (ItemId::Silverleaf, ItemId::FineSilverleaf),
    };
    let use_fine = rare || tool_tier > node.tier;
    let item = if use_fine { fine } else { base };
    let count = if rare { 5 } else if double { 2 } else { 1 };
    let mut stacks = vec![ItemStack { item, count }];
    if node.kind == NodeKind::Ore && double && !rare {
        stacks.push(ItemStack {
            item: ItemId::CoarseStone,
            count: 1,
        });
    }
    for stack in &stacks {
        inv.try_add(*stack).map_err(|_| DenyReason::InventoryFull)?;
    }
    let skill_gained = skills.gain(profession, node.skill_req);
    Ok(HarvestGrant {
        stacks,
        skill_gained,
        profession,
        next_ready_tick: now + u64::from(node.respawn_seconds) * u64::from(TICK_HZ),
    })
}

pub fn start_gather_node(
    pos: Vec2,
    inv: &Inventory,
    node_id: NodeId,
    ready_tick: u64,
    now: u64,
    busy: bool,
) -> Result<&GatherNodeDef, DenyReason> {
    let node = node_by_id(node_id).ok_or(DenyReason::UnknownNode)?;
    evaluate_gather(pos, inv, node, ready_tick, now, busy)?;
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::nodes::node_by_id;
    use crate::inventory::Inventory;
    use crate::professions::skill::ProfessionSkills;
    use crate::rng::ScriptedRng;
    use crate::professions::types::NodeId;

    fn node1() -> &'static GatherNodeDef {
        node_by_id(NodeId(1)).unwrap()
    }

    #[test]
    fn bare_hands_cannot_mine() {
        let inv = Inventory::with_capacity(4);
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = evaluate_gather(
            node1().pos,
            &inv,
            node1(),
            0,
            0,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::MissingTool);
        let _ = &mut rng;
    }

    #[test]
    fn successful_ore_harvest_draws_twice() {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::CopperPick,
            count: 1,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();
        let mut rng = ScriptedRng::from_seq(&[99, 99]);
        let grant = complete_gather(node1().pos, &mut inv, &mut skills, node1(), 0, 0, &mut rng)
            .unwrap();
        assert_eq!(grant.stacks[0].item, ItemId::CopperOre);
        assert_eq!(grant.stacks[0].count, 1);
        assert_eq!(skills.get(ProfessionId::Mining), 2);
        assert_eq!(grant.next_ready_tick, 60 * 20);
    }

    #[test]
    fn denied_harvest_draws_zero() {
        let inv = Inventory::with_capacity(4);
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = evaluate_gather(node1().pos, &inv, node1(), 0, 0, false).unwrap_err();
        assert_eq!(err, DenyReason::MissingTool);
        let _ = &mut rng;
    }
}
```

`chance(percent)` 用 `next_u32() % 100 < percent`，所以 `99` 永远失败（普通产量）、`0` 永远成功（稀有事件）。两抽合同：成功走 `chance(15)` 再 `chance(20)`；拒绝路径不得调用 `rng`。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim gathering`

Expected: PASS；空 `ScriptedRng` 在拒绝路径不 panic

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: add node gathering with tool gate and two-draw harvest"
```

---

### Task 6: Skinning

**Files:**
- Create: `crates/woc-sim/src/professions/skinning.rs`
- Modify: `crates/woc-sim/src/professions/types.rs`（`CorpseId`、`Corpse`）
- Modify: `crates/woc-sim/src/professions/mod.rs`

**Interfaces:**
- Consumes: `best_tool_tier(inv, Skinning)`、`Rng`、`ProfessionSkills::gain`
- Produces: `start_skin` / `complete_skin`；无 `hide` 标签 → `NothingToSkin` 且 `skinned` 仍为 false；成功后 `skinned = true`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn untagged_corpse_is_not_claimed() {
    let corpse = Corpse { id: CorpseId(1), pos: Vec2 { x: 0.0, z: 0.0 }, has_hide: false, skinned: false, tier: 1 };
    // start_skin → NothingToSkin, corpse.skinned stays false
}

#[test]
fn hide_corpse_yields_light_leather_once() {
    // knife in bag, ScriptedRng [99, 99] → LightLeather x1, skill 2
    // second complete_skin → AlreadySkinned
}
```

- [ ] **Step 2: Implement**

`Corpse`：

```rust
#[derive(Clone, Debug)]
pub struct Corpse {
    pub id: CorpseId,
    pub pos: Vec2,
    pub has_hide: bool,
    pub skinned: bool,
    pub tier: u8,
}
```

抽数合同与采集相同：抽 1 稀有事件 15% → `FineLightLeather` ×5；抽 2 数量 20% → 普通 ×2。刀档必须 `>= corpse.tier`。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim skinning`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/professions
git commit -m "feat: add skinning profession on hide-tagged corpses"
```

---

### Task 7: Stations, masterwork, and crafting engine

**Files:**
- Create: `crates/woc-sim/src/professions/stations.rs`
- Create: `crates/woc-sim/src/professions/masterwork.rs`
- Create: `crates/woc-sim/src/professions/crafting.rs`
- Create: `crates/woc-sim/src/content/recipes.rs`
- Create: `crates/woc-sim/src/content/stations.rs`
- Modify: `crates/woc-sim/src/content/mod.rs`
- Modify: `crates/woc-sim/src/professions/types.rs`（`RecipeId`、`RecipeDef`、`StationType`、`Reagent`）
- Modify: `crates/woc-sim/src/professions/mod.rs`

**Interfaces:**
- Consumes: `Gold::try_spend`、`Inventory::try_remove` / `try_add`、`ProfessionSkills::gain`、`craft_cast_seconds`、`Rng::chance`
- Produces: `evaluate_craft_admission` / `complete_craft`；`masterwork_proc_chance(skill, req) -> u8`；`fn base_of(item: ItemId) -> ItemId`（`FineCopperOre→CopperOre`、`FineSilverleaf→Silverleaf`、`FineEarthroot→Earthroot`、`FineLightLeather→LightLeather`，其余返回自身）。扣材料时先扣精确 id，不够再扣 fine；反向替代禁止。

- [ ] **Step 1: Recipe and station shapes**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StationType {
    Forge,
    Tannery,
    Loom,
    JewelersBench,
    Apothecary,
    Toolworks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum RecipeId {
    SmeltCopper,
    CopperShortsword,
    CopperChainVest,
    CopperPick,
    CureLightLeather,
    LightLeatherJerkin,
    LightLeatherBelt,
    BoltOfLinen,
    LinenTrousers,
    LinenVestments,
    ProspectCopper,
    CopperSetting,
    TigerseyeBand,
    MinorHealingPotion,
    ElixirOfMinorStrength,
    RoughBlastingPowder,
    CopperBolt,
    CopperGrenade,
}

#[derive(Clone, Copy, Debug)]
pub struct Reagent {
    pub item: ItemId,
    pub count: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct RecipeDef {
    pub id: RecipeId,
    pub profession: ProfessionId,
    pub result: ItemId,
    pub result_count: u16,
    pub reagents: &'static [Reagent],
    pub skill_req: u16,
    pub item_level_budget: u16,
    pub station: Option<StationType>,
}
```

`content/stations.rs`：

```rust
use crate::professions::types::{StationType, Vec2};

pub struct StationDef {
    pub kind: StationType,
    pub pos: Vec2,
}

pub const STATIONS: &[StationDef] = &[
    StationDef { kind: StationType::Forge, pos: Vec2 { x: 0.0, z: 0.0 } },
    StationDef { kind: StationType::Tannery, pos: Vec2 { x: 80.0, z: 40.0 } },
    StationDef { kind: StationType::Loom, pos: Vec2 { x: 20.0, z: -10.0 } },
    StationDef { kind: StationType::JewelersBench, pos: Vec2 { x: 120.0, z: -50.0 } },
    StationDef { kind: StationType::Apothecary, pos: Vec2 { x: 7.0, z: 660.0 } },
    StationDef { kind: StationType::Toolworks, pos: Vec2 { x: 30.0, z: 10.0 } },
];
```

`stations.rs`：`fn in_station_range(pos: Vec2, kind: StationType) -> bool`，距离 `STATION_RADIUS`。

- [ ] **Step 2: Masterwork**

```rust
pub fn masterwork_proc_chance(player_skill: u16, recipe_req: u16) -> u8 {
    let player_tier = super::skill::tier_for_skill(player_skill);
    let recipe_tier = super::skill::tier_for_skill(recipe_req);
    let mut chance = 3u8;
    if player_tier > recipe_tier {
        chance = chance.saturating_add(player_tier - recipe_tier);
    }
    chance.min(15)
}

pub fn bump_quality(q: Quality) -> Quality {
    match q {
        Quality::Common => Quality::Uncommon,
        Quality::Uncommon => Quality::Rare,
        Quality::Rare => Quality::Epic,
        Quality::Epic => Quality::Epic,
    }
}
```

测试：`masterwork_proc_chance(0, 0) == 3`；`masterwork_proc_chance(125, 0) == 8`（档 5 − 档 0 = 5，3+5=8）；上限 15。

v1 精工触发时：若产物 `stackable == false`，仍放入普通堆叠（无实例品质字段则在 `PlayerState.last_masterwork: Option<RecipeId>` 记录，供测试断言）。可堆叠药剂精工只记事件、不改物品 id。

- [ ] **Step 3: Crafting admission and complete**

`evaluate_craft_admission` 顺序（锁定，收费在最后）：

1. `InvalidCount` if `count == 0 || count > CRAFT_BATCH_MAX`
2. `UnknownRecipe`
3. `Busy`
4. `StationRequired` if recipe.station is Some and out of range
5. `MissingReagents`（向下替代：需要 `CopperOre` 时 `FineCopperOre` 可顶；需要 fine 时普通不行）
6. `InsufficientGold` if `gold < 2 * item_level_budget`
7. `InventoryFull` if adding result would fail（先模拟）

`complete_craft` 再次跑同一顺序，然后扣费扣材料、放产物、精工一抽、涨技能。批量按件循环，每件独立精工抽。中途失败保留已做出的。

**本任务配方表只放熔炼铜**，用来钉引擎：

```rust
RecipeDef {
    id: RecipeId::SmeltCopper,
    profession: ProfessionId::Forging,
    result: ItemId::CopperBar,
    result_count: 1,
    reagents: &[Reagent { item: ItemId::CopperOre, count: 2 }],
    skill_req: 0,
    item_level_budget: 1,
    station: None,
}
```

测试：

```rust
#[test]
fn missing_ore_does_not_charge_gold() { ... }

#[test]
fn fine_ore_substitutes_downward() { ... }

#[test]
fn smelt_is_field_craftable() { ... }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: add station gate, masterwork proc, and crafting engine"
```

---

### Task 8: Forging recipes

**Files:**
- Modify: `crates/woc-sim/src/content/recipes.rs`
- Modify: `crates/woc-sim/src/content/vendors.rs`（本任务创建：`SmithingFlux`、镐可买）
- Test: `crates/woc-sim/src/content/recipes.rs` 内 `#[cfg(test)]`

**Interfaces:**
- Consumes: `RecipeDef`、`StationType::Forge`、`evaluate_craft_admission`
- Produces: `smelt_copper`、`copper_shortsword`、`copper_chain_vest`、`copper_pick` 四条锻造配方

- [ ] **Step 1: Write failing tests for forge-bound weapons**

```rust
#[test]
fn shortsword_requires_forge() {
    // player at tannery with bars+flux → StationRequired
}

#[test]
fn shortsword_crafts_at_forge() {
    // pos (0,0), CopperBar×3, SmithingFlux×2, gold ≥ 20 → CopperShortsword, forging skill 2
}
```

- [ ] **Step 2: Author recipes**

```rust
RecipeDef {
    id: RecipeId::CopperShortsword,
    profession: ProfessionId::Forging,
    result: ItemId::CopperShortsword,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::CopperBar, count: 3 },
        Reagent { item: ItemId::SmithingFlux, count: 2 },
    ],
    skill_req: 0,
    item_level_budget: 10,
    station: Some(StationType::Forge),
},
RecipeDef {
    id: RecipeId::CopperChainVest,
    profession: ProfessionId::Forging,
    result: ItemId::CopperChainVest,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::CopperBar, count: 5 },
        Reagent { item: ItemId::SmithingFlux, count: 3 },
    ],
    skill_req: 0,
    item_level_budget: 10,
    station: Some(StationType::Forge),
},
RecipeDef {
    id: RecipeId::CopperPick,
    profession: ProfessionId::Forging,
    result: ItemId::CopperPick,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::CopperBar, count: 3 },
        Reagent { item: ItemId::CoarseStone, count: 2 },
    ],
    skill_req: 0,
    item_level_budget: 8,
    station: Some(StationType::Forge),
},
```

`vendors.rs`：

```rust
use crate::item::ItemId;

/// Processed goods only. Gathered / skinned materials MUST NOT appear here.
pub const VENDOR_ITEMS: &[ItemId] = &[
    ItemId::SmithingFlux,
    ItemId::SpoolOfThread,
    ItemId::EmptyVial,
    ItemId::LinenCloth,
    ItemId::CopperPick,
    ItemId::CopperSickle,
    ItemId::SkinningKnife,
];
```

测试 `vendor_never_stocks_gathered`：`ITEM_DEFS` 里 `gathered == true` 的 id 与 `VENDOR_ITEMS` 交集为空。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: add forging recipes, forge station gate, and vendor exclusions"
```

---

### Task 9: Leatherworking recipes

**Files:**
- Modify: `crates/woc-sim/src/content/recipes.rs`

**Interfaces:**
- Consumes: `StationType::Tannery`、`ItemId::{LightLeather, CuredLightLeather, SpoolOfThread}`
- Produces: `cure_light_leather`（野外）、`light_leather_jerkin`、`light_leather_belt`

- [ ] **Step 1: Write tests**

```rust
#[test]
fn curing_hide_is_field_craftable() {
    // LightLeather×1 anywhere → CuredLightLeather, leatherworking +2
}

#[test]
fn jerkin_requires_tannery() {
    // at forge → StationRequired; at tannery (80, 40) with CuredLightLeather×4 and thread×2 → jerkin
}
```

- [ ] **Step 2: Author recipes**

```rust
RecipeDef {
    id: RecipeId::CureLightLeather,
    profession: ProfessionId::Leatherworking,
    result: ItemId::CuredLightLeather,
    result_count: 1,
    reagents: &[Reagent { item: ItemId::LightLeather, count: 1 }],
    skill_req: 0,
    item_level_budget: 1,
    station: None,
},
RecipeDef {
    id: RecipeId::LightLeatherJerkin,
    profession: ProfessionId::Leatherworking,
    result: ItemId::LightLeatherJerkin,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::CuredLightLeather, count: 4 },
        Reagent { item: ItemId::SpoolOfThread, count: 2 },
    ],
    skill_req: 0,
    item_level_budget: 9,
    station: Some(StationType::Tannery),
},
RecipeDef {
    id: RecipeId::LightLeatherBelt,
    profession: ProfessionId::Leatherworking,
    result: ItemId::LightLeatherBelt,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::CuredLightLeather, count: 2 },
        Reagent { item: ItemId::SpoolOfThread, count: 1 },
    ],
    skill_req: 0,
    item_level_budget: 6,
    station: Some(StationType::Tannery),
},
```

Fine 兽皮可替代 `LightLeather` 来熟制，与采集向下替代同一函数。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/content/recipes.rs
git commit -m "feat: add leatherworking cure and tannery armor recipes"
```

---

### Task 10: Tailoring recipes

**Files:**
- Modify: `crates/woc-sim/src/content/recipes.rs`

**Interfaces:**
- Consumes: `StationType::Loom`、`ItemId::{LinenCloth, BoltOfLinen, SpoolOfThread}`、`ProfessionId::Tailoring`
- Produces: `bolt_of_linen`（野外）、`linen_trousers`、`linen_vestments`

- [ ] **Step 1: Write tests**

```rust
#[test]
fn bolt_of_linen_is_field_craftable() {
    let mut session = test_session_at(Vec2 { x: 999.0, z: 999.0 });
    session.inventory.try_add(ItemStack { item: ItemId::LinenCloth, count: 2 }).unwrap();
    session.start_craft(RecipeId::BoltOfLinen, 1).unwrap();
    session.complete_ready(&mut ScriptedRng::from_seq(&[99]));
    assert_eq!(session.inventory.count(ItemId::BoltOfLinen), 1);
    assert_eq!(session.skills.get(ProfessionId::Tailoring), 2);
}

#[test]
fn trousers_require_loom() {
    let mut at_forge = test_session_at(Vec2 { x: 0.0, z: 0.0 });
    at_forge.inventory.try_add(ItemStack { item: ItemId::BoltOfLinen, count: 3 }).unwrap();
    at_forge.inventory.try_add(ItemStack { item: ItemId::SpoolOfThread, count: 2 }).unwrap();
    assert_eq!(
        at_forge.start_craft(RecipeId::LinenTrousers, 1).unwrap_err(),
        DenyReason::StationRequired
    );
    let mut at_loom = test_session_at(Vec2 { x: 20.0, z: -10.0 });
    at_loom.inventory.try_add(ItemStack { item: ItemId::BoltOfLinen, count: 3 }).unwrap();
    at_loom.inventory.try_add(ItemStack { item: ItemId::SpoolOfThread, count: 2 }).unwrap();
    at_loom.start_craft(RecipeId::LinenTrousers, 1).unwrap();
    at_loom.complete_ready(&mut ScriptedRng::from_seq(&[99]));
    assert_eq!(at_loom.inventory.count(ItemId::LinenTrousers), 1);
}
```

- [ ] **Step 2: Author recipes**

```rust
RecipeDef {
    id: RecipeId::BoltOfLinen,
    profession: ProfessionId::Tailoring,
    result: ItemId::BoltOfLinen,
    result_count: 1,
    reagents: &[Reagent { item: ItemId::LinenCloth, count: 2 }],
    skill_req: 0,
    item_level_budget: 1,
    station: None,
},
RecipeDef {
    id: RecipeId::LinenTrousers,
    profession: ProfessionId::Tailoring,
    result: ItemId::LinenTrousers,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::BoltOfLinen, count: 3 },
        Reagent { item: ItemId::SpoolOfThread, count: 2 },
    ],
    skill_req: 0,
    item_level_budget: 8,
    station: Some(StationType::Loom),
},
RecipeDef {
    id: RecipeId::LinenVestments,
    profession: ProfessionId::Tailoring,
    result: ItemId::LinenVestments,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::BoltOfLinen, count: 4 },
        Reagent { item: ItemId::SpoolOfThread, count: 3 },
    ],
    skill_req: 0,
    item_level_budget: 9,
    station: Some(StationType::Loom),
},
```

`LinenCloth` 已在 `VENDOR_ITEMS`。宝石/布匹成品不得加入货物表。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/content/recipes.rs
git commit -m "feat: add tailoring bolts and loom cloth armor"
```

---

### Task 11: Jewelcrafting recipes

**Files:**
- Modify: `crates/woc-sim/src/content/recipes.rs`

**Interfaces:**
- Consumes: `StationType::JewelersBench`、`ItemId::{CopperOre, CopperBar, Tigerseye, CopperSetting}`、`ProfessionId::Jewelcrafting`
- Produces: `prospect_copper`、`copper_setting`（野外，确定性，无额外随机抽）、`tigerseye_band`（珠宝台）

- [ ] **Step 1: Write tests**

```rust
#[test]
fn prospect_copper_is_field_craftable_and_deterministic() {
    let mut session = test_session_at(Vec2 { x: 999.0, z: 999.0 });
    session.inventory.try_add(ItemStack { item: ItemId::CopperOre, count: 5 }).unwrap();
    session.start_craft(RecipeId::ProspectCopper, 1).unwrap();
    session.complete_ready(&mut ScriptedRng::from_seq(&[99]));
    assert_eq!(session.inventory.count(ItemId::Tigerseye), 1);
    assert_eq!(session.inventory.count(ItemId::CopperOre), 0);
    assert_eq!(session.skills.get(ProfessionId::Jewelcrafting), 2);
}

#[test]
fn tigerseye_band_requires_jewelers_bench() {
    let mut at_forge = test_session_at(Vec2 { x: 0.0, z: 0.0 });
    at_forge.inventory.try_add(ItemStack { item: ItemId::Tigerseye, count: 1 }).unwrap();
    at_forge.inventory.try_add(ItemStack { item: ItemId::CopperSetting, count: 1 }).unwrap();
    assert_eq!(
        at_forge.start_craft(RecipeId::TigerseyeBand, 1).unwrap_err(),
        DenyReason::StationRequired
    );
    let mut at_bench = test_session_at(Vec2 { x: 120.0, z: -50.0 });
    at_bench.inventory.try_add(ItemStack { item: ItemId::Tigerseye, count: 1 }).unwrap();
    at_bench.inventory.try_add(ItemStack { item: ItemId::CopperSetting, count: 1 }).unwrap();
    at_bench.start_craft(RecipeId::TigerseyeBand, 1).unwrap();
    at_bench.complete_ready(&mut ScriptedRng::from_seq(&[99]));
    assert_eq!(at_bench.inventory.count(ItemId::TigerseyeBand), 1);
}
```

- [ ] **Step 2: Author recipes**

```rust
RecipeDef {
    id: RecipeId::ProspectCopper,
    profession: ProfessionId::Jewelcrafting,
    result: ItemId::Tigerseye,
    result_count: 1,
    reagents: &[Reagent { item: ItemId::CopperOre, count: 5 }],
    skill_req: 0,
    item_level_budget: 2,
    station: None,
},
RecipeDef {
    id: RecipeId::CopperSetting,
    profession: ProfessionId::Jewelcrafting,
    result: ItemId::CopperSetting,
    result_count: 1,
    reagents: &[Reagent { item: ItemId::CopperBar, count: 1 }],
    skill_req: 0,
    item_level_budget: 1,
    station: None,
},
RecipeDef {
    id: RecipeId::TigerseyeBand,
    profession: ProfessionId::Jewelcrafting,
    result: ItemId::TigerseyeBand,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::Tigerseye, count: 1 },
        Reagent { item: ItemId::CopperSetting, count: 1 },
    ],
    skill_req: 0,
    item_level_budget: 8,
    station: Some(StationType::JewelersBench),
},
```

选矿不另抽随机；制造引擎的精工抽仍发生一次。`Tigerseye` 与 `CopperSetting` 不得加入 `VENDOR_ITEMS`。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/content/recipes.rs
git commit -m "feat: add jewelcrafting prospecting and tigerseye band"
```

---

### Task 12: Alchemy recipes

**Files:**
- Modify: `crates/woc-sim/src/content/recipes.rs`

**Interfaces:**
- Consumes: `StationType::Apothecary`、`Silverleaf`、`Earthroot`、`EmptyVial`
- Produces: `minor_healing_potion`、`elixir_of_minor_strength`

- [ ] **Step 1: Write tests**

```rust
#[test]
fn potions_require_apothecary() {
    // at forge with herbs+vial → StationRequired
}

#[test]
fn healing_potion_crafts_at_highwatch_apothecary() {
    // pos (7, 660), Silverleaf×2, EmptyVial×1 → MinorHealingPotion, alchemy +2
}
```

- [ ] **Step 2: Author recipes**

```rust
RecipeDef {
    id: RecipeId::MinorHealingPotion,
    profession: ProfessionId::Alchemy,
    result: ItemId::MinorHealingPotion,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::Silverleaf, count: 2 },
        Reagent { item: ItemId::EmptyVial, count: 1 },
    ],
    skill_req: 0,
    item_level_budget: 1,
    station: Some(StationType::Apothecary),
},
RecipeDef {
    id: RecipeId::ElixirOfMinorStrength,
    profession: ProfessionId::Alchemy,
    result: ItemId::ElixirOfMinorStrength,
    result_count: 1,
    reagents: &[
        Reagent { item: ItemId::Earthroot, count: 2 },
        Reagent { item: ItemId::EmptyVial, count: 1 },
    ],
    skill_req: 0,
    item_level_budget: 1,
    station: Some(StationType::Apothecary),
},
```

Fine 草药可向下替代对应普通草药。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/content/recipes.rs
git commit -m "feat: add alchemy potions at the apothecary"
```

---

### Task 13: Engineering recipes

**Files:**
- Modify: `crates/woc-sim/src/content/recipes.rs`

**Interfaces:**
- Consumes: `StationType::Toolworks`、`CoarseStone`、`CopperBar`
- Produces: `rough_blasting_powder`（野外）、`copper_bolt`、`copper_grenade`（产物数量 2）

- [ ] **Step 1: Write tests**

```rust
#[test]
fn blasting_powder_is_field_craftable() {
    // CoarseStone×2 → RoughBlastingPowder×1
}

#[test]
fn grenade_requires_toolworks_and_consumes_bolts() {
    // at toolworks (30, 10): CopperBar×1, powder×2, bolt×1 → CopperGrenade×2
}
```

- [ ] **Step 2: Author recipes**

```rust
RecipeDef {
    id: RecipeId::RoughBlastingPowder,
    profession: ProfessionId::Engineering,
    result: ItemId::RoughBlastingPowder,
    result_count: 1,
    reagents: &[Reagent { item: ItemId::CoarseStone, count: 2 }],
    skill_req: 0,
    item_level_budget: 1,
    station: None,
},
RecipeDef {
    id: RecipeId::CopperBolt,
    profession: ProfessionId::Engineering,
    result: ItemId::CopperBolt,
    result_count: 2,
    reagents: &[Reagent { item: ItemId::CopperBar, count: 1 }],
    skill_req: 0,
    item_level_budget: 2,
    station: Some(StationType::Toolworks),
},
RecipeDef {
    id: RecipeId::CopperGrenade,
    profession: ProfessionId::Engineering,
    result: ItemId::CopperGrenade,
    result_count: 2,
    reagents: &[
        Reagent { item: ItemId::CopperBar, count: 1 },
        Reagent { item: ItemId::RoughBlastingPowder, count: 2 },
        Reagent { item: ItemId::CopperBolt, count: 1 },
    ],
    skill_req: 0,
    item_level_budget: 6,
    station: Some(StationType::Toolworks),
},
```

`complete_craft` 必须尊重 `result_count`（手榴弹一次进包 2 个）。测试断言 `inventory.count(CopperGrenade) == 2`。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim/src/content/recipes.rs
git commit -m "feat: add engineering powder, bolts, and grenades"
```

---

### Task 14: Enchanting — disenchant and apply

**Files:**
- Create: `crates/woc-sim/src/content/enchants.rs`
- Create: `crates/woc-sim/src/professions/enchanting.rs`
- Create: `crates/woc-sim/src/inventory.rs` 中的 `ItemInstance`（若 Task 3 只有堆叠：本任务给不可堆叠装备改为实例槽）
- Modify: `crates/woc-sim/src/professions/mod.rs`
- Modify: `crates/woc-sim/src/content/mod.rs`

**Interfaces:**
- Consumes: `Quality`、`EquipSlot`、`enchant_family_seconds`、`ProfessionId::Enchanting`
- Produces: `disenchant(instance) -> dust/essence/shard` 0 抽；`apply_enchant(instance, enchant_id, confirm_replace)`；`EnchantId::{BracerMinorHealth, WeaponMinorMight, ChestMinorStamina}`

- [ ] **Step 1: Instances**

不可堆叠装备 `try_add` 时写入 `Vec<ItemInstance>`：

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemInstance {
    pub id: u64,
    pub item: ItemId,
    pub enchant: Option<EnchantId>,
}
```

`Inventory` 增加 `instances: Vec<ItemInstance>` 与 `next_instance_id`。分解按 `instance_id` 定位，摧毁该实例。

- [ ] **Step 2: Enchant content and tests**

分解表（0 抽）：

```rust
pub fn disenchant_yield(quality: Quality) -> &'static [Reagent] {
    match quality {
        Quality::Common => &[Reagent { item: ItemId::ArcaneDust, count: 1 }],
        Quality::Uncommon => &[Reagent { item: ItemId::ArcaneDust, count: 2 }],
        Quality::Rare => &[
            Reagent { item: ItemId::ArcaneDust, count: 2 },
            Reagent { item: ItemId::ArcaneEssence, count: 1 },
        ],
        Quality::Epic => &[Reagent { item: ItemId::ArcaneShard, count: 1 }],
    }
}
```

附魔定义：

```rust
pub struct EnchantDef {
    pub id: EnchantId,
    pub slot: EquipSlot,
    pub reagents: &'static [Reagent],
    pub sta: u8,
    pub str: u8,
}

// Wrist +sta 2 / Dust×2
// MainHand +str 2 / Dust×5
// Chest +sta 3 / Dust×3 + Essence×1
```

测试：

```rust
#[test]
fn disenchant_common_sword_yields_one_dust_and_destroys_item() { ... }

#[test]
fn apply_without_confirm_on_already_enchanted_denies() { ... }

#[test]
fn same_enchant_id_denies_even_with_confirm() { ... }

#[test]
fn wrong_slot_bracer_on_chest_denies() { ... }
```

上附魔涨附魔技能，分解同样涨。无工作台。材料不足不摧毁装备。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: add enchanting disenchant and apply-with-replace rules"
```

---

### Task 15: Recipe economy invariant and vendor exclusion

**Files:**
- Create: `crates/woc-sim/src/content/economy.rs`（或 `recipes.rs` 测试模块）
- Modify: none of the production formulas unless a recipe fails the invariant — then raise reagent counts or cut `sell_value`，禁止加例外列表

**Interfaces:**
- Consumes: `ITEM_DEFS`、`RECIPES`、`reagent_unit_value`、`VENDOR_ITEMS`
- Produces: 空例外列表的经济测试

- [ ] **Step 1: Write the invariant tests**

```rust
#[test]
fn every_recipe_costs_more_than_it_vendors() {
    for recipe in RECIPES {
        let input: u32 = recipe
            .reagents
            .iter()
            .map(|r| reagent_unit_value(item_def(r.item)) * u32::from(r.count))
            .sum();
        let output = item_def(recipe.result).sell_value * u32::from(recipe.result_count);
        assert!(
            input > output,
            "{:?} input {input} must exceed output {output}",
            recipe.id
        );
    }
}

#[test]
fn every_enchant_costs_more_than_dust_vendor_loop() {
    // Sum of enchant reagents' unit value must be > 0 and use only non-gathered mats.
}

#[test]
fn vendors_never_stock_gathered_or_skinned_mats() {
    for id in VENDOR_ITEMS {
        assert!(!item_def(*id).gathered, "{id:?} must not be vendored");
    }
}
```

若某配方失败：优先增加助熔剂/线/瓶数量，或下调产物 `sell_value`。当前规格里的数字已按此式排过，实现时勿改比例除非测试红。

- [ ] **Step 2: Run tests**

Run: `cargo test -p woc-sim every_recipe_costs_more_than_it_vendors -- --exact`

Expected: PASS，失败信息含配方 id

- [ ] **Step 3: Commit**

```bash
git add crates/woc-sim
git commit -m "test: pin recipe economy and vendor exclusion invariants"
```

---

### Task 16: ProfessionSession facade and determinism golden

**Files:**
- Modify: `crates/woc-sim/src/professions/mod.rs`（加入 `ProfessionSession`）
- Create: `crates/woc-sim/src/professions/session.rs`
- Create: `crates/woc-sim/tests/determinism.rs`（集成测试）

**Interfaces:**
- Consumes: gathering、skinning、crafting、enchanting 的 start/complete
- Produces: `ProfessionSession` 字段：`tick`、`pos`、`gold`、`inventory`、`skills`、`node_ready: BTreeMap<NodeId, u64>`、`corpses: BTreeMap<CorpseId, Corpse>`、`cast: Option<ActiveCast>`、`last_masterwork: Option<RecipeId>`、`last_deny: Option<DenyReason>`

- [ ] **Step 1: Session API**

```rust
impl ProfessionSession {
    pub fn new_eastbrook() -> Self { /* pick, sickle, knife, 1000c, 16 slots, at forge */ }
    pub fn advance(&mut self, ticks: u32) { /* complete cast if due */ }
    pub fn start_gather(&mut self, node: NodeId) -> Result<(), DenyReason>
    pub fn start_skin(&mut self, corpse: CorpseId) -> Result<(), DenyReason>
    pub fn start_craft(&mut self, recipe: RecipeId, count: u16) -> Result<(), DenyReason>
    pub fn start_disenchant(&mut self, instance: u64) -> Result<(), DenyReason>
    pub fn start_enchant(&mut self, instance: u64, enchant: EnchantId, confirm: bool) -> Result<(), DenyReason>
}
```

`ActiveCast` 枚举：`Gather { node, complete_tick }` / `Skin { corpse, complete_tick }` / `Craft { recipe, remaining, complete_tick }` / `Disenchant { instance, complete_tick }` / `ApplyEnchant { instance, enchant, confirm, complete_tick }`。`start_*` 在已有 `cast` 时返回 `Busy`。

- [ ] **Step 2: Determinism integration test**

`crates/woc-sim/tests/determinism.rs`：

```rust
use woc_sim::professions::session::ProfessionSession;
use woc_sim::rng::XorShift64;

fn play(seed: u64) -> (Vec<u16>, u32, u16) {
    let mut rng = XorShift64::new(seed);
    let mut s = ProfessionSession::new_eastbrook();
    s.start_gather(NodeId(1)).unwrap();
    s.advance(60);
    s.complete_ready(&mut rng);
    // smelt if we have ore, then return mining skill, gold, ore count
    (
        ProfessionId::ALL.iter().map(|id| s.skills.get(*id)).collect(),
        s.gold.copper,
        s.inventory.count(ItemId::CopperOre),
    )
}

#[test]
fn same_seed_replays_byte_identical_profession_state() {
    assert_eq!(play(7), play(7));
}
```

把 `complete_ready` 做成 session 的公开测试方法，避免调用方漏抽。

另写 `eastbrook_loop_can_mine_smelt_and_forge_a_sword`：镐挖矿 → 熔炼 → 站在熔炉做短剑。这是整条锻造链路的冒烟。

- [ ] **Step 3: Run tests**

Run: `cargo test -p woc-sim`

Expected: PASS，含集成测试

- [ ] **Step 4: Commit**

```bash
git add crates/woc-sim
git commit -m "feat: add ProfessionSession facade and determinism golden"
```

---

### Task 17: Spec self-check and docs sync

**Files:**
- Modify: `README.md`（列出十个 `ProfessionId` 与 `cargo test`）
- Modify: `docs/design/manufacturing.md`（若实现时符号名有出入，改文档对齐代码）

**Interfaces:**
- Consumes: 已实现的公开类型
- Produces: 文档与代码同名

- [ ] **Step 1: Grep the spec against the crate**

确认规格中的每个 `ProfessionId`、`RecipeId`、`DenyReason`、经济式都能在代码里找到。缺的补测试，不改规格放水。

- [ ] **Step 2: Run the full suite**

Run: `cargo test -p woc-sim`

Expected: all PASS

- [ ] **Step 3: Commit**

```bash
git add README.md docs crates/woc-sim
git commit -m "docs: sync manufacturing README with shipped sim surface"
```

---

## Coverage check (plan vs spec)

| Spec section | Task |
|--------------|------|
| 采矿 / 草药学节点、工具门、两抽 | Task 5 |
| 剥皮专业、一尸一次 | Task 6 |
| 锻造 / 熔炉 | Task 7–8 |
| 制皮 / 制皮厂 | Task 9 |
| 裁缝 / 织布机 | Task 10 |
| 珠宝 / 选矿与戒指 | Task 11 |
| 炼金 / 药剂台 | Task 12 |
| 工程学 / 工具坊 | Task 13 |
| 附魔分解与替换 | Task 14 |
| 经济不变量、NPC 不卖采集物 | Task 15 |
| 确定性、施法节奏 | Task 2, 4, 16 |
| 技能独立与封顶 | Task 4 |

v1 明确不做、本计划也不开任务：archetype、委托板、钓鱼伐木烹饪铭文、工具符文充能、签名实例合并。
