//! Hunter / warlock pet summon, dismiss, and combat AI.

use crate::combat::{deal_damage, dist2d, face_toward};
use crate::entity::Entity;
use crate::types::{MELEE_RANGE, MOB_SPEED, PLAYER_SWING_SEC};
use crate::world::{terrain_height, WORLD_SEED};
use woc_content::{pet_for_class, PetDef, PlayerClass};
use woc_protocol::{EntityId, EntityKind, SimEvent, DT};

/// Offset from owner when the pet is summoned.
const SUMMON_OFFSET: f32 = 1.5;
/// How close the pet stays when following (no combat).
const FOLLOW_RANGE: f32 = 3.0;

/// Living pet owned by `owner_id`, if any.
pub fn find_pet(entities: &[Entity], owner_id: EntityId) -> Option<EntityId> {
    entities
        .iter()
        .find(|e| e.kind == EntityKind::Pet && e.alive && e.owner_id == Some(owner_id))
        .map(|e| e.id)
}

/// Summon the class default pet beside the player. Replaces an existing pet.
///
/// Returns `false` if the player is missing, dead, or not a summoning class.
pub fn summon_pet(
    entities: &mut Vec<Entity>,
    next_id: &mut EntityId,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return false;
    };
    if entities[pi].kind != EntityKind::Player || !entities[pi].alive {
        return false;
    }
    let Some(class) = entities[pi].class_id else {
        return false;
    };
    let Some(def) = pet_for_class(class) else {
        events.push(SimEvent::Toast {
            message: "Your class cannot summon a pet.".into(),
        });
        return false;
    };

    let _ = dismiss_pet(entities, player_id, events);

    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return false;
    };
    let (px, pz, yaw) = (entities[pi].x, entities[pi].z, entities[pi].yaw);
    let ox = px + yaw.sin() * SUMMON_OFFSET;
    let oz = pz + yaw.cos() * SUMMON_OFFSET;

    let id = *next_id;
    *next_id += 1;
    let pet = create_pet(id, def, player_id, ox, oz);
    entities.push(pet);
    events.push(SimEvent::Toast {
        message: format!("{} joins the fight!", def.name),
    });
    true
}

/// Remove the player's active pet.
pub fn dismiss_pet(
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(pet_id) = find_pet(entities, player_id) else {
        return false;
    };
    entities.retain(|e| e.id != pet_id);
    events.push(SimEvent::Toast {
        message: "Pet dismissed.".into(),
    });
    true
}

fn create_pet(id: EntityId, def: &PetDef, owner_id: EntityId, x: f32, z: f32) -> Entity {
    let mut e = Entity::blank(id, EntityKind::Pet, def.name, Some(def.id), x, z);
    e.hp = def.hp;
    e.hp_max = def.hp;
    e.level = def.level;
    e.attack_damage = def.attack_damage;
    e.owner_id = Some(owner_id);
    e.auto_attack = true;
    e
}

/// Pet AI: follow owner; attack the owner's current living target.
///
/// Call once per sim tick (after player combat). Does not alter `TICK_PHASES`.
pub fn tick_pets(entities: &mut Vec<Entity>, events: &mut Vec<SimEvent>) {
    let pet_ids: Vec<EntityId> = entities
        .iter()
        .filter(|e| e.kind == EntityKind::Pet && e.alive)
        .map(|e| e.id)
        .collect();

    for pet_id in pet_ids {
        tick_one_pet(pet_id, entities, events);
    }

    let drop_ids: Vec<EntityId> = entities
        .iter()
        .filter(|e| e.kind == EntityKind::Pet)
        .filter(|e| {
            if !e.alive {
                return true;
            }
            let Some(oid) = e.owner_id else {
                return true;
            };
            !entities
                .iter()
                .any(|o| o.id == oid && o.kind == EntityKind::Player && o.alive)
        })
        .map(|e| e.id)
        .collect();
    if !drop_ids.is_empty() {
        entities.retain(|e| !drop_ids.contains(&e.id));
    }
}

fn tick_one_pet(pet_id: EntityId, entities: &mut [Entity], events: &mut Vec<SimEvent>) {
    let Some(pi) = entities.iter().position(|e| e.id == pet_id) else {
        return;
    };
    let Some(owner_id) = entities[pi].owner_id else {
        return;
    };
    let Some(oi) = entities.iter().position(|e| e.id == owner_id) else {
        // Owner gone — pet despawns next dismiss; mark dead.
        entities[pi].alive = false;
        return;
    };
    if entities[oi].kind != EntityKind::Player || !entities[oi].alive {
        entities[pi].alive = false;
        return;
    }

    let owner_target = entities[oi].target;
    let (ox, oz) = (entities[oi].x, entities[oi].z);

    // Resolve attack target: owner's living mob/player target (not the pet itself).
    let attack_tid = owner_target.and_then(|tid| {
        entities.iter().find(|e| {
            e.id == tid
                && e.alive
                && e.id != pet_id
                && e.id != owner_id
                && matches!(e.kind, EntityKind::Mob | EntityKind::Player)
        })
        .map(|e| e.id)
    });

    entities[pi].target = attack_tid;

    if let Some(tid) = attack_tid {
        let Some(ti) = entities.iter().position(|e| e.id == tid) else {
            return;
        };
        let (tx, tz) = (entities[ti].x, entities[ti].z);
        let d = dist2d(&entities[pi], &entities[ti]);
        if d > MELEE_RANGE * 0.85 {
            move_toward(&mut entities[pi], tx, tz, MOB_SPEED * 1.05);
        } else {
            entities[pi].yaw = face_toward(&entities[pi], &entities[ti]);
        }

        // Re-resolve after move.
        let Some(pi) = entities.iter().position(|e| e.id == pet_id) else {
            return;
        };
        let Some(ti) = entities.iter().position(|e| e.id == tid) else {
            return;
        };
        if dist2d(&entities[pi], &entities[ti]) <= MELEE_RANGE {
            entities[pi].swing_timer -= DT;
            if entities[pi].swing_timer <= 0.0 {
                entities[pi].swing_timer = PLAYER_SWING_SEC;
                let dmg = entities[pi].attack_damage;
                // Attribute kill credit to the owner for XP/loot.
                deal_damage(entities, owner_id, tid, dmg, Some("pet"), events);
                // Still show pet as damage source in a second event? Keep single event with owner.
            }
        }
    } else {
        // Follow owner when idle.
        let d = {
            let pet = &entities[pi];
            let dx = ox - pet.x;
            let dz = oz - pet.z;
            (dx * dx + dz * dz).sqrt()
        };
        if d > FOLLOW_RANGE {
            move_toward(&mut entities[pi], ox, oz, MOB_SPEED);
        }
    }
}

