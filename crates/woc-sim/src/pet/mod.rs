//! Hunter / warlock pet summon, dismiss, and combat AI.

use crate::combat::{deal_damage, dist2d_ids, face_toward_ids};
use crate::ecs::components::{ClassKit, Combat, Health, Identity, LootTable, Owner, Transform};
use crate::ecs::World;
use crate::entity_motion::step_toward;
use crate::types::{MELEE_RANGE, MOB_SPEED, PLAYER_SWING_SEC};
use woc_content::{pet_for_class, PlayerClass};
use woc_protocol::{EntityId, EntityKind, SimEvent, DT};

/// Offset from owner when the pet is summoned.
const SUMMON_OFFSET: f32 = 1.5;
/// How close the pet stays when following (no combat).
const FOLLOW_RANGE: f32 = 3.0;

/// Living pet owned by `owner_id`, if any.
pub fn find_pet(world: &World, owner_id: EntityId) -> Option<EntityId> {
    world.ids::<Owner>().into_iter().find(|&id| {
        world.get::<Owner>(id).map(|o| o.owner_id) == Some(owner_id)
            && world.get::<Health>(id).map(|h| h.alive).unwrap_or(false)
    })
}

/// Summon the class default pet beside the player. Replaces an existing pet.
pub fn summon_pet(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) -> bool {
    if world.get::<ClassKit>(player_id).is_none() {
        return false;
    }
    if !world
        .get::<Health>(player_id)
        .map(|h| h.alive)
        .unwrap_or(false)
    {
        return false;
    }
    let Some(class) = world.get::<ClassKit>(player_id).and_then(|k| k.class_id) else {
        return false;
    };
    let Some(def) = pet_for_class(class) else {
        events.push(SimEvent::Toast {
            message: "Your class cannot summon a pet.".into(),
        });
        return false;
    };

    let _ = dismiss_pet(world, player_id, events);

    let Some(t) = world.get::<Transform>(player_id).copied() else {
        return false;
    };
    let ox = t.x + t.yaw.sin() * SUMMON_OFFSET;
    let oz = t.z + t.yaw.cos() * SUMMON_OFFSET;

    let id = world.next_id();
    crate::ecs::spawn::create_pet(world, id, def, player_id, ox, oz);
    events.push(SimEvent::Toast {
        message: format!("{} joins the fight!", def.name),
    });
    true
}

/// Remove the player's active pet from columns.
pub fn dismiss_pet(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) -> bool {
    let Some(pet_id) = find_pet(world, player_id) else {
        return false;
    };
    world.despawn(pet_id);
    events.push(SimEvent::Toast {
        message: "Pet dismissed.".into(),
    });
    true
}

/// Pet AI: follow owner; attack the owner's current living target.
///
/// Call once per sim tick (after player combat). Does not alter `TICK_PHASES`.
/// Returns pet ids that were despawned (dead / owner gone).
pub fn tick_pets(world: &mut World, events: &mut Vec<SimEvent>) -> Vec<EntityId> {
    let pet_ids = world.ids::<Owner>();
    for pet_id in pet_ids {
        if world
            .get::<Health>(pet_id)
            .map(|h| h.alive)
            .unwrap_or(false)
        {
            tick_one_pet(pet_id, world, events);
        }
    }

    let drop_ids: Vec<EntityId> = world
        .ids::<Owner>()
        .into_iter()
        .filter(|&id| {
            if !world.get::<Health>(id).map(|h| h.alive).unwrap_or(false) {
                return true;
            }
            let Some(oid) = world.get::<Owner>(id).map(|o| o.owner_id) else {
                return true;
            };
            !(world.get::<ClassKit>(oid).is_some()
                && world.get::<Health>(oid).map(|h| h.alive).unwrap_or(false))
        })
        .collect();
    for id in &drop_ids {
        world.despawn(*id);
    }
    drop_ids
}

