# NPC services design — `1.11.0` / `npc-services`

**Status:** Implemented (1.11.0).
**Baseline:** rewrite `1.10.0` / `quest-depth` on `develop` (ECS `World` actor store).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `npc-services`.

Post-completion program (shipped): [`2026-08-13-post-completion-program-design.md`](2026-08-13-post-completion-program-design.md).  
Sim ECS (required): [`2026-08-13-sim-ecs-design.md`](2026-08-13-sim-ecs-design.md).

## 1. Goal

Town NPCs become the **authoritative front** for the services players expect at a hub: buy/sell, repair, profession training, class confirmation, and hearth bind.

Today a vendor is a boolean plus a static stock list, profession training has no NPC and no client button, and gear never wears out. This program closes that gap **without** turning every economy HUD (bank, mail, auction) into an NPC gate, and without gossip dialog trees.

> Talk to the right NPC. The sim decides prices, repair bills, and whether you actually learned the skill.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| `NpcDef` | `is_quest_giver`, `is_vendor`, `vendor_stock` |
| Buy / sell | `InteractAction::Buy` / `Sell`; infinite stock; any vendor buys any item including quest items |
| `VendorOffer.count` | Display-only; stock is never decremented |
| Weapon / armor `vendor_buy` | `0` — cannot be purchased even if listed |
| `TrainProfession` | Sim helper; **no range check, no NPC, client never sends it** |
| Talents | `LearnTalent` / `RespecTalents` from the N-panel; kits unlock on level-up |
| Bank / mail / AH | HUD actions; not NPC-gated |
| Durability | None. `Equipment` is `Option<String>` per slot; `InvStack` is `{item_id, count}` |
| NPCs | 11 defs (3 Eastbrook, 5 Eastfen/Mirefen, 3 Thornpeak) |
| Protocol | Rev **6**; additive `#[serde(default)]` fields preferred |

Honest remaining NPC debt:

1. **One service shape.** Adding repair/trainer/inn as more booleans on `NpcDef` repeats the fat-flag problem the ECS split already forbade for actors.
2. **Training is invisible.** Gathering/crafting tests call `train_profession` directly. A player in the Bevy client cannot learn herbalism.
3. **Trade is a checklist.** Quest items are sellable; there is no buyback; smiths cannot sell the weapons they should.
4. **Gear is immortal.** Combat never degrades weapons or armor, so a repair NPC would be flavor with no mechanic.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Boolean flags** | `is_repairer`, `is_trainer`, `is_innkeeper`, … | Fast; explodes; Talk still auto-opens everything | Reject |
| **B. Parallel content tables** | `VendorTable` / `TrainerTable` keyed by `npc_id` | Clean at 200 NPCs; three lookups for eleven rows | Reject for this scale |
| **C. `NpcService` slice on `NpcDef` + session snapshot (recommended)** | One service list per NPC; Talk opens a session; actions still sim-authoritative | One content migration; additive protocol | **Adopt** |

Keep `vendor_stock` on `NpcDef` (already the vendor table). Profession ids the NPC can teach live in a parallel `trains` slice so the tables stay `const`.

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.10.0** | `quest-depth` | Abandon, share, dailies, explore/escort, choice rewards (shipped) |
| **1.11.0** | `npc-services` | Vendor depth, durability/repair, trainers, hearth |

`PROTOCOL_REV` stays **8** (additive snapshot / action variants with `#[serde(default)]` / unknown-variant-safe tagged enums). Upstream pin stays **0.31.0**. Do not bump `Cargo.toml` in the planning change; the implementation wave tags `1.11.0`.

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
- Client never decides combat / loot / quest / **vendor / repair / train / hearth** outcomes.
- All sim RNG via mulberry32 on `Sim` only; no wall clock in sim (hearth cooldown is **tick-based**).
- English-only strings.
- New *per-actor* state is a `World` column. New *per-realm* state is a `Sim` field. Do not reintroduce a fat `Entity`.
- Tick-phase fingerprint stays `15038642330132466611`. Repair/train/hearth are interact actions or hooks **inside** `player_combat` / `mob_ai_combat`. No new named phase.

