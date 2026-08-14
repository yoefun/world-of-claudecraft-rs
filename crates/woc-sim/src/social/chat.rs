//! Say, party, raid, guild, and officer chat routing.

use crate::ecs::components::{ClassKit, Identity};
use crate::ecs::World;
use crate::social::guild::{GuildRank, GuildRoster, GUILD_MESSAGE_MAX};
use crate::social::party::{GroupKind, PartyRoster};
use woc_protocol::EntityId;

/// Chat delivery payload (host maps to `WsServerMsg::Chat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatEffect {
    Message {
        channel: String,
        from: String,
        text: String,
    },
    Error {
        message: String,
    },
}

/// Handle a chat request. Channels: `say`, `party`, `raid`, `guild`, `officer`.
pub fn handle_chat(
    roster: &PartyRoster,
    guilds: &GuildRoster,
    world: &World,
    speaker: EntityId,
    channel: &str,
    text: &str,
) -> Vec<ChatEffect> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![ChatEffect::Error {
            message: "Chat message is empty.".into(),
        }];
    }
    let Some(from) = world
        .get::<ClassKit>(speaker)
        .and_then(|_| world.get::<Identity>(speaker).map(|i| i.name.clone()))
    else {
        return vec![ChatEffect::Error {
            message: "You are not in the realm.".into(),
        }];
    };

    let channel = channel.trim().to_ascii_lowercase();
    match channel.as_str() {
        "say" => vec![ChatEffect::Message {
            channel: "say".into(),
            from,
            text: trimmed.to_string(),
        }],
        "party" => {
            if roster.party_id(speaker).is_none() {
                return vec![ChatEffect::Error {
                    message: "You are not in a party.".into(),
                }];
            }
            vec![ChatEffect::Message {
                channel: "party".into(),
                from,
                text: trimmed.to_string(),
            }]
        }
        "raid" => match roster.kind_of(speaker) {
            Some(GroupKind::Raid) => vec![ChatEffect::Message {
                channel: "raid".into(),
                from,
                text: trimmed.to_string(),
            }],
            _ => vec![ChatEffect::Error {
                message: "You are not in a raid.".into(),
            }],
        },
        "guild" => {
            let key = GuildRoster::member_key(world, speaker);
            if guilds.guild_id_of(&key).is_none() {
                return vec![ChatEffect::Error {
                    message: "You are not in a guild.".into(),
                }];
            }
            if trimmed.len() > GUILD_MESSAGE_MAX {
                return vec![ChatEffect::Error {
                    message: "Chat message is too long.".into(),
                }];
            }
            vec![ChatEffect::Message {
                channel: "guild".into(),
                from,
                text: trimmed.to_string(),
            }]
        }
        "officer" => {
            let key = GuildRoster::member_key(world, speaker);
            if guilds.guild_id_of(&key).is_none() {
                return vec![ChatEffect::Error {
                    message: "You are not in a guild.".into(),
                }];
            }
            match guilds.rank_of(&key) {
                Some(GuildRank::Leader) | Some(GuildRank::Officer) => {}
                _ => {
                    return vec![ChatEffect::Error {
                        message: "Only officers and the Guild Master can use officer chat.".into(),
                    }];
                }
            }
            if trimmed.len() > GUILD_MESSAGE_MAX {
                return vec![ChatEffect::Error {
                    message: "Chat message is too long.".into(),
                }];
            }
            vec![ChatEffect::Message {
                channel: "officer".into(),
                from,
                text: trimmed.to_string(),
            }]
        }
        other => vec![ChatEffect::Error {
            message: format!("Unknown chat channel '{other}'."),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::social::guild::GuildRoster;
    use crate::social::party::PartyRoster;
    use woc_content::PlayerClass;

    fn duo() -> (PartyRoster, World) {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Alice", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        let mut roster = PartyRoster::new();
        let _ = roster.invite(1, "Bob", &world, 0);
        let _ = roster.accept(2, &world);
        (roster, world)
    }

    #[test]
    fn say_emits_chat_message() {
        let (roster, world) = duo();
        let guilds = GuildRoster::new();
        let effects = handle_chat(&roster, &guilds, &world, 1, "say", "hello");
        assert_eq!(
            effects,
            vec![ChatEffect::Message {
                channel: "say".into(),
                from: "Alice".into(),
                text: "hello".into(),
            }]
        );
    }

    #[test]
    fn party_channel_requires_membership() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Solo", PlayerClass::Warrior, 0.0, 0.0);
        let roster = PartyRoster::new();
        let guilds = GuildRoster::new();
        let effects = handle_chat(&roster, &guilds, &world, 1, "party", "psst");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { .. }]));
    }

    #[test]
    fn party_channel_emits_when_grouped() {
        let (roster, world) = duo();
        let guilds = GuildRoster::new();
        let effects = handle_chat(&roster, &guilds, &world, 2, "party", "ready");
        assert_eq!(
            effects,
            vec![ChatEffect::Message {
                channel: "party".into(),
                from: "Bob".into(),
                text: "ready".into(),
            }]
        );
    }

    #[test]
    fn empty_text_rejected() {
        let (roster, world) = duo();
        let guilds = GuildRoster::new();
        let effects = handle_chat(&roster, &guilds, &world, 1, "say", "   ");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { .. }]));
    }

    #[test]
    fn raid_channel_requires_raid() {
        let (roster, world) = duo();
        let guilds = GuildRoster::new();
        let effects = handle_chat(&roster, &guilds, &world, 1, "raid", "hi");
        assert!(
            matches!(effects.as_slice(), [ChatEffect::Error { message }] if message == "You are not in a raid.")
        );
    }

    #[test]
    fn raid_channel_emits_after_convert() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Alice", PlayerClass::Warrior, 0.0, 0.0);
        let mut roster = PartyRoster::new();
        for (id, name) in [(2u32, "Bob"), (3, "Cara"), (4, "Dane"), (5, "Elin")] {
            crate::ecs::spawn::create_player(
                &mut world,
                id,
                name,
                PlayerClass::Mage,
                id as f32,
                0.0,
            );
            let _ = roster.invite(1, name, &world, 0);
            let _ = roster.accept(id, &world);
        }
        let _ = roster.convert_to_raid(1);
        let guilds = GuildRoster::new();
        let effects = handle_chat(&roster, &guilds, &world, 2, "raid", "pull");
        assert_eq!(
            effects,
            vec![ChatEffect::Message {
                channel: "raid".into(),
                from: "Bob".into(),
                text: "pull".into(),
            }]
        );
    }

    #[test]
    fn guild_channel_requires_membership() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Solo", PlayerClass::Warrior, 0.0, 0.0);
        let parties = PartyRoster::new();
        let guilds = GuildRoster::new();
        let effects = handle_chat(&parties, &guilds, &world, 1, "guild", "hi");
        assert_eq!(
            effects,
            vec![ChatEffect::Error {
                message: "You are not in a guild.".into(),
            }]
        );
    }

    #[test]
    fn guild_channel_rejects_long_message() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Alice", PlayerClass::Warrior, 0.0, 0.0);
        let parties = PartyRoster::new();
        let mut guilds = GuildRoster::new();
        let _ = guilds.create(1, "Vale Watch", &world);
        let long = "x".repeat(201);
        let effects = handle_chat(&parties, &guilds, &world, 1, "guild", &long);
        assert_eq!(
            effects,
            vec![ChatEffect::Error {
                message: "Chat message is too long.".into(),
            }]
        );
    }

    #[test]
    fn officer_channel_rejects_members() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Alice", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        let parties = PartyRoster::new();
        let mut guilds = GuildRoster::new();
        let _ = guilds.create(1, "Vale Watch", &world);
        let _ = guilds.invite(1, "Bob", 0, &world);
        let _ = guilds.accept(2, 1, &world);
        let denied = handle_chat(&parties, &guilds, &world, 2, "officer", "secret");
        assert_eq!(
            denied,
            vec![ChatEffect::Error {
                message: "Only officers and the Guild Master can use officer chat.".into(),
            }]
        );
        let ok = handle_chat(&parties, &guilds, &world, 1, "officer", "secret");
        assert_eq!(
            ok,
            vec![ChatEffect::Message {
                channel: "officer".into(),
                from: "Alice".into(),
                text: "secret".into(),
            }]
        );
    }
}
