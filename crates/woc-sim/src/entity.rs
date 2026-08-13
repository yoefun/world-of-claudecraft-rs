//! Entity factories and live entity state.

use std::collections::{BTreeSet, HashMap};

use crate::types::player_hp;
use crate::world::{ground_height, WORLD_SEED};
use woc_content::{
    class_def, known_abilities_at_level, mob, npc, ItemKind, PlayerClass, ResourceType,
};
use woc_protocol::{EntityId, EntityKind};

pub use crate::ecs::components::{
    AuraInstance, CastState, Equipment, InvStack, QuestProgress, QuestState,
};

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub name: String,
    pub template_id: Option<String>,
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
    pub attack_damage: f32,
    pub armor: f32,
    pub class_id: Option<PlayerClass>,
    pub resource_type: Option<ResourceType>,
    pub primary_ability: Option<String>,
    /// Ability ids unlocked from the class kit at the current level.
    pub known_abilities: Vec<String>,
    /// Per-ability cooldown remaining (seconds), keyed by ability id.
    pub ability_cds: HashMap<String, f32>,
    pub inventory: Vec<Option<InvStack>>,
    pub equipment: Equipment,
    pub quest_log: Vec<QuestProgress>,
    pub open_vendor_npc: Option<EntityId>,
    /// Player experience toward next level (mobs/NPCs unused).
    pub xp: u32,
    /// Player copper currency (mobs/NPCs unused).
    pub copper: u32,
    /// Active auras (Wave 1 combat core).
    pub auras: Vec<AuraInstance>,
    /// Ability currently being cast, if any.
    pub cast: Option<CastState>,
    /// Global cooldown remaining (blocks starting another ability).
    pub gcd: f32,
    /// Threat table keyed by attacker id (mobs; players unused).
    pub threat: HashMap<EntityId, f32>,
    /// Death corpse X (player only; set on death, cleared on spirit release).
    pub corpse_x: Option<f32>,
    /// Death corpse Z (player only; set on death, cleared on spirit release).
    pub corpse_z: Option<f32>,
    /// Mob respawn countdown while dead (0 when alive / unarmed).
    pub respawn_timer: f32,
    /// Owning player id for summoned pets (`None` for everyone else).
    pub owner_id: Option<EntityId>,
    /// Current zone id (overworld or instance).
    pub zone_id: String,
    /// Unspent talent points.
    pub talent_points: u32,
    /// Learned talent ranks keyed by talent id.
    pub talents: HashMap<String, u32>,
    /// Personal bank inventory (fixed slot count).
    pub bank: Vec<Option<InvStack>>,
    /// PvP honor currency.
    pub honor: u32,
    /// Open-world PvP flag.
    pub pvp_flagged: bool,
    /// Profession skill ranks keyed by profession id.
    pub professions: HashMap<String, u32>,
    /// Active dungeon/instance id, if any.
    pub instance_id: Option<String>,
    /// Zero-based room index while inside a delve.
    pub delve_room: Option<u32>,
    /// Durable character UUID string when bound to persistence (`None` offline).
    pub durable_id: Option<String>,
    /// Completed deed ids (one-shot honor awards).
    pub completed_deeds: BTreeSet<String>,
    /// Horizontal air / swim velocity (x).
    pub vx: f32,
    /// Horizontal air / swim velocity (z).
    pub vz: f32,
    /// Vertical velocity (jump / fall / fly).
    pub vy: f32,
    /// True while feet are on walkable support (or treading water).
    pub on_ground: bool,
    /// True for the duration of a deliberate jump arc.
    pub jumping: bool,
    /// Highest Y reached during the current airborne/fall segment.
    pub fall_start_y: f32,
    /// Travel flight (no gravity; Space/Ctrl for vertical).
    pub flying: bool,
}

impl Entity {
    pub fn ground_at(x: f32, z: f32) -> f32 {
        ground_height(x, z, WORLD_SEED)
    }

    pub(crate) fn blank(
        id: EntityId,
        kind: EntityKind,
        name: &str,
        template_id: Option<&str>,
        x: f32,
        z: f32,
    ) -> Self {
        let y = Self::ground_at(x, z);
        Self {
            id,
            kind,
            name: name.to_string(),
            template_id: template_id.map(|s| s.to_string()),
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
            loot_copper: 0,
            loot_item: None,
            xp_value: 0,
            attack_damage: 0.0,
            armor: 0.0,
            class_id: None,
            resource_type: None,
            primary_ability: None,
            known_abilities: Vec::new(),
            ability_cds: HashMap::new(),
            inventory: vec![None; crate::types::BACKPACK_SLOTS],
            equipment: Equipment::default(),
            quest_log: Vec::new(),
            open_vendor_npc: None,
            xp: 0,
            copper: 0,
            auras: Vec::new(),
            cast: None,
            gcd: 0.0,
            threat: HashMap::new(),
            corpse_x: None,
            corpse_z: None,
            respawn_timer: 0.0,
            owner_id: None,
            zone_id: "eastbrook".into(),
            talent_points: 0,
            talents: HashMap::new(),
            bank: vec![None; crate::types::BANK_SLOTS],
            honor: 0,
            pvp_flagged: false,
            professions: HashMap::new(),
            instance_id: None,
            delve_room: None,
            durable_id: None,
            completed_deeds: BTreeSet::new(),
            vx: 0.0,
            vz: 0.0,
            vy: 0.0,
            on_ground: true,
            jumping: false,
            fall_start_y: y,
            flying: false,
        }
    }
}