```
woc-content NpcDef.services     woc-sim interaction / combat / professions
        │                                    │
        ▼                                    ▼
 Talk → Bags.open_vendor_npc  →  TickSnapshot.open_vendor + open_npc
        │                                    │
        ▼                                    ▼
 Buy / Sell / RepairAll / TrainProfession / TrainClass / BindHearth
 UseHearthstone (self, no NPC target)
```

`Bags.open_vendor_npc` is the **open NPC session** (name kept for persist/session compatibility). Trainers and innkeepers set it on Talk even when they are not vendors. `CloseVendor` still clears it.

### 5.1 Content: `NpcService`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcService {
    QuestGiver,
    Vendor,
    Repair,
    ProfessionTrainer,
    ClassTrainer,
    Innkeeper,
}

pub struct NpcDef {
    pub id: &'static str,
    pub name: &'static str,
    pub greeting: &'static str,
    pub services: &'static [NpcService],
    pub vendor_stock: &'static [VendorOffer],
    /// Profession ids this NPC can teach. Empty unless `ProfessionTrainer` is listed.
    pub trains: &'static [&'static str],
}
```

Helpers (methods, so existing `is_vendor` / `is_quest_giver` **field** reads become `is_vendor()`):

- `is_quest_giver`, `is_vendor`, `can_repair`, `is_profession_trainer`, `is_class_trainer`, `is_innkeeper`
- `trains_profession(&self, id: &str) -> bool`

`VendorOffer.count` stays display-only. Stock does **not** decrement. Limited restock is out of scope.

### 5.2 Locked NPC roster

| Id | Zone | Services | Notes |
| --- | --- | --- | --- |
| `captain_alden` | Eastbrook | QuestGiver, ClassTrainer | Existing quest giver; class confirmation |
| `trader_wilkes` | Eastbrook | Vendor | Food / water (unchanged stock) |
| `town_crier` | Eastbrook | *(none)* | Flavor greeting only |
| `smith_brann` | Eastbrook **new** | Vendor, Repair, ProfessionTrainer | `trains`: `mining`, `blacksmithing`; sells weapons/armor with `vendor_buy > 0` |
| `herbalist_wren` | Eastbrook **new** | ProfessionTrainer | `trains`: `herbalism`, `alchemy` |
| `innkeeper_mara` | Eastbrook **new** | Innkeeper | Bind hearth |
| `warden_selene` | Eastfen | QuestGiver | Unchanged role |
| `apothecary_vex` | Eastfen | QuestGiver, Vendor, ProfessionTrainer | Adds `herbalism`, `alchemy` |
| `scout_darian` | Eastfen | QuestGiver | Unchanged |
| `keeper_orla` | Mirefen | QuestGiver | Unchanged |
| `ferryman_noll` | Mirefen | QuestGiver, Vendor | Unchanged |
| `commander_elara` | Thornpeak | QuestGiver | Unchanged |
| `pathfinder_toren` | Thornpeak | QuestGiver | Unchanged |
| `quartermaster_bren` | Thornpeak | Vendor, Repair | Repair at the high watch |

New `NpcSpot`s (Eastbrook, near existing town cluster, not on mob camps):

| Id | x | z |
| --- | --- | --- |
| `smith_brann` | 8.0 | 4.0 |
| `herbalist_wren` | -6.0 | 6.0 |
| `innkeeper_mara` | 2.0 | 8.0 |

`smith_brann` stock (all must have `vendor_buy > 0`):

- `worn_sword` × 1 — set `vendor_buy = 20` (sell stays 5)
- `wooden_buckler` × 1 — set `vendor_buy = 16` (sell stays 4)
- `copper_shortsword` × 1 — set `vendor_buy = 48` (sell stays 12)
- `recruit_tunic` × 1 — set `vendor_buy = 16` (sell stays 4)

Other starter weapons (`worn_mace`, …) stay `vendor_buy = 0` unless a later vendor lists them.

### 5.3 Buy / sell rules

Existing Talk → vendor panel stays. Additional rules:

1. **Quest items cannot be sold.** `ItemKind::Quest` → toast `"This item is needed for a quest."` and no copper.
2. **Buyback.** On a successful sell, push `{item_id, count, durability, copper}` onto `Bags.buyback` (cap **6**, oldest dropped). Session-only: not persisted; cleared on `CloseVendor`, zone change, death, and persist export. `InteractAction::Buyback { slot: u8 }` buys that entry back for **the same copper** it sold for. Inventory-full / not-enough-copper toasts match Buy.
3. **Buy still requires** the NPC to have `Vendor` and the item to be in `vendor_stock`. Price remains `ItemDef.vendor_buy * count`.
4. **Sell still requires** the NPC to have `Vendor` and an open session with that NPC in `INTERACT_RANGE`.

Do not implement per-tick stock, reputation discounts, or sell filters by item kind beyond quest.

### 5.4 Durability and repair

`ItemDef` gains `max_durability: u32` (**0** = not gear). Helpers set:

| Kind | `max_durability` |
| --- | --- |
| Weapon | **40** |
| Armor | **30** |
| Consumable / Junk / Quest | **0** |

Item instance wear:

- `InvStack.durability: Option<u32>` — `None` for non-gear; gear defaults to `max_durability` on grant/loot/craft/buy.
- `Bags.equipment_wear: EquipmentWear` — parallel `Option<u32>` per equip slot. `None` while the slot is empty. Missing wear on a filled slot (old saves) is treated as **full**.

`Equipment` stays `Option<String>` per slot. Do **not** change that type; wear is adjacent state on `Bags` (player-only), not a new actor column and not a fat `Entity`.

Combat wear (inside existing combat phases, after a **connecting** hit; misses do not wear):

- Player auto-attack hits: `main_hand` wear −1 (floor 0).
- Player **takes melee damage** from a mob/pet/player swing (`deal_damage` where target has `Bags` and source is not the player): each occupied armor slot (head/chest/legs/feet/off_hand) −1. Weapon is not worn by being hit.
- Abilities do not spend extra durability.

At **0** durability, `recalc_player_stats` contributes **0** attack power / armor from that slot. The item stays equipped (no auto-unequip). Toast once when a slot reaches 0: `"Your {item} is broken."`

Repair:

- `InteractAction::RepairAll` against an NPC with `Repair`, in range, with that NPC as the open session (Talk first, same as vendor).
- Cost = **1 copper per missing durability point** summed over equipped gear **and** bag gear (`max_durability > 0`).
- All-or-nothing: if `copper < cost`, toast `"Not enough copper."` and change nothing.
- On success: set every gear instance to `max_durability`, subtract copper, `recalc_player_stats`, toast `"Repaired for {cost} copper."`
- Snapshot field `repair_cost: u32` on the NPC session so the client can label the button without computing.

### 5.5 Trainers

**Profession trainer (the real gate).**

`InteractAction::TrainProfession { id }` must:

1. Target an NPC (`EntityKind::Npc`) in `INTERACT_RANGE`.
2. NPC def has `ProfessionTrainer` and `trains_profession(id)`.
3. Then call the existing `train_profession` mutation (idempotent, skill floor 1).

`host.rs` currently routes `TrainProfession` **without** using `target_id`. That path must pass the interact target into the profession module.

Direct `train_profession(&mut world, …)` stays for gather/craft unit tests so they do not need a spawned trainer. Production and new gating tests go through `InteractAction`.

The Bevy client must send `TrainProfession` (it currently never does). Talk to a trainer opens the session; the HUD lists `trains` as buttons.

**Class trainer (confirmation seam, not a talent gate).**

Kits already unlock via `known_abilities_at_level` on level-up. Do **not** move `LearnTalent` behind an NPC (that would force a town trip to spend points).

`InteractAction::TrainClass` against a `ClassTrainer` in range:

1. Refresh `known_abilities` from `known_abilities_at_level(class, level)` (idempotent).
2. Toast `"You are trained through level {n}."`

This is the hook later ability-rank trainers would use. Respec stays on the N-panel.

### 5.6 Innkeeper / hearth

New **player** component (not on NPCs):

```rust
pub struct Hearth {
    pub zone_id: String,
    pub x: f32,
    pub z: f32,
    /// Sim tick when `UseHearthstone` becomes legal again.
    pub ready_tick: u64,
}
```

Column on `World` (`hearth: SparseSet<Hearth>`), insert on `create_player` only. `clear_all_columns` must remove it.

Spawn default: Eastbrook layout spawn (`EASTBROOK.player_spawn_x/z`, `zone_id = "eastbrook"`), `ready_tick = 0`.

- `BindHearth` — NPC has `Innkeeper`, in range. Copies player `Transform` + `Identity.zone_id` onto `Hearth`, toast `"Hearthbound."`
- `UseHearthstone` — routed on `Sim` in `host.rs` like `SummonPet` (so it can read `self.tick`; `target_id` ignored). If `self.tick < ready_tick`, toast `"Hearthstone is not ready."` Else teleport to bound `zone_id`/`x`/`z` using the same population + instance-clear path as `load_overworld_zone` (but **do not** snap to the zone default spawn — use the bound coordinates), then `ready_tick = self.tick + 18_000` (15 minutes at 20 Hz). `BindHearth`, `RepairAll`, `TrainClass`, and `TrainProfession` keep using `WorldHost::interact`'s `target_id` as the NPC.

No hearthstone item. No rest-XP. Cooldown uses `Sim.tick`, never wall clock.

Persist: additive fields on `PlayerPersistentState` / `CharacterSave` / `InvStackDto` with `#[serde(default)]`. Old rows load as full durability and Eastbrook hearth.

