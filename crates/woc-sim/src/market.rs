//! Auction house listings.

use crate::entity::{grant_into, remove_item, Entity};
use woc_protocol::{EntityId, EntityKind, MarketListingSnapshot, SimEvent};

/// Listing duration in ticks (~1 hour at 20 Hz).
pub const LISTING_TTL_TICKS: u64 = 20 * 60 * 60;
/// Flat listing fee in copper.
pub const LISTING_FEE: u32 = 5;

#[derive(Debug, Clone)]
pub struct Listing {
    pub id: u32,
    pub seller_id: EntityId,
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

    pub fn list_item(
        &mut self,
        entities: &mut [Entity],
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
        let Some(player) = entities.iter_mut().find(|e| e.id == seller) else {
            return false;
        };
        if player.copper < LISTING_FEE {
            events.push(SimEvent::Toast {
                message: "Cannot afford listing fee.".into(),
            });
            return false;
        }
        let Some(Some(stack)) = player.inventory.get(bag_slot as usize).cloned() else {
            events.push(SimEvent::Toast {
                message: "Empty bag slot.".into(),
            });
            return false;
        };
        let take = count.min(stack.count).max(1);
        if !remove_item(&mut player.inventory, &stack.item_id, take) {
            return false;
        }
        player.copper -= LISTING_FEE;
        let listing_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.listings.push(Listing {
            id: listing_id,
            seller_id: seller,
            seller_name: player.name.clone(),
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
        entities: &mut [Entity],
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
        if listing.seller_id == buyer {
            events.push(SimEvent::Toast {
                message: "Cannot buy your own listing.".into(),
            });
            return false;
        }
        let Some(buyer_e) = entities.iter_mut().find(|e| e.id == buyer) else {
            return false;
        };
        if buyer_e.copper < listing.price {
            events.push(SimEvent::Toast {
                message: "Not enough copper.".into(),
            });
            return false;
        }
        if !grant_into(&mut buyer_e.inventory, &listing.item_id, listing.count) {
            events.push(SimEvent::Toast {
                message: "Bags are full.".into(),
            });
            return false;
        }
        buyer_e.copper -= listing.price;
        let seller_name = listing.seller_name.clone();
        if let Some(seller) = entities.iter_mut().find(|e| e.id == listing.seller_id) {
            seller.copper = seller.copper.saturating_add(listing.price);
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
        entities: &mut [Entity],
        seller: EntityId,
        listing_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let Some(idx) = self.listings.iter().position(|l| l.id == listing_id) else {
            return false;
        };
        if self.listings[idx].seller_id != seller {
            return false;
        }
        let listing = self.listings.remove(idx);
        if let Some(player) = entities.iter_mut().find(|e| e.id == seller) {
            let _ = grant_into(&mut player.inventory, &listing.item_id, listing.count);
        }
        events.push(SimEvent::Toast {
            message: "Listing cancelled.".into(),
        });
        true
    }

    pub fn tick_expire(&mut self, now_tick: u64, entities: &mut [Entity]) {
        let mut keep = Vec::new();
        for listing in self.listings.drain(..) {
            if listing.expires_tick <= now_tick {
                if let Some(seller) = entities
                    .iter_mut()
                    .find(|e| e.id == listing.seller_id && e.kind == EntityKind::Player)
                {
                    let _ = grant_into(&mut seller.inventory, &listing.item_id, listing.count);
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
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    #[test]
    fn list_and_buy() {
        let mut entities = vec![
            create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "Bob", PlayerClass::Mage, 1.0, 0.0),
        ];
        entities[0].copper = 50;
        entities[1].copper = 200;
        assert!(grant_into(&mut entities[0].inventory, "silverleaf", 1));
        let slot = entities[0]
            .inventory
            .iter()
            .position(|s| {
                s.as_ref()
                    .map(|st| st.item_id == "silverleaf")
                    .unwrap_or(false)
            })
            .unwrap() as u8;
        let mut ah = AuctionHouse::new();
        let mut events = Vec::new();
        assert!(ah.list_item(&mut entities, 1, slot, 1, 40, 0, &mut events));
        assert_eq!(ah.listings.len(), 1);
        let id = ah.listings[0].id;
        assert!(ah.buy(&mut entities, 2, id, &mut events));
        assert!(ah.listings.is_empty());
        assert_eq!(entities[1].copper, 160);
        assert!(entities[0].copper >= 40);
    }
}
