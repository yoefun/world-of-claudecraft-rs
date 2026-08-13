//! Persistent guild roster (durable character id), unlike ephemeral parties.

use std::collections::HashMap;

use crate::ecs::components::{ClassKit, Health, Identity};
use crate::ecs::World;
use crate::mail::Mailbox;
use woc_protocol::EntityId;

pub const MAX_GUILD_MEMBERS: usize = 100;
pub const GUILD_INVITE_TTL_TICKS: u64 = 1_200;
pub const GUILD_MOTD_MAX: usize = 240;
pub const GUILD_MESSAGE_MAX: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildRank {
    Leader,
    Officer,
    Member,
}

impl GuildRank {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Leader => "leader",
            Self::Officer => "officer",
            Self::Member => "member",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Leader => "Guild Master",
            Self::Officer => "Officer",
            Self::Member => "Member",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "leader" => Some(Self::Leader),
            "officer" => Some(Self::Officer),
            "member" => Some(Self::Member),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildMember {
    pub durable_id: String,
    pub name: String,
    pub class_id: String,
    pub level: u32,
    pub rank: GuildRank,
}

#[derive(Debug, Clone)]
pub struct Guild {
    pub id: u32,
    pub name: String,
    pub motd: String,
    pub motd_set_by: String,
    pub members: Vec<GuildMember>,
}

#[derive(Debug, Clone)]
pub struct PendingInvite {
    pub guild_id: u32,
    pub guild_name: String,
    pub from_name: String,
    pub expires_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildEffect {
    Notice { to: EntityId, message: String },
    GuildNotice { guild_id: u32, message: String },
    Error { to: EntityId, message: String },
    Chat {
        guild_id: u32,
        channel: String,
        from: String,
        text: String,
        officer_only: bool,
    },
}

#[derive(Debug, Default, Clone)]
pub struct GuildRoster {
    next_id: u32,
    guilds: HashMap<u32, Guild>,
    membership: HashMap<String, u32>,
    pending: HashMap<String, PendingInvite>,
}

pub fn validate_guild_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() < 3 || trimmed.len() > 24 {
        return None;
    }
    let bytes = trimmed.as_bytes();
    if !bytes[0].is_ascii_alphabetic() || !bytes[trimmed.len() - 1].is_ascii_alphabetic() {
        return None;
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') {
        return None;
    }
    if trimmed.contains("  ") {
        return None;
    }
    Some(trimmed.to_string())
}

impl GuildRoster {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            ..Default::default()
        }
    }

    pub fn member_key(world: &World, player_id: EntityId) -> String {
        Mailbox::mailbox_key(world, player_id)
    }

    pub fn guild_id_of(&self, durable: &str) -> Option<u32> {
        self.membership.get(durable).copied()
    }

    pub fn guild(&self, id: u32) -> Option<&Guild> {
        self.guilds.get(&id)
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn set_next_id(&mut self, id: u32) {
        self.next_id = id.max(1);
    }

    pub fn all_guilds(&self) -> Vec<Guild> {
        let mut v: Vec<_> = self.guilds.values().cloned().collect();
        v.sort_by_key(|g| g.id);
        v
    }

    pub fn create(&mut self, actor: EntityId, raw_name: &str, world: &World) -> Vec<GuildEffect> {
        if world.get::<ClassKit>(actor).is_none() {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You are not in the realm.".into(),
            }];
        }
        let Some(name) = validate_guild_name(raw_name) else {
            return vec![GuildEffect::Error {
                to: actor,
                message: "Guild names are 3-24 letters (spaces allowed).".into(),
            }];
        };
        let key = Self::member_key(world, actor);
        if self.membership.contains_key(&key) {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You are already in a guild.".into(),
            }];
        }
        let taken = self
            .guilds
            .values()
            .any(|g| g.name.eq_ignore_ascii_case(&name));
        if taken {
            return vec![GuildEffect::Error {
                to: actor,
                message: format!("A guild named '{name}' already exists."),
            }];
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let member = live_member(world, actor, GuildRank::Leader);
        self.guilds.insert(
            id,
            Guild {
                id,
                name: name.clone(),
                motd: String::new(),
                motd_set_by: String::new(),
                members: vec![member],
            },
        );
        self.membership.insert(key, id);
        vec![GuildEffect::Notice {
            to: actor,
            message: format!("You found the guild <{name}>! You are its Guild Master."),
        }]
    }

    pub fn expire_pending(&mut self, tick: u64) {
        self.pending.retain(|_, p| p.expires_tick > tick);
    }

    pub fn pending_for(&self, durable: &str) -> Option<&PendingInvite> {
        self.pending.get(durable)
    }

    pub fn invite(
        &mut self,
        actor: EntityId,
        target_name: &str,
        tick: u64,
        world: &World,
    ) -> Vec<GuildEffect> {
        self.expire_pending(tick);
        if world.get::<ClassKit>(actor).is_none() {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You are not in the realm.".into(),
            }];
        }
        let actor_key = Self::member_key(world, actor);
        let Some(&guild_id) = self.membership.get(&actor_key) else {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You are not in a guild.".into(),
            }];
        };
        let Some(guild) = self.guilds.get(&guild_id) else {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You are not in a guild.".into(),
            }];
        };
        let actor_rank = guild
            .members
            .iter()
            .find(|m| m.durable_id == actor_key)
            .map(|m| m.rank);
        if actor_rank == Some(GuildRank::Member) {
            return vec![GuildEffect::Error {
                to: actor,
                message: "Only officers and the Guild Master may invite.".into(),
            }];
        }
        let Some(target_id) = find_player_by_name(world, target_name) else {
            return vec![GuildEffect::Error {
                to: actor,
                message: format!("No player named '{target_name}'."),
            }];
        };
        if target_id == actor {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You cannot invite yourself.".into(),
            }];
        }
        let target_key = Self::member_key(world, target_id);
        let target_display = world
            .get::<Identity>(target_id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| target_name.to_string());
        if self.membership.contains_key(&target_key) {
            return vec![GuildEffect::Error {
                to: actor,
                message: format!("{target_display} is already in a guild."),
            }];
        }
        if self.pending.contains_key(&target_key) {
            return vec![GuildEffect::Error {
                to: actor,
                message: format!("{target_display} already has a pending guild invitation."),
            }];
        }
        let guild = self.guilds.get(&guild_id).unwrap();
        if guild.members.len() >= MAX_GUILD_MEMBERS {
            return vec![GuildEffect::Error {
                to: actor,
                message: "Your guild is full.".into(),
            }];
        }
        let guild_name = guild.name.clone();
        let from_name = world
            .get::<Identity>(actor)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "Someone".into());
        self.pending.insert(
            target_key,
            PendingInvite {
                guild_id,
                guild_name,
                from_name,
                expires_tick: tick + GUILD_INVITE_TTL_TICKS,
            },
        );
        vec![GuildEffect::Notice {
            to: actor,
            message: format!("You have invited {target_display} to the guild."),
        }]
    }

    pub fn accept(&mut self, actor: EntityId, tick: u64, world: &World) -> Vec<GuildEffect> {
        self.expire_pending(tick);
        let key = Self::member_key(world, actor);
        let Some(inv) = self.pending.remove(&key) else {
            return vec![GuildEffect::Error {
                to: actor,
                message: "The guild invitation has expired.".into(),
            }];
        };
        if inv.expires_tick <= tick {
            return vec![GuildEffect::Error {
                to: actor,
                message: "The guild invitation has expired.".into(),
            }];
        }
        if self.membership.contains_key(&key) {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You are already in a guild.".into(),
            }];
        }
        let Some(guild) = self.guilds.get_mut(&inv.guild_id) else {
            return vec![GuildEffect::Error {
                to: actor,
                message: "That guild no longer exists.".into(),
            }];
        };
        if guild.members.len() >= MAX_GUILD_MEMBERS {
            return vec![GuildEffect::Error {
                to: actor,
                message: "That guild is full.".into(),
            }];
        }
        guild.members.push(live_member(world, actor, GuildRank::Member));
        let guild_id = guild.id;
        let joiner = world
            .get::<Identity>(actor)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "Someone".into());
        self.membership.insert(key, guild_id);
        vec![GuildEffect::GuildNotice {
            guild_id,
            message: format!("{joiner} has joined the guild."),
        }]
    }

    pub fn decline(&mut self, actor: EntityId, world: &World) -> Vec<GuildEffect> {
        let key = Self::member_key(world, actor);
        self.pending.remove(&key);
        Vec::new()
    }

    pub fn leave(&mut self, actor: EntityId, world: &World) -> Vec<GuildEffect> {
        let key = Self::member_key(world, actor);
        let Some(gid) = self.membership.remove(&key) else {
            return vec![GuildEffect::Error {
                to: actor,
                message: "You are not in a guild.".into(),
            }];
        };
        let Some(mut guild) = self.guilds.remove(&gid) else {
            return Vec::new();
        };
        let name = guild.name.clone();
        let rank = guild
            .members
            .iter()
            .find(|m| m.durable_id == key)
            .map(|m| m.rank);
        let actor_name = world
            .get::<Identity>(actor)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "Someone".into());
        let others = guild.members.len().saturating_sub(1);
        if rank == Some(GuildRank::Leader) && others > 0 {
            self.membership.insert(key, gid);
            self.guilds.insert(gid, guild);
            return vec![GuildEffect::Error {
                to: actor,
                message: "As Guild Master you must promote a new leader or disband the guild before leaving.".into(),
            }];
        }
        guild.members.retain(|m| m.durable_id != key);
        if guild.members.is_empty() {
            return vec![GuildEffect::Notice {
                to: actor,
                message: format!("You have left <{name}>. The guild has disbanded."),
            }];
        }
        self.guilds.insert(gid, guild);
        vec![
            GuildEffect::Notice {
                to: actor,
                message: format!("You have left <{name}>."),
            },
            GuildEffect::GuildNotice {
                guild_id: gid,
                message: format!("{actor_name} has left the guild."),
            },
        ]
    }
}

