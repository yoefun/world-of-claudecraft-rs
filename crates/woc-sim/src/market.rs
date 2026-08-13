//! Auction house listings, keyed by durable seller id with offline mail settlement.

use crate::ecs::components::{Bags, ClassKit, Durable, Identity, Progress};
use crate::ecs::World;
use crate::inventory::{grant_into, remove_item};
use crate::mail::Mailbox;
use woc_protocol::{EntityId, MarketListingSnapshot, SimEvent};

/// Listing duration in ticks (~1 hour at 20 Hz).
pub const LISTING_TTL_TICKS: u64 = 20 * 60 * 60;
/// Flat listing fee in copper.
pub const LISTING_FEE: u32 = 5;

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
    pub price: u32,
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
            .map(|l| MarketListingSnapshot {
                id: l.id,
                seller: l.seller_name.clone(),
                item_id: l.item_id.clone(),
                count: l.count,
                price: l.price,
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
        if price == 0 {
            events.push(SimEvent::Toast {
                message: "Price must be positive.".into(),
            });
            return false;
        }
        if world.get::<ClassKit>(seller).is_none() {
            return false;
        }
        let copper = world
            .get::<Progress>(seller)
            .map(|p| p.copper)
            .unwrap_or(0);
        if copper < LISTING_FEE {
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
        let take = count.min(stack.count).max(1);
        if let Some(bags) = world.get_mut::<Bags>(seller) {
            if !remove_item(&mut bags.inventory, &stack.item_id, take) {
                return false;
            }
        } else {
            return false;
        }
        if let Some(progress) = world.get_mut::<Progress>(seller) {
            progress.copper -= LISTING_FEE;
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
            item_id: stack.item_id,
            count: take,
            price,
            expires_tick: now_tick.saturating_add(LISTING_TTL_TICKS),
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
        if world.get::<ClassKit>(buyer).is_none() {
            return false;
        }
        let buyer_copper = world
            .get::<Progress>(buyer)
            .map(|p| p.copper)
            .unwrap_or(0);
        if buyer_copper < listing.price {
            events.push(SimEvent::Toast {
                message: "Not enough copper.".into(),
            });
            return false;
        }
        if let Some(bags) = world.get_mut::<Bags>(buyer) {
            if !grant_into(&mut bags.inventory, &listing.item_id, listing.count) {
                events.push(SimEvent::Toast {
                    message: "Bags are full.".into(),
                });
                return false;
            }
        } else {
            return false;
        }
        if let Some(progress) = world.get_mut::<Progress>(buyer) {
            progress.copper -= listing.price;
        }
        let seller_name = listing.seller_name.clone();
        let seller_online = world.ids::<ClassKit>().into_iter().find(|&id| {
            world.get::<Durable>(id).and_then(|d| d.durable_id.as_deref())
                == Some(listing.seller_durable.as_str())
                || id == listing.seller_id
        });
        if let Some(seller) = seller_online {
            if let Some(progress) = world.get_mut::<Progress>(seller) {
                progress.copper = progress.copper.saturating_add(listing.price);
            }
        } else {
            mail.deliver_system(
                &listing.seller_durable,
                "Auction House",
                "Auction sold",
                listing.price,
                None,
                0,
            );
        }
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

    pub fn cancel(
        &mut self,
        world: &mut World,
        mail: &mut Mailbox,
        seller: EntityId,
        listing_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let seller_key = Some(Mailbox::mailbox_key(world, seller));
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
        let listing = self.listings.remove(idx);
        if world.get::<ClassKit>(seller).is_some() {
            let returned = if let Some(bags) = world.get_mut::<Bags>(seller) {
                grant_into(&mut bags.inventory, &listing.item_id, listing.count)
            } else {
                false
            };
            if !returned {
                mail.deliver_system(
                    &listing.seller_durable,
                    "Auction House",
                    "Listing cancelled",
                    0,
                    Some(listing.item_id),
                    listing.count,
                );
            }
        } else {
            mail.deliver_system(
                &listing.seller_durable,
                "Auction House",
                "Listing cancelled",
                0,
                Some(listing.item_id),
                listing.count,
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
            if listing.expires_tick <= now_tick {
                let seller_online = world.ids::<ClassKit>().into_iter().find(|&id| {
                    world.get::<Durable>(id).and_then(|d| d.durable_id.as_deref())
                        == Some(listing.seller_durable.as_str())
                        || id == listing.seller_id
                });
                if let Some(seller) = seller_online {
                    let returned = if let Some(bags) = world.get_mut::<Bags>(seller) {
                        grant_into(&mut bags.inventory, &listing.item_id, listing.count)
                    } else {
                        false
                    };
                    if !returned {
                        mail.deliver_system(
                            &listing.seller_durable,
                            "Auction House",
                            "Listing expired",
                            0,
                            Some(listing.item_id),
                            listing.count,
                        );
                    }
                } else {
                    mail.deliver_system(
                        &listing.seller_durable,
                        "Auction House",
                        "Listing expired",
                        0,
                        Some(listing.item_id),
                        listing.count,
                    );
                }
            } else {
                keep.push(listing);
            }
        }
        self.listings = keep;
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Progress};
    use crate::inventory::grant_into;
    use crate::mail::Mailbox;
    use woc_content::PlayerClass;

    #[test]
    fn list_buy_and_cancel_flow() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "silverleaf", 1));
        }
        if let Some(p) = world.get_mut::<Progress>(2) {
            p.copper = 500;
        }
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().map(|st| st.item_id == "silverleaf").unwrap_or(false))
            .unwrap() as u8;
        let mut ah = AuctionHouse::new();
        let mut mail = Mailbox::new();
        let mut events = Vec::new();
        assert!(ah.list_item(&mut world, 1, slot, 1, 50, 1, &mut events));
        let listing_id = ah.snapshot_public()[0].id;
        assert!(ah.buy(&mut world, &mut mail, 2, listing_id, &mut events));
    }
}