fn move_toward(pet: &mut Entity, tx: f32, tz: f32, speed: f32) {
    let dx = tx - pet.x;
    let dz = tz - pet.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < 0.01 {
        return;
    }
    let step = speed * DT;
    pet.x += dx / d * step.min(d);
    pet.z += dz / d * step.min(d);
    pet.y = terrain_height(pet.x, pet.z, WORLD_SEED);
    pet.yaw = dx.atan2(dz);
}

/// True when `class` can summon.
pub fn can_summon(class: PlayerClass) -> bool {
    pet_for_class(class).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{create_mob_from_template, create_player};
    use woc_protocol::DT;

    fn hunter_at(id: EntityId, x: f32, z: f32) -> Entity {
        create_player(id, "Hunt", PlayerClass::Hunter, x, z)
    }

    fn warlock_at(id: EntityId, x: f32, z: f32) -> Entity {
        create_player(id, "Lock", PlayerClass::Warlock, x, z)
    }

    #[test]
    fn hunter_can_summon_and_dismiss_pet() {
        let mut entities = vec![hunter_at(1, 10.0, -5.0)];
        let mut next_id = 2;
        let mut events = Vec::new();
        assert!(summon_pet(&mut entities, &mut next_id, 1, &mut events));
        assert!(find_pet(&entities, 1).is_some());
        let pet = entities.iter().find(|e| e.kind == EntityKind::Pet).unwrap();
        assert_eq!(pet.template_id.as_deref(), Some("hunter_wolf"));
        assert_eq!(pet.owner_id, Some(1));
        assert!(dismiss_pet(&mut entities, 1, &mut events));
        assert!(find_pet(&entities, 1).is_none());
        assert!(!entities.iter().any(|e| e.kind == EntityKind::Pet));
    }

    #[test]
    fn warlock_summons_imp() {
        let mut entities = vec![warlock_at(1, 10.0, -5.0)];
        let mut next_id = 2;
        let mut events = Vec::new();
        assert!(summon_pet(&mut entities, &mut next_id, 1, &mut events));
        let pet = entities.iter().find(|e| e.kind == EntityKind::Pet).unwrap();
        assert_eq!(pet.template_id.as_deref(), Some("warlock_imp"));
    }

    #[test]
    fn warrior_cannot_summon() {
        let mut entities = vec![create_player(1, "War", PlayerClass::Warrior, 10.0, -5.0)];
        let mut next_id = 2;
        let mut events = Vec::new();
        assert!(!summon_pet(&mut entities, &mut next_id, 1, &mut events));
        assert!(find_pet(&entities, 1).is_none());
    }

    #[test]
    fn pet_attacks_owner_target() {
        let mut hunter = hunter_at(1, 10.0, -5.0);
        let mut wolf = create_mob_from_template(2, "young_wolf", 12.0, -5.0).unwrap();
        wolf.home_x = wolf.x;
        wolf.home_z = wolf.z;
        hunter.target = Some(2);
        let mut entities = vec![hunter, wolf];
        let mut next_id = 3;
        let mut events = Vec::new();
        assert!(summon_pet(&mut entities, &mut next_id, 1, &mut events));
        let pet_id = find_pet(&entities, 1).unwrap();
        // Place pet in melee range of the wolf.
        if let Some(p) = entities.iter_mut().find(|e| e.id == pet_id) {
            p.x = 12.0;
            p.z = -5.0;
            p.swing_timer = 0.0;
        }
        let hp_before = entities.iter().find(|e| e.id == 2).unwrap().hp;
        for _ in 0..5 {
            tick_pets(&mut entities, &mut events);
        }
        let hp_after = entities.iter().find(|e| e.id == 2).unwrap().hp;
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
        let mut entities = vec![hunter_at(1, 10.0, -5.0)];
        let mut next_id = 2;
        let mut events = Vec::new();
        assert!(summon_pet(&mut entities, &mut next_id, 1, &mut events));
        let first = find_pet(&entities, 1).unwrap();
        assert!(summon_pet(&mut entities, &mut next_id, 1, &mut events));
        let second = find_pet(&entities, 1).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            entities.iter().filter(|e| e.kind == EntityKind::Pet).count(),
            1
        );
    }
}
