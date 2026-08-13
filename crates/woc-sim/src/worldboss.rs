//! World boss and deed completion (one-shot honor awards).

use crate::ecs::components::Progress;
use crate::ecs::World;
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
pub fn on_boss_killed(
    world: &mut World,
    player_id: EntityId,
    template_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(deed) = DEEDS.iter().find(|d| d.boss_template == template_id) else {
        return false;
    };
    let Some(progress) = world.get_mut::<Progress>(player_id) else {
        return false;
    };
    if !progress.completed_deeds.insert(deed.id.to_string()) {
        return false;
    }
    progress.honor = progress.honor.saturating_add(25);
    events.push(SimEvent::Toast {
        message: format!("Deed complete: {}", deed.name),
    });
    events.push(SimEvent::HonorGained {
        player: player_id,
        amount: 25,
    });
    true
}

/// Dual-write shim for kill-reward code still holding a fat `Entity`.
pub fn on_boss_killed_entity(
    player: &mut Entity,
    template_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let mut world = World::new();
    crate::ecs::spawn::sync_entity_to_world(&mut world, player);
    let ok = on_boss_killed(&mut world, player.id, template_id, events);
    crate::ecs::spawn::apply_world_to_entity(&world, player);
    ok
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
        let mut world = crate::ecs::spawn::world_from_entities(std::slice::from_ref(&player));
        assert!(on_boss_killed(&mut world, 1, "mire_terror", &mut events));
        crate::ecs::spawn::apply_world_to_entity(&world, &mut player);
        assert_eq!(player.honor, 25);
        assert!(player.completed_deeds.contains("eastfen_mire_terror"));
        events.clear();
        assert!(!on_boss_killed(&mut world, 1, "mire_terror", &mut events));
        crate::ecs::spawn::apply_world_to_entity(&world, &mut player);
        assert_eq!(player.honor, 25);
    }
}