pub fn create_player(id: EntityId, name: &str, class: PlayerClass, x: f32, z: f32) -> Entity {
    let def = class_def(class);
    let hp = player_hp(def.base_hp, 1);
    let mut e = Entity::blank(id, EntityKind::Player, name, Some(def.id.as_str()), x, z);
    e.hp = hp;
    e.hp_max = hp;
    e.resource = match def.resource_type {
        ResourceType::Rage => 0.0,
        ResourceType::Mana | ResourceType::Energy => def.resource_max * 0.5,
    };
    e.resource_max = def.resource_max;
    e.class_id = Some(class);
    e.resource_type = Some(def.resource_type);
    e.primary_ability = Some(def.primary_ability.to_string());
    refresh_known_abilities(&mut e);
    e.attack_damage = def.attack_power;
    e.equipment.main_hand = Some(def.start_weapon.to_string());
    e.equipment.chest = Some(def.start_chest.to_string());
    for (item_id, count) in def.start_items {
        let _ = grant_into(&mut e.inventory, item_id, *count);
    }
    crate::stats::recalc_player_stats(&mut e);
    e
}

/// Sync `known_abilities` from the class kit and current level.
pub fn refresh_known_abilities(player: &mut Entity) {
    let Some(class) = player.class_id else {
        player.known_abilities.clear();
        return;
    };
    player.known_abilities = known_abilities_at_level(class, player.level)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
}

pub fn create_mob_from_template(id: EntityId, template_id: &str, x: f32, z: f32) -> Option<Entity> {
    let t = mob(template_id)?;
    let mut e = Entity::blank(id, EntityKind::Mob, t.name, Some(t.id), x, z);
    e.hp = t.hp;
    e.hp_max = t.hp;
    e.level = t.level;
    e.xp_value = t.xp;
    e.attack_damage = t.attack_damage;
    Some(e)
}

pub fn create_npc_from_template(id: EntityId, template_id: &str, x: f32, z: f32) -> Option<Entity> {
    let t = npc(template_id)?;
    let mut e = Entity::blank(id, EntityKind::Npc, t.name, Some(t.id), x, z);
    e.hp = 1000.0;
    e.hp_max = 1000.0;
    Some(e)
}

pub fn create_loot(id: EntityId, x: f32, z: f32, copper: u32, item: Option<String>) -> Entity {
    let mut e = Entity::blank(id, EntityKind::Loot, "Loot", None, x, z);
    e.loot_copper = copper;
    e.loot_item = item;
    e
}

/// Insert into backpack with stacking. Returns false if full.
pub fn grant_into(inv: &mut [Option<InvStack>], item_id: &str, count: u32) -> bool {
    if count == 0 {
        return true;
    }
    let stack_size = woc_content::item(item_id)
        .map(|d| d.stack_size.max(1))
        .unwrap_or(20);
    let unstacked = woc_content::item(item_id)
        .map(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
        .unwrap_or(false);
    let max_stack = if unstacked { 1 } else { stack_size };

    let mut remaining = count;
    if max_stack > 1 {
        for stack in inv.iter_mut().flatten() {
            if stack.item_id == item_id && stack.count < max_stack {
                let space = max_stack - stack.count;
                let add = remaining.min(space);
                stack.count += add;
                remaining -= add;
                if remaining == 0 {
                    return true;
                }
            }
        }
    }
    while remaining > 0 {
        let Some(empty) = inv.iter_mut().find(|s| s.is_none()) else {
            return false;
        };
        let add = remaining.min(max_stack);
        *empty = Some(InvStack {
            item_id: item_id.to_string(),
            count: add,
        });
        remaining -= add;
    }
    true
}

pub fn count_item(inv: &[Option<InvStack>], item_id: &str) -> u32 {
    inv.iter()
        .filter_map(|s| s.as_ref())
        .filter(|s| s.item_id == item_id)
        .map(|s| s.count)
        .sum()
}

pub fn remove_item(inv: &mut [Option<InvStack>], item_id: &str, count: u32) -> bool {
    if count_item(inv, item_id) < count {
        return false;
    }
    let mut remaining = count;
    for slot in inv.iter_mut() {
        if remaining == 0 {
            break;
        }
        let Some(stack) = slot.as_mut() else {
            continue;
        };
        if stack.item_id != item_id {
            continue;
        }
        let take = remaining.min(stack.count);
        stack.count -= take;
        remaining -= take;
        if stack.count == 0 {
            *slot = None;
        }
    }
    remaining == 0
}

#[cfg(test)]
mod architecture {
    use super::Entity;

    /// Stack layout of the frozen fat `Entity`. Lower this as ECS migration
    /// extracts columns. Raising it means someone added a field — don't; add a
    /// component (`docs/architecture/ecs.md`).
    const FAT_ENTITY_SIZE_CEILING: usize = 864;

    #[test]
    fn fat_entity_stack_size_must_not_grow() {
        let size = std::mem::size_of::<Entity>();
        assert!(
            size <= FAT_ENTITY_SIZE_CEILING,
            "Entity is {size} bytes (ceiling {FAT_ENTITY_SIZE_CEILING}). \
             Do not add fields to Entity; add a sparse component \
             (docs/architecture/ecs.md)."
        );
    }
}
