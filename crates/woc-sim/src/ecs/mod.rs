//! Typed sparse-column ECS for the deterministic sim.
//!
//! Not Bevy. Iteration order is spawn/insertion order.

pub mod components;
pub mod query;
pub mod sparse;
pub mod spawn;
pub mod world;

pub use components::{Identity, LootPile, Transform};
pub use query::{living_player_ids, player_ids};
pub use sparse::SparseSet;
pub use world::World;

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::EntityKind;

    #[test]
    fn spawn_skips_zero_and_lookup_is_by_id() {
        let mut w = World::new();
        let a = w.spawn();
        let b = w.spawn();
        assert!(a >= 1);
        assert_eq!(b, a + 1);
        w.insert(
            a,
            Identity {
                kind: EntityKind::Loot,
                name: "pile".into(),
                template_id: None,
                zone_id: "eastbrook".into(),
            },
        );
        w.insert(
            a,
            Transform {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                yaw: 0.0,
            },
        );
        assert_eq!(w.get::<Identity>(a).unwrap().name, "pile");
        assert!(w.get::<Transform>(b).is_none());
        assert!(w.despawn(a));
        assert!(w.get::<Identity>(a).is_none());
        assert!(w.get::<Transform>(a).is_none());
        assert!(w.contains(b));
    }

    #[test]
    fn ids_follow_insert_order() {
        let mut w = World::new();
        let a = w.spawn();
        let b = w.spawn();
        w.insert(
            b,
            Transform {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
            },
        );
        w.insert(
            a,
            Transform {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
            },
        );
        assert_eq!(w.ids::<Transform>(), vec![b, a]);
    }

    #[test]
    fn set_next_id_and_column_helpers() {
        let mut w = World::new();
        w.set_next_id(40);
        assert_eq!(w.next_id(), 40);
        let id = w.spawn();
        assert_eq!(id, 40);
        assert_eq!(w.spawn_count(), 1);
        w.insert(
            id,
            Transform {
                x: 3.0,
                y: 0.0,
                z: 4.0,
                yaw: 0.0,
            },
        );
        w.get_mut::<Transform>(id).unwrap().x = 6.0;
        assert_eq!(w.get::<Transform>(id).unwrap().x, 6.0);
        assert!(w.remove::<Transform>(id).is_some());
        assert_eq!(
            crate::ecs::components::dist2d(&w, id, id),
            None,
            "no transform after remove"
        );
        let live: Vec<_> = w.live_ids().collect();
        assert_eq!(live, vec![id]);
    }

    #[test]
    fn adopt_keeps_existing_id() {
        let mut w = World::new();
        w.adopt(9);
        assert!(w.contains(9));
        assert_eq!(w.next_id(), 10);
        w.adopt(9);
        assert_eq!(w.spawn_count(), 1);
    }

    #[test]
    fn despawn_drops_sparse_columns() {
        let mut w = World::new();
        let id = w.spawn();
        w.insert(
            id,
            crate::ecs::components::LootPile {
                copper: 5,
                item: None,
            },
        );
        w.insert(
            id,
            crate::ecs::components::Bags {
                inventory: vec![None; crate::types::BACKPACK_SLOTS],
                equipment: crate::ecs::components::Equipment::default(),
                open_vendor_npc: None,
            },
        );
        assert!(w.despawn(id));
        assert!(w.ids::<crate::ecs::components::LootPile>().is_empty());
        assert!(w.ids::<crate::ecs::components::Bags>().is_empty());
        assert!(!w.contains(id));
    }

    #[test]
    fn sim_does_not_store_a_homogeneous_entity_vec() {
        let src = include_str!("../sim.rs");
        assert!(
            !src.contains("entities: Vec<"),
            "Sim must not grow a Vec of blob actors; use World columns"
        );
    }
}
