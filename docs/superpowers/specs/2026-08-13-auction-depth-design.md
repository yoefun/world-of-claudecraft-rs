# Auction-depth design — `1.14.0` / `auction-depth`

**Status:** Implemented (1.14.0).  
**Baseline:** rewrite `1.13.0` / `gear-slots` on `develop` (ECS `World`; manufacturing ECS wired; durability + MH enchant already shipped).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `auction-depth`.

Related: completion economy slice [`2026-07-28-rust-rewrite-completion-design.md`](2026-07-28-rust-rewrite-completion-design.md); NPC services [`2026-08-13-npc-services-design.md`](2026-08-13-npc-services-design.md); gear slots [`2026-08-13-gear-slots-design.md`](2026-08-13-gear-slots-design.md).

## 1. Goal

The 1.0-pre market is a **buyout bulletin board**: list a bag stack by `item_id` + count, pay 5c, buy the whole listing, cancel or expire, mail copper if the seller is offline. After gear-depth / gear-slots that is no longer honest — listing a worn, enchanted sword and buying it back yields a **full-durability unenchanted** clone. The U-panel only lists junk/consumables and never requires a town NPC.

This program turns that slice into a **sim-authoritative buyout auction house** without becoming a second WoW auction client.

> Talk to Auctioneer Lise. List the actual bag stack (wear and enchant included). Buyers pay the posted price. The house keeps 5%. Proceeds always arrive as mail.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| `Sim.market: AuctionHouse` | Realm resource (not an actor column). `pvp_and_market` expires listings. |
| `Listing` | `id`, ephemeral `seller_id`, `seller_durable`, `seller_name`, `item_id`, `count`, `price`, `expires_tick` |
| List | `MarketList { bag_slot, count, price }`; fee **5c**; TTL **72_000** ticks (~1 h at 20 Hz); `remove_item` by `item_id` (not the named slot) |
| Buy | Full listing; refuse self-buy; grant via `grant_into` (fresh `InvStack::new`) |
| Cancel / expire | Bag-first if seller online; else system mail |
| Sold (online seller) | Copper added to `Progress` immediately |
| Sold (offline seller) | System mail `"Auction sold"` with copper |
| Persist | `RealmEconomy.market` JSON; `MarketListingDto` has no durability / enchant |
| Mail | `MailItem` has `item_id` + `item_count` only |
| Client | **U** panel; **L** lists first junk/consumable at `vendor_sell * 5`; **O** buys first affordable; **X** cancels first `mine` |
| NPC | No auctioneer. NPC-services explicitly deferred banker / mailbox / auctioneer gates as a UX regression |
| Protocol | Rev **8**; `MarketListingSnapshot` is `id/seller/item_id/count/price/mine` |

Honest remaining AH debt:

1. **Instance amnesia.** Listings and mail drop `InvStack.durability` and `InvStack.enchant_id`. Gear-slots made this a real bug.
2. **Wrong stack.** `list_item` reads `bag_slot` then `remove_item(item_id)`, so a second stack of the same id can be consumed instead.
3. **Quest items list.** Vendor sell already blocks `ItemKind::Quest`; the AH does not.
4. **Debug HUD.** **L** refuses weapons/armor/crafts. Price and target listing are not chosen by the player beyond “first junk / first affordable / first mine”.
5. **Settlement split.** Online sellers get silent copper; offline sellers get mail. Players cannot tell the AH paid them.
6. **No sink.** 5c listing fee is the only copper destroyed. Crafted gear has no AH cut.
7. **No town front.** Bank/mail stay HUD. AH is the one economy verb that still feels like a GM overlay.

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Full WoW AH** | Bids, 12/24/48 h durations, deposits returned on sale, search/sort/filter, faction houses, commodity vs unique tabs | Huge protocol + empty theater on an 8-player realm | Reject |
| **B. Fidelity-only** | Keep HUD-anywhere; persist durability/enchant on listings and mail; slot-accurate take | Fixes the gear bug; AH still a debug overlay | Reject as the whole program (keep the fidelity work) |
| **C. Auctioneer + instance-preserving buyout (recommended)** | Eastbrook auctioneer session; list/buy/cancel gated like vendor; listings store the bag stack; 5% house cut; proceeds always mailed; **U** still paints | One NPC + additive protocol + mail DTO fields | **Adopt** |

Bidding, duration tiers, deposits-returned-on-sale, search boxes, and a second faction house stay out. Buyout is the only price.

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.13.0** | `gear-slots` | Dual-wield, Finger2, quality, MH enchant (shipped) |
| **1.14.0** | `auction-depth` | Auctioneer, instance listings, house cut, mail settlement |

