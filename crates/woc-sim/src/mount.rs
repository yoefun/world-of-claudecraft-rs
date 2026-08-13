//! Mount riding: train, learn, summon, toggle.

use crate::combat;
use crate::corpse::has_corpse_marker_world;
use crate::ecs::components::{Health, InstanceAt, Motion, Progress, Riding, Transform};
use crate::ecs::World;
use crate::player_motion::is_swimming_at;
use crate::world::{ground_height, WORLD_SEED};
use woc_content::{mount, MountKind, riding_rank_by_n};
use woc_protocol::{EntityId, SimEvent};

pub fn is_mounted(world: &World, player_id: EntityId) -> bool {
    world
        .get::<Riding>(player_id)
        .and_then(|r| r.active_id.as_ref())
        .is_some()
}

pub fn dismount(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) -> bool {
    let Some(riding) = world.get_mut::<Riding>(player_id) else {
        return false;
    };
    if riding.active_id.is_none() {
        return false;
    }
    riding.active_id = None;
    if let Some(m) = world.get_mut::<Motion>(player_id) {
        if m.flying {
            m.flying = false;
        }
    }
    events.push(SimEvent::Toast {
        message: "You dismount.".into(),
    });
    true
}

pub fn summon_mount(
    world: &mut World,
    player_id: EntityId,
    mount_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let alive = world
        .get::<Health>(player_id)
        .map(|h| h.alive)
        .unwrap_or(false);
    if !alive || has_corpse_marker_world(world, player_id) {
        events.push(SimEvent::Toast {
            message: "You cannot mount here.".into(),
        });
        return false;
    }
    if world
        .get::<InstanceAt>(player_id)
        .and_then(|i| i.instance_id.clone())
        .is_some()
    {
        events.push(SimEvent::Toast {
            message: "You cannot mount here.".into(),
        });
        return false;
    }

    let def = mount(mount_id);
    let (x, y, z) = world
        .get::<Transform>(player_id)
        .map(|t| (t.x, t.y, t.z))
        .unwrap_or((0.0, 0.0, 0.0));
    if def.is_some_and(|d| d.kind == MountKind::Ground) && is_swimming_at(x, y, z) {
        events.push(SimEvent::Toast {
            message: "You cannot mount here.".into(),
        });
        return false;
    }

    if combat::is_stealthed(world, player_id) {
        events.push(SimEvent::Toast {
            message: "You cannot mount here.".into(),
        });
        return false;
    }

    let rank = world
        .get::<Riding>(player_id)
        .map(|r| r.rank)
        .unwrap_or(0);
    if rank == 0 {
        events.push(SimEvent::Toast {
            message: "You need riding training.".into(),
        });
        return false;
    }

    let Some(def) = def else {
        events.push(SimEvent::Toast {
            message: "You do not know a mount.".into(),
        });
        return false;
    };

    let known = world
        .get::<Riding>(player_id)
        .map(|r| r.known.contains(mount_id))
        .unwrap_or(false);
    if !known {
        events.push(SimEvent::Toast {
            message: "You do not know a mount.".into(),
        });
        return false;
    }

    if rank < def.riding_rank {
        events.push(SimEvent::Toast {
            message: "Your riding skill is too low.".into(),
        });
        return false;
    }
    if def.kind == MountKind::Flying && rank < 3 {
        events.push(SimEvent::Toast {
            message: "Your riding skill is too low.".into(),
        });
        return false;
    }

    let Some(riding) = world.get_mut::<Riding>(player_id) else {
        return false;
    };
    riding.active_id = Some(mount_id.to_string());
    riding.last_id = Some(mount_id.to_string());

    combat::strip_travel_forms(world, player_id);

    if def.kind == MountKind::Flying {
        let lift = world.get::<Transform>(player_id).map(|t| {
            (
                t.x,
                t.y.max(ground_height(t.x, t.z, WORLD_SEED) + 1.5),
            )
        });
        if let Some((_, y)) = lift {
            if let Some(t) = world.get_mut::<Transform>(player_id) {
                t.y = y;
            }
            if let Some(m) = world.get_mut::<Motion>(player_id) {
                m.flying = true;
                m.on_ground = false;
                m.jumping = false;
                m.vy = 0.0;
                m.fall_start_y = y;
            }
        }
    } else if let Some(m) = world.get_mut::<Motion>(player_id) {
        m.flying = false;
    }

    events.push(SimEvent::Toast {
        message: "You mount up.".into(),
    });
    true
}

