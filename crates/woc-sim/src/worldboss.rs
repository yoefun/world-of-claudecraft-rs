//! World boss and deed completion (one-shot honor awards).

use crate::entity::Entity;
use woc_protocol::{EntityId, SimEvent};

/// Minimal deed definition.
#[derive(Debug, Clone, Copy)]
pub struct DeedDef {
    pub id: &'static str,
    pub name: &'static str,
    pub boss_template: &'static str,
}

pub static DEEDS: &[DeedDef] = &[DeedDef {
    id: "eastfen_mire_terror",
    name: "Slay the Mire Terror",
    boss_template: "mire_terror",
}];

/// Credit a deed when a matching world boss template is killed.
///
/// Returns true only the first time a player completes the deed.
pub fn on_boss_killed(player: &mut Entity, template_id: &str, events: &mut Vec<SimEvent>) -> bool {
    let Some(deed) = DEEDS.iter().find(|d| d.boss_template == template_id) else {
        return false;
    };
    if !player.completed_deeds.insert(deed.id.to_string()) {
        return false;
    }
    events.push(SimEvent::Toast {
        message: format!("Deed complete: {}", deed.name),
    });
    events.push(SimEvent::HonorGained {
        player: player.id,
        amount: 25,
    });
    player.honor = player.honor.saturating_add(25);
    let _ = player_id_touch(player.id);
    true
}

fn player_id_touch(id: EntityId) -> EntityId {
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    #[test]
    fn deed_credits_honor_once() {
        let mut player = create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        let mut events = Vec::new();
        assert!(on_boss_killed(&mut player, "mire_terror", &mut events));
        assert_eq!(player.honor, 25);
        assert!(player.completed_deeds.contains("eastfen_mire_terror"));
        events.clear();
        assert!(!on_boss_killed(&mut player, "mire_terror", &mut events));
        assert_eq!(player.honor, 25);
    }
}
