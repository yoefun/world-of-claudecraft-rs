//! Player mailbox (send / collect / return / expire), keyed by durable
//! character id when present, plus the realm character directory that makes
//! offline sends possible.

use std::collections::HashMap;

use crate::ecs::components::{Bags, ClassKit, Durable, Health, Identity, InvStack, Progress};
use crate::ecs::World;
use crate::inventory::{grant_stack, take_from_slot};
use woc_content::{ItemKind, ItemQuality};
use woc_protocol::{EntityId, MailSnapshot, SimEvent};

/// Flat postage in copper charged for player-to-player mail.
pub const MAIL_POSTAGE: u32 = 1;
/// Max player-to-player + system mails per durable mailbox key.
pub const MAIL_INBOX_CAP: usize = 20;
/// Time-to-live for a player parcel before it auto-returns (24h at 20 Hz).
pub const MAIL_TTL_TICKS: u64 = 1_728_000;

/// Realm-wide character name → durable mailbox key lookup. Lets `send` reach
/// characters that have no live `ClassKit` entity in this process. This is a
/// `Sim` field, not a `World` column: it is per-realm bookkeeping, not
/// per-actor gameplay state.
#[derive(Debug, Clone, Default)]
pub struct CharacterDirectory {
    by_name: HashMap<String, String>,
}

impl CharacterDirectory {
    /// Register (or overwrite) a character's mailbox key by name. Last writer
    /// wins; renames are out of scope.
    pub fn register(&mut self, name: &str, durable_key: impl Into<String>) {
        self.by_name
            .insert(name.to_ascii_lowercase(), durable_key.into());
    }

