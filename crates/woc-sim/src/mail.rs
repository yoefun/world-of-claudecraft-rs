//! Player mailbox (send / collect), keyed by durable character id when present.

use std::collections::HashMap;

use crate::entity::{grant_into, remove_item, Entity};
use woc_protocol::{EntityId, EntityKind, MailSnapshot, SimEvent};

/// Durable mailbox entry (survives reconnect / restart when persisted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailItem {
    pub id: u32,
    pub from: String,
    /// Recipient durable character UUID, or `local:{entity_id}` offline.
    pub to_durable: String,
    pub subject: String,
    pub copper: u32,
    pub item_id: Option<String>,
    pub item_count: u32,
}

#[derive(Debug, Default)]
pub struct Mailbox {
    next_id: u32,
    /// durable recipient key → mails
    inbox: HashMap<String, Vec<MailItem>>,
}

impl Mailbox {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            inbox: HashMap::new(),
        }
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn set_next_id(&mut self, id: u32) {
        self.next_id = id.max(1);
    }

    pub fn all_mails(&self) -> Vec<MailItem> {
        let mut out: Vec<_> = self.inbox.values().flatten().cloned().collect();
        out.sort_by_key(|m| m.id);
        out
    }

    pub fn load_mails(&mut self, mails: Vec<MailItem>, next_id: u32) {
        self.inbox.clear();
        let mut max_id = 0u32;
        for mail in mails {
            max_id = max_id.max(mail.id);
            self.inbox
                .entry(mail.to_durable.clone())
                .or_default()
                .push(mail);
        }
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);
    }

    pub fn mailbox_key(player: &Entity) -> String {
        player
            .durable_id
            .clone()
            .unwrap_or_else(|| format!("local:{}", player.id))
    }

    pub fn snapshot_for_entity(
        &self,
        player_id: EntityId,
        entities: &[Entity],
    ) -> Vec<MailSnapshot> {
        let Some(player) = entities.iter().find(|e| e.id == player_id) else {
            return Vec::new();
        };
        let key = Self::mailbox_key(player);
        self.inbox
            .get(&key)
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

    #[allow(clippy::too_many_arguments)]
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
        let to = entities
            .iter()
            .find(|e| e.kind == EntityKind::Player && e.name.eq_ignore_ascii_case(to_name));
        let Some(to) = to else {
            events.push(SimEvent::Toast {
                message: "Recipient not found (must be online).".into(),
            });
            return false;
        };
        let to_id = to.id;
        let to_key = Self::mailbox_key(to);
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
        self.inbox
            .entry(to_key.clone())
            .or_default()
            .push(MailItem {
                id: mail_id,
                from: from_name,
                to_durable: to_key,
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

    /// Deliver system mail (auction proceeds / returns) to a durable key.
    pub fn deliver_system(
        &mut self,
        to_durable: &str,
        from: &str,
        subject: &str,
        copper: u32,
        item_id: Option<String>,
        item_count: u32,
    ) -> u32 {
        let mail_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.inbox
            .entry(to_durable.to_string())
            .or_default()
            .push(MailItem {
                id: mail_id,
                from: from.to_string(),
                to_durable: to_durable.to_string(),
                subject: subject.to_string(),
                copper,
                item_id,
                item_count,
            });
        mail_id
    }

    pub fn collect(
        &mut self,
        entities: &mut [Entity],
        player: EntityId,
        mail_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let key = {
            let Some(p) = entities.iter().find(|e| e.id == player) else {
                return false;
            };
            Self::mailbox_key(p)
        };
        let Some(mails) = self.inbox.get_mut(&key) else {
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
                self.inbox.entry(key).or_default().insert(idx, mail);
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
        entities[0].durable_id = Some("ada".into());
        entities[1].durable_id = Some("bob".into());
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
        // Reconnect Bob under new entity id — mail still addressable by durable key.
        entities[1].id = 99;
        assert!(box_.collect(&mut entities, 99, 1, &mut events));
        assert_eq!(entities[1].copper, 25);
        assert!(crate::entity::count_item(&entities[1].inventory, "silverleaf") >= 1);
    }

    #[test]
    fn load_mails_roundtrip() {
        let mut box_ = Mailbox::new();
        box_.deliver_system("ada", "AH", "Sold", 40, None, 0);
        let all = box_.all_mails();
        let next = box_.next_id();
        let mut box2 = Mailbox::new();
        box2.load_mails(all, next);
        assert_eq!(
            box2.snapshot_for_entity(
                1,
                &[{
                    let mut p = create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
                    p.durable_id = Some("ada".into());
                    p
                }]
            )
            .len(),
            1
        );
    }
}
