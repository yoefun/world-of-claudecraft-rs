//! Auction house listings, keyed by durable seller id with offline mail settlement.

use crate::ecs::components::{Bags, ClassKit, Durable, Identity, InvStack, Progress};
use crate::ecs::World;
use crate::inventory::{grant_stack, take_from_slot};
use crate::mail::Mailbox;
use woc_content::ItemQuality;
use woc_protocol::{EntityId, MarketListingSnapshot, SimEvent};

/// Ticks in one real-time hour at 20 Hz.
pub const TICKS_PER_HOUR: u64 = 20 * 60 * 60;
/// Default listing duration (12 hours) in ticks.
pub const LISTING_TTL_TICKS: u64 = 12 * TICKS_PER_HOUR;
/// Flat 12-hour listing fee in copper.
pub const LISTING_FEE: u32 = 5;
/// House cut is `price / SALE_CUT_DEN` (5% floored).
pub const SALE_CUT_NUM: u32 = 1;
pub const SALE_CUT_DEN: u32 = 20;

pub fn sale_cut(price: u32) -> u32 {
    price.saturating_mul(SALE_CUT_NUM) / SALE_CUT_DEN
}

pub fn sale_proceeds(price: u32) -> u32 {
    price.saturating_sub(sale_cut(price))
}

pub fn duration_ticks(hours: u32) -> Option<u64> {
    match hours {
        0 | 12 => Some(12 * TICKS_PER_HOUR),
        24 => Some(24 * TICKS_PER_HOUR),
        48 => Some(48 * TICKS_PER_HOUR),
        _ => None,
    }
}

pub fn duration_fee(hours: u32) -> Option<u32> {
    match hours {
        0 | 12 => Some(5),
        24 => Some(10),
        48 => Some(20),
        _ => None,
    }
}