`PROTOCOL_REV` stays **8**. New snapshot / DTO fields use `#[serde(default)]`. Upstream pin stays **0.31.0**. Do not bump `Cargo.toml` / `VERSION.toml` in the planning change; the implementation wave tags `1.14.0`.

Tick-phase fingerprint stays **`3214741777866168171`**. No new named phase. Expiry stays inside `pvp_and_market`.

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
- Client never decides prices, cuts, listing success, or mail contents.
- All sim RNG via mulberry32 on `Sim` only; listing TTL is tick-based (`LISTING_TTL_TICKS = 72_000`), never wall clock.
- English-only strings.
- Auction house stays a **per-realm** `Sim.market` resource. Do not add an actor column for listings. Do not reintroduce a fat `Entity`.
- Equipment stays on `Bags`. Listings copy `InvStack` fields; they do not invent a parallel item-instance type.

```
woc-content NpcDef.services=Auctioneer     woc-sim market / mail / inventory
        │                                         │
        ▼                                         ▼
 Talk → Bags.open_vendor_npc  →  TickSnapshot.open_npc.can_auction
        │                                         │
        ▼                                         ▼
 MarketList / MarketBuy / MarketCancel (host-gated)
        │
        ▼
 Listing { InvStack fields }  →  persist RealmEconomy  →  mail settlement
```

Direct `AuctionHouse::list_item` / `buy` / `cancel` stay callable from unit tests without an NPC (same pattern as `train_profession()` vs `InteractAction::TrainProfession`). Production traffic goes through `WorldHost::interact` in `host.rs`, which requires an auctioneer session.

### 5.1 Content: Auctioneer Lise

Add `NpcService::Auctioneer`. Helpers:

- `is_auctioneer(&self) -> bool`

`opens_npc_session` includes auctioneers. `service_name` maps `Auctioneer` → `"auctioneer"`.

Locked Eastbrook NPC:

| Id | Zone | Services | Spot |
| --- | --- | --- | --- |
| `auctioneer_lise` | Eastbrook **new** | Auctioneer | `x: 4.0`, `z: 6.0` |

```text
name: Auctioneer Lise
greeting: "List it. The house takes its cut."
vendor_stock: []
trains: []
```

No second auctioneer in Eastfen / Thornpeak. No banker / mailbox NPC (those HUDs stay ungated).

Integrity: every `Auctioneer` has empty `vendor_stock` and empty `trains` (they are not vendors). Zone spot resolves.

### 5.2 Inventory primitives

`crates/woc-sim/src/inventory.rs` gains two helpers used by market **and** mail:

```rust
/// Remove `count` from a specific bag slot, preserving durability / enchant.
pub fn take_from_slot(inv: &mut [Option<InvStack>], slot: u8, count: u32) -> Option<InvStack>

/// Insert a concrete stack. Merge only with the same item_id + durability + enchant_id
/// when the catalog stack size allows. Weapons/armor stay unstacked (existing rule).
pub fn grant_stack(inv: &mut [Option<InvStack>], incoming: InvStack) -> bool
```

`grant_into(item_id, count)` remains and is implemented as `grant_stack(InvStack::new(item_id, count))` so loot / vendor / craft keep current “fresh item” behavior.

`list_item` **must** call `take_from_slot`, never `remove_item` by id.

### 5.3 Listing payload

```rust
pub struct Listing {
    pub id: u32,
    pub seller_id: EntityId,
    pub seller_durable: String,
    pub seller_name: String,
    pub item_id: String,
    pub count: u32,
    pub durability: Option<u32>,
    pub enchant_id: Option<String>,
    pub price: u32,
    pub expires_tick: u64,
}
```

List rules (after ClassKit + fee + non-zero price checks, before taking the stack):

1. Named slot is occupied.
2. Catalog item exists.
3. `ItemKind::Quest` → toast `"This item is needed for a quest."` and no change.
4. `take = count.min(stack.count).max(1)`.
5. Deduct `LISTING_FEE` (still **5** copper, not refunded on cancel/expire).
6. Push listing with the taken stack’s `durability` / `enchant_id` and `expires_tick = now_tick + LISTING_TTL_TICKS`.

Weapons, armor, jewelry, crafts, junk, and consumables are all listable. Equipped gear is not (bags only). Bank slots are not.

Buy grants the listing’s stack via `grant_stack`, not `grant_into`. If bags cannot accept it, toast `"Bags are full."` and leave the listing.

