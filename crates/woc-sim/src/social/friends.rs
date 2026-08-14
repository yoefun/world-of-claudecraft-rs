//! Persistent friend / ignore books (durable character id).

use std::collections::HashMap;

use crate::ecs::components::{ClassKit, Health, Identity};
use crate::ecs::World;
use crate::mail::{CharacterDirectory, Mailbox};
use woc_protocol::{EntityId, PlayerIntent};

pub const MAX_FRIENDS: usize = 50;
pub const MAX_IGNORE: usize = 50;
pub const WHISPER_MAX: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendEntry {
    pub durable_id: String,
    pub name: String,
    pub class_id: String,
    pub level: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SocialBook {
    pub owner_durable: String,
    pub friends: Vec<FriendEntry>,
    pub ignored: Vec<FriendEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialEffect {
    Notice {
        to: EntityId,
        message: String,
    },
    Error {
        to: EntityId,
        message: String,
    },
    Chat {
        to: EntityId,
        channel: String,
        from: String,
        text: String,
    },
}

/// Routed friend command / whisper delivery (host expands to one live player).
#[derive(Debug, Clone)]
pub enum SocialDelivery {
    To {
        player: EntityId,
        msg: woc_protocol::WsServerMsg,
    },
}

struct Resolved {
    durable: String,
    name: String,
    class_id: String,
    level: u32,
    entity_id: Option<EntityId>,
}

#[derive(Debug, Default, Clone)]
pub struct FriendRoster {
    books: HashMap<String, SocialBook>,
}

impl FriendRoster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn owner_key(world: &World, player_id: EntityId) -> String {
        Mailbox::mailbox_key(world, player_id)
    }

    pub fn book_of(&self, owner_durable: &str) -> Option<&SocialBook> {
        self.books.get(owner_durable)
    }

    pub fn all_books(&self) -> Vec<SocialBook> {
        let mut out: Vec<_> = self
            .books
            .values()
            .filter(|b| !b.friends.is_empty() || !b.ignored.is_empty())
            .cloned()
            .collect();
        out.sort_by(|a, b| a.owner_durable.cmp(&b.owner_durable));
        out
    }

    pub fn load_books(&mut self, books: Vec<SocialBook>) {
        self.books.clear();
        for book in books {
            if book.friends.is_empty() && book.ignored.is_empty() {
                continue;
            }
            self.books.insert(book.owner_durable.clone(), book);
        }
    }

    pub fn add(
        &mut self,
        owner_id: EntityId,
        name: &str,
        world: &World,
        directory: &CharacterDirectory,
    ) -> Vec<SocialEffect> {
        let owner = Self::owner_key(world, owner_id);
        let Some(target) = self.resolve(name, world, directory) else {
            return vec![error(
                owner_id,
                format!("No player named '{}'.", name.trim()),
            )];
        };
        if target.durable == owner {
            return vec![error(owner_id, "You cannot add yourself.")];
        }
        if self
            .book_of(&owner)
            .is_some_and(|b| b.ignored.iter().any(|e| e.durable_id == target.durable))
        {
            return vec![error(
                owner_id,
                format!("Unignore {} before adding them as a friend.", target.name),
            )];
        }
        if self
            .book_of(&owner)
            .is_some_and(|b| b.friends.iter().any(|e| e.durable_id == target.durable))
        {
            return vec![error(
                owner_id,
                format!("{} is already on your friends list.", target.name),
            )];
        }
        let book = self.ensure_book(&owner);
        if book.friends.len() >= MAX_FRIENDS {
            return vec![error(owner_id, "Your friends list is full.")];
        }
        let display = target.name.clone();
        book.friends.push(entry_from_resolved(&target));
        vec![notice(
            owner_id,
            format!("{display} has been added to your friends list."),
        )]
    }

    pub fn remove(
        &mut self,
        owner_id: EntityId,
        name: &str,
        world: &World,
        directory: &CharacterDirectory,
    ) -> Vec<SocialEffect> {
        let owner = Self::owner_key(world, owner_id);
        let Some(target) = self.resolve(name, world, directory) else {
            return vec![error(
                owner_id,
                format!("No player named '{}'.", name.trim()),
            )];
        };
        let Some(book) = self.books.get_mut(&owner) else {
            return vec![error(
                owner_id,
                format!("{} is not on your friends list.", target.name),
            )];
        };
        let before = book.friends.len();
        book.friends.retain(|e| e.durable_id != target.durable);
        if book.friends.len() == before {
            return vec![error(
                owner_id,
                format!("{} is not on your friends list.", target.name),
            )];
        }
        vec![notice(
            owner_id,
            format!("{} has been removed from your friends list.", target.name),
        )]
    }

    pub fn ignore(
        &mut self,
        owner_id: EntityId,
        name: &str,
        world: &World,
        directory: &CharacterDirectory,
    ) -> Vec<SocialEffect> {
        let owner = Self::owner_key(world, owner_id);
        let Some(target) = self.resolve(name, world, directory) else {
            return vec![error(
                owner_id,
                format!("No player named '{}'.", name.trim()),
            )];
        };
        if target.durable == owner {
            return vec![error(owner_id, "You cannot ignore yourself.")];
        }
        if self
            .book_of(&owner)
            .is_some_and(|b| b.ignored.iter().any(|e| e.durable_id == target.durable))
        {
            return vec![error(
                owner_id,
                format!("{} is already on your ignore list.", target.name),
            )];
        }
        let book = self.ensure_book(&owner);
        if book.ignored.len() >= MAX_IGNORE {
            return vec![error(owner_id, "Your ignore list is full.")];
        }
        book.friends.retain(|e| e.durable_id != target.durable);
        let display = target.name.clone();
        book.ignored.push(entry_from_resolved(&target));
        vec![notice(owner_id, format!("{display} is now being ignored."))]
    }

    pub fn unignore(
        &mut self,
        owner_id: EntityId,
        name: &str,
        world: &World,
        directory: &CharacterDirectory,
    ) -> Vec<SocialEffect> {
        let owner = Self::owner_key(world, owner_id);
        let Some(target) = self.resolve(name, world, directory) else {
            return vec![error(
                owner_id,
                format!("No player named '{}'.", name.trim()),
            )];
        };
        let Some(book) = self.books.get_mut(&owner) else {
            return vec![error(
                owner_id,
                format!("{} is not on your ignore list.", target.name),
            )];
        };
        let before = book.ignored.len();
        book.ignored.retain(|e| e.durable_id != target.durable);
        if book.ignored.len() == before {
            return vec![error(
                owner_id,
                format!("{} is not on your ignore list.", target.name),
            )];
        }
        vec![notice(
            owner_id,
            format!("{} is no longer being ignored.", target.name),
        )]
    }

    pub fn whisper(
        &self,
        from: EntityId,
        name: &str,
        text: &str,
        world: &World,
        directory: &CharacterDirectory,
        intents: &HashMap<EntityId, PlayerIntent>,
    ) -> Vec<SocialEffect> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return vec![error(from, "Chat message is empty.")];
        }
        if trimmed.len() > WHISPER_MAX {
            return vec![error(from, "Chat message is too long.")];
        }
        let Some(target) = self.resolve(name, world, directory) else {
            return vec![error(from, format!("No player named '{}'.", name.trim()))];
        };
        let owner = Self::owner_key(world, from);
        if target.durable == owner {
            return vec![error(from, "You cannot whisper yourself.")];
        }
        if self
            .book_of(&owner)
            .is_some_and(|b| b.ignored.iter().any(|e| e.durable_id == target.durable))
        {
            return vec![error(from, format!("You are ignoring {}.", target.name))];
        }
        if self
            .book_of(&target.durable)
            .is_some_and(|b| b.ignored.iter().any(|e| e.durable_id == owner))
        {
            return vec![error(from, format!("{} is ignoring you.", target.name))];
        }
        let Some(target_id) = target.entity_id.filter(|id| intents.contains_key(id)) else {
            return vec![error(from, format!("{} is not online.", target.name))];
        };
        let speaker = world
            .get::<Identity>(from)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "Unknown".into());
        vec![
            SocialEffect::Chat {
                to: target_id,
                channel: "whisper".into(),
                from: speaker,
                text: trimmed.to_string(),
            },
            SocialEffect::Chat {
                to: from,
                channel: "whisper".into(),
                from: format!("To {}", target.name),
                text: trimmed.to_string(),
            },
        ]
    }

    pub fn remove_character(&mut self, durable: &str) {
        self.books.remove(durable);
        for book in self.books.values_mut() {
            book.friends.retain(|e| e.durable_id != durable);
            book.ignored.retain(|e| e.durable_id != durable);
        }
    }

    pub fn refresh_entry(&mut self, world: &World, durable: &str) {
        let Some(id) = find_player_by_durable(world, durable) else {
            return;
        };
        let Some(live) = live_projection(world, id) else {
            return;
        };
        for book in self.books.values_mut() {
            for list in [&mut book.friends, &mut book.ignored] {
                if let Some(entry) = list.iter_mut().find(|e| e.durable_id == durable) {
                    *entry = live.clone();
                }
            }
        }
    }

    pub fn presence(
        &mut self,
        player_id: EntityId,
        online: bool,
        world: &World,
        intents: &HashMap<EntityId, PlayerIntent>,
    ) -> Vec<SocialEffect> {
        let durable = Self::owner_key(world, player_id);
        if online {
            self.refresh_entry(world, &durable);
        }
        let display = world
            .get::<Identity>(player_id)
            .map(|i| i.name.clone())
            .or_else(|| {
                self.books.values().find_map(|b| {
                    b.friends
                        .iter()
                        .chain(b.ignored.iter())
                        .find(|e| e.durable_id == durable)
                        .map(|e| e.name.clone())
                })
            })
            .unwrap_or_else(|| "Unknown".into());
        let message = if online {
            format!("{display} has come online.")
        } else {
            format!("{display} has gone offline.")
        };
        let mut effects = Vec::new();
        for id in intents.keys().copied() {
            if id == player_id {
                continue;
            }
            let owner = Self::owner_key(world, id);
            let listed = self
                .book_of(&owner)
                .is_some_and(|b| b.friends.iter().any(|e| e.durable_id == durable));
            if listed {
                effects.push(notice(id, message.clone()));
            }
        }
        effects
    }

    pub fn snapshot_for(
        &self,
        player_id: EntityId,
        world: &World,
        intents: &HashMap<EntityId, PlayerIntent>,
    ) -> (
        Vec<woc_protocol::FriendSnapshot>,
        Vec<woc_protocol::IgnoredSnapshot>,
    ) {
        let owner = Self::owner_key(world, player_id);
        let Some(book) = self.book_of(&owner) else {
            return (Vec::new(), Vec::new());
        };
        let mut friends: Vec<_> = book
            .friends
            .iter()
            .map(|e| {
                let live_id = find_player_by_durable(world, &e.durable_id);
                let online = live_id.is_some_and(|id| intents.contains_key(&id));
                let live = if online {
                    live_id.and_then(|id| live_projection(world, id))
                } else {
                    None
                };
                let zone_id = if online {
                    live_id
                        .and_then(|id| world.get::<Identity>(id).map(|i| i.zone_id.clone()))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                woc_protocol::FriendSnapshot {
                    name: live
                        .as_ref()
                        .map(|l| l.name.clone())
                        .unwrap_or_else(|| e.name.clone()),
                    class_id: live
                        .as_ref()
                        .map(|l| l.class_id.clone())
                        .unwrap_or_else(|| e.class_id.clone()),
                    level: live.as_ref().map(|l| l.level).unwrap_or(e.level),
                    online,
                    zone_id,
                }
            })
            .collect();
        friends.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| a.name.cmp(&b.name)));
        let mut ignored: Vec<_> = book
            .ignored
            .iter()
            .map(|e| woc_protocol::IgnoredSnapshot {
                name: e.name.clone(),
            })
            .collect();
        ignored.sort_by(|a, b| a.name.cmp(&b.name));
        (friends, ignored)
    }

    fn ensure_book(&mut self, owner: &str) -> &mut SocialBook {
        self.books
            .entry(owner.to_string())
            .or_insert_with(|| SocialBook {
                owner_durable: owner.to_string(),
                friends: Vec::new(),
                ignored: Vec::new(),
            })
    }

    fn cached_projection(&self, durable: &str) -> Option<FriendEntry> {
        self.books.values().find_map(|b| {
            b.friends
                .iter()
                .chain(b.ignored.iter())
                .find(|e| e.durable_id == durable)
                .cloned()
        })
    }

    fn resolve(
        &self,
        raw: &str,
        world: &World,
        directory: &CharacterDirectory,
    ) -> Option<Resolved> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(durable) = directory.lookup(trimmed) {
            let durable = durable.to_string();
            if let Some(id) = find_player_by_durable(world, &durable) {
                if let Some(live) = live_projection(world, id) {
                    return Some(Resolved {
                        durable: live.durable_id,
                        name: live.name,
                        class_id: live.class_id,
                        level: live.level,
                        entity_id: Some(id),
                    });
                }
            }
            let cached = self.cached_projection(&durable);
            return Some(Resolved {
                name: cached
                    .as_ref()
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| trimmed.to_string()),
                class_id: cached
                    .as_ref()
                    .map(|c| c.class_id.clone())
                    .unwrap_or_default(),
                level: cached.as_ref().map(|c| c.level).unwrap_or(1),
                durable,
                entity_id: None,
            });
        }
        let id = find_player_by_name(world, trimmed)?;
        let live = live_projection(world, id)?;
        Some(Resolved {
            durable: live.durable_id,
            name: live.name,
            class_id: live.class_id,
            level: live.level,
            entity_id: Some(id),
        })
    }
}

