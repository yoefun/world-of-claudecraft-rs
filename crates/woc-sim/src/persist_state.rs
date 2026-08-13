//! Host-neutral player progression snapshot for save/load bridges.
//!
//! Kept free of `woc-persist` so the sim stays host-agnostic. The server maps
//! these fields to/from durable `CharacterSave` DTOs.

use std::collections::{BTreeSet, HashMap};

use crate::ecs::components::{
    Bags, Bank, ClassKit, Durable, Health, Identity, Progress, QuestLog, Transform,
};
use crate::ecs::World;
use crate::entity::{
    create_player, refresh_known_abilities, Entity, Equipment, InvStack, QuestProgress, QuestState,
};
use crate::stats::recalc_player_stats;
use crate::types::{BACKPACK_SLOTS, BANK_SLOTS};
use woc_content::PlayerClass;
use woc_protocol::EntityId;

/// Serializable player progression (mirrors durable character fields).
#[derive(Debug, Clone)]
pub struct PlayerPersistentState {
    pub durable_id: Option<String>,
    pub level: u32,
    pub xp: u32,
    pub copper: u32,
    pub pos_x: f32,
    pub pos_z: f32,
    pub inventory: Vec<Option<InvStack>>,
    pub equipment: Equipment,
    pub quests: Vec<QuestProgress>,
    pub zone_id: String,
    pub talent_points: u32,
    pub talents: HashMap<String, u32>,
    pub bank: Vec<Option<InvStack>>,
    pub honor: u32,
    pub professions: HashMap<String, u32>,
    pub pvp_flagged: bool,
    pub completed_deeds: BTreeSet<String>,
}

impl PlayerPersistentState {
    /// True when the durable row was never played (empty bags + starter level).
    pub fn is_virgin(&self) -> bool {
        self.level <= 1
            && self.xp == 0
            && self.copper == 0
            && self.inventory.iter().all(|s| s.is_none())
            && self.bank.iter().all(|s| s.is_none())
            && self.equipment.main_hand.is_none()
            && self.equipment.off_hand.is_none()
            && self.equipment.head.is_none()
            && self.equipment.chest.is_none()
            && self.equipment.legs.is_none()
            && self.equipment.feet.is_none()
            && self.quests.is_empty()
            && self.talents.is_empty()
            && self.professions.is_empty()
            && self.completed_deeds.is_empty()
            && self.honor == 0
            && !self.pvp_flagged
    }
}

/// Export durable fields from a live player in the sparse-column world.
pub fn export_player_state(world: &World, player_id: EntityId) -> Option<PlayerPersistentState> {
    if world.get::<ClassKit>(player_id).is_none() {
        return None;
    }
    Some(PlayerPersistentState {
        durable_id: world
            .get::<Durable>(player_id)
            .and_then(|d| d.durable_id.clone()),
        level: world
            .get::<Health>(player_id)
            .map(|h| h.level)
            .unwrap_or(1),
        xp: world.get::<Progress>(player_id).map(|p| p.xp).unwrap_or(0),
        copper: world
            .get::<Progress>(player_id)
            .map(|p| p.copper)
            .unwrap_or(0),
        pos_x: world.get::<Transform>(player_id).map(|t| t.x).unwrap_or(0.0),
        pos_z: world.get::<Transform>(player_id).map(|t| t.z).unwrap_or(0.0),
        inventory: world
            .get::<Bags>(player_id)
            .map(|b| b.inventory.clone())
            .unwrap_or_default(),
        equipment: world
            .get::<Bags>(player_id)
            .map(|b| b.equipment.clone())
            .unwrap_or_default(),
        quests: world
            .get::<QuestLog>(player_id)
            .map(|q| q.quest_log.clone())
            .unwrap_or_default(),
        zone_id: world
            .get::<Identity>(player_id)
            .map(|i| i.zone_id.clone())
            .unwrap_or_default(),
        talent_points: world
            .get::<Progress>(player_id)
            .map(|p| p.talent_points)
            .unwrap_or(0),
        talents: world
            .get::<Progress>(player_id)
            .map(|p| p.talents.clone())
            .unwrap_or_default(),
        bank: world
            .get::<Bank>(player_id)
            .map(|b| b.bank.clone())
            .unwrap_or_default(),
        honor: world
            .get::<Progress>(player_id)
            .map(|p| p.honor)
            .unwrap_or(0),
        professions: world
            .get::<Progress>(player_id)
            .map(|p| p.professions.clone())
            .unwrap_or_default(),
        pvp_flagged: world
            .get::<Progress>(player_id)
            .map(|p| p.pvp_flagged)
            .unwrap_or(false),
        completed_deeds: world
            .get::<Progress>(player_id)
            .map(|p| p.completed_deeds.clone())
            .unwrap_or_default(),
    })
}