    /// Case-insensitive name lookup.
    pub fn lookup(&self, name: &str) -> Option<&str> {
        self.by_name
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

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
    pub durability: Option<u32>,
    pub enchant_id: Option<String>,
    pub quality: Option<ItemQuality>,
    pub bound: bool,
    /// Tick after which an uncollected player parcel auto-returns. `0` means
    /// system mail that never expires.
    pub expires_tick: u64,
    /// Sender's durable mailbox key, used to route `MailReturn` / expiry.
    /// `None` for system mail (returning it discards instead).
    pub return_to: Option<String>,
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

    pub fn mailbox_key(world: &World, player_id: EntityId) -> String {
        world
            .get::<Durable>(player_id)
            .and_then(|d| d.durable_id.clone())
            .unwrap_or_else(|| format!("local:{player_id}"))
    }

    fn mails_to_snapshot(mails: &[MailItem]) -> Vec<MailSnapshot> {
        mails
            .iter()
            .map(|m| MailSnapshot {
                id: m.id,
                from: m.from.clone(),
                subject: m.subject.clone(),
                copper: m.copper,
                item_id: m.item_id.clone(),
                item_count: m.item_count,
                durability: m.durability,
                enchant_id: m.enchant_id.clone(),
                quality: m.quality.map(|q| q.as_str().to_string()),
                bound: m.bound,
                expires_tick: m.expires_tick,
            })
            .collect()
    }

    pub fn snapshot_for_entity(&self, player_id: EntityId, world: &World) -> Vec<MailSnapshot> {
        if world.get::<ClassKit>(player_id).is_none() {
            return Vec::new();
        }
        let key = Self::mailbox_key(world, player_id);
        self.inbox
            .get(&key)
            .map(|mails| Self::mails_to_snapshot(mails))
            .unwrap_or_default()
    }

    /// Same as [`Mailbox::snapshot_for_entity`] but by durable key directly,
    /// for callers (tests, persist bridge) that have no live entity.
    pub fn snapshot_for_entity_key(&self, key: &str) -> Vec<MailSnapshot> {
        self.inbox
            .get(key)
            .map(|mails| Self::mails_to_snapshot(mails))
            .unwrap_or_default()
    }

    /// Send a parcel. Resolves the recipient via the realm `directory` first,
    /// then falls back to a live `ClassKit` + `Identity.name` scan so parked
    /// players remain mailable even if the directory missed them.
    #[allow(clippy::too_many_arguments)]
    pub fn send(
        &mut self,
        world: &mut World,
        from: EntityId,
        to_name: &str,
        copper: u32,
        bag_slot: Option<u8>,
        count: u32,
        now_tick: u64,
        directory: &CharacterDirectory,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        if world.get::<ClassKit>(from).is_none() {
            return false;
        }
        let alive = world.get::<Health>(from).map(|h| h.alive).unwrap_or(false);
        if !alive {
            return false;
        }

        let to_key = if let Some(key) = directory.lookup(to_name) {
            key.to_string()
        } else if let Some(to_id) = world.ids::<ClassKit>().into_iter().find(|&id| {
            world
                .get::<Identity>(id)
                .is_some_and(|i| i.name.eq_ignore_ascii_case(to_name))
        }) {
            Self::mailbox_key(world, to_id)
        } else {
            events.push(SimEvent::Toast {
                message: "Recipient not found.".into(),
            });
            return false;
        };

        let sender_key = Self::mailbox_key(world, from);
        if to_key == sender_key {
            events.push(SimEvent::Toast {
                message: "Cannot mail yourself.".into(),
            });
            return false;
        }

        if bag_slot.is_none() && copper == 0 {
            events.push(SimEvent::Toast {
                message: "Mail is empty.".into(),
            });
            return false;
        }

        if let Some(slot) = bag_slot {
            let item_id = world
                .get::<Bags>(from)
                .and_then(|b| b.inventory.get(slot as usize))
                .and_then(|s| s.as_ref().map(|st| st.item_id.clone()));
            let Some(item_id) = item_id else {
                events.push(SimEvent::Toast {
                    message: "Empty bag slot.".into(),
                });
                return false;
            };
            let is_quest = woc_content::item(&item_id)
                .map(|d| matches!(d.kind, ItemKind::Quest))
                .unwrap_or(false);
            if is_quest {
                events.push(SimEvent::Toast {
                    message: "This item is needed for a quest.".into(),
                });
                return false;
            }
            let bound = world
                .get::<Bags>(from)
                .and_then(|b| b.inventory.get(slot as usize))
                .and_then(|s| s.as_ref().map(|st| st.bound))
                .unwrap_or(false);
            if bound {
                events.push(SimEvent::Toast {
                    message: "That item is soulbound.".into(),
                });
                return false;
            }
        }

        let sender_copper = world.get::<Progress>(from).map(|p| p.copper).unwrap_or(0);
        if sender_copper < MAIL_POSTAGE.saturating_add(copper) {
            events.push(SimEvent::Toast {
                message: "Not enough copper.".into(),
            });
            return false;
        }

        if self.inbox.get(&to_key).map(|v| v.len()).unwrap_or(0) >= MAIL_INBOX_CAP {
            events.push(SimEvent::Toast {
                message: "Mailbox is full.".into(),
            });
            return false;
        }

        let attachment = if let Some(slot) = bag_slot {
            let taken = world
                .get_mut::<Bags>(from)
                .and_then(|b| take_from_slot(&mut b.inventory, slot, count));
            let Some(taken) = taken else {
                events.push(SimEvent::Toast {
                    message: "Empty bag slot.".into(),
                });
                return false;
            };
            Some(taken)
        } else {
            None
        };

        if let Some(progress) = world.get_mut::<Progress>(from) {
            progress.copper -= MAIL_POSTAGE.saturating_add(copper);
        }

        let from_name = world
            .get::<Identity>(from)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let mail_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let (item_id, item_count, durability, enchant_id, quality, bound) = match attachment {
            Some(stack) => (
                Some(stack.item_id),
                stack.count,
                stack.durability,
                stack.enchant_id,
                stack.quality,
                stack.bound,
            ),
            None => (None, 0, None, None, None, false),
        };
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
                durability,
                enchant_id,
                quality,
                bound,
                expires_tick: now_tick.saturating_add(MAIL_TTL_TICKS),
                return_to: Some(sender_key),
            });
        events.push(SimEvent::MailSent {
            from,
            to_name: to_name.to_string(),
            mail_id,
        });
        true
    }

    /// Deliver system mail (auction proceeds / returns) to a durable key.
    /// Bypasses postage and the inbox cap. `expires_tick` is always `0` and
    /// `return_to` is always `None` (system mail cannot re-return).
    pub fn deliver_system(
        &mut self,
        to_durable: &str,
        from: &str,
        subject: &str,
        copper: u32,
        attachment: Option<InvStack>,
    ) -> u32 {
        let mail_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let (item_id, item_count, durability, enchant_id, quality, bound) = match attachment {
            Some(stack) => (
                Some(stack.item_id),
                stack.count,
                stack.durability,
                stack.enchant_id,
                stack.quality,
                stack.bound,
            ),
            None => (None, 0, None, None, None, false),
        };
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
                durability,
                enchant_id,
                quality,
                bound,
                expires_tick: 0,
                return_to: None,
            });
        mail_id
    }

    /// Collect a mail's copper and item into the player's bags/wallet.
    pub fn collect(
        &mut self,
        world: &mut World,
        player: EntityId,
        mail_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let key = Self::mailbox_key(world, player);
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
        if let Some(ref item_id) = mail.item_id {
            let mut stack = InvStack::new(item_id, mail.item_count.max(1));
            if mail.durability.is_some() {
                stack.durability = mail.durability;
            }
            stack.enchant_id = mail.enchant_id.clone();
            stack.quality = mail.quality;
            stack.bound = mail.bound;
            let placed = if let Some(bags) = world.get_mut::<Bags>(player) {
                grant_stack(&mut bags.inventory, stack)
            } else {
                false
            };
            if !placed {
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
        if let Some(progress) = world.get_mut::<Progress>(player) {
            progress.copper = progress.copper.saturating_add(mail.copper);
        }
        events.push(SimEvent::MailCollected { player, mail_id });
        true
    }

    /// Return an uncollected mail to its sender (or discard system mail).
    pub fn return_mail(
        &mut self,
        world: &mut World,
        player: EntityId,
        mail_id: u32,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let key = Self::mailbox_key(world, player);
        let Some(mails) = self.inbox.get_mut(&key) else {
            events.push(SimEvent::Toast {
                message: "Mail not found.".into(),
            });
            return false;
        };
        let Some(idx) = mails.iter().position(|m| m.id == mail_id) else {
            events.push(SimEvent::Toast {
                message: "Mail not found.".into(),
            });
            return false;
        };
        let mail = mails.remove(idx);
        self.route_return_or_discard(mail, events, false);
        true
    }

    /// Drain expired player parcels (`expires_tick > 0 && now_tick >=
    /// expires_tick`) and return them to their sender as system mail. System
    /// mail (`expires_tick == 0`) never expires. Does not emit realm-wide
    /// toasts (unlike interactive [`Mailbox::return_mail`]).
    pub fn tick_expire(&mut self, now_tick: u64, events: &mut Vec<SimEvent>) {
        let mut keys: Vec<String> = self.inbox.keys().cloned().collect();
        keys.sort();
        let mut expired = Vec::new();
        for key in keys {
            let Some(mails) = self.inbox.get_mut(&key) else {
                continue;
            };
            let mut i = 0;
            while i < mails.len() {
                if mails[i].expires_tick > 0 && now_tick >= mails[i].expires_tick {
                    expired.push(mails.remove(i));
                } else {
                    i += 1;
                }
            }
        }
        expired.sort_by_key(|m| m.id);
        for mail in expired {
            self.route_return_or_discard(mail, events, true);
        }
    }

    fn route_return_or_discard(
        &mut self,
        mail: MailItem,
        events: &mut Vec<SimEvent>,
        silent: bool,
    ) {
        if let Some(return_to) = mail.return_to.clone() {
            let attachment = mail.item_id.clone().map(|item_id| InvStack {
                item_id,
                count: mail.item_count.max(1),
                durability: mail.durability,
                enchant_id: mail.enchant_id.clone(),
                quality: mail.quality,
                bound: mail.bound,
            });
            self.deliver_system(
                &return_to,
                "Mail",
                &format!("Returned: {}", mail.subject),
                mail.copper,
                attachment,
            );
            if !silent {
                events.push(SimEvent::Toast {
                    message: "Mail returned.".into(),
                });
            }
        } else if !silent {
            events.push(SimEvent::Toast {
                message: "Mail discarded.".into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Durable, InvStack, Progress};
    use crate::inventory::{count_item, grant_into};
    use woc_content::{ItemQuality, PlayerClass};

    #[test]
    fn send_and_collect_copper_and_item() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        if let Some(d) = world.get_mut::<Durable>(2) {
            d.durable_id = Some("bob".into());
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "silverleaf", 2));
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
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(box_.send(
            &mut world,
            1,
            "Bob",
            25,
            Some(slot),
            1,
            0,
            &dir,
            &mut events
        ));
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 74); // 100 - 25 - postage
                                                                  // Rebind Bob to a new id under same durable key.
        world.despawn(2);
        crate::ecs::spawn::create_player(&mut world, 99, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(99) {
            d.durable_id = Some("bob".into());
        }
        assert!(box_.collect(&mut world, 99, 1, &mut events));
        assert_eq!(world.get::<Progress>(99).unwrap().copper, 25);
        assert!(count_item(&world.get::<Bags>(99).unwrap().inventory, "silverleaf") >= 1);
    }

    #[test]
    fn load_mails_roundtrip() {
        let mut box_ = Mailbox::new();
        box_.deliver_system("ada", "AH", "Sold", 40, None);
        let all = box_.all_mails();
        let next = box_.next_id();
        let mut box2 = Mailbox::new();
        box2.load_mails(all, next);
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        assert_eq!(box2.snapshot_for_entity(1, &world).len(), 1);
    }

    #[test]
    fn send_offline_via_directory_preserves_instance() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 10;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "worn_sword", 1));
        }
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|x| x.item_id == "worn_sword"))
            .unwrap();
        if let Some(bags) = world.get_mut::<Bags>(1) {
            if let Some(st) = bags.inventory[slot].as_mut() {
                st.durability = Some(12);
                st.enchant_id = Some("coarse_sharpening".into());
            }
        }
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(box_.send(
            &mut world,
            1,
            "Bob",
            0,
            Some(slot as u8),
            1,
            0,
            &dir,
            &mut events,
        ));
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 9); // postage
        crate::ecs::spawn::create_player(&mut world, 99, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(99) {
            d.durable_id = Some("bob".into());
        }
        assert!(box_.collect(&mut world, 99, 1, &mut events));
        let got = world
            .get::<Bags>(99)
            .unwrap()
            .inventory
            .iter()
            .flatten()
            .find(|s| s.item_id == "worn_sword")
            .unwrap();
        assert_eq!(got.durability, Some(12));
        assert_eq!(got.enchant_id.as_deref(), Some("coarse_sharpening"));
    }

    #[test]
    fn player_mail_expires_and_returns() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        if let Some(d) = world.get_mut::<Durable>(2) {
            d.durable_id = Some("bob".into());
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 5;
        }
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(box_.send(&mut world, 1, "Bob", 2, None, 0, 0, &dir, &mut events));
        box_.tick_expire(MAIL_TTL_TICKS, &mut events);
        assert!(box_.snapshot_for_entity(2, &world).is_empty());
        let returned = &box_.snapshot_for_entity(1, &world)[0];
        assert_eq!(returned.subject, "Returned: Parcel");
        assert_eq!(returned.copper, 2);
    }

    #[test]
    fn inbox_cap_blocks_player_mail_not_system() {
        let mut box_ = Mailbox::new();
        for _ in 0..MAIL_INBOX_CAP {
            box_.deliver_system("bob", "Ada", "Parcel", 1, None);
        }
        // player cap counts all mails in the inbox including system
        // Spec: system bypasses cap on deliver_system, player send checks len >= cap
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 50;
        }
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut events = Vec::new();
        assert!(!box_.send(&mut world, 1, "Bob", 1, None, 0, 0, &dir, &mut events));
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::Toast { message } if message == "Mailbox is full.")));
        box_.deliver_system("bob", "Auction House", "Sold", 40, None);
        assert_eq!(
            box_.snapshot_for_entity_key("bob").len(),
            MAIL_INBOX_CAP + 1
        );
    }

    #[test]
    fn send_refuses_quest_item() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "boar_tusk", 1));
        }
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|x| x.item_id == "boar_tusk"))
            .unwrap() as u8;
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(!box_.send(&mut world, 1, "Bob", 0, Some(slot), 1, 0, &dir, &mut events));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "This item is needed for a quest."
        )));
    }

    #[test]
    fn send_refuses_recipient_not_found() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        let dir = CharacterDirectory::default();
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(!box_.send(&mut world, 1, "Nobody", 0, None, 0, 0, &dir, &mut events));
        assert!(events.iter().any(
            |e| matches!(e, SimEvent::Toast { message } if message == "Recipient not found.")
        ));
    }

    #[test]
    fn send_refuses_self() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 10;
        }
        let dir = CharacterDirectory::default();
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(!box_.send(&mut world, 1, "Ada", 1, None, 0, 0, &dir, &mut events));
        assert!(events.iter().any(
            |e| matches!(e, SimEvent::Toast { message } if message == "Cannot mail yourself.")
        ));
    }

    #[test]
    fn send_refuses_empty_mail() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(!box_.send(&mut world, 1, "Bob", 0, None, 0, 0, &dir, &mut events));
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::Toast { message } if message == "Mail is empty.")));
    }

    #[test]
    fn send_refuses_insufficient_copper() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 0;
        }
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(!box_.send(&mut world, 1, "Bob", 1, None, 0, 0, &dir, &mut events));
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::Toast { message } if message == "Not enough copper.")));
    }

    #[test]
    fn return_mail_routes_to_sender_and_discards_system_mail() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        if let Some(d) = world.get_mut::<Durable>(2) {
            d.durable_id = Some("bob".into());
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 5;
        }
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(box_.send(&mut world, 1, "Bob", 2, None, 0, 0, &dir, &mut events));
        assert!(box_.return_mail(&mut world, 2, 1, &mut events));
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::Toast { message } if message == "Mail returned.")));
        assert!(box_.snapshot_for_entity(2, &world).is_empty());
        assert_eq!(
            box_.snapshot_for_entity(1, &world)[0].subject,
            "Returned: Parcel"
        );

        // System mail has no `return_to`, so returning it discards instead.
        let sys_id = box_.deliver_system("ada", "Auction House", "Sold", 5, None);
        events.clear();
        assert!(box_.return_mail(&mut world, 1, sys_id, &mut events));
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::Toast { message } if message == "Mail discarded.")));
    }

    #[test]
    fn collect_restores_listed_wear() {
        let mut box_ = Mailbox::new();
        box_.deliver_system(
            "ada",
            "Auction House",
            "Listing expired",
            0,
            Some(InvStack {
                item_id: "worn_sword".into(),
                count: 1,
                durability: Some(7),
                enchant_id: Some("coarse_sharpening".into()),
                quality: Some(ItemQuality::Uncommon),
                bound: false,
            }),
        );
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("ada".into());
        }
        let mut events = Vec::new();
        assert!(box_.collect(&mut world, 1, 1, &mut events));
        let sword = world
            .get::<Bags>(1)
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

    #[test]
    fn send_refuses_soulbound_items() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 50;
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
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob");
        let mut box_ = Mailbox::new();
        let mut events = Vec::new();
        assert!(!box_.send(&mut world, 1, "Bob", 0, Some(0), 1, 0, &dir, &mut events));
        assert!(box_.all_mails().is_empty());
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "That item is soulbound."
        )));
    }

    #[test]
    fn interact_mail_requires_mailbox_session() {
        use crate::ecs::components::{Identity, Transform};
        use woc_protocol::{InteractAction, WorldHost};
        let mut sim = crate::sim::Sim::new_eastbrook("Ada", PlayerClass::Warrior);
        let pid = sim.player_id;
        WorldHost::interact(&mut sim, pid, 0, InteractAction::MailCollect { mail_id: 1 });
        assert!(sim.events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Talk to a mailbox first."
        )));

        let post = sim
            .world
            .ids::<Identity>()
            .into_iter()
            .find(|&id| {
                sim.world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some("mailbox_post")
            })
            .expect("mailbox_post");
        if let Some(nt) = sim.world.get::<Transform>(post).cloned() {
            if let Some(p) = sim.world.get_mut::<Transform>(pid) {
                p.x = nt.x;
                p.z = nt.z;
            }
        }
        WorldHost::interact(&mut sim, pid, post, InteractAction::Talk);
        sim.events.clear();
        WorldHost::interact(
            &mut sim,
            pid,
            post,
            InteractAction::MailCollect { mail_id: 1 },
        );
        assert!(!sim.events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Talk to a mailbox first."
        )));
    }
}