fn tick_one_pet(pet_id: EntityId, world: &mut World, events: &mut Vec<SimEvent>) {
    let Some(owner_id) = world.get::<Owner>(pet_id).map(|o| o.owner_id) else {
        return;
    };
    if world.get::<ClassKit>(owner_id).is_none()
        || !world
            .get::<Health>(owner_id)
            .map(|h| h.alive)
            .unwrap_or(false)
    {
        if let Some(h) = world.get_mut::<Health>(pet_id) {
            h.alive = false;
        }
        return;
    }

    let owner_target = world.get::<Combat>(owner_id).and_then(|c| c.target);
    let Some(ot) = world.get::<Transform>(owner_id).copied() else {
        return;
    };

    let attack_tid = owner_target.filter(|&tid| {
        tid != pet_id
            && tid != owner_id
            && world.get::<Health>(tid).map(|h| h.alive).unwrap_or(false)
            && (world.get::<LootTable>(tid).is_some() || world.get::<ClassKit>(tid).is_some())
    });

    if let Some(c) = world.get_mut::<Combat>(pet_id) {
        c.target = attack_tid;
    }

    if let Some(tid) = attack_tid {
        let d = dist2d_ids(world, pet_id, tid);
        if d > MELEE_RANGE * 0.85 {
            let Some(tt) = world.get::<Transform>(tid).copied() else {
                return;
            };
            let _ = step_toward(world, pet_id, tt.x, tt.z, MOB_SPEED * 1.05);
        } else {
            let yaw = face_toward_ids(world, pet_id, tid);
            if let Some(t) = world.get_mut::<Transform>(pet_id) {
                t.yaw = yaw;
            }
        }

        if dist2d_ids(world, pet_id, tid) <= MELEE_RANGE {
            let mut swing = false;
            let mut dmg = 0.0;
            if let Some(c) = world.get_mut::<Combat>(pet_id) {
                c.swing_timer -= DT;
                if c.swing_timer <= 0.0 {
                    c.swing_timer = PLAYER_SWING_SEC;
                    dmg = c.attack_damage;
                    swing = true;
                }
            }
            if swing {
                deal_damage(world, owner_id, tid, dmg, Some("pet"), events);
            }
        }
    } else {
        let Some(pt) = world.get::<Transform>(pet_id).copied() else {
            return;
        };
        let dx = ot.x - pt.x;
        let dz = ot.z - pt.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d > FOLLOW_RANGE {
            let _ = step_toward(world, pet_id, ot.x, ot.z, MOB_SPEED);
        }
    }
}

/// True when `class` can summon.
pub fn can_summon(class: PlayerClass) -> bool {
    pet_for_class(class).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::DT;

    fn hunter_world() -> World {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hunt", PlayerClass::Hunter, 10.0, -5.0);
        world
    }

    #[test]
    fn hunter_can_summon_and_dismiss_pet() {
        let mut world = hunter_world();
        let mut events = Vec::new();
        assert!(summon_pet(&mut world, 1, &mut events));
        assert!(find_pet(&world, 1).is_some());
        let pet = find_pet(&world, 1).unwrap();
        assert_eq!(
            world.get::<Identity>(pet).unwrap().template_id.as_deref(),
            Some("hunter_wolf")
        );
        assert_eq!(world.get::<Owner>(pet).unwrap().owner_id, 1);
        assert!(dismiss_pet(&mut world, 1, &mut events));
        assert!(find_pet(&world, 1).is_none());
        assert!(!world
            .ids::<Owner>()
            .into_iter()
            .any(|id| { world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Pet) }));
    }

    #[test]
    fn warlock_summons_imp() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Lock", PlayerClass::Warlock, 10.0, -5.0);
        let mut events = Vec::new();
        assert!(summon_pet(&mut world, 1, &mut events));
        let pet = find_pet(&world, 1).unwrap();
        assert_eq!(
            world.get::<Identity>(pet).unwrap().template_id.as_deref(),
            Some("warlock_imp")
        );
    }

    #[test]
    fn warrior_cannot_summon() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "War", PlayerClass::Warrior, 10.0, -5.0);
        let mut events = Vec::new();
        assert!(!summon_pet(&mut world, 1, &mut events));
        assert!(find_pet(&world, 1).is_none());
    }

    #[test]
    fn pet_attacks_owner_target() {
        let mut world = hunter_world();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 12.0, -5.0)
            .unwrap();
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
        }
        let mut events = Vec::new();
        assert!(summon_pet(&mut world, 1, &mut events));
        let pet_id = find_pet(&world, 1).unwrap();
        if let Some(t) = world.get_mut::<Transform>(pet_id) {
            t.x = 12.0;
            t.z = -5.0;
        }
        if let Some(c) = world.get_mut::<Combat>(pet_id) {
            c.swing_timer = 0.0;
        }
        let hp_before = world.get::<Health>(2).unwrap().hp;
        for _ in 0..5 {
            let _ = tick_pets(&mut world, &mut events);
        }
        let hp_after = world.get::<Health>(2).unwrap().hp;
        assert!(
            hp_after < hp_before,
            "pet should damage owner target ({hp_after} < {hp_before})"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                SimEvent::Damage {
                    source: 1,
                    target: 2,
                    ..
                }
            )),
            "damage credited to owner for kill rewards"
        );
        let _ = DT;
    }

    #[test]
    fn summon_replaces_existing_pet() {
        let mut world = hunter_world();
        let mut events = Vec::new();
        assert!(summon_pet(&mut world, 1, &mut events));
        let first = find_pet(&world, 1).unwrap();
        assert!(summon_pet(&mut world, 1, &mut events));
        let second = find_pet(&world, 1).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            world
                .ids::<Owner>()
                .into_iter()
                .filter(|&id| world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Pet))
                .count(),
            1
        );
    }
}