fn notice(to: EntityId, message: impl Into<String>) -> SocialEffect {
    SocialEffect::Notice {
        to,
        message: message.into(),
    }
}

fn error(to: EntityId, message: impl Into<String>) -> SocialEffect {
    SocialEffect::Error {
        to,
        message: message.into(),
    }
}

fn entry_from_resolved(target: &Resolved) -> FriendEntry {
    FriendEntry {
        durable_id: target.durable.clone(),
        name: target.name.clone(),
        class_id: target.class_id.clone(),
        level: target.level,
    }
}

fn live_projection(world: &World, id: EntityId) -> Option<FriendEntry> {
    let kit = world.get::<ClassKit>(id)?;
    Some(FriendEntry {
        durable_id: FriendRoster::owner_key(world, id),
        name: world
            .get::<Identity>(id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "Unknown".into()),
        class_id: kit
            .class_id
            .map(|c| c.as_str().to_string())
            .unwrap_or_default(),
        level: world.get::<Health>(id).map(|h| h.level).unwrap_or(1),
    })
}

fn find_player_by_name(world: &World, name: &str) -> Option<EntityId> {
    world
        .ids::<ClassKit>()
        .into_iter()
        .find(|&id| world.get::<Identity>(id).is_some_and(|i| i.name == name))
}

