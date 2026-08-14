//! Sparse component catalog. Add a column here — never a field on a blob Entity.
//!
//! | Component | Who has it |
//! | --- | --- |
//! | `Identity`, `Transform` | all |
//! | `Health` | player, mob, npc, pet (not loot) |
//! | `Combat`, `Auras` | player, mob, pet |
//! | `Home`, `Threat`, `LootTable`, `Respawn` | mob |
//! | `LootPile` | loot |
//! | `GatherNodeState` | gather nodes |
//! | `Skinnable` | beast loot piles |
//! | `Owner` | pet |
//! | `Escort` | escort NPC (quest follower; not Owner) |
//! | `ClassKit`, `Bags`, `QuestLog`, `Progress`, `Reputation`, `Bank`, `Motion`, `Spirit`, `InstanceAt`, `Durable`, `Hearth`, `Riding`, `ProfessionCast` | player |
//!
//! Full field list: `docs/superpowers/specs/2026-08-13-sim-ecs-design.md` §4.4.

use std::collections::{BTreeSet, HashMap};

use crate::ecs::{SparseSet, World};
use woc_content::{item, ItemQuality, PlayerClass, ResourceType};
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
    pub durability: Option<u32>,
    pub enchant_id: Option<String>,
    pub quality: Option<ItemQuality>,
    pub bound: bool,
}

impl InvStack {
    pub fn new(item_id: impl Into<String>, count: u32) -> Self {
        let item_id = item_id.into();
        let durability =
            item(&item_id).and_then(|d| (d.max_durability > 0).then_some(d.max_durability));
        Self {
            item_id,
            count,
            durability,
            enchant_id: None,
            quality: None,
            bound: false,
        }
    }

