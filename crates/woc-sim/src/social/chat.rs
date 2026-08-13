//! Say, party, and raid chat routing.

use crate::ecs::components::{ClassKit, Identity};
use crate::ecs::World;
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

/// Handle a chat request. Channels: `say` (realm-wide scaffold), `party` (party only).
pub fn handle_chat(
    roster: &PartyRoster,
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
        "raid" => {
            match roster.kind_of(speaker) {
                Some(GroupKind::Raid) => vec![ChatEffect::Message {
                    channel: "raid".into(),
                    from,
                    text: trimmed.to_string(),
                }],
                _ => vec![ChatEffect::Error {
                    message: "You are not in a raid.".into(),
                }],
            }
        }
        other => vec![ChatEffect::Error {
            message: format!("Unknown chat channel '{other}'."),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let effects = handle_chat(&roster, &world, 1, "say", "hello");
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
        let effects = handle_chat(&roster, &world, 1, "party", "psst");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { .. }]));
    }

    #[test]
    fn party_channel_emits_when_grouped() {
        let (roster, world) = duo();
        let effects = handle_chat(&roster, &world, 2, "party", "ready");
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
        let effects = handle_chat(&roster, &world, 1, "say", "   ");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { .. }]));
    }

    #[test]
    fn raid_channel_requires_raid() {
        let (roster, world) = duo();
        let effects = handle_chat(&roster, &world, 1, "raid", "hi");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { message }] if message == "You are not in a raid."));
    }
}