### 5.7 Protocol (additive, rev 6)

New `InteractAction` variants (tagged `type`, same enum):

```text
RepairAll
Buyback { slot: u8 }
TrainClass
BindHearth
UseHearthstone
```

`TrainProfession` already exists.

New snapshot (all `#[serde(default)]` so old peers omit them):

```rust
pub struct NpcSessionSnapshot {
    pub npc_id: EntityId,
    pub npc_name: String,
    pub greeting: String,
    pub services: Vec<String>, // "vendor", "repair", "profession_trainer", ...
    pub stock: Vec<VendorOfferSnapshot>,
    pub train_professions: Vec<String>,
    pub can_repair: bool,
    pub repair_cost: u32,
    pub can_bind: bool,
    pub buyback: Vec<BuybackSnapshot>,
}

pub struct BuybackSnapshot {
    pub slot: u8,
    pub item_id: String,
    pub count: u32,
    pub price: u32,
}
```

`TickSnapshot.open_npc: Option<NpcSessionSnapshot>` (default `None`).

Keep `open_vendor` populated **when the session NPC has `Vendor`**, so the existing vendor panel keeps working. New chrome reads `open_npc`.

Durability on the wire:

- `InvSlotSnapshot.durability: Option<u32>` (`serde default`)
- `EquipmentSnapshot` gains parallel optional wear fields `main_hand_durability: Option<u32>`, … (`serde default`)
- `TickSnapshot.hearth_ready_tick: u64` (`serde default`) and `hearth_zone_id: String` (`serde default`) so the HUD can show cooldown without a new panel type