Cancel / expire return that same stack (bag-first if the seller is a live `ClassKit` player; otherwise system mail with instance fields).

### 5.4 House cut and mail settlement

```rust
pub const SALE_CUT_NUM: u32 = 1;
pub const SALE_CUT_DEN: u32 = 20; // 5%, floored

pub fn sale_cut(price: u32) -> u32 {
    price / SALE_CUT_DEN
}

pub fn sale_proceeds(price: u32) -> u32 {
    price.saturating_sub(sale_cut(price))
}
```

Buyer pays `listing.price` in full. House cut is destroyed (copper sink). Seller always receives **mail**, even when online:

| Event | Mail from | Subject | Copper | Item |
| --- | --- | --- | --- | --- |
| Sold | `Auction House` | `Auction sold` | `sale_proceeds(price)` | none |
| Cancelled (bags full or offline) | `Auction House` | `Listing cancelled` | 0 | listed stack |
| Expired (bags full or offline) | `Auction House` | `Listing expired` | 0 | listed stack |

Do not add copper to `Progress` on sale. Existing `buy_mails_proceeds_when_seller_offline` stays valid; add an online-seller case that asserts `Progress.copper` unchanged and inbox copper equals proceeds.

Cancel/expire that successfully `grant_stack` into bags still skip mail (current bag-first behavior).

### 5.5 Mail instance fields

`MailItem`, `MailSnapshot`, `MailDto` gain:

```rust
#[serde(default)]
pub durability: Option<u32>,
#[serde(default)]
pub enchant_id: Option<String>,
```

`deliver_system` grows matching parameters (or takes an `Option<InvStack>` for the attachment). `collect` uses `grant_stack` when an item is present. Old JSON rows omit the keys → `None` → `grant_stack` / `InvStack::new` treats missing durability as catalog max for gear (same rule as character persist).

### 5.6 Host gate

`WorldHost::interact` for `MarketList` / `MarketBuy` / `MarketCancel`:

1. Player has `ClassKit`.
2. `Bags.open_vendor_npc` is `Some(npc_id)`.
3. That entity is `EntityKind::Npc`, in `INTERACT_RANGE`, and `npc(template).is_auctioneer()`.
4. Then call the existing `AuctionHouse` method.

Otherwise toast `"Talk to an auctioneer first."` (out of range uses the existing `"Too far away."` only if we go through the generic interact range check; the market arms today bypass `handle_interact` and live in `host.rs`, so the helper must distance-check itself with `dist2d`).

`CloseVendor` still clears the session. Walking out of range does not auto-clear (same as vendor).

### 5.7 Protocol (additive, rev 8)

`MarketListingSnapshot`:

```rust
#[serde(default)]
pub durability: Option<u32>,
#[serde(default)]
pub enchant_id: Option<String>,
#[serde(default)]
pub expires_tick: u64,
```

`NpcSessionSnapshot`:

```rust
#[serde(default)]
pub can_auction: bool,
```

`MailSnapshot` durability / enchant_id as in §5.5.

No new `InteractAction` variants. Roundtrip tests include omit-key JSON for the new fields.

### 5.8 Persist

`MarketListingDto` additive `durability`, `enchant_id` (`#[serde(default)]`). `seller_id` stays ephemeral and is exported as `0` on load (already). `bridge.rs` copies the new fields both ways.

Postgres `realm_economy` is opaque JSONB — no new migration. Memory persist follows the same DTO.

### 5.9 Client (presentation only)

- Nameplates / world markers / map tags: `[A]` auctioneer (gold-ish cue, distinct from `[$]` vendor). Combine with other tags if an NPC ever has multiple.
- Talk to an auctioneer: existing session snapshot; HUD treats `can_auction` as “open the market panel” (`show_market = true`) the same way vendor Talk opens vendor chrome.
- **U** still toggles the market panel anywhere (window-shopping). **L / O / X** still send `MarketList` / `MarketBuy` / `MarketCancel`; the sim refuses them without a session.
- Listing lines show catalog name when known, durability `12/40`, and enchant suffix when present. Remaining time is optional (`expires_tick` in snapshot); if shown, format as minutes from `snap.tick` vs `expires_tick` only if a tick field exists on the snapshot — **do not add `TickSnapshot.tick`**. If remaining time cannot be derived, omit it; `expires_tick` is still on the wire for later HUD.
- **L** lists the first **non-quest** bag stack (any kind), count 1, price `vendor_sell.max(1) * 5`. Help text: `"[L] List 1×{name} for {price}c (+5c fee)"`.
- **O** / **X** unchanged (first affordable not-mine / first mine).
- Session help: when `can_auction`, append `"[U] Auction"`.

