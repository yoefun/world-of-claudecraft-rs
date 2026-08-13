//! Host-neutral player progression snapshot for save/load bridges.
//!
//! Kept free of `woc-persist` so the sim stays host-agnostic. The server maps
//! these fields to/from durable `CharacterSave` DTOs.

use std::collections::{BTreeSet, HashMap};

use crate::ecs::components::{
    Auras, Bags, Bank, ClassKit, Combat, Durable, Equipment, EquipmentWear, Health, Hearth,
    Identity, InstanceAt, InvStack, Progress, QuestLog, QuestProgress, QuestState, Riding, Spirit,
    Transform,
};
use crate::ecs::World;
use crate::stats::recalc_player_stats;
use crate::types::{BACKPACK_SLOTS, BANK_SLOTS};
use woc_content::{PlayerClass, EASTBROOK};
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
    pub equipment_wear: EquipmentWear,
    pub equipment_enchants: crate::ecs::components::EquipmentEnchants,
    pub quests: Vec<QuestProgress>,
    pub zone_id: String,
    pub talent_points: u32,
    pub talents: HashMap<String, u32>,
    pub bank: Vec<Option<InvStack>>,
    pub bank_copper: u32,
    pub honor: u32,
    pub professions: HashMap<String, u32>,
    pub pvp_flagged: bool,
    pub completed_deeds: BTreeSet<String>,
    pub hearth_zone_id: String,
    pub hearth_x: f32,
    pub hearth_z: f32,
    pub hearth_ready_tick: u64,
    pub stance_id: String,
    pub riding_rank: u8,
    pub known_mounts: BTreeSet<String>,
    pub last_mount: String,
}

impl PlayerPersistentState {
    /// True when the durable row was never played (empty bags + starter level).
    pub fn is_virgin(&self) -> bool {
        self.level <= 1
            && self.xp == 0
            && self.copper == 0
            && self.inventory.iter().all(|s| s.is_none())
            && self.bank.iter().all(|s| s.is_none())
            && self.bank_copper == 0
            && self.equipment.main_hand.is_none()
            && self.equipment.off_hand.is_none()
            && self.equipment.head.is_none()
            && self.equipment.chest.is_none()
            && self.equipment.legs.is_none()
            && self.equipment.feet.is_none()
            && self.equipment.neck.is_none()
            && self.equipment.finger.is_none()
            && self.equipment.finger2.is_none()
            && self.equipment_wear == EquipmentWear::default()
            && self.equipment_enchants == crate::ecs::components::EquipmentEnchants::default()
            && self.quests.is_empty()
            && self.talents.is_empty()
            && self.professions.is_empty()
            && self.completed_deeds.is_empty()
            && self.honor == 0
            && !self.pvp_flagged
            && self.riding_rank == 0
            && self.known_mounts.is_empty()
    }
}

/// Export durable fields from a live player in the sparse-column world.
pub fn export_player_state(world: &World, player_id: EntityId) -> Option<PlayerPersistentState> {
    world.get::<ClassKit>(player_id)?;
    Some(PlayerPersistentState {
        durable_id: world
            .get::<Durable>(player_id)
            .and_then(|d| d.durable_id.clone()),
        level: world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1),
        xp: world.get::<Progress>(player_id).map(|p| p.xp).unwrap_or(0),
        copper: world
            .get::<Progress>(player_id)
            .map(|p| p.copper)
            .unwrap_or(0),
        pos_x: world
            .get::<Transform>(player_id)
            .map(|t| t.x)
            .unwrap_or(0.0),
        pos_z: world
            .get::<Transform>(player_id)
            .map(|t| t.z)
            .unwrap_or(0.0),
        inventory: world
            .get::<Bags>(player_id)
            .map(|b| b.inventory.clone())
            .unwrap_or_default(),
        equipment: world
            .get::<Bags>(player_id)
            .map(|b| b.equipment.clone())
            .unwrap_or_default(),
        equipment_wear: world
            .get::<Bags>(player_id)
            .map(|b| b.equipment_wear.clone())
            .unwrap_or_default(),
        equipment_enchants: world
            .get::<Bags>(player_id)
            .map(|b| b.equipment_enchants.clone())
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
        bank_copper: world
            .get::<Bank>(player_id)
            .map(|b| b.bank_copper)
            .unwrap_or(0),
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
        hearth_zone_id: world
            .get::<Hearth>(player_id)
            .map(|h| h.zone_id.clone())
            .unwrap_or_else(default_hearth_zone_id),
        hearth_x: world
            .get::<Hearth>(player_id)
            .map(|h| h.x)
            .unwrap_or(EASTBROOK.player_spawn_x),
        hearth_z: world
            .get::<Hearth>(player_id)
            .map(|h| h.z)
            .unwrap_or(EASTBROOK.player_spawn_z),
        hearth_ready_tick: world
            .get::<Hearth>(player_id)
            .map(|h| h.ready_tick)
            .unwrap_or(0),
        stance_id: world
            .get::<ClassKit>(player_id)
            .and_then(|k| k.stance_id.clone())
            .unwrap_or_default(),
        riding_rank: world.get::<Riding>(player_id).map(|r| r.rank).unwrap_or(0),
        known_mounts: world
            .get::<Riding>(player_id)
            .map(|r| r.known.clone())
            .unwrap_or_default(),
        last_mount: world
            .get::<Riding>(player_id)
            .and_then(|r| r.last_id.clone())
            .unwrap_or_default(),
    })
}