### 5.8 Client (presentation only)

- Nameplates / world markers: `[!]` quest, `[$]` vendor, `[#]` repair, `[T]` trainer, `[H]` inn (combine when multiple).
- Vendor panel: existing buy buttons; **Repair** if `can_repair`; **Buyback** rows; `[V]` still sells first junk.
- Trainer: when `train_professions` is non-empty, list Train buttons (send `TrainProfession` with the NPC as target). Class trainer: one **Train class** button (`TrainClass`).
- Innkeeper: **Bind hearth** button. Key **H** (not in a text field) sends `UseHearthstone`.
- Character sheet / bags: show `durability/max` on gear lines (e.g. `Worn Sword 12/40`). Broken (`0`) in red.

Client does not compute repair cost or prices.

### 5.9 Persist

| Field | Default for old rows |
| --- | --- |
| `InvStackDto.durability` | `None` → treat as max if gear |
| `CharacterSave` hearth `zone_id/x/z/ready_tick` | Eastbrook spawn, `ready_tick = 0` |
| Equipment wear | omitted → full |

`Bags.buyback` and `open_vendor_npc` stay **excluded** from export (already true for the vendor session).

## 6. Definition of done

1. Every `NpcDef` uses `services` + `trains`; content integrity: vendor stock items exist, `trains` ids exist in `PROFESSIONS`, every `ProfessionTrainer` has a non-empty `trains`, every `Vendor` has non-empty `vendor_stock`, every zone NPC spot resolves.
2. Buy ration from `trader_wilkes` still spends copper (existing test stays green).
3. Selling `boar_tusk` (quest) is refused; selling `wolf_fang` (junk) credits `vendor_sell` and appears in buyback; buyback restores the stack and copper.
4. A connecting player auto-attack decrements main-hand durability; a mob melee hit decrements occupied armor; at 0 the slot adds 0 AP/armor; `RepairAll` at `smith_brann` restores gear for 1c/point.
5. `TrainProfession { id: "mining" }` without a trainer NPC in range fails; talking to `smith_brann` then training succeeds; `herbalist_wren` cannot teach `mining`.
6. `TrainClass` at `captain_alden` toasts the level line and refreshes the kit.
7. `BindHearth` at `innkeeper_mara` then `UseHearthstone` from Wolf Run returns the player to the bound point; a second use before 18_000 ticks toasts not ready.
8. Bevy client: train / repair / bind / hearthstone key / durability readout. `cargo check -p woc-client` green.
9. `TICK_PHASES` fingerprint unchanged. `PROTOCOL_REV` remains 6.
10. `docs/parity/STATUS.md` + `ROADMAP.md` + demo steps updated when the implementation wave lands.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Banker / mailbox / auctioneer NPCs | Those HUDs already work; gating them is a UX regression |
| Flight masters / mounts / stables | No mount system |
| Limited vendor stock / restock ticks | Display `count` is enough |
| Gossip dialog trees / multi-page menus | Session snapshot + action buttons |
| Rest XP / inn logout bonus | No rest system |
| Weapon-skill trainers / riding trainers | No weapon-skill stats |
| Gating `LearnTalent` or `RespecTalents` behind class trainers | N-panel already shipped |
| Durability loss on death | Combat wear is the only sink |
| Reputation discounts / faction | No reputation |
| Reintroducing a fat actor struct | Violates `AGENTS.md` |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |
| New tick phase | Wear hooks existing combat; services are interacts |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| `TrainProfession` tests that go through `WorldHost::interact` start failing | Keep `train_profession()` as a direct helper; only the action path requires an NPC |
| `Equipment: Option<String>` call sites assume no wear | Wear is parallel on `Bags`; stats consult both |
| Additive protocol missed a default | Every new snapshot field has `#[serde(default)]`; roundtrip tests include omit-key JSON |
| Hearth teleport skips zone population | Reuse `ensure_zone_population` / instance-clear from `load_overworld_zone` |
| Fingerprint churn from combat wear | Wear is inside `player_combat` / `deal_damage`; do not rename phases |
| Client still uses `NpcDef.is_vendor` fields | Switch all call sites to methods in the same content task |

## 9. Success demo (human)

1. Buy a Traveler's Ration from Trader Wilkes; sell a Wolf Fang; buy it back.
2. Try to sell a Boar Tusk — refused.
3. Kill wolves until the starter sword durability drops; stats fall when it hits 0; repair at Smith Brann.
4. Train Mining at Brann; train Herbalism at Wren; Brann refuses Herbalism.
5. Talk to Captain Alden → Train class → toast with current level.
6. Bind at Innkeeper Mara, run toward Wolf Run, press **H** — back at the inn. Press **H** again immediately — not ready.

When §6 is green, tag `1.11.0`.