/// Apply durable state onto an existing player entity (after spawn).
///
/// Virgin saves keep the class starter kit from [`create_player`]. Non-virgin
/// saves replace inventory/equipment/progression entirely.
pub fn apply_player_state(player: &mut Entity, state: &PlayerPersistentState) {
    player.durable_id = state.durable_id.clone();
    player.completed_deeds = state.completed_deeds.clone();
    player.pvp_flagged = state.pvp_flagged;
    player.honor = state.honor;
    player.talent_points = state.talent_points;
    player.talents = state.talents.clone();
    player.professions = state.professions.clone();
    player.quest_log = state.quests.clone();

    if state.is_virgin() {
        // Keep starter kit; only stamp durable id / zone preference.
        if !state.zone_id.is_empty() {
            player.zone_id = state.zone_id.clone();
        }
        sync_recalc_player_stats(player);
        refresh_known_abilities(player);
        return;
    }

    player.level = state.level.max(1);
    player.xp = state.xp;
    player.copper = state.copper;
    player.zone_id = if state.zone_id.is_empty() {
        "eastbrook".into()
    } else {
        state.zone_id.clone()
    };
    // Never restore mid-instance from save — always land in overworld zone.
    player.instance_id = None;
    player.delve_room = None;
    if player.zone_id.starts_with("instance:") || player.zone_id.starts_with("delve:") {
        player.zone_id = "eastbrook".into();
    }

    player.inventory = pad_slots(state.inventory.clone(), BACKPACK_SLOTS);
    player.bank = pad_slots(state.bank.clone(), BANK_SLOTS);
    player.equipment = state.equipment.clone();
    player.x = state.pos_x;
    player.z = state.pos_z;
    player.y = Entity::ground_at(player.x, player.z);
    player.home_x = player.x;
    player.home_z = player.z;
    player.alive = true;
    player.corpse_x = None;
    player.corpse_z = None;
    player.auras.clear();
    player.cast = None;
    player.target = None;
    player.auto_attack = false;
    player.open_vendor_npc = None;
    player.threat.clear();
    refresh_known_abilities(player);
    sync_recalc_player_stats(player);
    player.hp = player.hp_max;
    if let Some(rt) = player.resource_type {
        player.resource = match rt {
            woc_content::ResourceType::Rage => 0.0,
            woc_content::ResourceType::Mana | woc_content::ResourceType::Energy => {
                player.resource_max * 0.5
            }
        };
    }
}

fn sync_recalc_player_stats(player: &mut Entity) {
    let mut world = crate::ecs::World::new();
    crate::ecs::spawn::sync_entity_to_world(&mut world, player);
    recalc_player_stats(&mut world, player.id);
    crate::ecs::spawn::apply_world_to_entity(&world, player);
}

fn pad_slots(mut slots: Vec<Option<InvStack>>, size: usize) -> Vec<Option<InvStack>> {
    if slots.len() < size {
        slots.resize(size, None);
    } else if slots.len() > size {
        slots.truncate(size);
    }
    slots
}

/// Build a player entity from class + durable state.
pub fn create_player_from_state(
    id: EntityId,
    name: &str,
    class: PlayerClass,
    state: &PlayerPersistentState,
) -> Entity {
    let (sx, sz) = if state.is_virgin() {
        (
            woc_content::EASTBROOK.player_spawn_x,
            woc_content::EASTBROOK.player_spawn_z,
        )
    } else {
        (state.pos_x, state.pos_z)
    };
    let mut player = create_player(id, name, class, sx, sz);
    apply_player_state(&mut player, state);
    player
}

/// Parse quest state strings from durable DTOs.
pub fn quest_state_from_str(s: &str) -> QuestState {
    match s {
        "ready" => QuestState::Ready,
        "completed" => QuestState::Completed,
        _ => QuestState::Active,
    }
}

pub fn quest_state_to_str(s: QuestState) -> &'static str {
    match s {
        QuestState::Active => "active",
        QuestState::Ready => "ready",
        QuestState::Completed => "completed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_content::PlayerClass;

    #[test]
    fn virgin_save_keeps_starter_kit() {
        let state = PlayerPersistentState {
            durable_id: Some("char-1".into()),
            level: 1,
            xp: 0,
            copper: 0,
            pos_x: 0.0,
            pos_z: 0.0,
            inventory: vec![],
            equipment: Equipment::default(),
            quests: vec![],
            zone_id: "eastbrook".into(),
            talent_points: 0,
            talents: HashMap::new(),
            bank: vec![],
            honor: 0,
            professions: HashMap::new(),
            pvp_flagged: false,
            completed_deeds: BTreeSet::new(),
        };
        assert!(state.is_virgin());
        let player = create_player_from_state(1, "Ada", PlayerClass::Warrior, &state);
        assert_eq!(player.durable_id.as_deref(), Some("char-1"));
        assert!(player.equipment.main_hand.is_some());
        assert!(player.inventory.iter().any(|s| s.is_some()));
    }

    #[test]
    fn non_virgin_restore_roundtrip() {
        let mut base = create_player(1, "Ada", PlayerClass::Mage, 10.0, 20.0);
        base.durable_id = Some("abc".into());
        base.level = 5;
        base.xp = 120;
        base.copper = 77;
        base.honor = 10;
        base.talent_points = 2;
        base.talents.insert("mage_arcane_power".into(), 2);
        base.completed_deeds.insert("eastfen_mire_terror".into());
        base.zone_id = "eastfen".into();
        let world = crate::ecs::spawn::world_from_entities(&[base]);
        let exported = export_player_state(&world, 1).unwrap();
        assert!(!exported.is_virgin());
        let restored = create_player_from_state(9, "Ada", PlayerClass::Mage, &exported);
        assert_eq!(restored.level, 5);
        assert_eq!(restored.xp, 120);
        assert_eq!(restored.copper, 77);
        assert_eq!(restored.honor, 10);
        assert_eq!(restored.talents.get("mage_arcane_power"), Some(&2));
        assert!(restored.completed_deeds.contains("eastfen_mire_terror"));
        assert_eq!(restored.zone_id, "eastfen");
        assert!((restored.x - 10.0).abs() < 1e-3);
        assert!((restored.z - 20.0).abs() < 1e-3);
    }
}
