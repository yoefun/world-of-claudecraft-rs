//! Sparse component catalog. Add a column here — never a field on a blob Entity.
//!
//! | Component | Who has it |
//! | --- | --- |
//! | `Identity`, `Transform` | all |
//! | `Health` | player, mob, npc, pet (not loot) |
//! | `Combat`, `Auras` | player, mob, pet |
//! | `Home`, `Threat`, `LootTable`, `Respawn` | mob |
//! | `LootPile` | loot |
//! | `Owner` | pet |
//! | `Escort` | escort NPC (quest follower; not Owner) |
//! | `ClassKit`, `Bags`, `QuestLog`, `Progress`, `Bank`, `Motion`, `Spirit`, `InstanceAt`, `Durable` | player |
//!
//! Full field list: `docs/superpowers/specs/2026-08-13-sim-ecs-design.md` §4.4.

use std::collections::{BTreeSet, HashMap};

use crate::ecs::{SparseSet, World};
use woc_content::{PlayerClass, ResourceType};
use woc_protocol::{EntityId, EntityKind};

pub trait Component: Sized + 'static {
    fn storage(world: &World) -> &SparseSet<Self>;
    fn storage_mut(world: &mut World) -> &mut SparseSet<Self>;
}

macro_rules! impl_component {
    ($ty:ty, $field:ident) => {
        impl Component for $ty {
            fn storage(world: &World) -> &SparseSet<Self> {
                &world.$field
            }
            fn storage_mut(world: &mut World) -> &mut SparseSet<Self> {
                &mut world.$field
            }
        }
    };
}

/// Live aura instance (DoT/HoT/buff) on an entity.
#[derive(Debug, Clone)]
pub struct AuraInstance {
    pub id: String,
    pub remaining: f32,
    pub stacks: u32,
    /// Countdown to next damage/heal tick.
    pub tick_timer: f32,
    pub tick_interval: f32,
    /// Positive = DoT damage per tick.
    pub tick_damage: f32,
    /// Positive = HoT healing per tick.
    pub tick_heal: f32,
    pub source: EntityId,
    pub stun: bool,
    /// Horizontal speed multiplier (`1.0` = unchanged).
    pub move_mult: f32,
    /// Remaining damage this aura soaks before HP.
    pub absorb: f32,
    /// Removed when the bearer takes a hit (fear, travel form).
    pub breaks_on_damage: bool,
    /// Outgoing damage multiplier (`1.0` = unchanged).
    pub damage_mult: f32,
    /// Damage dealt back to a melee attacker of the bearer.
    pub thorns: f32,
    /// Extra armor while the aura remains.
    pub armor_flat: f32,
}

/// In-progress ability cast.
#[derive(Debug, Clone)]
pub struct CastState {
    pub ability_id: String,
    pub elapsed: f32,
    pub duration: f32,
    pub target: EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvStack {
    pub item_id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Equipment {
    pub main_hand: Option<String>,
    pub off_hand: Option<String>,
    pub head: Option<String>,
    pub chest: Option<String>,
    pub legs: Option<String>,
    pub feet: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestState {
    Active,
    Ready,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestProgress {
    pub quest_id: String,
    pub state: QuestState,
    pub counts: Vec<u32>,
    pub completed_tick: u64,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub kind: EntityKind,
    pub name: String,
    pub template_id: Option<String>,
    pub zone_id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}

#[derive(Debug, Clone)]
pub struct Health {
    pub hp: f32,
    pub hp_max: f32,
    pub alive: bool,
    pub level: u32,
}

#[derive(Debug, Clone)]
pub struct Combat {
    pub attack_damage: f32,
    pub armor: f32,
    pub swing_timer: f32,
    pub ability_cd: f32,
    pub auto_attack: bool,
    pub target: Option<EntityId>,
    pub gcd: f32,
    pub cast: Option<CastState>,
    /// Seconds remaining before the actor may start a new cast / instant.
    pub cast_lockout: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Auras {
    pub auras: Vec<AuraInstance>,
}

#[derive(Debug, Clone, Copy)]
pub struct Home {
    pub home_x: f32,
    pub home_z: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Threat {
    pub threat: HashMap<EntityId, f32>,
}

#[derive(Debug, Clone, Default)]
pub struct LootTable {
    pub loot_copper: u32,
    pub loot_item: Option<String>,
    pub xp_value: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Respawn {
    pub respawn_timer: f32,
}

#[derive(Debug, Clone, Default)]
pub struct LootPile {
    pub copper: u32,
    pub item: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Owner {
    pub owner_id: EntityId,
}

/// NPC following a player for an escort objective. Not a pet (`Owner`).
#[derive(Debug, Clone)]
pub struct Escort {
    pub player_id: EntityId,
    pub quest_id: String,
    pub dest_x: f32,
    pub dest_z: f32,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct ClassKit {
    pub class_id: Option<PlayerClass>,
    pub resource: f32,
    pub resource_max: f32,
    pub resource_type: Option<ResourceType>,
    pub primary_ability: Option<String>,
    pub known_abilities: Vec<String>,
    pub ability_cds: HashMap<String, f32>,
    /// Rogue combo points, 0–5.
    pub combo_points: u8,
    pub stealthed: bool,
    /// Warrior stance or shapeshift id.
    pub stance_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Bags {
    pub inventory: Vec<Option<InvStack>>,
    pub equipment: Equipment,
    pub open_vendor_npc: Option<EntityId>,
}

#[derive(Debug, Clone, Default)]
pub struct QuestLog {
    pub quest_log: Vec<QuestProgress>,
}

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub xp: u32,
    pub copper: u32,
    pub talent_points: u32,
    pub talents: HashMap<String, u32>,
    pub honor: u32,
    pub pvp_flagged: bool,
    pub professions: HashMap<String, u32>,
    pub completed_deeds: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct Bank {
    pub bank: Vec<Option<InvStack>>,
    /// Copper stored in the personal bank vault.
    pub bank_copper: u32,
}

#[derive(Debug, Clone)]
pub struct Motion {
    pub vx: f32,
    pub vz: f32,
    pub vy: f32,
    pub on_ground: bool,
    pub jumping: bool,
    pub fall_start_y: f32,
    pub flying: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Spirit {
    pub corpse_x: Option<f32>,
    pub corpse_z: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct InstanceAt {
    pub instance_id: Option<String>,
    pub delve_room: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Durable {
    pub durable_id: Option<String>,
}

impl_component!(Identity, identity);
impl_component!(Transform, transform);
impl_component!(Health, health);
impl_component!(Combat, combat);
impl_component!(Auras, auras);
impl_component!(Home, home);
impl_component!(Threat, threat);
impl_component!(LootTable, loot_table);
impl_component!(Respawn, respawn);
impl_component!(LootPile, loot_pile);
impl_component!(Owner, owner);
impl_component!(Escort, escort);
impl_component!(ClassKit, class_kit);
impl_component!(Bags, bags);
impl_component!(QuestLog, quest_log);
impl_component!(Progress, progress);
impl_component!(Bank, bank);
impl_component!(Motion, motion);
impl_component!(Spirit, spirit);
impl_component!(InstanceAt, instance_at);
impl_component!(Durable, durable);

/// 2D ground distance using Transform columns (replaces combat::dist2d on Entity).
pub fn dist2d(world: &World, a: EntityId, b: EntityId) -> Option<f32> {
    let ta = world.get::<Transform>(a)?;
    let tb = world.get::<Transform>(b)?;
    let dx = ta.x - tb.x;
    let dz = ta.z - tb.z;
    Some((dx * dx + dz * dz).sqrt())
}
