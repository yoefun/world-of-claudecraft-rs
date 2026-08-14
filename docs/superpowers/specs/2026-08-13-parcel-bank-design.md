# Parcel and bank (warehouse) design — `1.20.0` / `parcel-bank`

**Status:** Implemented (1.20.0, rebased onto `1.16.0` / `economy-depth`).  
**Baseline:** rewrite `1.19.0` / `guilds` on `develop` (instance-preserving bank/AH/mail, soulbound, banker + mailbox NPCs).  
**Upstream pin (unchanged):** World of ClaudeCraft `0.31.0` (`a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`).  
**Goal label:** `parcel-bank`.

NPC services left banker/mailbox NPCs as non-goals (`2026-08-13-npc-services-design.md` §7). This program originally rejected NPC-gating K/I. `1.16.0` `economy-depth` shipped Eastbrook Banker Holme and Eastbrook Post anyway; this rebase **keeps those gates** and adds offline parcels, postage/cap/expiry/return, client compose, and warehouse repair on top.

## 1. Goal

Personal **bank (warehouse)** and **mail parcels** become honest item-instance systems: a worn enchanted sword deposited, mailed, listed, or withdrawn is the same sword. A player can send a parcel to a character who is not in this realm process. The Bevy client can send and collect mail, and can bank more than junk.

> Bank the sword you actually wore. Mail it to an offline alt. Collect it later with the same durability and enchant.

## 2. Baseline (already shipped)

