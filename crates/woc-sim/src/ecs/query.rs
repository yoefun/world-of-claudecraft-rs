//! Shared entity-classification queries.
//!
//! One home for "which entities are X", so call sites query the column rather
//! than branching on `Identity.kind` (see `AGENTS.md` and
//! `docs/architecture/ecs.md` rule 3).

use crate::ecs::components::{ClassKit, Health};
use crate::ecs::World;
use woc_protocol::EntityId;

/// Every player in the realm, dead or alive, in `ClassKit` insertion order.
///
/// `ClassKit` is exclusively a player column — `ecs::spawn::create_player`
/// inserts it and no other factory does — so this is exact, not an approximation.
pub fn player_ids(world: &World) -> Vec<EntityId> {
    world.ids::<ClassKit>()
}

/// Players that are currently alive. Named apart from [`player_ids`] so that
/// choosing between "all players" and "living players" is explicit at the call
/// site rather than a silent difference behind one name.
pub fn living_player_ids(world: &World) -> Vec<EntityId> {
    world
        .ids::<ClassKit>()
        .into_iter()
        .filter(|&id| world.get::<Health>(id).is_some_and(|h| h.alive))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_content::PlayerClass;

    #[test]
    fn player_ids_finds_players_and_only_players() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "meadow_wolf", 5.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 3, "Bob", PlayerClass::Mage, 1.0, 0.0);
        crate::ecs::spawn::create_loot(&mut world, 4, 0.0, 0.0, 10, None);

        assert_eq!(player_ids(&world), vec![1, 3]);
        assert_eq!(living_player_ids(&world), vec![1, 3]);
    }

    #[test]
    fn living_player_ids_drops_the_dead_but_player_ids_keeps_them() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        world.get_mut::<Health>(2).unwrap().alive = false;

        assert_eq!(player_ids(&world), vec![1, 2]);
        assert_eq!(living_player_ids(&world), vec![1]);
    }

    #[test]
    fn despawned_players_leave_both_lists() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
        assert!(world.despawn(1));

        assert_eq!(player_ids(&world), vec![2]);
        assert_eq!(living_player_ids(&world), vec![2]);
    }
}
