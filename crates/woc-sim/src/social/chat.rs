//! Say and party chat routing.

use crate::entity::Entity;
use crate::social::party::PartyRoster;
use woc_protocol::{EntityId, EntityKind};

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
    entities: &[Entity],
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
    let Some(from) = entities
        .iter()
        .find(|e| e.id == speaker && e.kind == EntityKind::Player)
        .map(|e| e.name.clone())
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
    use crate::entity::create_player;
    use crate::social::party::PartyRoster;
    use woc_content::PlayerClass;

    fn duo() -> (PartyRoster, Vec<Entity>) {
        let entities = vec![
            create_player(1, "Alice", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "Bob", PlayerClass::Mage, 1.0, 0.0),
        ];
        let mut roster = PartyRoster::new();
        roster.invite(1, "Bob", &entities);
        roster.accept(2, &entities);
        (roster, entities)
    }

    #[test]
    fn say_emits_chat_message() {
        let (roster, entities) = duo();
        let effects = handle_chat(&roster, &entities, 1, "say", "hello");
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
        let entities = vec![create_player(1, "Solo", PlayerClass::Warrior, 0.0, 0.0)];
        let roster = PartyRoster::new();
        let effects = handle_chat(&roster, &entities, 1, "party", "psst");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { .. }]));
    }

    #[test]
    fn party_channel_emits_when_grouped() {
        let (roster, entities) = duo();
        let effects = handle_chat(&roster, &entities, 2, "party", "ready");
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
        let (roster, entities) = duo();
        let effects = handle_chat(&roster, &entities, 1, "say", "   ");
        assert!(matches!(effects.as_slice(), [ChatEffect::Error { .. }]));
    }
}
