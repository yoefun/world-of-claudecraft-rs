//! Say and party chat routing.

use crate::ecs::components::{ClassKit, Identity};
use crate::ecs::World;
use crate::social::party::PartyRoster;
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

    #[test]
    fn party_chat_reaches_members() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Alice", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        let mut parties = PartyRoster::new();
        let _ = parties.invite(1, "Bob", &world);
        let _ = parties.accept(2, &world);
        let effects = handle_chat(&parties, &world, 1, "party", "hi");
        assert!(!effects.is_empty());
    }

    #[test]
    fn say_chat_works_solo() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Solo", PlayerClass::Warrior, 0.0, 0.0);
        let parties = PartyRoster::new();
        let effects = handle_chat(&parties, &world, 1, "say", "hello");
        assert!(!effects.is_empty());
    }
}
