//! Entity factories and live entity state.

use crate::types::{warrior_hp, WARRIOR_RAGE_MAX, WOLF_HP};
use crate::world::{terrain_height, WORLD_SEED};
use woc_protocol::{EntityId, EntityKind};

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub hp: f32,
    pub hp_max: f32,
    pub level: u32,
    pub resource: f32,
    pub resource_max: f32,
    pub alive: bool,
    pub home_x: f32,
    pub home_z: f32,
    pub swing_timer: f32,
    pub ability_cd: f32,
    pub auto_attack: bool,
    pub target: Option<EntityId>,
    pub loot_copper: u32,
    pub loot_item: Option<String>,
    pub xp_value: u32,
}

impl Entity {
    pub fn ground_at(x: f32, z: f32) -> f32 {
        terrain_height(x, z, WORLD_SEED)
    }
}

pub fn create_warrior(id: EntityId, name: &str, x: f32, z: f32) -> Entity {
    let y = Entity::ground_at(x, z);
    let hp = warrior_hp(1);
    Entity {
        id,
        kind: EntityKind::Player,
        name: name.to_string(),
        x,
        y,
        z,
        yaw: 0.0,
        hp,
        hp_max: hp,
        level: 1,
        resource: 0.0,
        resource_max: WARRIOR_RAGE_MAX,
        alive: true,
        home_x: x,
        home_z: z,
        swing_timer: 0.0,
        ability_cd: 0.0,
        auto_attack: false,
        target: None,
        loot_copper: 0,
        loot_item: None,
        xp_value: 0,
    }
}

pub fn create_wolf(id: EntityId, name: &str, x: f32, z: f32, xp: u32) -> Entity {
    let y = Entity::ground_at(x, z);
    Entity {
        id,
        kind: EntityKind::Mob,
        name: name.to_string(),
        x,
        y,
        z,
        yaw: 0.0,
        hp: WOLF_HP,
        hp_max: WOLF_HP,
        level: 1,
        resource: 0.0,
        resource_max: 0.0,
        alive: true,
        home_x: x,
        home_z: z,
        swing_timer: 0.0,
        ability_cd: 0.0,
        auto_attack: false,
        target: None,
        loot_copper: 0,
        loot_item: None,
        xp_value: xp,
    }
}

pub fn create_loot(id: EntityId, x: f32, z: f32, copper: u32, item: Option<String>) -> Entity {
    let y = Entity::ground_at(x, z);
    Entity {
        id,
        kind: EntityKind::Loot,
        name: "Loot".to_string(),
        x,
        y,
        z,
        yaw: 0.0,
        hp: 1.0,
        hp_max: 1.0,
        level: 1,
        resource: 0.0,
        resource_max: 0.0,
        alive: true,
        home_x: x,
        home_z: z,
        swing_timer: 0.0,
        ability_cd: 0.0,
        auto_attack: false,
        target: None,
        loot_copper: copper,
        loot_item: item,
        xp_value: 0,
    }
}