/// Apply durable state onto an existing player in World (after spawn).
///
/// Virgin saves keep the class starter kit from [`crate::ecs::spawn::create_player`].
/// Non-virgin saves replace inventory/equipment/progression entirely.
pub fn apply_player_state(world: &mut World, player_id: EntityId, state: &PlayerPersistentState) {
    if let Some(d) = world.get_mut::<Durable>(player_id) {
        d.durable_id = state.durable_id.clone();
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.completed_deeds = state.completed_deeds.clone();
        p.pvp_flagged = state.pvp_flagged;
        p.honor = state.honor;
        p.talent_points = state.talent_points;
        p.talents = state.talents.clone();
        p.professions = state.professions.clone();
    }
    if let Some(q) = world.get_mut::<QuestLog>(player_id) {
        q.quest_log = state.quests.clone();
    }
    if let Some(hearth) = world.get_mut::<Hearth>(player_id) {
        hearth.zone_id = if state.hearth_zone_id.is_empty() {
            default_hearth_zone_id()
        } else {
            state.hearth_zone_id.clone()
        };
        hearth.x = state.hearth_x;
        hearth.z = state.hearth_z;
        hearth.ready_tick = state.hearth_ready_tick;
    }

    if state.is_virgin() {
        if !state.zone_id.is_empty() {
            if let Some(i) = world.get_mut::<Identity>(player_id) {
                i.zone_id = state.zone_id.clone();
            }
        }
        recalc_player_stats(world, player_id);
        crate::ecs::spawn::refresh_known_abilities(world, player_id);
        return;
    }

    if let Some(h) = world.get_mut::<Health>(player_id) {
        h.level = state.level.max(1);
        h.alive = true;
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.xp = state.xp;
        p.copper = state.copper;
    }
    let mut zone_id = if state.zone_id.is_empty() {
        "eastbrook".into()
    } else {
        state.zone_id.clone()
    };
    if zone_id.starts_with("instance:") || zone_id.starts_with("delve:") {
        zone_id = "eastbrook".into();
    }
    if let Some(i) = world.get_mut::<Identity>(player_id) {
        i.zone_id = zone_id;
    }
    if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
        inst.instance_id = None;
        inst.delve_room = None;
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.inventory = pad_slots(state.inventory.clone(), BACKPACK_SLOTS);
        bags.equipment = state.equipment.clone();
        bags.equipment_wear = state.equipment_wear.clone();
        bags.equipment_enchants = state.equipment_enchants.clone();
        bags.open_vendor_npc = None;
        bags.buyback.clear();
    }
    if let Some(bank) = world.get_mut::<Bank>(player_id) {
        bank.bank = pad_slots(state.bank.clone(), BANK_SLOTS);
        bank.bank_copper = state.bank_copper;
    }
    let y = crate::ecs::spawn::ground_at(state.pos_x, state.pos_z);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.x = state.pos_x;
        t.z = state.pos_z;
        t.y = y;
    }
    if let Some(s) = world.get_mut::<Spirit>(player_id) {
        s.corpse_x = None;
        s.corpse_z = None;
    }
    if let Some(a) = world.get_mut::<Auras>(player_id) {
        a.auras.clear();
    }
    if let Some(c) = world.get_mut::<Combat>(player_id) {
        c.cast = None;
        c.target = None;
        c.auto_attack = false;
    }
    if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
        kit.stance_id = if state.stance_id.is_empty() {
            None
        } else {
            Some(state.stance_id.clone())
        };
    }
    if let Some(riding) = world.get_mut::<Riding>(player_id) {
        riding.rank = state.riding_rank;
        riding.known = state.known_mounts.clone();
        riding.last_id = if state.last_mount.is_empty() {
            None
        } else {
            Some(state.last_mount.clone())
        };
        riding.active_id = None;
    }
    crate::ecs::spawn::refresh_known_abilities(world, player_id);
    recalc_player_stats(world, player_id);
    crate::combat::apply_spawn_identity(world, player_id);
    if let Some(h) = world.get_mut::<Health>(player_id) {
        h.hp = h.hp_max;
    }
    let rt = world
        .get::<ClassKit>(player_id)
        .and_then(|k| k.resource_type);
    let rmax = world
        .get::<ClassKit>(player_id)
        .map(|k| k.resource_max)
        .unwrap_or(0.0);
    if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
        if let Some(rt) = rt {
            kit.resource = match rt {
                woc_content::ResourceType::Rage => 0.0,
                woc_content::ResourceType::Mana | woc_content::ResourceType::Energy => rmax * 0.5,
            };
        }
    }
}