| Piece | State |
| --- | --- |
| Bags | `BACKPACK_SLOTS = 16` on player `Bags.inventory` |
| Bank | Player `Bank` column: `BANK_SLOTS = 24` + `bank_copper`; persist on `CharacterSave` |
| Bank actions | `BankDeposit` / `BankWithdraw` / copper variants; HUD **K** |
| Bank client | **G** first *junk* only; **H**/1–9 withdraw; **J**/**Y** all copper |
| Mail | `Sim.mail: Mailbox` keyed by durable id; persist `RealmEconomy.mail` |
| Mail send | Protocol `MailSend { to_name, copper, bag_slot, count }`; recipient must currently have a `ClassKit` entity (parked counts; never-spawned this process does not) |
| Mail subject | Hardcoded `"Parcel"` |
| Mail client | **I** panel; **P** collects *first* mail; **never sends** |
| Instance data | `InvStack.durability` / `enchant_id`; snapshots already carry both |
| Moves | `remove_item` + `grant_into` — match by `item_id`, spawn a **new** `InvStack::new` (full durability, no enchant) |
| AH | List/buy/expire via mail; listings store `item_id` + `count` only |
| Repair | Equipped + bag gear; **not** banked gear |
| Protocol | Rev **8**; additive `#[serde(default)]` |
| Tick fingerprint | `3214741777866168171` (`TICK_PHASES` length 10, including `profession_casts`) |

Honest remaining storage debt:

1. **Instance identity is a lie.** Deposit, withdraw, mail attach/collect, and AH list all go through `remove_item`/`grant_into`. Two silverleaf stacks can steal from the wrong slot. A 12/40 enchanted sword comes back 40/40 with no oil.
2. **Warehouse HUD is junk-only.** The sim will bank a weapon; the client **G** helper only picks `ItemKind::Junk`.
3. **Parcels are receive-only in the client.** `MailSend` exists; no key sends it. Collect is first-row only.
4. **Name lookup requires a live entity.** Parked players work (they keep `ClassKit`). A character who exists only in persist after a realm restart cannot be mailed, even though AH already delivers to durable keys.
5. **No postage, cap, expiry, or return.** Inbox grows forever. Failed/unwanted parcels have no reverse path.
6. **Repair ignores the warehouse.** Banked broken gear stays broken until withdrawn.
7. **Quest items** can be mailed (vendor already blocks selling them).

## 3. Approaches considered

| Approach | What it does | Cost | Verdict |
| --- | --- | --- | --- |
| **A. Banker / mailbox NPC gate** | Talk to an NPC before K/I work | Repeats the npc-services rejection; wilderness banking becomes a town trip | Reject |
| **B. Full WoW mail** | Body text, COD, multi-attach, stationery, account-wide bank, bag expansions | Chrome and economy scope; fights HUD-only client | Reject |
| **C. Instance-preserving warehouse + offline parcels (recommended)** | Slot-accurate stack moves; directory-keyed send; client send/collect; postage/cap/expiry/return; repair includes bank | One inventory helper; additive protocol; no new actor column | **Adopt** |

Keep **K** / **I** HUD-gated. Do not add `NpcService::Banker` / `Mailbox`.

## 4. Version map

| Rewrite | Parity | Theme |
| --- | --- | --- |
| **1.13.0** | `gear-slots` | Dual-wield, Finger2, quality, MH enchant (shipped) |
| **1.14.0** | `parcel-bank` | Warehouse + parcel instance identity, offline send, client compose |

`PROTOCOL_REV` stays **8**. New mail/listing fields use `#[serde(default)]`. New `InteractAction::MailReturn` is a tagged enum variant (unknown-variant-safe on old peers is not required: client and server ship together; old clients simply never send it). Upstream pin stays **0.31.0**. Do not bump `Cargo.toml` / `VERSION.toml` in the planning change; the implementation wave tags `1.14.0`.

## 5. Architecture

Unchanged invariants:

- `woc-sim` / `woc-content` have no Bevy / wgpu / axum / tokio runtime deps.
- Client never decides bank/mail/AH outcomes.
- All sim RNG via mulberry32 on `Sim` only. Mail expiry uses `Sim.tick`, never wall clock.
- English-only strings.
- Bank stays a **player column**. Mailbox and the new character directory stay **per-realm `Sim` fields**. Do not reintroduce a fat `Entity`.
- Tick-phase fingerprint stays `3214741777866168171`. Mail expiry hooks **inside** `pvp_and_market` next to `AuctionHouse::tick_expire`. No new named phase.

```
take_from_slot / put_stack          CharacterDirectory (Sim)
        │                                    │
        ▼                                    ▼
 BankDeposit/Withdraw  ←→  Bags / Bank
 MailSend/Collect/Return  ←→  Mailbox (durable key)
 MarketList/Buy/Expire    ←→  Listing instance fields → mail
        │
        ▼
 TickSnapshot.bank / .mail  →  Bevy K / I panels
```

`grant_into` / `remove_item` stay for **new** grants (loot, craft, quest, vendor buy). Economy *moves* use slot helpers.

### 5.1 Slot-accurate stack moves

In `crates/woc-sim/src/inventory.rs`:

```rust
/// Remove up to `count` from an absolute slot. Preserves durability and enchant
/// on the taken fragment. Returns `None` if the slot is empty.
pub fn take_from_slot(
    inv: &mut [Option<InvStack>],
    slot: usize,
    count: u32,
) -> Option<InvStack>;

/// Insert an existing instance. Merge only when `item_id`, `durability`, and
/// `enchant_id` all match and the item is stackable (not Weapon/Armor).
/// Returns `Err(stack)` if no space.
pub fn put_stack(
    inv: &mut [Option<InvStack>],
    stack: InvStack,
) -> Result<(), InvStack>;
```

Rules:

- `take = count.min(stack.count).max(1)`. Remainder stays in the original slot.
- Weapon/Armor never merge (`max_stack = 1`), matching `grant_into`.
- Bank deposit: `take_from_slot` bags → `put_stack` bank; on bank full, `put_stack` back to bags and toast `"Bank is full."`
- Bank withdraw: the reverse; toast `"Bags are full."`
- Mail attach / AH list: `take_from_slot` bags; on later failure, `put_stack` back.
- Mail collect / AH return: `put_stack` into bags (not `grant_into`).

Quest items (`ItemKind::Quest`): **may be banked**, **must not be mailed or AH-listed**. Toast `"This item is needed for a quest."` (same copy as vendor sell).

### 5.2 Bank (warehouse)

No new column. `Bank { bank, bank_copper }` unchanged in shape.

Additional rules:

1. Deposit/withdraw already take absolute slots. Keep that. Fix the move path to §5.1.
2. Copper vault unchanged (wallet ↔ `bank_copper`).
3. `repair_cost` / `repair_all` **include banked gear** (`Bank.bank` stacks with `max_durability > 0`). Cost remains 1 copper per missing point, all-or-nothing.
4. Client **G** deposits the first **non-quest** bag stack (any kind except `ItemKind::Quest`), not junk-only.
5. **K** and **I** become mutually exclusive (opening one closes the other) so **Y** is unambiguous.

Do not add bank tabs, bought bag slots, or account-wide storage.

### 5.3 Character directory (offline parcels)

New **realm** resource on `Sim`, not a World column:

```rust
pub struct CharacterDirectory {
    /// lowercase name → durable mailbox key
    by_name: HashMap<String, String>,
}
```

`Sim.directory: CharacterDirectory`.

- `register(name, durable_key)` overwrites that name (rename is out of scope; last writer wins).
- `lookup(name) -> Option<&str>` is case-insensitive.
- `spawn_player` / `spawn_player_with_state` register the live name + `Mailbox::mailbox_key`.
- Server realm boot (`build_shared` after `apply_economy_to_sim`): `Persist::list_mailbox_directory() -> Vec<(String, Uuid)>` then `register` every row. This covers characters who have never Hello'd in *this* process.
- HTTP `create_character` does **not** touch the live `Sim` (the WS realm is a process `OnceCell`). A character created after boot becomes mail-able on first spawn, or after the next realm process start when the directory is reloaded. Tests call `directory.register` directly.
- Offline Bevy host: only spawned names exist; that is enough for two local `spawn_player`s.

`MailSend` resolution order:

1. Directory lookup by `to_name`.
2. If missing, existing live `ClassKit` + `Identity.name` scan (covers `local:{id}` offline entities with no durable UUID).
3. Else toast `"Recipient not found."` (drop the parenthetical “must be online”).

Parked players remain mail-able via (2) even if directory missed them. Never-spawned persist characters are mail-able via (1).

Cannot mail yourself: compare sender durable key (or entity id for `local:`) to the resolved recipient key.

### 5.4 Parcel rules

`MailItem` gains instance + routing fields:

```rust
pub struct MailItem {
    pub id: u32,
    pub from: String,
    pub to_durable: String,
    pub subject: String,
    pub copper: u32,
    pub item_id: Option<String>,
    pub item_count: u32,
    pub durability: Option<u32>,      // serde default
    pub enchant_id: Option<String>,   // serde default
    pub expires_tick: u64,            // serde default 0 = never
    pub return_to: Option<String>,    // sender durable key; None = system
}
```

Constants (`woc-sim/src/mail.rs`):

| Name | Value |
| --- | --- |
| `MAIL_POSTAGE` | `1` copper |
| `MAIL_INBOX_CAP` | `20` player-to-player mails per durable key |
| `MAIL_TTL_TICKS` | `1_728_000` (24 h at 20 Hz) |

Send (`Mailbox::send`):

1. Resolve recipient (§5.3). Fail → `"Recipient not found."`
2. Reject self → `"Cannot mail yourself."`
3. Reject empty (no item and `copper == 0`) → `"Mail is empty."`
4. Reject quest attachment → `"This item is needed for a quest."`
5. `wallet < postage + copper` → `"Not enough copper."`
6. Recipient inbox `len() >= MAIL_INBOX_CAP` → `"Mailbox is full."` (no take). Count **all** mails in that inbox (player + system). `deliver_system` ignores the cap.
7. `take_from_slot` if `bag_slot` is `Some`; fail empty slot as today
8. Subtract `postage + copper` from wallet; postage is **not** attached
9. Push `MailItem` with `subject: "Parcel"`, `expires_tick: now_tick + MAIL_TTL_TICKS`, `return_to: Some(sender_key)`, copied durability/enchant
10. Emit existing `MailSent`

`send` needs `now_tick: u64` (pass `self.tick` from `host.rs`, same pattern as `UseHearthstone` / market list).

System `deliver_system` (AH proceeds/returns):

- Does **not** consume postage or inbox cap.
- `expires_tick = 0`, `return_to = None`.
- New optional durability/enchant args so AH returns preserve instance.

Collect: `put_stack` the attachment; if bags full, restore the mail at the same index (unchanged rollback). Copper still applied only after the item lands. Empty-item copper mail always succeeds.

Return (`InteractAction::MailReturn { mail_id }`):

1. Remove the mail from the collector's inbox.
2. If `return_to` is `Some(key)`, `deliver_system` to that key with `from: "Mail"`, `subject: "Returned: {old subject}"`, same copper/item/durability/enchant, `expires_tick = 0`.
3. Else discard (system mail: player may still **collect**; return of system mail discards with toast `"Mail discarded."`).
4. Emit `SimEvent::Toast { "Mail returned." }` on success.

Expiry (`Mailbox::tick_expire(now_tick)`), called from `pvp_and_market` after `market.tick_expire`:

- For each mail with `expires_tick > 0 && now_tick >= expires_tick`, treat as `MailReturn` without a player (no toast to a specific client; optional realm toast skipped).
- System mail (`expires_tick == 0`) never expires.

### 5.5 Auction listings (same instance helper)

`Listing` gains `durability: Option<u32>` and `enchant_id: Option<String>` (`#[serde(default)]` on `MarketListingDto`). `list_item` uses `take_from_slot` and refuses quest items. Expire/cancel/buy that return an item use `put_stack` via `deliver_system` with those fields. Public `MarketListingSnapshot` does **not** need wear on the wire for this program (HUD already shows item id + count). Persist the fields so a restart does not mint a fresh sword.

### 5.6 Protocol (additive, rev 8)

New action:

```text
MailReturn { mail_id: u32 }
```

`MailSnapshot` additive fields (all `#[serde(default)]`):

```rust
pub durability: Option<u32>,
pub enchant_id: Option<String>,
pub expires_tick: u64,
```

`TickSnapshot` additive:

```rust
#[serde(default)]
pub mail_postage: u32, // 1 when the snapshot owner is a player; 0 otherwise
```

Client must not compute postage; it prints `mail_postage` on the send line.

`host.rs` `MailSend` passes `self.tick`. `MailReturn` routes to `Mailbox::return_mail`.

### 5.7 Persist

| Field | Default for old rows |
| --- | --- |
| `MailDto.durability` / `enchant_id` | `None` |
| `MailDto.expires_tick` | `0` (never expire — old inbox stays until collected) |
| `MailDto.return_to` | `None` (old player mail cannot auto-return; still collectable) |
| `MarketListingDto.durability` / `enchant_id` | `None` |

New persist API:

```rust
impl Persist {
    pub async fn list_mailbox_directory(&self) -> PersistResult<Vec<(String, Uuid)>>;
}
```

Memory: iterate the character map. Postgres: `SELECT name, id FROM characters`. No migration (new JSON keys only). Character create already stores unique names.

### 5.8 Client (presentation only)

Bank **K**:

- Opening bank closes mail (and vice versa).
- **G** deposits first non-quest bag stack (full count).
- **H** / **1–9** / **J** / **Y** unchanged.
- Slot lines already show `item_id`; append ` dur {n}` / `[enchant]` when snapshot has them (bags already know this pattern from the C-sheet).

Mail **I**:

- List every mail with `#id`, from, subject, copper, item, durability/enchant if present.
- **P** collect first; **1–9** collect numbered rows (same pattern as bank withdraw).
- **X** `MailReturn` the first mail.
- Recipient buffer on `UiFlags.mail_to: String`. Opening mail seeds it from the current target's player name when that entity is a player; otherwise leaves the previous buffer.
- **Enter** toggles compose focus. While focused, `KeyboardInput` text appends (ascii name chars, max 24, same rules as `validate_character_name` length), Backspace deletes, movement keys are ignored for intents, **Esc** blurs without closing the panel.
- **S** sends `MailSend`: `to_name` = buffer if non-empty else current target player name; `bag_slot` = first non-quest stack; `count` = that stack's count; `copper = 0`. Toast local `"Sending parcel…"`; sim toasts errors.
- **Y** (mail open, bank closed) sends remaining wallet copper after postage (`bag_slot: None`, `copper: snapshot.progress.copper.saturating_sub(snap.mail_postage)`). If that copper is 0, toast `"Mail is empty."` locally or let the sim refuse.
- Help line: `[S] Send item to {name} · [Y] Send wallet copper · [P]/[1–9] Collect · [X] Return · Enter compose · postage {n}c`

Client does not compute expiry times beyond showing `expires_tick` when `> 0` as a raw tick number is ugly — omit countdown; expiry is a sim behavior with tests, not HUD chrome.

### 5.9 Server / offline host

- Realm construction after `apply_economy_to_sim`: load directory via `list_mailbox_directory`.
- `spawn_player` / `spawn_player_with_state`: register inside `Sim` so offline `woc-client` and unit tests get names without persist.

## 6. Definition of done

1. Deposit a 12/40 `worn_sword` with `coarse_sharpening` into the bank and withdraw it: same slot instance (`durability == 12`, `enchant_id` preserved). Fingerprint unchanged.
2. Client **G** banks `silverleaf` (not junk). Quest `boar_tusk` is refused for mail/AH, allowed for bank.
3. `RepairAll` at `smith_brann` charges for missing durability on equipped + bag + **bank** gear and restores all three.
4. `MailSend` to a directory-registered name with **no** live `ClassKit` succeeds; collect after `spawn_player_with_state` under that durable key returns the same worn/enchanted stack.
5. Bevy client: **I** then **S** sends to target/buffer; **1–9** collect; **X** returns. `cargo check -p woc-client` green.
6. Postage 1c is deducted; empty mail toasts `"Mail is empty."`; 21st player-to-player mail toasts `"Mailbox is full."`; system AH mail still delivers at cap.
7. Player parcel with `expires_tick = now+1` returns to sender on the next `pvp_and_market` phase as `"Returned: Parcel"`. System mail does not expire.
8. AH list of a worn enchanted sword, cancel or expire, returns the same instance via mail.
9. Old `MailDto` JSON without new keys still loads. `PROTOCOL_REV` remains **8**. `tick_phase_order_fingerprint_locked` stays `3214741777866168171`.
10. `docs/parity/STATUS.md` + `ROADMAP.md` + `DEMO.md` step 6 updated when the implementation wave tags `1.14.0`.

## 7. Explicit non-goals

| Skip | Rationale |
| --- | --- |
| Banker / mailbox / auctioneer NPCs | HUD stays; gating is a UX regression (npc-services §7) |
| Account-wide / guild bank, bought tabs | Personal 24 slots are enough |
| Mail body, COD, multi-attach, CC | One stack + copper is the parcel |
| Typed chat box / whisper | Compose field is mail-panel-only |
| Bag-full → auto-mail overflow | Quest-loop explicitly refused mailbox fallback |
| Destroy item without mail return | **X** returns; no extra delete |
| Per-tick postage scaling / COD insurance | Flat 1c |
| Showing remaining mail time in HUD | Sim expiry is tested; chrome is a later polish |
| Reintroducing a fat actor struct | Violates `AGENTS.md` |
| Bumping upstream past 0.31.0 | Dedicated pin PR only |
| New tick phase | Expiry hooks `pvp_and_market` |

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| `grant_into` call sites keep resetting wear | Only bank/mail/AH moves switch to `take_from_slot`/`put_stack`; loot/craft stay `grant_into` |
| Directory misses a parked player | Live `ClassKit` scan remains fallback |
| Postgres directory query forgotten | Memory + Persist facade tests; server boot calls the API |
| Compose focus steals combat keys | Focus is explicit Enter; Esc blurs; intents ignored only while focused |
| Fingerprint churn | No new phase name; expire is a call inside phase 7 |
| Old mails never expire | `expires_tick` default 0 is deliberate so old inbox is not mass-returned |
| AH snapshot omit-key JSON | `#[serde(default)]` + roundtrip test with omitted keys |

## 9. Success demo (human)

1. Wear down the starter sword, apply a Coarse Whetstone, **K** **G** until the sword banks, withdraw it — still worn and enchanted.
2. Repair at Brann with a broken sword sitting in the bank — bill includes it; bank sword is full.
3. Two characters: log Ada out (park or full restart). On Bob, **I**, type `Ada`, **S** a herb parcel. Log Ada in — inbox has the herb; **P** collects.
4. Send copper with **Y**; collect on the other character.
5. Fill Bob's inbox to 20 player parcels — 21st toasts full. An AH sale still mails proceeds.
6. Leave a parcel uncollected until TTL (or a test hook) — sender receives `"Returned: Parcel"`.

When §6 is green, tag `1.14.0`.