fn find_player_by_durable(world: &World, durable: &str) -> Option<EntityId> {
    world
        .ids::<ClassKit>()
        .into_iter()
        .find(|&id| FriendRoster::owner_key(world, id) == durable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_content::PlayerClass;

    fn world_with_players(n: usize) -> World {
        let mut world = World::new();
        let names = ["Alice", "Bob", "Carol", "Dave", "Eve"];
        let classes = [
            PlayerClass::Warrior,
            PlayerClass::Mage,
            PlayerClass::Rogue,
            PlayerClass::Priest,
            PlayerClass::Hunter,
        ];
        for i in 0..n {
            crate::ecs::spawn::create_player(
                &mut world,
                (i + 1) as EntityId,
                names[i],
                classes[i],
                i as f32,
                0.0,
            );
        }
        world
    }

    fn dir_from_world(world: &World) -> CharacterDirectory {
        let mut dir = CharacterDirectory::default();
        for id in world.ids::<ClassKit>() {
            if let Some(name) = world.get::<Identity>(id).map(|i| i.name.clone()) {
                dir.register(&name, FriendRoster::owner_key(world, id));
            }
        }
        dir
    }

    fn intents_for(ids: &[EntityId]) -> HashMap<EntityId, PlayerIntent> {
        ids.iter()
            .copied()
            .map(|id| (id, PlayerIntent::default()))
            .collect()
    }

    #[test]
    fn add_online_name_seats_friend() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let effects = roster.add(1, "Bob", &world, &dir);
        assert!(effects.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message }
                if message == "Bob has been added to your friends list."
        )));
        let book = roster.book_of(&FriendRoster::owner_key(&world, 1)).unwrap();
        assert_eq!(book.friends.len(), 1);
        assert_eq!(book.friends[0].name, "Bob");
        assert_eq!(book.friends[0].class_id, "mage");
        assert_eq!(book.friends[0].level, 1);
        assert_eq!(
            book.friends[0].durable_id,
            FriendRoster::owner_key(&world, 2)
        );
    }

    #[test]
    fn add_via_directory_when_target_not_in_world() {
        let world = world_with_players(1);
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob-durable");
        let mut roster = FriendRoster::new();
        let effects = roster.add(1, "Bob", &world, &dir);
        assert!(effects.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message }
                if message == "Bob has been added to your friends list."
        )));
        let book = roster.book_of(&FriendRoster::owner_key(&world, 1)).unwrap();
        assert_eq!(book.friends[0].durable_id, "bob-durable");
        assert_eq!(book.friends[0].name, "Bob");
    }

    #[test]
    fn add_rejects_self_unknown_and_duplicate() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let self_err = roster.add(1, "Alice", &world, &dir);
        assert!(self_err.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message } if message == "You cannot add yourself."
        )));
        let missing = roster.add(1, "Zed", &world, &dir);
        assert!(missing.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message } if message == "No player named 'Zed'."
        )));
        let _ = roster.add(1, "Bob", &world, &dir);
        let dup = roster.add(1, "Bob", &world, &dir);
        assert!(dup.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message }
                if message == "Bob is already on your friends list."
        )));
    }

    #[test]
    fn add_is_unidirectional() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        assert!(roster
            .book_of(&FriendRoster::owner_key(&world, 2))
            .map(|b| b.friends.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn remove_drops_friend_not_ignore() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        let ok = roster.remove(1, "Bob", &world, &dir);
        assert!(ok.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message }
                if message == "Bob has been removed from your friends list."
        )));
        assert!(roster
            .book_of(&FriendRoster::owner_key(&world, 1))
            .unwrap()
            .friends
            .is_empty());
        let missing = roster.remove(1, "Bob", &world, &dir);
        assert!(missing.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message }
                if message == "Bob is not on your friends list."
        )));
    }

    #[test]
    fn ignore_kicks_friend_and_blocks_readd() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        let ign = roster.ignore(1, "Bob", &world, &dir);
        assert!(ign.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message } if message == "Bob is now being ignored."
        )));
        let book = roster.book_of(&FriendRoster::owner_key(&world, 1)).unwrap();
        assert!(book.friends.is_empty());
        assert_eq!(book.ignored.len(), 1);
        let blocked = roster.add(1, "Bob", &world, &dir);
        assert!(blocked.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message }
                if message == "Unignore Bob before adding them as a friend."
        )));
    }

    #[test]
    fn ignore_rejects_self_duplicate_and_unignore_roundtrip() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let self_err = roster.ignore(1, "Alice", &world, &dir);
        assert!(self_err.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message } if message == "You cannot ignore yourself."
        )));
        let _ = roster.ignore(1, "Bob", &world, &dir);
        let dup = roster.ignore(1, "Bob", &world, &dir);
        assert!(dup.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message }
                if message == "Bob is already on your ignore list."
        )));
        let ok = roster.unignore(1, "Bob", &world, &dir);
        assert!(ok.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message }
                if message == "Bob is no longer being ignored."
        )));
        let missing = roster.unignore(1, "Bob", &world, &dir);
        assert!(missing.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message }
                if message == "Bob is not on your ignore list."
        )));
    }

    #[test]
    fn friends_and_ignore_lists_have_caps() {
        let world = world_with_players(1);
        let mut dir = CharacterDirectory::default();
        let mut roster = FriendRoster::new();
        for i in 0..MAX_FRIENDS {
            let name = format!("F{i:02}");
            dir.register(&name, format!("f-{i}"));
            let out = roster.add(1, &name, &world, &dir);
            assert!(
                out.iter().any(|e| matches!(e, SocialEffect::Notice { .. })),
                "{name}"
            );
        }
        dir.register("Overflow", "overflow");
        let full = roster.add(1, "Overflow", &world, &dir);
        assert!(full.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message } if message == "Your friends list is full."
        )));

        let mut roster2 = FriendRoster::new();
        for i in 0..MAX_IGNORE {
            let name = format!("I{i:02}");
            dir.register(&name, format!("i-{i}"));
            let out = roster2.ignore(1, &name, &world, &dir);
            assert!(
                out.iter().any(|e| matches!(e, SocialEffect::Notice { .. })),
                "{name}"
            );
        }
        dir.register("TooMany", "too-many");
        let ign_full = roster2.ignore(1, "TooMany", &world, &dir);
        assert!(ign_full.iter().any(|e| matches!(
            e,
            SocialEffect::Error { to: 1, message } if message == "Your ignore list is full."
        )));
    }

    #[test]
    fn remove_character_sweeps_all_books() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        let _ = roster.ignore(1, "Bob", &world, &dir);
        let bob = FriendRoster::owner_key(&world, 2);
        let _ = roster.add(2, "Alice", &world, &dir);
        roster.remove_character(&bob);
        assert!(roster.book_of(&bob).is_none());
        let alice = roster.book_of(&FriendRoster::owner_key(&world, 1)).unwrap();
        assert!(alice.friends.iter().all(|e| e.durable_id != bob));
        assert!(alice.ignored.iter().all(|e| e.durable_id != bob));
    }

    #[test]
    fn whisper_delivers_both_sides() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let intents = intents_for(&[1, 2]);
        let roster = FriendRoster::new();
        let effects = roster.whisper(1, "Bob", " pull west ", &world, &dir, &intents);
        assert!(effects.iter().any(|e| matches!(
            e,
            SocialEffect::Chat { to: 2, channel, from, text }
                if channel == "whisper" && from == "Alice" && text == "pull west"
        )));
        assert!(effects.iter().any(|e| matches!(
            e,
            SocialEffect::Chat { to: 1, channel, from, text }
                if channel == "whisper" && from == "To Bob" && text == "pull west"
        )));
    }

    #[test]
    fn whisper_rejects_empty_long_self_offline_and_ignore() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let intents = intents_for(&[1, 2]);
        let mut roster = FriendRoster::new();
        assert!(roster
            .whisper(1, "Bob", "  ", &world, &dir, &intents)
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "Chat message is empty.")
            }));
        let long = "x".repeat(WHISPER_MAX + 1);
        assert!(roster
            .whisper(1, "Bob", &long, &world, &dir, &intents)
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "Chat message is too long.")
            }));
        assert!(roster
            .whisper(1, "Alice", "hi", &world, &dir, &intents)
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "You cannot whisper yourself.")
            }));
        let parked = intents_for(&[1]);
        assert!(roster
            .whisper(1, "Bob", "hi", &world, &dir, &parked)
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "Bob is not online.")
            }));
        let _ = roster.ignore(1, "Bob", &world, &dir);
        assert!(roster
            .whisper(1, "Bob", "hi", &world, &dir, &intents)
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "You are ignoring Bob.")
            }));
        let mut roster2 = FriendRoster::new();
        let _ = roster2.ignore(2, "Alice", &world, &dir);
        assert!(roster2
            .whisper(1, "Bob", "hi", &world, &dir, &intents)
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "Bob is ignoring you.")
            }));
    }

    #[test]
    fn presence_notifies_online_friends_only() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        let intents_both = intents_for(&[1, 2]);
        let came = roster.presence(2, true, &world, &intents_both);
        assert!(came.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message } if message == "Bob has come online."
        )));
        let alice_parked = intents_for(&[2]);
        let went = roster.presence(2, false, &world, &alice_parked);
        assert!(went.is_empty());
    }

    #[test]
    fn park_does_not_mutate_lists() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        let before = roster
            .book_of(&FriendRoster::owner_key(&world, 1))
            .unwrap()
            .clone();
        let _ = roster.presence(2, false, &world, &intents_for(&[1]));
        let after = roster.book_of(&FriendRoster::owner_key(&world, 1)).unwrap();
        assert_eq!(before.friends, after.friends);
    }

    #[test]
    fn snapshot_sorts_online_first_then_name() {
        let world = world_with_players(3);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Carol", &world, &dir);
        let _ = roster.add(1, "Bob", &world, &dir);
        let intents = intents_for(&[1, 2]);
        let (friends, ignored) = roster.snapshot_for(1, &world, &intents);
        assert!(ignored.is_empty());
        assert_eq!(friends[0].name, "Bob");
        assert!(friends[0].online);
        assert!(!friends[0].zone_id.is_empty());
        assert_eq!(friends[1].name, "Carol");
        assert!(!friends[1].online);
        assert_eq!(friends[1].zone_id, "");
    }

    #[test]
    fn add_succeeds_when_target_ignores_you() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.ignore(2, "Alice", &world, &dir);
        let effects = roster.add(1, "Bob", &world, &dir);
        assert!(effects.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message }
                if message == "Bob has been added to your friends list."
        )));
        assert!(roster
            .whisper(1, "Bob", "hi", &world, &dir, &intents_for(&[1, 2]))
            .iter()
            .any(|e| {
                matches!(e, SocialEffect::Error { message, .. } if message == "Bob is ignoring you.")
            }));
    }

    #[test]
    fn add_directory_lookup_is_case_insensitive() {
        let world = world_with_players(1);
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob-durable");
        let mut roster = FriendRoster::new();
        let effects = roster.add(1, "BOB", &world, &dir);
        assert!(effects.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message }
                if message == "BOB has been added to your friends list."
        )));
        let book = roster.book_of(&FriendRoster::owner_key(&world, 1)).unwrap();
        assert_eq!(book.friends[0].durable_id, "bob-durable");
    }

    #[test]
    fn all_books_omits_empty_after_remove() {
        let world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        let _ = roster.remove(1, "Bob", &world, &dir);
        assert!(roster.all_books().is_empty());
    }

    #[test]
    fn presence_refreshes_cached_projection() {
        let mut world = world_with_players(1);
        let mut dir = CharacterDirectory::default();
        dir.register("Bob", "bob-durable");
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "bob", &world, &dir);
        let before = roster
            .book_of(&FriendRoster::owner_key(&world, 1))
            .unwrap()
            .friends[0]
            .clone();
        assert_eq!(before.name, "bob");
        assert_eq!(before.class_id, "");
        assert_eq!(before.level, 1);

        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        world
            .get_mut::<crate::ecs::components::Durable>(2)
            .unwrap()
            .durable_id = Some("bob-durable".into());
        world.get_mut::<Health>(2).unwrap().level = 9;

        let came = roster.presence(2, true, &world, &intents_for(&[1, 2]));
        assert!(came.iter().any(|e| matches!(
            e,
            SocialEffect::Notice { to: 1, message } if message == "Bob has come online."
        )));
        let after = &roster
            .book_of(&FriendRoster::owner_key(&world, 1))
            .unwrap()
            .friends[0];
        assert_eq!(after.name, "Bob");
        assert_eq!(after.class_id, "mage");
        assert_eq!(after.level, 9);
    }

    #[test]
    fn snapshot_uses_live_level_when_friend_is_online() {
        let mut world = world_with_players(2);
        let dir = dir_from_world(&world);
        let mut roster = FriendRoster::new();
        let _ = roster.add(1, "Bob", &world, &dir);
        world.get_mut::<Health>(2).unwrap().level = 12;
        let (friends, _) = roster.snapshot_for(1, &world, &intents_for(&[1, 2]));
        assert_eq!(friends[0].name, "Bob");
        assert_eq!(friends[0].level, 12);
        assert!(friends[0].online);
        let (offline, _) = roster.snapshot_for(1, &world, &intents_for(&[1]));
        assert_eq!(offline[0].level, 1);
        assert!(!offline[0].online);
    }
}