fn pad_slots(mut slots: Vec<Option<InvStack>>, size: usize) -> Vec<Option<InvStack>> {
    if slots.len() < size {
        slots.resize(size, None);
    } else if slots.len() > size {
        slots.truncate(size);
    }
    slots
}

fn default_hearth_zone_id() -> String {
    "eastbrook".into()
}

/// Spawn a player from class + durable state into `world`.
pub fn create_player_from_state(
    world: &mut World,
    id: EntityId,
    name: &str,
    class: PlayerClass,
    state: &PlayerPersistentState,
) -> EntityId {
    let (sx, sz) = if state.is_virgin() {
        (
            woc_content::EASTBROOK.player_spawn_x,
            woc_content::EASTBROOK.player_spawn_z,
        )
    } else {
        (state.pos_x, state.pos_z)
    };
    crate::ecs::spawn::create_player(world, id, name, class, sx, sz);
    apply_player_state(world, id, state);
    id
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
    fn virgin_keeps_starter_kit() {
        let state = PlayerPersistentState {
            durable_id: Some("abc".into()),
            level: 1,
            xp: 0,
            copper: 0,
            pos_x: 0.0,
            pos_z: 0.0,
            inventory: vec![],
            equipment: Equipment::default(),
            equipment_wear: EquipmentWear::default(),
            equipment_enchants: Default::default(),
            quests: vec![],
            zone_id: "eastbrook".into(),
            talent_points: 0,
            talents: Default::default(),
            bank: vec![],
            bank_copper: 0,
            honor: 0,
            professions: Default::default(),
            pvp_flagged: false,
            completed_deeds: Default::default(),
            hearth_zone_id: default_hearth_zone_id(),
            hearth_x: EASTBROOK.player_spawn_x,
            hearth_z: EASTBROOK.player_spawn_z,
            hearth_ready_tick: 0,
            stance_id: String::new(),
            riding_rank: 0,
            known_mounts: Default::default(),
            last_mount: String::new(),
        };
        assert!(state.is_virgin());
        let mut world = World::new();
        create_player_from_state(&mut world, 1, "Ada", PlayerClass::Warrior, &state);
        assert!(world.get::<Bags>(1).unwrap().equipment.main_hand.is_some());
    }

    /// Ports base's `non_virgin_restore_roundtrip` forward. Every field here had
    /// to be re-plumbed from a flat `Entity` field into a specific column during
    /// the migration — `talents` / `completed_deeds` / `talent_points` / `honor`
    /// into `Progress`, `zone_id` into `Identity`, position into `Transform`,
    /// `durable_id` into `Durable` — so this is the remapping regression net.
    #[test]
    fn round_trip_preserves_progression() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Mage, 10.0, 20.0);
        if let Some(d) = world.get_mut::<Durable>(1) {
            d.durable_id = Some("abc".into());
        }
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 5;
        }
        if let Some(i) = world.get_mut::<Identity>(1) {
            i.zone_id = "eastfen".into();
        }
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.xp = 120;
            p.copper = 77;
            p.honor = 10;
            p.talent_points = 2;
            p.talents.insert("mage_arcane_power".into(), 2);
            p.completed_deeds.insert("eastfen_mire_terror".into());
        }
        if let Some(bank) = world.get_mut::<Bank>(1) {
            bank.bank_copper = 30;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.equipment.neck = Some("fang_pendant".into());
            bags.equipment.finger = Some("boar_tusk_ring".into());
            bags.equipment.finger2 = Some("boar_tusk_ring".into());
            bags.equipment_wear.main_hand = Some(17);
            bags.equipment_enchants.main_hand = Some("coarse_sharpening".into());
            bags.equipment_enchants.off_hand = Some("minor_wizard_oil".into());
        }
        if let Some(hearth) = world.get_mut::<Hearth>(1) {
            hearth.zone_id = "eastfen".into();
            hearth.x = 12.0;
            hearth.z = 34.0;
            hearth.ready_tick = 77;
        }

        let exported = export_player_state(&world, 1).unwrap();
        assert!(!exported.is_virgin());

        let mut world2 = World::new();
        create_player_from_state(&mut world2, 9, "Ada", PlayerClass::Mage, &exported);
        let restored = export_player_state(&world2, 9).unwrap();

        assert_eq!(restored.durable_id.as_deref(), Some("abc"));
        assert_eq!(restored.level, 5);
        assert_eq!(restored.xp, 120);
        assert_eq!(restored.copper, 77);
        assert_eq!(restored.bank_copper, 30);
        assert_eq!(restored.honor, 10);
        assert_eq!(restored.talent_points, 2);
        assert_eq!(restored.talents.get("mage_arcane_power"), Some(&2));
        assert!(restored.completed_deeds.contains("eastfen_mire_terror"));
        assert_eq!(restored.zone_id, "eastfen");
        assert_eq!(restored.hearth_zone_id, "eastfen");
        assert!((restored.hearth_x - 12.0).abs() < 1e-3);
        assert!((restored.hearth_z - 34.0).abs() < 1e-3);
        assert_eq!(restored.hearth_ready_tick, 77);
        assert!((restored.pos_x - 10.0).abs() < 1e-3);
        assert!((restored.pos_z - 20.0).abs() < 1e-3);
        assert_eq!(restored.equipment.neck.as_deref(), Some("fang_pendant"));
        assert_eq!(restored.equipment.finger.as_deref(), Some("boar_tusk_ring"));
        assert_eq!(
            restored.equipment.finger2.as_deref(),
            Some("boar_tusk_ring")
        );
        assert_eq!(restored.equipment_wear.main_hand, Some(17));
        assert_eq!(
            restored.equipment_enchants.main_hand.as_deref(),
            Some("coarse_sharpening")
        );
        assert_eq!(
            restored.equipment_enchants.off_hand.as_deref(),
            Some("minor_wizard_oil")
        );
        assert!(!restored.is_virgin());
    }

    #[test]
    fn riding_round_trips() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        {
            let r = world.get_mut::<crate::ecs::components::Riding>(1).unwrap();
            r.rank = 2;
            r.known.insert("brown_pony".into());
            r.last_id = Some("brown_pony".into());
            r.active_id = Some("brown_pony".into());
        }
        let state = export_player_state(&world, 1).unwrap();
        assert_eq!(state.riding_rank, 2);
        assert!(state.known_mounts.contains("brown_pony"));
        assert_eq!(state.last_mount, "brown_pony");
        let mut world2 = World::new();
        create_player_from_state(&mut world2, 2, "Ada", PlayerClass::Warrior, &state);
        let r = world2.get::<crate::ecs::components::Riding>(2).unwrap();
        assert_eq!(r.rank, 2);
        assert!(r.known.contains("brown_pony"));
        assert_eq!(r.last_id.as_deref(), Some("brown_pony"));
        assert!(r.active_id.is_none(), "load starts dismounted");
    }

    #[test]
    fn warrior_stance_id_roundtrips() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tank", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 5;
        }
        crate::combat::cycle_stance(&mut world, 1, &mut Vec::new());
        let exported = export_player_state(&world, 1).unwrap();
        assert_eq!(exported.stance_id, "defensive");
        assert!(!exported.is_virgin());

        let mut world2 = World::new();
        create_player_from_state(&mut world2, 2, "Tank", PlayerClass::Warrior, &exported);
        assert_eq!(
            world2.get::<ClassKit>(2).unwrap().stance_id.as_deref(),
            Some("defensive")
        );
        assert!(world2
            .get::<Auras>(2)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == "defensive_stance"));
    }
}