    pub fn with_loot_bind(mut self) -> Self {
        if item(&self.item_id).is_some_and(|d| d.bind == woc_content::ItemBind::OnPickup) {
            self.bound = true;
        }
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Equipment {
    pub main_hand: Option<String>,
    pub off_hand: Option<String>,
    pub head: Option<String>,
    pub chest: Option<String>,
    pub legs: Option<String>,
    pub feet: Option<String>,
    pub neck: Option<String>,
    pub finger: Option<String>,
    pub finger2: Option<String>,
    pub shoulder: Option<String>,
    pub back: Option<String>,
    pub wrist: Option<String>,
    pub hands: Option<String>,
    pub waist: Option<String>,
    pub trinket: Option<String>,
    pub trinket2: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquipmentWear {
    pub main_hand: Option<u32>,
    pub off_hand: Option<u32>,
    pub head: Option<u32>,
    pub chest: Option<u32>,
    pub legs: Option<u32>,
    pub feet: Option<u32>,
    pub shoulder: Option<u32>,
    pub back: Option<u32>,
    pub wrist: Option<u32>,
    pub hands: Option<u32>,
    pub waist: Option<u32>,
}

impl EquipmentWear {
    pub fn max_for_item(item_id: &str) -> Option<u32> {
        item(item_id).and_then(|d| (d.max_durability > 0).then_some(d.max_durability))
    }

    pub fn full_for_equipment(equipment: &Equipment) -> Self {
        Self {
            main_hand: equipment.main_hand.as_deref().and_then(Self::max_for_item),
            off_hand: equipment.off_hand.as_deref().and_then(Self::max_for_item),
            head: equipment.head.as_deref().and_then(Self::max_for_item),
            chest: equipment.chest.as_deref().and_then(Self::max_for_item),
            legs: equipment.legs.as_deref().and_then(Self::max_for_item),
            feet: equipment.feet.as_deref().and_then(Self::max_for_item),
            shoulder: equipment.shoulder.as_deref().and_then(Self::max_for_item),
            back: equipment.back.as_deref().and_then(Self::max_for_item),
            wrist: equipment.wrist.as_deref().and_then(Self::max_for_item),
            hands: equipment.hands.as_deref().and_then(Self::max_for_item),
            waist: equipment.waist.as_deref().and_then(Self::max_for_item),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquipmentEnchants {
    pub main_hand: Option<String>,
    pub off_hand: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquipmentQualities {
    pub main_hand: Option<ItemQuality>,
    pub off_hand: Option<ItemQuality>,
    pub head: Option<ItemQuality>,
    pub chest: Option<ItemQuality>,
    pub legs: Option<ItemQuality>,
    pub feet: Option<ItemQuality>,
    pub neck: Option<ItemQuality>,
    pub finger: Option<ItemQuality>,
    pub finger2: Option<ItemQuality>,
    pub shoulder: Option<ItemQuality>,
    pub back: Option<ItemQuality>,
    pub wrist: Option<ItemQuality>,
    pub hands: Option<ItemQuality>,
    pub waist: Option<ItemQuality>,
    pub trinket: Option<ItemQuality>,
    pub trinket2: Option<ItemQuality>,
}

#[derive(Debug, Clone)]
pub struct BuybackEntry {
    pub item_id: String,
    pub count: u32,
    pub durability: Option<u32>,
    pub enchant_id: Option<String>,
    pub quality: Option<ItemQuality>,
    pub copper: u32,
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
    pub spell_power: f32,
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
    pub quality: Option<ItemQuality>,
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
    pub equipment_wear: EquipmentWear,
    pub equipment_enchants: EquipmentEnchants,
    pub equipment_qualities: EquipmentQualities,
    pub open_vendor_npc: Option<EntityId>,
    pub buyback: Vec<BuybackEntry>,
}

#[derive(Debug, Clone)]
pub struct Hearth {
    pub zone_id: String,
    pub x: f32,
    pub z: f32,
    pub ready_tick: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Riding {
    pub rank: u8,
    pub known: BTreeSet<String>,
    pub last_id: Option<String>,
    pub active_id: Option<String>,
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
    pub last_masterwork: Option<String>,
    pub completed_deeds: BTreeSet<String>,
}

/// Per-faction standing values. Missing ids read as Neutral 0.
#[derive(Debug, Clone, Default)]
pub struct Reputation {
    pub values: HashMap<String, i32>,
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

/// In-progress profession cast (separate from combat `CastState`).
#[derive(Debug, Clone)]
pub struct ProfessionCast {
    pub kind: ProfessionCastKind,
    pub complete_tick: u64,
}

#[derive(Debug, Clone)]
pub enum ProfessionCastKind {
    Gather {
        node_id: EntityId,
    },
    Skin {
        corpse_id: EntityId,
    },
    Craft {
        recipe_id: String,
        remaining: u16,
    },
    Disenchant {
        bag_slot: u8,
    },
    ApplyEnchant {
        bag_slot: u8,
        enchant_id: String,
        confirm: bool,
    },
}

#[derive(Debug, Clone, Default)]
pub struct GatherNodeState {
    pub ready_tick: u64,
}

#[derive(Debug, Clone)]
pub struct Skinnable {
    pub tier: u8,
    pub skinned: bool,
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
impl_component!(Hearth, hearth);
impl_component!(Riding, riding);
impl_component!(QuestLog, quest_log);
impl_component!(Progress, progress);
impl_component!(Reputation, reputation);
impl_component!(Bank, bank);
impl_component!(Motion, motion);
impl_component!(Spirit, spirit);
impl_component!(InstanceAt, instance_at);
impl_component!(Durable, durable);
impl_component!(ProfessionCast, profession_cast);
impl_component!(GatherNodeState, gather_node_state);
impl_component!(Skinnable, skinnable);

/// 2D ground distance using Transform columns (replaces combat::dist2d on Entity).
pub fn dist2d(world: &World, a: EntityId, b: EntityId) -> Option<f32> {
    let ta = world.get::<Transform>(a)?;
    let tb = world.get::<Transform>(b)?;
    let dx = ta.x - tb.x;
    let dz = ta.z - tb.z;
    Some((dx * dx + dz * dz).sqrt())
}
