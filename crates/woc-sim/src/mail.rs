//! Player mailbox (send / collect).

use std::collections::HashMap;

use crate::entity::{grant_into, remove_item, Entity};
use woc_protocol::{EntityId, EntityKind, MailSnapshot, SimEvent};

#[derive(Debug, Clone)]
pub struct MailItem {
    pub id: u32,
    pub from: String,
    pub to_player: EntityId,
    pub subject: String,
    pub copper: u32,
    pub item_id: Option<String>,
    pub item_count: u32,
}

#[derive(Debug, Default)]
pub struct Mailbox {
    next_id: u32,
    /// recipient → mails
    inbox: HashMap<EntityId, Vec<MailItem>>,
}

impl Mailbox {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            inbox: HashMap::new(),
        }
    }

    pub fn snapshot_for(&self, player_id: EntityId) -> Vec<MailSnapshot> {
        self.inbox
            .get(&player_id)
            .map(|mails| {
                mails
                    .iter()
                    .map(|m| MailSnapshot {
                        id: m.id,
                        from: m.from.clone(),
                        subject: m.subject.clone(),
                        copper: m.copper,
                        item_id: m.item_id.clone(),
                        item_count: m.item_count,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn send(
        &mut self,
        entities: &mut [Entity],
        from: EntityId,
        to_name: &str,
        copper: u32,
        bag_slot: Option<u8>,
        count: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let to_id = entities
            .iter()
            .find(|e| e.kind == EntityKind::Player && e.name.eq_ignore_ascii_case(to_name))
            .map(|e| e.id);
        let Some(to_id) = to_id else {
            events.push(SimEvent::Toast {
                message: "Recipient not found.".into(),
            });
            return false;
        };
        if to_id == from {
            events.push(SimEvent::Toast {
                message: "Cannot mail yourself.".into(),
            });
            return false;
        }

        let Some(sender) = entities.iter_mut().find(|e| e.id == from) else {
            return false;
        };
        if !sender.alive {
            return false;
        }
        if copper > sender.copper {
            events.push(SimEvent::Toast {
                message: "Not enough copper.".into(),
            });
            return false;
        }

        let mut item_id = None;
        let mut item_count = 0u32;
        if let Some(slot) = bag_slot {
            let Some(Some(stack)) = sender.inventory.get(slot as usize).cloned() else {
                events.push(SimEvent::Toast {
                    message: "Empty bag slot.".into(),
                });
                return false;
            };
            let take = count.min(stack.count).max(1);
            if !remove_item(&mut sender.inventory, &stack.item_id, take) {
                return false;
            }
            item_id = Some(stack.item_id);
            item_count = take;
        }

        sender.copper -= copper;
        let from_name = sender.name.clone();
        let mail_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.inbox.entry(to_id).or_default().push(MailItem {
            id: mail_id,
            from: from_name,
            to_player: to_id,
            subject: "Parcel".into(),
            copper,
            item_id,
            item_count,
        });
        events.push(SimEvent::MailSent {
            from,
            to_name: to_name.to_string(),
            mail_id,
        });
        true
    }

    pub fn collect(
        &mut self,
        entities: &mut [Entity],
        player: EntityId,
        mail_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let Some(mails) = self.inbox.get_mut(&player) else {
            return false;
        };
        let Some(idx) = mails.iter().position(|m| m.id == mail_id) else {
            events.push(SimEvent::Toast {
                message: "Mail not found.".into(),
            });
            return false;
        };
        let mail = mails.remove(idx);
        let Some(recv) = entities.iter_mut().find(|e| e.id == player) else {
            return false;
        };
        if let Some(ref item_id) = mail.item_id {
            if !grant_into(&mut recv.inventory, item_id, mail.item_count.max(1)) {
                // Put mail back.
                self.inbox.entry(player).or_default().insert(idx, mail);
                events.push(SimEvent::Toast {
                    message: "Bags are full.".into(),
                });
                return false;
            }
            events.push(SimEvent::ItemGained {
                player,
                item_id: item_id.clone(),
                count: mail.item_count.max(1),
            });
        }
        recv.copper = recv.copper.saturating_add(mail.copper);
        events.push(SimEvent::MailCollected { player, mail_id });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    #[test]
    fn send_and_collect_copper_and_item() {
        let mut entities = vec![
            create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "Bob", PlayerClass::Mage, 1.0, 0.0),
        ];
        entities[0].copper = 100;
        assert!(grant_into(&mut entities[0].inventory, "silverleaf", 2));
        let slot = entities[0]
            .inventory
            .iter()
            .position(|s| {
                s.as_ref()
                    .map(|st| st.item_id == "silverleaf")
                    .unwrap_or(false)
            })
            .unwrap() as u8;
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(box_.send(&mut entities, 1, "Bob", 25, Some(slot), 1, &mut events));
        assert_eq!(entities[0].copper, 75);
        assert!(box_.collect(&mut entities, 2, 1, &mut events));
        assert_eq!(entities[1].copper, 25);
        assert!(crate::entity::count_item(&entities[1].inventory, "silverleaf") >= 1);
    }
}
