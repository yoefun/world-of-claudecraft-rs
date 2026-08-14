# Economy-depth design — `1.16.0` / `economy-depth`

**Status:** Implemented (1.16.0).  
**Baseline:** rewrite `1.15.0` / `gear-more` on `develop` (plus auctioneer/instance-listing work from this wave).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `economy-depth`.

Related: auction depth [`2026-08-13-auction-depth-design.md`](2026-08-13-auction-depth-design.md); NPC services [`2026-08-13-npc-services-design.md`](2026-08-13-npc-services-design.md).

## 1. Goal

Auction-depth left five honest non-goals: bidding, duration tiers, search/pagination, soulbound, and banker/mailbox NPCs. This program ships those on the same buyout house without a second WoW auction client — and lands Auctioneer Lise / instance listings that were parallel to reputation and gear-more on `develop`.

> Bid or buyout at Auctioneer Lise. Pick 12 / 24 / 48 hours. Bound gear cannot leave the character. Talk to the banker and the post to use vault and mail.

## 2. Non-goals (still)

- Deposits returned on sale, faction houses, commodity vs unique tabs.
- Sim-authoritative search pages (realm is ~8 players; the snapshot still dumps listings).
- Bound-on-vendor-buy or mailing bound items “to self”.
- New tick phase. Expiry stays in `pvp_and_market`.
- Protocol rev bump. Stay on **8** with additive `#[serde(default)]`.

## 3. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.14.0** | `reputation` | Hub factions, standing, vendor gates |
| **1.15.0** | `gear-more` | Extra slots, Hunter DW, OH enchant, loot quality |
| **1.16.0** | `economy-depth` | Auctioneer, bids, 12/24/48 h, soulbound, banker + mailbox |

Tick-phase fingerprint stays **`3214741777866168171`**. Auction house stays a **per-realm** `Sim.market` resource. English-only toasts. Client never decides prices, bids, binds, or cuts.

Do not bump `VERSION.toml` until the implementation wave is green.

## 4. Bidding

`Listing.price` remains **buyout** (0 = no buyout). Additive:

- `start_bid` — minimum first bid. 0 with `price > 0` means **buyout-only** (1.14.0 clients).
- `current_bid` — 0 until someone bids.
- `bidder_durable` / `bidder_name` — high bidder, if any.

`InteractAction::MarketBid { listing_id, amount }`. `MarketList` gains additive `start_bid` and `duration_hours`.

Rules:

- At least one of buyout / start_bid must be positive. If both, `start_bid <= price`.
- Cannot bid on own listing. Cannot bid if you already hold the high bid.
- Buyout-only (`start_bid == 0` and no bidder): toast `"This listing is buyout only."`
- First bid `>= start_bid`. Later bids `>= current_bid + max(1, current_bid / 20)`.
- Bid copper is taken immediately. Previous bidder is mailed `"Outbid"` for their copper.
- Buyout pays `price`. Previous bidder is refunded. Same-bidder buyout credits the held bid (`pay price - current_bid`).
- Cancel after a bid: `"Cannot cancel after a bid."`
- Expire with a bidder: mail `"Auction won"` (item) to the bidder and `"Auction sold"` (proceeds after 5% cut) to the seller.
- Expire with no bidder: return item as today (`"Listing expired"`).

## 5. Duration tiers

20 Hz tick clock, never wall clock.

| Hours | Ticks | Listing fee |
| --- | --- | --- |
| 12 (default; `duration_hours == 0` from old peers) | 864_000 | 5c |
| 24 | 1_728_000 | 10c |
| 48 | 3_456_000 | 20c |

Invalid duration: `"Duration must be 12, 24, or 48 hours."`  
`LISTING_TTL_TICKS` becomes the 12-hour default. Fee is no longer a single `LISTING_FEE` constant for every listing.

## 6. Search + pagination (client)

Snapshot still lists the whole house. The U-panel filters by catalog name / `item_id` (case-insensitive) and pages **8** rows.

- `/` enters search; letters append; Backspace deletes; Esc leaves search (second Esc closes the panel).
- `[` / `]` previous / next page.
- `,` / `.` cycle listing duration 12 → 24 → 48 for **L**.
- **B** (while U is open) bids the minimum on the first filtered listing that is not yours and is not buyout-only. **B** does not toggle bags while the market is open (same pattern as **L** vs quest log).
- **L** lists buyout at `vendor_sell.max(1)*5` with `start_bid = max(1, buyout / 2)` so new listings are biddable.

## 7. Soulbound

Catalog `ItemBind { None, OnEquip, OnPickup }` on `ItemDef`.

- Weapons / armor / jewelry: **OnEquip**.
- Quest items: **OnPickup**.
- Consumables, junk, mats, tools: **None**.

`InvStack.bound: bool` (persist + snapshot + mail + listing copies). Merge stacks only when `bound` matches.

Bind sources:

- `grant_item` (loot / quest rewards / sim grants): set `bound` when catalog is OnPickup.
- `grant_into` / vendor / craft: unbound.
- Unequip: OnEquip / OnPickup items return to bags bound.

Blocks: auction list and player mail (`"That item is soulbound."`). Vendor sell and bank deposit stay allowed. Quest items still cannot be listed (`"This item is needed for a quest."`).

## 8. Banker / mailbox NPCs

Eastbrook:

| Id | Name | Spot | Greeting |
| --- | --- | --- | --- |
| `banker_holme` | Banker Holme | `(6.0, 6.0)` | `"Your coin is safer with me."` |
| `mailbox_post` | Eastbrook Post | `(0.0, 8.0)` | `"Leave it. We'll see it through."` |

`NpcService::Banker` / `NpcService::Mailbox`. Talk opens the NPC session (and the client **K** / **I** panels). Host gates `Bank*` with `"Talk to a banker first."` and `MailSend` / `MailCollect` with `"Talk to a mailbox first."` Direct `bank::deposit` / `Mailbox::send` stay ungated for unit tests (same pattern as `list_item` / `train_profession`).

Markers: `[B]` banker, `[M]` mailbox. Session snapshot additive `can_bank` / `can_mail`.

Bank deposit/withdraw copy the named stack (`take_from_slot` + `grant_stack`) so bound / wear / enchant survive the vault.

## 9. Protocol / persist

Rev stays **8**. Additive defaults:

- `MarketList.start_bid`, `MarketList.duration_hours`
- `InteractAction::MarketBid`
- listing snapshot: `start_bid`, `current_bid`, `bidder`, `bound`
- `InvSlotSnapshot.bound`, `MailSnapshot.bound`
- `NpcSessionSnapshot.can_bank`, `can_mail`
- DTOs: `InvStackDto.bound`, `MailDto.bound`, listing bid/start/bidder/bound fields

## 10. Locked copy

- `"Talk to an auctioneer first."`
- `"Talk to a banker first."`
- `"Talk to a mailbox first."`
- `"This item is needed for a quest."`
- `"That item is soulbound."`
- `"This listing is buyout only."`
- `"Cannot cancel after a bid."`
- `"Duration must be 12, 24, or 48 hours."`
- `"Auction sold"` / `"Auction won"` / `"Outbid"` / `"Listing cancelled"` / `"Listing expired"` from `"Auction House"`
