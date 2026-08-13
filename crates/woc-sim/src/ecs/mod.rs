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
        assert!(w.adopt(9));
        assert!(w.contains(9));
        assert_eq!(w.next_id(), 10);
        assert!(!w.adopt(9), "re-adopting a live id must report failure");
        assert_eq!(w.spawn_count(), 1);
    }

    /// `next_id()` reserves nothing, so reading it twice hands out the same id.
    /// The second adopt must refuse rather than alias the live entity.
    #[test]
    fn adopt_rejects_duplicate_and_zero() {
        let mut w = World::new();
        let id = w.next_id();
        assert_eq!(w.next_id(), id, "next_id is a pure getter");
        assert!(w.adopt(id));
        assert!(!w.adopt(id));
        assert_eq!(w.spawn_count(), 1);

        assert!(!w.adopt(0), "id 0 is never live");
        assert!(!w.contains(0));
        assert_eq!(w.spawn_count(), 1);
    }

    #[test]
    fn insert_on_a_non_live_id_writes_nothing() {
        let mut w = World::new();
        let live = w.spawn();
        let never_live = live + 100;

        assert!(!w.insert(
            never_live,
            Transform {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
            },
        ));
        assert!(w.get::<Transform>(never_live).is_none());
        assert!(w.ids::<Transform>().is_empty());

        assert!(w.insert(
            live,
            Transform {
                x: 2.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
            },
        ));
        assert_eq!(w.get::<Transform>(live).unwrap().x, 2.0);
    }

    /// Release builds must not be able to write orphan columns for an id that
    /// was despawned; the old `debug_assert!` compiled that guard out.
    #[test]
    fn insert_after_despawn_writes_nothing() {
        let mut w = World::new();
        let id = w.spawn();
        assert!(w.despawn(id));

        assert!(!w.insert(
            id,
            Identity {
                kind: EntityKind::Loot,
                name: "ghost".into(),
                template_id: None,
                zone_id: "eastbrook".into(),
            },
        ));
        assert!(w.get::<Identity>(id).is_none());
        assert!(w.ids::<Identity>().is_empty());
    }

    /// Factories must adopt in every build. `debug_assert!(world.adopt(id))`
    /// looks like a guard but elides the adopt under `--release`, so run this
    /// with `cargo test --release` as well as in debug.
    #[test]
    fn factories_make_their_id_live_and_populate_columns() {
        let mut w = World::new();
        let id = w.next_id();
        crate::ecs::spawn::create_loot(&mut w, id, 1.0, 2.0, 5, None);

        assert!(w.contains(id), "factory must adopt the id");
        assert_eq!(w.spawn_count(), 1);
        assert!(w.get::<crate::ecs::components::LootPile>(id).is_some());
        assert_eq!(w.get::<Transform>(id).unwrap().x, 1.0);
    }

    /// Two `next_id()` reads with no adopt between them yield the same id. The
    /// second adopt now reports `false`, which is what the `debug_assert!` at
    /// every factory call site fires on. `insert` cannot catch this case — the
    /// id *is* live — so detection has to happen at adopt.
    #[test]
    fn a_double_next_id_read_is_detected_at_adopt() {
        let mut w = World::new();
        let first = w.next_id();
        let second = w.next_id();
        assert_eq!(first, second);

        assert!(w.adopt(first));
        assert!(!w.adopt(second), "the aliasing adopt must report failure");
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
                equipment_wear: crate::ecs::components::EquipmentWear::default(),
                equipment_enchants: crate::ecs::components::EquipmentEnchants::default(),
                open_vendor_npc: None,
                buyback: Vec::new(),
            },
        );
        w.insert(
            id,
            crate::ecs::components::Hearth {
                zone_id: "eastbrook".into(),
                x: 2.0,
                z: 4.0,
                ready_tick: 0,
            },
        );
        assert!(w.despawn(id));
        assert!(w.ids::<crate::ecs::components::LootPile>().is_empty());
        assert!(w.ids::<crate::ecs::components::Bags>().is_empty());
        assert!(w.ids::<crate::ecs::components::Hearth>().is_empty());
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