pub fn toggle_mount(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    if is_mounted(world, player_id) {
        dismount(world, player_id, events);
        return;
    }

    let mount_id = world.get::<Riding>(player_id).and_then(|r| {
        if let Some(last) = r.last_id.clone() {
            return Some(last);
        }
        if r.known.len() == 1 {
            return r.known.iter().next().cloned();
        }
        None
    });

    let Some(mount_id) = mount_id else {
        events.push(SimEvent::Toast {
            message: "You do not know a mount.".into(),
        });
        return;
    };

    summon_mount(world, player_id, &mount_id, events);
}

pub fn learn_mount(
    world: &mut World,
    player_id: EntityId,
    mount_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = mount(mount_id) else {
        return false;
    };
    let Some(riding) = world.get_mut::<Riding>(player_id) else {
        return false;
    };
    riding.known.insert(mount_id.to_string());
    riding.last_id = Some(mount_id.to_string());
    events.push(SimEvent::Toast {
        message: format!("You learn to ride the {}.", def.name),
    });
    true
}

pub fn train_riding(
    world: &mut World,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let rank = world
        .get::<Riding>(player_id)
        .map(|r| r.rank)
        .unwrap_or(0);
    let next = rank + 1;
    let Some(def) = riding_rank_by_n(next) else {
        events.push(SimEvent::Toast {
            message: "You already know that rank.".into(),
        });
        return false;
    };
    let level = world
        .get::<Health>(player_id)
        .map(|h| h.level)
        .unwrap_or(1);
    if level < def.level_req {
        events.push(SimEvent::Toast {
            message: "You are too low level.".into(),
        });
        return false;
    }
    let copper = world
        .get::<Progress>(player_id)
        .map(|p| p.copper)
        .unwrap_or(0);
    if copper < def.copper {
        events.push(SimEvent::Toast {
            message: "Not enough copper.".into(),
        });
        return false;
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper -= def.copper;
    }
    if let Some(riding) = world.get_mut::<Riding>(player_id) {
        riding.rank = next;
    }
    events.push(SimEvent::Toast {
        message: format!("Learned {}.", def.name),
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::spawn::create_player;
    use crate::ecs::components::{Health, Motion, Progress, Riding};
    use woc_content::PlayerClass;
    use woc_protocol::{EntityId, PlayerIntent, SimEvent};

    fn warrior() -> (World, EntityId) {
        let mut world = World::new();
        create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        (world, 1)
    }

    fn toast_text(events: &[SimEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                SimEvent::Toast { message } => Some(message.clone()),
                _ => None,
            })
            .collect()
    }

    fn mount_pony(world: &mut World, id: EntityId) {
        world.get_mut::<Riding>(id).unwrap().rank = 1;
        world.get_mut::<Riding>(id).unwrap().known.insert("brown_pony".into());
        let mut events = Vec::new();
        assert!(summon_mount(world, id, "brown_pony", &mut events));
    }

    #[test]
    fn damage_dismounts() {
        let (mut world, id) = warrior();
        mount_pony(&mut world, id);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 3.0, 0.0);
        let mut events = Vec::new();
        crate::combat::deal_damage(&mut world, 2, id, 5.0, None, true, &mut events);
        assert!(world.get::<Riding>(id).unwrap().active_id.is_none());
    }

    #[test]
    fn instance_refuses_mount() {
        let (mut world, id) = warrior();
        world.get_mut::<crate::ecs::components::InstanceAt>(id).unwrap().instance_id =
            Some("eastbrook_crypt#1".into());
        world.get_mut::<Riding>(id).unwrap().rank = 1;
        world.get_mut::<Riding>(id).unwrap().known.insert("brown_pony".into());
        let mut events = Vec::new();
        assert!(!summon_mount(&mut world, id, "brown_pony", &mut events));
        assert!(toast_text(&events).iter().any(|m| m == "You cannot mount here."));
    }

    #[test]
    fn untrained_toggle_does_not_fly() {
        let (mut world, id) = warrior();
        let mut events = Vec::new();
        toggle_mount(&mut world, id, &mut events);
        assert!(!world.get::<Motion>(id).unwrap().flying);
        assert!(world.get::<Riding>(id).unwrap().active_id.is_none());
        assert!(toast_text(&events).iter().any(|m| m == "You do not know a mount."));
    }

    #[test]
    fn summon_mount_failure_checks_prefer_training_over_unknown() {
        let (mut world, id) = warrior();
        let mut events = Vec::new();
        assert!(!summon_mount(&mut world, id, "nonexistent_mount", &mut events));
        assert_eq!(toast_text(&events), vec!["You need riding training.".to_string()]);
    }

    #[test]
    fn summon_mount_failure_checks_prefer_stealth_over_unknown() {
        let (mut world, id) = warrior();
        world.get_mut::<crate::ecs::components::ClassKit>(id).unwrap().stealthed = true;
        let mut events = Vec::new();
        assert!(!summon_mount(&mut world, id, "nonexistent_mount", &mut events));
        assert_eq!(toast_text(&events), vec!["You cannot mount here.".to_string()]);
    }

    #[test]
    fn pony_is_faster_than_foot() {
        let (mut world, id) = warrior();
        world.get_mut::<Riding>(id).unwrap().rank = 1;
        world.get_mut::<Riding>(id).unwrap().known.insert("brown_pony".into());
        let mut events = Vec::new();
        assert!(summon_mount(&mut world, id, "brown_pony", &mut events));
        let z0 = world.get::<Transform>(id).unwrap().z;
        let intent = PlayerIntent {
            move_z: 1.0,
            facing: 0.0,
            ..Default::default()
        };
        let _ = crate::player_motion::step_player_motion(&mut world, id, &intent);
        let mounted_dz = world.get::<Transform>(id).unwrap().z - z0;

        let (mut foot, fid) = warrior();
        let z1 = foot.get::<Transform>(fid).unwrap().z;
        let _ = crate::player_motion::step_player_motion(&mut foot, fid, &intent);
        let foot_dz = foot.get::<Transform>(fid).unwrap().z - z1;
        assert!(mounted_dz > foot_dz * 1.4);
    }

    #[test]
    fn gryphon_toggle_allows_ascend() {
        let (mut world, id) = warrior();
        world.get_mut::<Health>(id).unwrap().level = 8;
        world.get_mut::<Riding>(id).unwrap().rank = 3;
        world.get_mut::<Riding>(id).unwrap().known.insert("tawny_gryphon".into());
        world.get_mut::<Riding>(id).unwrap().last_id = Some("tawny_gryphon".into());
        let mut events = Vec::new();
        toggle_mount(&mut world, id, &mut events);
        assert!(world.get::<Motion>(id).unwrap().flying);
        let start_y = world.get::<Transform>(id).unwrap().y;
        let up = PlayerIntent {
            jump: true,
            ..Default::default()
        };
        for _ in 0..10 {
            let _ = crate::player_motion::step_player_motion(&mut world, id, &up);
        }
        assert!(world.get::<Transform>(id).unwrap().y > start_y + 2.0);
    }

    #[test]
    fn apprentice_then_pony_summons() {
        let (mut world, id) = warrior();
        world.get_mut::<Health>(id).unwrap().level = 2;
        world.get_mut::<Progress>(id).unwrap().copper = 40;
        let mut events = Vec::new();
        assert!(learn_mount(&mut world, id, "brown_pony", &mut events));
        // rank still 0 → summon must fail
        assert!(!summon_mount(&mut world, id, "brown_pony", &mut events));
        world.get_mut::<Riding>(id).unwrap().rank = 1;
        events.clear();
        assert!(summon_mount(&mut world, id, "brown_pony", &mut events));
        assert_eq!(
            world.get::<Riding>(id).unwrap().active_id.as_deref(),
            Some("brown_pony")
        );
        assert!(!world.get::<Motion>(id).unwrap().flying);
        assert!(toast_text(&events).iter().any(|m| m == "You mount up."));
    }
}