Client does not compute the house cut or remaining TTL as a gate.

## 6. Definition of done

1. `auctioneer_lise` exists in Eastbrook with `Auctioneer` only; content integrity: spot resolves; `is_auctioneer()` true; not a vendor.
2. Listing a worn enchanted `worn_sword` (durability 7, enchant id `coarse_sharpening`) and buying it on another character grants that same durability and enchant, not a fresh sword.
3. Two bag stacks of `silverleaf`; `MarketList` on slot 1 removes from slot 1 only.
4. Listing `boar_tusk` (`ItemKind::Quest`) toasts `"This item is needed for a quest."` and does not charge the fee.
5. `MarketList` / `MarketBuy` / `MarketCancel` through `WorldHost::interact` without an auctioneer session toast `"Talk to an auctioneer first."`; Talk to Lise then list succeeds.
6. Buy of a 50c listing mails the seller **48c** (cut = `price / 20` = 2c). Seller `Progress.copper` does not increase on sale even if online. Listing fee 5c still deducted at list time.
7. Expire at `expires_tick` returns the instance stack (bag or `"Listing expired"` mail with durability/enchant).
8. Economy JSON without the new keys still loads; missing durability on mailed gear is treated as catalog max.
9. Bevy client: `[A]` marker, **L** can list weapons, listing line shows wear/enchant, Talk to Lise opens **U**. `cargo check -p woc-client` green.
10. `TICK_PHASES` fingerprint remains `3214741777866168171`. `PROTOCOL_REV` remains **8**.
11. `docs/parity/STATUS.md` + `ROADMAP.md` + `DEMO.md` updated when the implementation wave lands. Planning change only adds this spec, the plan, and a ROADMAP pointer.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Bids / bid increments / outbid mail | Empty on an 8-player realm; buyout is the product |
| 12/24/48 h duration picker | One TTL already exists |
| Deposit returned on successful sale | Listing fee 5c is the list cost; cut is the sale cost |
| Search / sort / category tabs / pagination | Full listing dump fits the snapshot |
| Commodity bulk vs unique item AH split | `grant_stack` merge rules are enough |
| Banker / mailbox NPCs | HUD **K** / **I** stay; only AH is gated |
| Listing from bank or equipped slots | Bags only |
| Soulbound / BoE | No bind system |
| Per-listing notes / undercut helper | Client chrome stays keyboard lines |
| Faction-split houses | One realm market |
| New tick phase | Expiry stays in `pvp_and_market` |
| Reintroducing a fat actor struct | Violates `AGENTS.md` |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |
| Protocol rev 9 | Additive defaults only |

NPC-services §7 listed “auctioneer NPCs” as a non-goal because the HUD already worked. This program **narrowly reverses that** for the auction house only. Bank and mail stay HUD-gated.

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Host gate breaks DEMO “press U then L” | Implementation updates DEMO to Talk to Lise first; unit tests on `AuctionHouse` stay ungated |
| `NpcSessionSnapshot` / `MailItem` struct literals fail to compile | Additive field with `#[serde(default)]`; update the few literals in protocol/client/sim tests |
| `grant_into` callers change behavior | Keep `grant_into` as fresh-stack insert; only market/mail collect use `grant_stack` for attachments that carry instance fields |
| Old economy JSON | serde defaults; missing durability on gear → catalog max at grant time |
| Online seller used to see copper instantly | Document the mail-always change; toast on buy stays `ItemGained` for the buyer; seller sees mail on **I** |
| Fingerprint churn | Do not rename or reorder phases |
| Two stacks same id, different wear | `take_from_slot` + `grant_stack` equality on durability/enchant; weapons already unstacked |

## 9. Success demo (human)

1. Eastbrook: nameplate `Auctioneer Lise [A]`. **E** Talk → market panel opens.
2. Put a wolf fang and a worn sword in bags. **U** then **L** lists the fang (or sword if it is first non-quest). Fee 5c.
3. Wear the starter sword down, apply a whetstone, **L** that sword. Second client talks to Lise, **O** buys it — sword arrives worn and enchanted.
4. Seller opens **I** and collects `"Auction sold"` for 95% of the price (floored).
5. Try to list a boar tusk — refused.
6. **X** cancels an unsold listing; item returns to bags with the same wear.
7. Wait out TTL (or advance ticks in a test) — expired listing mails the item back.

When §6 is green, tag `1.14.0`.