fn live_member(world: &World, id: EntityId, rank: GuildRank) -> GuildMember {
    GuildMember {
        durable_id: GuildRoster::member_key(world, id),
        name: world
            .get::<Identity>(id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "Unknown".into()),
        class_id: world
            .get::<ClassKit>(id)
            .and_then(|k| k.class_id)
            .map(|c| c.as_str().to_string())
            .unwrap_or_default(),
        level: world.get::<Health>(id).map(|h| h.level).unwrap_or(1),
        rank,
    }
}

fn find_player_by_name(world: &World, name: &str) -> Option<EntityId> {
    world.ids::<ClassKit>().into_iter().find(|&id| {
        world
            .get::<Identity>(id)
            .is_some_and(|i| i.name == name)
    })
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

    #[test]
    fn validate_guild_name_accepts_letters_and_single_spaces() {
        assert_eq!(validate_guild_name("  Vale Watch  ").as_deref(), Some("Vale Watch"));
        assert!(validate_guild_name("ab").is_none());
        assert!(validate_guild_name("Vale  Watch").is_none());
        assert!(validate_guild_name("Vale Watch 1").is_none());
    }

    #[test]
    fn create_seats_founder_as_leader() {
        let world = world_with_players(1);
        let mut roster = GuildRoster::new();
        let effects = roster.create(1, "Vale Watch", &world);
        assert!(effects.iter().any(|e| matches!(
            e,
            GuildEffect::Notice { to: 1, message } if message.contains("<Vale Watch>")
        )));
        let gid = roster.guild_id_of(&GuildRoster::member_key(&world, 1)).unwrap();
        let g = roster.guild(gid).unwrap();
        assert_eq!(g.members.len(), 1);
        assert_eq!(g.members[0].rank, GuildRank::Leader);
        assert_eq!(g.members[0].name, "Alice");
    }

    #[test]
    fn create_rejects_bad_name_and_duplicate() {
        let world = world_with_players(2);
        let mut roster = GuildRoster::new();
        let bad = roster.create(1, "x", &world);
        assert!(matches!(bad.as_slice(), [GuildEffect::Error { .. }]));
        let _ = roster.create(1, "Vale Watch", &world);
        let dup = roster.create(2, "vale watch", &world);
        assert!(matches!(dup.as_slice(), [GuildEffect::Error { to: 2, .. }]));
        assert!(
            matches!(&dup[0], GuildEffect::Error { message, .. } if message.contains("already exists"))
        );
        let again = roster.create(1, "Other Name", &world);
        assert!(matches!(again.as_slice(), [GuildEffect::Error { to: 1, .. }]));
        assert!(
            matches!(&again[0], GuildEffect::Error { message, .. } if message.contains("already in a guild"))
        );
    }

    #[test]
    fn invite_accept_adds_member() {
        let world = world_with_players(2);
        let mut roster = GuildRoster::new();
        let _ = roster.create(1, "Vale Watch", &world);
        let inv = roster.invite(1, "Bob", 10, &world);
        assert!(inv.iter().any(|e| matches!(e, GuildEffect::Notice { to: 1, .. })));
        let acc = roster.accept(2, 11, &world);
        assert!(acc.iter().any(|e| matches!(e, GuildEffect::GuildNotice { .. })));
        let gid = roster.guild_id_of(&GuildRoster::member_key(&world, 2)).unwrap();
        assert_eq!(roster.guild(gid).unwrap().members.len(), 2);
    }

    #[test]
    fn invite_expires_and_member_cannot_invite() {
        let world = world_with_players(3);
        let mut roster = GuildRoster::new();
        let _ = roster.create(1, "Vale Watch", &world);
        let _ = roster.invite(1, "Bob", 0, &world);
        let late = roster.accept(2, GUILD_INVITE_TTL_TICKS + 1, &world);
        assert!(late.iter().any(|e| matches!(
            e,
            GuildEffect::Error { message, .. } if message.contains("expired")
        )));
        let _ = roster.invite(1, "Bob", 0, &world);
        let _ = roster.accept(2, 1, &world);
        let denied = roster.invite(2, "Carol", 2, &world);
        assert!(denied.iter().any(|e| matches!(
            e,
            GuildEffect::Error { to: 2, message } if message.contains("Only officers")
        )));
    }

    #[test]
    fn leader_cannot_leave_while_others_remain() {
        let world = world_with_players(2);
        let mut roster = GuildRoster::new();
        let _ = roster.create(1, "Vale Watch", &world);
        let _ = roster.invite(1, "Bob", 0, &world);
        let _ = roster.accept(2, 1, &world);
        let blocked = roster.leave(1, &world);
        assert!(blocked.iter().any(|e| matches!(
            e,
            GuildEffect::Error { to: 1, message } if message.contains("promote a new leader")
        )));
        let left = roster.leave(2, &world);
        assert!(left.iter().any(|e| matches!(e, GuildEffect::GuildNotice { .. })));
        let last = roster.leave(1, &world);
        assert!(last.iter().any(|e| matches!(
            e,
            GuildEffect::Notice { to: 1, message } if message.contains("disbanded")
        )));
        assert!(roster.guild_id_of(&GuildRoster::member_key(&world, 1)).is_none());
    }
}