pub fn min_next_bid(listing: &Listing) -> Option<u32> {
    if listing.start_bid == 0 && listing.current_bid == 0 {
        return None;
    }
    if listing.bidder_durable.is_none() {
        return Some(listing.start_bid.max(1));
    }
    Some(
        listing
            .current_bid
            .saturating_add((listing.current_bid / 20).max(1)),
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct Listing {
    pub id: u32,
    /// Ephemeral entity id when seller is online (may be stale).
    pub seller_id: EntityId,
    /// Durable character UUID / local key.
    pub seller_durable: String,
    pub seller_name: String,
    pub item_id: String,
    pub count: u32,
    pub durability: Option<u32>,
    pub enchant_id: Option<String>,
    pub quality: Option<ItemQuality>,
    pub bound: bool,
    pub price: u32,
    pub start_bid: u32,
    pub current_bid: u32,
    pub bidder_durable: Option<String>,
    pub bidder_name: Option<String>,
    pub expires_tick: u64,
}

#[derive(Debug, Default)]
pub struct AuctionHouse {
    next_id: u32,
    pub listings: Vec<Listing>,
}

impl AuctionHouse {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            listings: Vec::new(),
        }
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn set_next_id(&mut self, id: u32) {
        self.next_id = id.max(1);
    }

    pub fn load_listings(&mut self, listings: Vec<Listing>, next_id: u32) {
        let max_id = listings.iter().map(|l| l.id).max().unwrap_or(0);
        self.listings = listings;
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);
    }

    pub fn snapshot_public(&self) -> Vec<MarketListingSnapshot> {
        self.listings
            .iter()
            .map(|l| listing_snapshot(l, false))
            .collect()
    }

    /// Public listings with `mine` flagged for the viewing seller.
    pub fn snapshot_for(&self, viewer: EntityId, world: &World) -> Vec<MarketListingSnapshot> {
        let viewer_key = (viewer != 0).then(|| Mailbox::mailbox_key(world, viewer));
        self.listings
            .iter()
            .map(|l| {
                let mine = viewer != 0
                    && (l.seller_id == viewer
                        || viewer_key.as_ref().is_some_and(|k| k == &l.seller_durable));
                listing_snapshot(l, mine)
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_item(
        &mut self,
        world: &mut World,
        seller: EntityId,
        bag_slot: u8,
        count: u32,
        price: u32,
        now_tick: u64,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        self.list_item_ex(
            world, seller, bag_slot, count, price, 0, 12, now_tick, events,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_item_ex(
        &mut self,
        world: &mut World,
        seller: EntityId,
        bag_slot: u8,
        count: u32,
        price: u32,
        start_bid: u32,
        duration_hours: u32,
        now_tick: u64,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        if price == 0 && start_bid == 0 {
            events.push(SimEvent::Toast {
                message: "Set a starting bid or buyout.".into(),
            });
            return false;
        }
        if price > 0 && start_bid > price {
            events.push(SimEvent::Toast {
                message: "Starting bid must be at most the buyout.".into(),
            });
            return false;
        }
        let Some(ttl) = duration_ticks(duration_hours) else {
            events.push(SimEvent::Toast {
                message: "Duration must be 12, 24, or 48 hours.".into(),
            });
            return false;
        };
        let Some(fee) = duration_fee(duration_hours) else {
            return false;
        };
        if world.get::<ClassKit>(seller).is_none() {
            return false;
        }
        let copper = world.get::<Progress>(seller).map(|p| p.copper).unwrap_or(0);
        if copper < fee {
            events.push(SimEvent::Toast {
                message: "Cannot afford listing fee.".into(),
            });
            return false;
        }
        let stack = world
            .get::<Bags>(seller)
            .and_then(|b| b.inventory.get(bag_slot as usize))
            .and_then(|s| s.clone());
        let Some(stack) = stack else {
            events.push(SimEvent::Toast {
                message: "Empty bag slot.".into(),
            });
            return false;
        };
        if stack.bound {
            events.push(SimEvent::Toast {
                message: "That item is soulbound.".into(),
            });
            return false;
        }
        if woc_content::item(&stack.item_id).is_some_and(|d| d.kind == woc_content::ItemKind::Quest)
        {
            events.push(SimEvent::Toast {
                message: "This item is needed for a quest.".into(),
            });
            return false;
        }
        let take = count.min(stack.count).max(1);
        let Some(taken) = (if let Some(bags) = world.get_mut::<Bags>(seller) {
            take_from_slot(&mut bags.inventory, bag_slot, take)
        } else {
            None
        }) else {
            return false;
        };
        if let Some(progress) = world.get_mut::<Progress>(seller) {
            progress.copper -= fee;
        }
        let seller_durable = Mailbox::mailbox_key(world, seller);
        let seller_name = world
            .get::<Identity>(seller)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let listing_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.listings.push(Listing {
            id: listing_id,
            seller_id: seller,
            seller_durable,
            seller_name,
            item_id: taken.item_id,
            count: taken.count,
            durability: taken.durability,
            enchant_id: taken.enchant_id,
            quality: taken.quality,
            bound: taken.bound,
            price,
            start_bid,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: now_tick.saturating_add(ttl),
        });
        events.push(SimEvent::MarketListed {
            player: seller,
            listing_id,
        });
        true
    }

    pub fn buy(
        &mut self,
        world: &mut World,
        mail: &mut Mailbox,
        buyer: EntityId,
        listing_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let Some(idx) = self.listings.iter().position(|l| l.id == listing_id) else {
            events.push(SimEvent::Toast {
                message: "Listing not found.".into(),
            });
            return false;
        };
        let listing = self.listings[idx].clone();
        let buyer_durable = Mailbox::mailbox_key(world, buyer);
        if listing.seller_durable == buyer_durable || listing.seller_id == buyer {
            events.push(SimEvent::Toast {
                message: "Cannot buy your own listing.".into(),
            });
            return false;
        }
        if listing.price == 0 {
            events.push(SimEvent::Toast {
                message: "No buyout on this listing.".into(),
            });
            return false;
        }
        if world.get::<ClassKit>(buyer).is_none() {
            return false;
        }
        let already_bid = listing
            .bidder_durable
            .as_ref()
            .is_some_and(|k| k == &buyer_durable);
        let due = if already_bid {
            listing.price.saturating_sub(listing.current_bid)
        } else {
            listing.price
        };
        let buyer_copper = world.get::<Progress>(buyer).map(|p| p.copper).unwrap_or(0);
        if buyer_copper < due {
            events.push(SimEvent::Toast {
                message: "Not enough copper.".into(),
            });
            return false;
        }
        let granted = if let Some(bags) = world.get_mut::<Bags>(buyer) {
            grant_stack(&mut bags.inventory, listing_stack(&listing))
        } else {
            false
        };
        if !granted {
            events.push(SimEvent::Toast {
                message: "Bags are full.".into(),
            });
            return false;
        }
        if let Some(progress) = world.get_mut::<Progress>(buyer) {
            progress.copper -= due;
        }
        if let (Some(prev_key), Some(prev_name)) = (
            listing.bidder_durable.as_ref(),
            listing.bidder_name.as_ref(),
        ) {
            if prev_key != &buyer_durable {
                mail.deliver_system(
                    prev_key,
                    "Auction House",
                    "Outbid",
                    listing.current_bid,
                    None,
                );
                let _ = prev_name;
            }
        }
        let seller_name = listing.seller_name.clone();
        mail.deliver_system(
            &listing.seller_durable,
            "Auction House",
            "Auction sold",
            sale_proceeds(listing.price),
            None,
        );
        self.listings.remove(idx);
        events.push(SimEvent::ItemGained {
            player: buyer,
            item_id: listing.item_id,
            count: listing.count,
        });
        events.push(SimEvent::MarketSold {
            listing_id,
            buyer,
            seller_name,
        });
        true
    }

    pub fn bid(
        &mut self,
        world: &mut World,
        mail: &mut Mailbox,
        bidder: EntityId,
        listing_id: u32,
        amount: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let Some(idx) = self.listings.iter().position(|l| l.id == listing_id) else {
            events.push(SimEvent::Toast {
                message: "Listing not found.".into(),
            });
            return false;
        };
        let listing = self.listings[idx].clone();
        let bidder_durable = Mailbox::mailbox_key(world, bidder);
        if listing.seller_durable == bidder_durable || listing.seller_id == bidder {
            events.push(SimEvent::Toast {
                message: "Cannot bid on your own listing.".into(),
            });
            return false;
        }
        let Some(min_bid) = min_next_bid(&listing) else {
            events.push(SimEvent::Toast {
                message: "This listing is buyout only.".into(),
            });
            return false;
        };
        if listing
            .bidder_durable
            .as_ref()
            .is_some_and(|k| k == &bidder_durable)
        {
            events.push(SimEvent::Toast {
                message: "You already hold the high bid.".into(),
            });
            return false;
        }
        if amount < min_bid {
            events.push(SimEvent::Toast {
                message: "Bid is too low.".into(),
            });
            return false;
        }
        if listing.price > 0 && amount >= listing.price {
            events.push(SimEvent::Toast {
                message: "Use buyout for that price.".into(),
            });
            return false;
        }
        if world.get::<ClassKit>(bidder).is_none() {
            return false;
        }
        let copper = world.get::<Progress>(bidder).map(|p| p.copper).unwrap_or(0);
        if copper < amount {
            events.push(SimEvent::Toast {
                message: "Not enough copper.".into(),
            });
            return false;
        }
        if let Some(progress) = world.get_mut::<Progress>(bidder) {
            progress.copper -= amount;
        }
        if let Some(prev_key) = listing.bidder_durable.as_ref() {
            mail.deliver_system(
                prev_key,
                "Auction House",
                "Outbid",
                listing.current_bid,
                None,
            );
        }
        let bidder_name = world
            .get::<Identity>(bidder)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let row = &mut self.listings[idx];
        row.current_bid = amount;
        row.bidder_durable = Some(bidder_durable);
        row.bidder_name = Some(bidder_name);
        events.push(SimEvent::Toast {
            message: format!("Bid {amount}c on listing #{listing_id}."),
        });
        true
    }

    pub fn cancel(
        &mut self,
        world: &mut World,
        mail: &mut Mailbox,
        seller: EntityId,
        listing_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        // Only a live player can own a listing. Without this guard the durable-key
        // comparison below degrades to matching a synthesized `local:{id}` string,
        // which a recycled id can satisfy after a restart.
        let seller_key = world
            .get::<ClassKit>(seller)
            .is_some()
            .then(|| Mailbox::mailbox_key(world, seller));
        let Some(idx) = self.listings.iter().position(|l| l.id == listing_id) else {
            return false;
        };
        let owned = self.listings[idx].seller_id == seller
            || seller_key
                .as_ref()
                .is_some_and(|k| k == &self.listings[idx].seller_durable);
        if !owned {
            return false;
        }
        if self.listings[idx].bidder_durable.is_some() {
            events.push(SimEvent::Toast {
                message: "Cannot cancel after a bid.".into(),
            });
            return false;
        }
        let listing = self.listings.remove(idx);
        let stack = listing_stack(&listing);
        if world.get::<ClassKit>(seller).is_some() {
            let returned = if let Some(bags) = world.get_mut::<Bags>(seller) {
                grant_stack(&mut bags.inventory, stack.clone())
            } else {
                false
            };
            if !returned {
                mail.deliver_system(
                    &listing.seller_durable,
                    "Auction House",
                    "Listing cancelled",
                    0,
                    Some(stack),
                );
            }
        } else {
            mail.deliver_system(
                &listing.seller_durable,
                "Auction House",
                "Listing cancelled",
                0,
                Some(stack),
            );
        }
        events.push(SimEvent::Toast {
            message: "Listing cancelled.".into(),
        });
        true
    }

    pub fn tick_expire(&mut self, now_tick: u64, world: &mut World, mail: &mut Mailbox) {
        let mut keep = Vec::new();
        for listing in self.listings.drain(..) {
            if listing.expires_tick > now_tick {
                keep.push(listing);
                continue;
            }
            if let Some(bidder_key) = listing.bidder_durable.clone() {
                mail.deliver_system(
                    &bidder_key,
                    "Auction House",
                    "Auction won",
                    0,
                    Some(listing_stack(&listing)),
                );
                mail.deliver_system(
                    &listing.seller_durable,
                    "Auction House",
                    "Auction sold",
                    sale_proceeds(listing.current_bid),
                    None,
                );
                continue;
            }
            let seller_online = world.ids::<ClassKit>().into_iter().find(|&id| {
                world
                    .get::<Durable>(id)
                    .and_then(|d| d.durable_id.as_deref())
                    == Some(listing.seller_durable.as_str())
                    || id == listing.seller_id
            });
            if let Some(seller) = seller_online {
                let stack = listing_stack(&listing);
                let returned = if let Some(bags) = world.get_mut::<Bags>(seller) {
                    grant_stack(&mut bags.inventory, stack.clone())
                } else {
                    false
                };
                if !returned {
                    mail.deliver_system(
                        &listing.seller_durable,
                        "Auction House",
                        "Listing expired",
                        0,
                        Some(stack),
                    );
                }
            } else {
                mail.deliver_system(
                    &listing.seller_durable,
                    "Auction House",
                    "Listing expired",
                    0,
                    Some(listing_stack(&listing)),
                );
            }
        }
        self.listings = keep;
    }
}

fn listing_snapshot(listing: &Listing, mine: bool) -> MarketListingSnapshot {
    MarketListingSnapshot {
        id: listing.id,
        seller: listing.seller_name.clone(),
        item_id: listing.item_id.clone(),
        count: listing.count,
        price: listing.price,
        mine,
        durability: listing.durability,
        enchant_id: listing.enchant_id.clone(),
        quality: listing.quality.map(|q| q.as_str().to_string()),
        expires_tick: listing.expires_tick,
        start_bid: listing.start_bid,
        current_bid: listing.current_bid,
        bidder: listing.bidder_name.clone(),
        bound: listing.bound,
    }
}

fn listing_stack(listing: &Listing) -> InvStack {
    InvStack {
        item_id: listing.item_id.clone(),
        count: listing.count,
        durability: listing.durability,
        enchant_id: listing.enchant_id.clone(),
        quality: listing.quality,
        bound: listing.bound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Durable, Identity, InvStack, Progress};
    use crate::inventory::grant_into;
    use crate::mail::Mailbox;
    use woc_content::PlayerClass;
    use woc_protocol::EntityId;

    #[test]
    fn list_buy_and_cancel_flow() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "silverleaf", 1));
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(p) = world.get_mut::<Progress>(2) {
            p.copper = 500;
        }
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| {
                s.as_ref()
                    .map(|st| st.item_id == "silverleaf")
                    .unwrap_or(false)
            })
            .unwrap() as u8;
        let mut ah = AuctionHouse::new();
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(ah.list_item(&mut world, 1, slot, 1, 50, 1, &mut events));
        let listing_id = ah.snapshot_public()[0].id;
        assert!(ah.buy(&mut world, &mut mail, 2, listing_id, &mut events));
    }

    #[test]
    fn buy_mails_proceeds_when_seller_offline() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(2) {
            d.durable_id = Some("bob".into());
        }
        if let Some(p) = world.get_mut::<Progress>(2) {
            p.copper = 200;
        }
        let mut ah = AuctionHouse::new();
        ah.listings.push(Listing {
            id: 1,
            seller_id: 1,
            seller_durable: "ada".into(),
            seller_name: "Ada".into(),
            item_id: "silverleaf".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
            price: 40,
            start_bid: 0,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: 9999,
        });
        ah.next_id = 2;
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(ah.buy(&mut world, &mut mail, 2, 1, &mut events));
        assert_eq!(mail.all_mails().len(), 1);
        assert_eq!(mail.all_mails()[0].copper, 38);
    }

    /// A listing left by a pre-restart player with no durable id carries the
    /// synthesized key `local:{id}`. After the restart that id can be handed to a
    /// different entity, so cancel must require the caller to be a live player.
    #[test]
    fn cancel_rejects_absent_seller_matching_a_synthesized_key() {
        let mut world = World::new();
        let mut ah = AuctionHouse::new();
        ah.listings.push(Listing {
            id: 1,
            seller_id: 99,
            seller_durable: "local:7".into(),
            seller_name: "Ada".into(),
            item_id: "silverleaf".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
            price: 40,
            start_bid: 0,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: 9999,
        });
        ah.next_id = 2;
        let mut mail = Mailbox::new();
        let mut events = Vec::new();

        // Entity 7 does not exist at all.
        assert!(!ah.cancel(&mut world, &mut mail, 7, 1, &mut events));
        assert_eq!(ah.listings.len(), 1);

        // Entity 7 exists but is a mob, not a player, so it holds no ClassKit.
        crate::ecs::spawn::create_mob_from_template(&mut world, 7, "meadow_wolf", 0.0, 0.0);
        assert!(!ah.cancel(&mut world, &mut mail, 7, 1, &mut events));
        assert_eq!(ah.listings.len(), 1);
    }

    #[test]
    fn cancel_succeeds_for_the_live_player_holding_the_key() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 7, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        let mut ah = AuctionHouse::new();
        ah.listings.push(Listing {
            id: 1,
            seller_id: 99,
            seller_durable: "local:7".into(),
            seller_name: "Ada".into(),
            item_id: "silverleaf".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
            price: 40,
            start_bid: 0,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: 9999,
        });
        ah.next_id = 2;
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(ah.cancel(&mut world, &mut mail, 7, 1, &mut events));
        assert!(ah.listings.is_empty());
    }

    #[test]
    fn list_takes_the_named_slot_not_another_stack_of_the_same_id() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.inventory[0] = Some(InvStack {
                item_id: "silverleaf".into(),
                count: 3,
                durability: None,
                enchant_id: None,
                quality: None,
                bound: false,
            });
            bags.inventory[1] = Some(InvStack {
                item_id: "silverleaf".into(),
                count: 2,
                durability: None,
                enchant_id: None,
                quality: None,
                bound: false,
            });
        }
        let mut ah = AuctionHouse::new();
        let mut events = Vec::new();
        assert!(ah.list_item(&mut world, 1, 1, 1, 12, 1, &mut events));
        let bags = world.get::<Bags>(1).unwrap();
        assert_eq!(bags.inventory[0].as_ref().unwrap().count, 3);
        assert_eq!(bags.inventory[1].as_ref().unwrap().count, 1);
        assert_eq!(ah.listings[0].count, 1);
    }

    #[test]
    fn list_refuses_quest_items() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "boar_tusk", 1));
        }
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "boar_tusk"))
            .unwrap() as u8;
        let mut ah = AuctionHouse::new();
        let mut events = Vec::new();
        assert!(!ah.list_item(&mut world, 1, slot, 1, 20, 1, &mut events));
        assert!(ah.listings.is_empty());
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 100);
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "This item is needed for a quest."
        )));
    }

    #[test]
    fn list_stores_durability_and_enchant() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.inventory[0] = Some(InvStack {
                item_id: "worn_sword".into(),
                count: 1,
                durability: Some(7),
                enchant_id: Some("coarse_sharpening".into()),
                quality: Some(ItemQuality::Rare),
                bound: false,
            });
        }
        let mut ah = AuctionHouse::new();
        let mut events = Vec::new();
        assert!(ah.list_item(&mut world, 1, 0, 1, 25, 1, &mut events));
        assert_eq!(ah.listings[0].durability, Some(7));
        assert_eq!(
            ah.listings[0].enchant_id.as_deref(),
            Some("coarse_sharpening")
        );
        assert_eq!(ah.listings[0].quality, Some(ItemQuality::Rare));
    }

    #[test]
    fn sale_cut_is_five_percent_floored() {
        assert_eq!(sale_cut(50), 2);
        assert_eq!(sale_proceeds(50), 48);
        assert_eq!(sale_cut(19), 0);
        assert_eq!(sale_proceeds(19), 19);
    }

    #[test]
    fn buy_always_mails_proceeds_even_when_seller_is_online() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(p) = world.get_mut::<Progress>(2) {
            p.copper = 200;
        }
        let mut ah = AuctionHouse::new();
        ah.listings.push(Listing {
            id: 1,
            seller_id: 1,
            seller_durable: "ada".into(),
            seller_name: "Ada".into(),
            item_id: "silverleaf".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
            price: 50,
            start_bid: 0,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: 9999,
        });
        ah.set_next_id(2);
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(ah.buy(&mut world, &mut mail, 2, 1, &mut events));
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 100);
        assert_eq!(world.get::<Progress>(2).unwrap().copper, 150);
        assert_eq!(mail.all_mails().len(), 1);
        assert_eq!(mail.all_mails()[0].copper, 48);
        assert_eq!(mail.all_mails()[0].subject, "Auction sold");
    }

    #[test]
    fn buy_grants_the_listed_wear_and_enchant() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(2) {
            p.copper = 200;
        }
        let mut ah = AuctionHouse::new();
        ah.listings.push(Listing {
            id: 1,
            seller_id: 1,
            seller_durable: "ada".into(),
            seller_name: "Ada".into(),
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(7),
            enchant_id: Some("coarse_sharpening".into()),
            quality: Some(ItemQuality::Uncommon),
            bound: false,
            price: 40,
            start_bid: 0,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: 9999,
        });
        ah.set_next_id(2);
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(ah.buy(&mut world, &mut mail, 2, 1, &mut events));
        let sword = world
            .get::<Bags>(2)
            .unwrap()
            .inventory
            .iter()
            .flatten()
            .find(|s| s.item_id == "worn_sword")
            .unwrap();
        assert_eq!(sword.durability, Some(7));
        assert_eq!(sword.enchant_id.as_deref(), Some("coarse_sharpening"));
        assert_eq!(sword.quality, Some(ItemQuality::Uncommon));
    }

    fn npc_id_by_template(world: &World, template: &str) -> EntityId {
        world
            .ids::<Identity>()
            .into_iter()
            .find(|&id| {
                world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some(template)
            })
            .expect(template)
    }

    #[test]
    fn interact_market_list_requires_auctioneer_session() {
        use woc_protocol::{InteractAction, WorldHost};
        let mut sim = crate::sim::Sim::new_eastbrook("Ada", PlayerClass::Warrior);
        let pid = sim.player_id;
        if let Some(p) = sim.world.get_mut::<Progress>(pid) {
            p.copper = 100;
        }
        if let Some(bags) = sim.world.get_mut::<Bags>(pid) {
            assert!(grant_into(&mut bags.inventory, "silverleaf", 1));
        }
        let slot = sim
            .world
            .get::<Bags>(pid)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "silverleaf"))
            .unwrap() as u8;
        WorldHost::interact(
            &mut sim,
            pid,
            0,
            InteractAction::MarketList {
                bag_slot: slot,
                count: 1,
                price: 12,
                start_bid: 0,
                duration_hours: 0,
            },
        );
        assert!(sim.market.listings.is_empty());
        assert!(sim.events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Talk to an auctioneer first."
        )));

        let lise = npc_id_by_template(&sim.world, "auctioneer_lise");
        if let Some(nt) = sim
            .world
            .get::<crate::ecs::components::Transform>(lise)
            .cloned()
        {
            if let Some(p) = sim.world.get_mut::<crate::ecs::components::Transform>(pid) {
                p.x = nt.x;
                p.z = nt.z;
            }
        }
        WorldHost::interact(&mut sim, pid, lise, InteractAction::Talk);
        sim.events.clear();
        WorldHost::interact(
            &mut sim,
            pid,
            lise,
            InteractAction::MarketList {
                bag_slot: slot,
                count: 1,
                price: 12,
                start_bid: 0,
                duration_hours: 0,
            },
        );
        assert_eq!(sim.market.listings.len(), 1);
    }

    #[test]
    fn list_refuses_soulbound_items() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.inventory[0] = Some(InvStack {
                item_id: "silverleaf".into(),
                count: 1,
                durability: None,
                enchant_id: None,
                quality: None,
                bound: true,
            });
        }
        let mut ah = AuctionHouse::new();
        let mut events = Vec::new();
        assert!(!ah.list_item(&mut world, 1, 0, 1, 20, 1, &mut events));
        assert!(ah.listings.is_empty());
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "That item is soulbound."
        )));
    }

    #[test]
    fn duration_tiers_set_fee_and_ttl() {
        assert_eq!(duration_ticks(12), Some(864_000));
        assert_eq!(duration_ticks(24), Some(1_728_000));
        assert_eq!(duration_ticks(48), Some(3_456_000));
        assert_eq!(duration_fee(12), Some(5));
        assert_eq!(duration_fee(24), Some(10));
        assert_eq!(duration_fee(48), Some(20));
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "silverleaf", 1));
        }
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "silverleaf"))
            .unwrap() as u8;
        let mut ah = AuctionHouse::new();
        let mut events = Vec::new();
        assert!(!ah.list_item_ex(&mut world, 1, slot, 1, 20, 0, 7, 1, &mut events));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Duration must be 12, 24, or 48 hours."
        )));
        events.clear();
        assert!(ah.list_item_ex(&mut world, 1, slot, 1, 20, 0, 24, 1, &mut events));
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 90);
        assert_eq!(ah.listings[0].expires_tick, 1 + 1_728_000);
    }

    #[test]
    fn bid_outbid_and_expire_settles_winner() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 3, "Cat", PlayerClass::Rogue, 2.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(2) {
            d.durable_id = Some("bob".into());
        }
        if let Some(d) = world.get_mut::<Durable>(3) {
            d.durable_id = Some("cat".into());
        }
        if let Some(p) = world.get_mut::<Progress>(2) {
            p.copper = 200;
        }
        if let Some(p) = world.get_mut::<Progress>(3) {
            p.copper = 200;
        }
        let mut ah = AuctionHouse::new();
        ah.listings.push(Listing {
            id: 1,
            seller_id: 1,
            seller_durable: "ada".into(),
            seller_name: "Ada".into(),
            item_id: "silverleaf".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
            price: 80,
            start_bid: 10,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: 50,
        });
        ah.set_next_id(2);
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(ah.bid(&mut world, &mut mail, 2, 1, 10, &mut events));
        assert_eq!(world.get::<Progress>(2).unwrap().copper, 190);
        assert!(ah.bid(&mut world, &mut mail, 3, 1, 12, &mut events));
        assert_eq!(world.get::<Progress>(3).unwrap().copper, 188);
        let outbid = mail
            .all_mails()
            .into_iter()
            .find(|m| m.subject == "Outbid")
            .unwrap();
        assert_eq!(outbid.copper, 10);
        assert_eq!(outbid.to_durable, "bob");
        assert!(!ah.cancel(&mut world, &mut mail, 1, 1, &mut events));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Cannot cancel after a bid."
        )));
        ah.tick_expire(50, &mut world, &mut mail);
        assert!(ah.listings.is_empty());
        let won = mail
            .all_mails()
            .into_iter()
            .find(|m| m.subject == "Auction won")
            .unwrap();
        assert_eq!(won.to_durable, "cat");
        assert_eq!(won.item_id.as_deref(), Some("silverleaf"));
        let sold = mail
            .all_mails()
            .into_iter()
            .find(|m| m.subject == "Auction sold")
            .unwrap();
        assert_eq!(sold.copper, sale_proceeds(12));
    }

    #[test]
    fn buyout_only_rejects_bids() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(2) {
            p.copper = 200;
        }
        let mut ah = AuctionHouse::new();
        ah.listings.push(Listing {
            id: 1,
            seller_id: 1,
            seller_durable: "ada".into(),
            seller_name: "Ada".into(),
            item_id: "silverleaf".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
            price: 40,
            start_bid: 0,
            current_bid: 0,
            bidder_durable: None,
            bidder_name: None,
            expires_tick: 9999,
        });
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(!ah.bid(&mut world, &mut mail, 2, 1, 10, &mut events));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "This listing is buyout only."
        )));
    }
}
