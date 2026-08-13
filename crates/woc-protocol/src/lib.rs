//! Shared wire / host types between sim, client, and server.

use serde::{Deserialize, Serialize};

/// Stable entity identifier within one `Sim` instance.
pub type EntityId = u32;

/// Protocol revision for snapshot / WS envelopes (0.1 was implicit rev 1).
/// Rev 3: authenticated Hello (`token` + `character_id`) and inventory slot indices.
/// Rev 4: jump / swim / flight intent + motion snapshot flags.
/// Rev 5: clear_target intent + ability_bar kit slots for combat HUD.
/// Rev 6: pending loot / bank copper / market `mine`. Hello may also carry additive
/// `protocol_rev` / `rewrite_version` identity; omitting them is valid JSON and
/// the server refuses those Hellos (policy, not a wire bump).
/// Rev 7: combo / stealth / stance / absorb snapshot + identity interacts.
/// Rev 8: quest abandon/share, optional turn-in reward choice.
pub const PROTOCOL_REV: u32 = 8;

/// Fixed sim rate matching upstream World of ClaudeCraft.
pub const TICK_RATE: u32 = 20;
pub const DT: f32 = 1.0 / TICK_RATE as f32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Player,
    Mob,
    Npc,
    Loot,
    /// Summoned hunter/warlock companion.
    Pet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilitySlot {
    /// Class primary ability.
    Primary = 1,
    Slot2 = 2,
    Slot3 = 3,
    Slot4 = 4,
    Slot5 = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Feet,
    Neck,
    Finger,
    Finger2,
}

/// Stable profession denial id. Sim never emits English copy for these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfessionDeny {
    OutOfRange,
    NodeNotReady,
    MissingTool,
    ToolTierTooLow,
    InventoryFull,
    UnknownNode,
    Busy,
    CorpseGone,
    NothingToSkin,
    AlreadySkinned,
    MissingKnife,
    UnknownRecipe,
    MissingReagents,
    InsufficientGold,
    StationRequired,
    InvalidCount,
    UnknownEnchant,
    WrongSlot,
    AlreadyEnchanted,
    SameEnchant,
    NotInstanced,
    Dead,
    NotPlayer,
    UnknownProfession,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractAction {
    Talk,
    AcceptQuest {
        quest_id: String,
    },
    TurnInQuest {
        quest_id: String,
        #[serde(default)]
        reward_choice: Option<u32>,
    },
    AbandonQuest {
        quest_id: String,
    },
    ShareQuest {
        quest_id: String,
    },
    Buy {
        item_id: String,
        count: u32,
    },
    Sell {
        bag_slot: u8,
        count: u32,
    },
    Equip {
        bag_slot: u8,
    },
    Unequip {
        equip_slot: EquipSlot,
    },
    /// Use a bag item (consumable heal, etc.). Additive Wave 0.3.
    UseItem {
        bag_slot: u8,
    },
    LootCorpse {
        target_id: EntityId,
    },
    CloseVendor,
    RepairAll,
    Buyback {
        slot: u8,
    },
    TrainClass,
    BindHearth,
    UseHearthstone,
    /// Release spirit while dead and begin the corpse run.
    ReleaseSpirit,
    /// Train a profession by content id.
    TrainProfession {
        id: String,
    },
    /// Gather from a world node.
    Gather {
        node_id: EntityId,
    },
    /// Deposit bag items into the bank.
    BankDeposit {
        bag_slot: u8,
        count: u32,
    },
    /// Withdraw bank items into the bag.
    BankWithdraw {
        bank_slot: u8,
        count: u32,
    },
    /// Deposit copper from wallet into the bank vault.
    BankDepositCopper {
        amount: u32,
    },
    /// Withdraw copper from the bank vault into the wallet.
    BankWithdrawCopper {
        amount: u32,
    },
    /// Summon the class pet (hunter / warlock).
    SummonPet,
    /// Dismiss the active pet.
    DismissPet,
    /// Spend one talent point into a talent id.
    LearnTalent {
        talent_id: String,
    },
    /// Refund talent points (respec).
    RespecTalents,
    /// Craft a recipe by content id.
    Craft {
        recipe_id: String,
    },
    /// Skin a loot pile that still has a hide.
    Skin {
        corpse_id: EntityId,
    },
    /// Disenchant the gear in a bag slot.
    Disenchant {
        bag_slot: u8,
    },
    /// Apply an enchant to gear in a bag slot.
    ApplyEnchant {
        bag_slot: u8,
        enchant_id: String,
        #[serde(default)]
        confirm: bool,
    },
    /// Send mail (copper and/or one bag stack) to a player name.
    MailSend {
        to_name: String,
        copper: u32,
        bag_slot: Option<u8>,
        count: u32,
    },
    /// Collect a mail by id into bag/copper.
    MailCollect {
        mail_id: u32,
    },
    /// Return uncollected mail to sender (postage refund rules in sim).
    MailReturn {
        mail_id: u32,
    },
    /// List a bag stack on the auction house.
    MarketList {
        bag_slot: u8,
        count: u32,
        price: u32,
    },
    /// Buy an auction listing by id.
    MarketBuy {
        listing_id: u32,
    },
    /// Cancel own listing.
    MarketCancel {
        listing_id: u32,
    },
    /// Challenge target player to a duel.
    DuelChallenge,
    /// Accept a pending duel.
    DuelAccept,
    /// Toggle open-world PvP flag.
    TogglePvp,
    /// Travel through a portal / zone transition.
    EnterPortal {
        zone_id: String,
    },
    /// Enter a dungeon instance (party-aware).
    EnterDungeon {
        dungeon_id: String,
    },
    /// Enter a dedicated multi-room solo delve.
    EnterDelve {
        delve_id: String,
    },
    /// Advance the active delve after clearing its current room.
    AdvanceDelve,
    /// Leave the current instance back to the overworld zone.
    LeaveInstance,
    /// Toggle rogue stealth (Z). Other classes toast.
    ToggleStealth,
    /// Cycle warrior stance (F).
    CycleStance,
    /// Toggle shaman/druid form (F).
    ToggleForm,
    /// Need roll on pending party loot.
    LootNeed {
        loot_id: EntityId,
    },
    /// Greed roll on pending party loot.
    LootGreed {
        loot_id: EntityId,
    },
    /// Pass on pending party loot.
    LootPass {
        loot_id: EntityId,
    },
    /// Party leader sets loot mode (`ffa` | `need_greed`).
    SetLootMode {
        mode: String,
    },
}

/// Per-tick intent from a local or remote player.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PlayerIntent {
    /// Forward/back wish in [-1, 1] (camera-relative on the client).
    pub move_x: f32,
    /// Strafe wish in [-1, 1].
    pub move_z: f32,
    /// Desired yaw in radians (world space).
    pub facing: f32,
    /// Start/continue auto-attack against `target_id`.
    pub attack: bool,
    /// Fire ability on this slot (if ready).
    pub ability: Option<AbilitySlot>,
    /// Selected target (mob or none).
    pub target_id: Option<EntityId>,
    /// Jump / swim-hop / fly-ascend (Space).
    #[serde(default)]
    pub jump: bool,
    /// Swim dive / fly descend (Ctrl / C).
    #[serde(default)]
    pub descend: bool,
    /// Toggle travel flight (V just pressed).
    #[serde(default)]
    pub fly_toggle: bool,
    /// Clear current target and stop auto-attack (Esc).
    #[serde(default)]
    pub clear_target: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub kind: EntityKind,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub hp: f32,
    pub hp_max: f32,
    pub level: u32,
    pub name: String,
    /// Class resource (rage/mana/energy); unused for mobs/loot.
    pub resource: f32,
    pub resource_max: f32,
    pub alive: bool,
    #[serde(default)]
    pub template_id: Option<String>,
    /// Feet planted on walkable support (default true for older peers).
    #[serde(default = "default_true")]
    pub on_ground: bool,
    /// Travel-flight mode (no gravity).
    #[serde(default)]
    pub flying: bool,
    /// Treading / submerged in a lake body.
    #[serde(default)]
    pub swimming: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvSlotSnapshot {
    /// Absolute bag/bank slot index (holes allowed). Defaults to 0 for old peers.
    #[serde(default)]
    pub slot: u8,
    pub item_id: String,
    pub count: u32,
    #[serde(default)]
    pub durability: Option<u32>,
    #[serde(default)]
    pub enchant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EquipmentSnapshot {
    pub main_hand: Option<String>,
    pub off_hand: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    pub chest: Option<String>,
    #[serde(default)]
    pub legs: Option<String>,
    #[serde(default)]
    pub feet: Option<String>,
    #[serde(default)]
    pub neck: Option<String>,
    #[serde(default)]
    pub finger: Option<String>,
    #[serde(default)]
    pub finger2: Option<String>,
    #[serde(default)]
    pub main_hand_enchant: Option<String>,
    #[serde(default)]
    pub main_hand_durability: Option<u32>,
    #[serde(default)]
    pub off_hand_durability: Option<u32>,
    #[serde(default)]
    pub head_durability: Option<u32>,
    #[serde(default)]
    pub chest_durability: Option<u32>,
    #[serde(default)]
    pub legs_durability: Option<u32>,
    #[serde(default)]
    pub feet_durability: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestLogEntry {
    pub quest_id: String,
    /// "active" | "ready" | "completed"
    pub state: String,
    pub counts: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorOfferSnapshot {
    pub item_id: String,
    pub count: u32,
    pub price: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorSnapshot {
    pub npc_id: EntityId,
    pub npc_name: String,
    pub stock: Vec<VendorOfferSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuybackSnapshot {
    pub slot: u8,
    pub item_id: String,
    pub count: u32,
    pub price: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcSessionSnapshot {
    pub npc_id: EntityId,
    pub npc_name: String,
    #[serde(default)]
    pub greeting: String,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub stock: Vec<VendorOfferSnapshot>,
    #[serde(default)]
    pub train_professions: Vec<String>,
    #[serde(default)]
    pub can_repair: bool,
    #[serde(default)]
    pub repair_cost: u32,
    #[serde(default)]
    pub can_bind: bool,
    #[serde(default)]
    pub buyback: Vec<BuybackSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProgress {
    pub xp: u32,
    pub xp_to_level: u32,
    pub level: u32,
    pub copper: u32,
    /// Deprecated 0.1 stub; prefer `inventory`.
    #[serde(default)]
    pub bag_item: Option<String>,
    #[serde(default)]
    pub class_id: String,
    #[serde(default)]
    pub resource_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AuraSnapshot {
    pub id: String,
    pub remaining: f32,
    pub stacks: u32,
}

/// One action-bar binding exposed to the client HUD (slots 1–5).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AbilityBarSlot {
    /// Discriminant matching `AbilitySlot` (1=Primary … 5=Slot5).
    pub slot: u8,
    pub ability_id: String,
    pub name: String,
    /// Known at the player's current level.
    pub known: bool,
    /// Ready to fire (known, off CD, GCD free, not casting, affordable).
    pub ready: bool,
    /// Remaining cooldown seconds (0 when ready / unknown).
    pub cooldown: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CastSnapshot {
    /// Ability currently being cast.
    pub ability_id: String,
    /// Cast progress in \[0, 1\].
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSnapshot {
    pub tick: u64,
    pub player_id: EntityId,
    pub entities: Vec<EntitySnapshot>,
    pub progress: PlayerProgress,
    #[serde(default)]
    pub target_id: Option<EntityId>,
    #[serde(default)]
    pub ability_ready: bool,
    #[serde(default)]
    pub ability_cooldown: f32,
    #[serde(default = "default_protocol_rev")]
    pub protocol_rev: u32,
    #[serde(default)]
    pub inventory: Vec<InvSlotSnapshot>,
    #[serde(default)]
    pub equipment: EquipmentSnapshot,
    #[serde(default)]
    pub quest_log: Vec<QuestLogEntry>,
    #[serde(default)]
    pub open_vendor: Option<VendorSnapshot>,
    #[serde(default)]
    pub open_npc: Option<NpcSessionSnapshot>,
    #[serde(default)]
    pub ability_name: String,
    /// Active auras on the local player (Wave 1).
    #[serde(default)]
    pub auras: Vec<AuraSnapshot>,
    /// In-progress cast bar, if any.
    #[serde(default)]
    pub cast: Option<CastSnapshot>,
    /// Class kit action bar (slots 1–5). Empty for older peers.
    #[serde(default)]
    pub ability_bar: Vec<AbilityBarSlot>,
    /// Remaining global cooldown seconds.
    #[serde(default)]
    pub gcd: f32,
    /// True when sticky auto-attack is engaged.
    #[serde(default)]
    pub auto_attack: bool,
    /// True when the local player is dead.
    #[serde(default)]
    pub is_dead: bool,
    /// Party membership, if any.
    #[serde(default)]
    pub party_id: Option<u32>,
    /// Current overworld / instance zone id.
    #[serde(default)]
    pub zone_id: String,
    /// Tick when hearthstone becomes usable again.
    #[serde(default)]
    pub hearth_ready_tick: u64,
    /// Bound hearth destination zone id.
    #[serde(default)]
    pub hearth_zone_id: String,
    /// Unspent talent points.
    #[serde(default)]
    pub talent_points: u32,
    /// Learned talents (id → rank).
    #[serde(default)]
    pub talents: Vec<TalentRankSnapshot>,
    /// Bank slots (empty slots omitted or zero-count).
    #[serde(default)]
    pub bank: Vec<InvSlotSnapshot>,
    /// Waiting mail headers.
    #[serde(default)]
    pub mail: Vec<MailSnapshot>,
    /// Auction listings visible to this player (own + public).
    #[serde(default)]
    pub market: Vec<MarketListingSnapshot>,
    /// PvP honor currency.
    #[serde(default)]
    pub honor: u32,
    /// Open-world PvP flag.
    #[serde(default)]
    pub pvp_flagged: bool,
    /// Profession skill ranks (id → skill).
    #[serde(default)]
    pub professions: Vec<ProfessionSkillSnapshot>,
    /// Party loot mode when in a party (`ffa` | `need_greed`).
    #[serde(default)]
    pub loot_mode: Option<String>,
    /// Pending Need/Greed rolls the local player may still vote on.
    #[serde(default)]
    pub pending_loot: Vec<PendingLootSnapshot>,
    /// Copper stored in the personal bank vault.
    #[serde(default)]
    pub bank_copper: u32,
    /// Rogue combo points (0–5).
    #[serde(default)]
    pub combo_points: u8,
    /// True while the local player is stealthed.
    #[serde(default)]
    pub stealthed: bool,
    /// Warrior stance / form id; empty when none.
    #[serde(default)]
    pub stance_id: String,
    /// Remaining absorb on the local player.
    #[serde(default)]
    pub absorb: f32,
    /// Derived attack power from gear and stats.
    #[serde(default)]
    pub attack_power: f32,
    /// Derived armor from gear and stats.
    #[serde(default)]
    pub armor: f32,
    /// Derived spell power from gear and stats.
    #[serde(default)]
    pub spell_power: f32,
    /// Postage in copper the sim will charge for player-to-player mail.
    #[serde(default)]
    pub mail_postage: u32,
}

/// A party loot roll awaiting Need / Greed / Pass.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PendingLootSnapshot {
    pub loot_id: EntityId,
    pub item_id: String,
    pub copper: u32,
    /// True when the local player already submitted a roll.
    #[serde(default)]
    pub rolled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TalentRankSnapshot {
    pub talent_id: String,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MailSnapshot {
    pub id: u32,
    pub from: String,
    pub subject: String,
    pub copper: u32,
    pub item_id: Option<String>,
    pub item_count: u32,
    #[serde(default)]
    pub durability: Option<u32>,
    #[serde(default)]
    pub enchant_id: Option<String>,
    #[serde(default)]
    pub expires_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MarketListingSnapshot {
    pub id: u32,
    pub seller: String,
    pub item_id: String,
    pub count: u32,
    pub price: u32,
    /// True when this listing belongs to the viewing player.
    #[serde(default)]
    pub mine: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ProfessionSkillSnapshot {
    pub id: String,
    pub skill: u32,
}

fn default_protocol_rev() -> u32 {
    PROTOCOL_REV
}

impl Default for PlayerProgress {
    fn default() -> Self {
        Self {
            xp: 0,
            xp_to_level: 0,
            level: 1,
            copper: 0,
            bag_item: None,
            class_id: String::new(),
            resource_type: String::new(),
        }
    }
}

impl Default for TickSnapshot {
    fn default() -> Self {
        Self {
            tick: 0,
            player_id: 0,
            entities: Vec::new(),
            progress: PlayerProgress::default(),
            target_id: None,
            ability_ready: false,
            ability_cooldown: 0.0,
            protocol_rev: PROTOCOL_REV,
            inventory: Vec::new(),
            equipment: EquipmentSnapshot::default(),
            quest_log: Vec::new(),
            open_vendor: None,
            open_npc: None,
            ability_name: String::new(),
            auras: Vec::new(),
            cast: None,
            ability_bar: Vec::new(),
            gcd: 0.0,
            auto_attack: false,
            is_dead: false,
            party_id: None,
            zone_id: String::new(),
            hearth_ready_tick: 0,
            hearth_zone_id: String::new(),
            talent_points: 0,
            talents: Vec::new(),
            bank: Vec::new(),
            mail: Vec::new(),
            market: Vec::new(),
            honor: 0,
            pvp_flagged: false,
            professions: Vec::new(),
            loot_mode: None,
            pending_loot: Vec::new(),
            bank_copper: 0,
            combo_points: 0,
            stealthed: false,
            stance_id: String::new(),
            absorb: 0.0,
            attack_power: 0.0,
            armor: 0.0,
            spell_power: 0.0,
            mail_postage: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimEvent {
    Damage {
        source: EntityId,
        target: EntityId,
        amount: f32,
        ability: Option<String>,
    },
    Kill {
        killer: EntityId,
        victim: EntityId,
        victim_name: String,
    },
    Loot {
        player: EntityId,
        copper: u32,
        item: Option<String>,
    },
    LevelUp {
        player: EntityId,
        level: u32,
    },
    Toast {
        message: String,
    },
    QuestAccepted {
        player: EntityId,
        quest_id: String,
    },
    QuestProgress {
        player: EntityId,
        quest_id: String,
        objective_index: u32,
        current: u32,
        required: u32,
        text: String,
    },
    QuestCompleted {
        player: EntityId,
        quest_id: String,
    },
    QuestAbandoned {
        player: EntityId,
        quest_id: String,
    },
    ItemGained {
        player: EntityId,
        item_id: String,
        count: u32,
    },
    ItemLost {
        player: EntityId,
        item_id: String,
        count: u32,
    },
    Equipped {
        player: EntityId,
        item_id: String,
        slot: EquipSlot,
    },
    VendorOpen {
        player: EntityId,
        npc_id: EntityId,
    },
    NpcDialog {
        player: EntityId,
        npc_id: EntityId,
        text: String,
    },
    PlayerDied {
        player: EntityId,
    },
    AuraApplied {
        player: EntityId,
        id: String,
        remaining: f32,
        stacks: u32,
    },
    TalentLearned {
        player: EntityId,
        talent_id: String,
        rank: u32,
    },
    TalentRespec {
        player: EntityId,
    },
    LootRoll {
        loot_id: EntityId,
        player: EntityId,
        choice: String,
        roll: u32,
    },
    LootAwarded {
        loot_id: EntityId,
        winner: EntityId,
        item_id: String,
    },
    MailSent {
        from: EntityId,
        to_name: String,
        mail_id: u32,
    },
    MailCollected {
        player: EntityId,
        mail_id: u32,
    },
    Crafted {
        player: EntityId,
        recipe_id: String,
        item_id: String,
        count: u32,
    },
    Gathered {
        player: EntityId,
        node_id: String,
        item_id: String,
        count: u32,
    },
    ProfessionDenied {
        player: EntityId,
        reason: ProfessionDeny,
    },
    Skinned {
        player: EntityId,
        corpse_id: EntityId,
        item_id: String,
        count: u32,
    },
    Disenchanted {
        player: EntityId,
        item_id: String,
    },
    EnchantApplied {
        player: EntityId,
        item_id: String,
        enchant_id: String,
    },
    MarketListed {
        player: EntityId,
        listing_id: u32,
    },
    MarketSold {
        listing_id: u32,
        buyer: EntityId,
        seller_name: String,
    },
    DuelStarted {
        a: EntityId,
        b: EntityId,
    },
    DuelEnded {
        winner: EntityId,
        loser: EntityId,
    },
    HonorGained {
        player: EntityId,
        amount: u32,
    },
    ZoneChanged {
        player: EntityId,
        zone_id: String,
    },
    InstanceEntered {
        player: EntityId,
        dungeon_id: String,
    },
    InstanceLeft {
        player: EntityId,
    },
    DelveRoomCleared {
        player: EntityId,
        delve_id: String,
        room: u32,
    },
    DelveCompleted {
        player: EntityId,
        delve_id: String,
        reward_copper: u32,
        reward_item: Option<String>,
    },
}

/// Host facade shared by offline Bevy and online server.
pub trait WorldHost {
    fn push_intent(&mut self, player_id: EntityId, intent: PlayerIntent);
    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction);
    fn tick_once(&mut self) -> (TickSnapshot, Vec<SimEvent>);
    fn snapshot_for(&self, player_id: EntityId) -> TickSnapshot;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsClientMsg {
    /// Join the realm. Online clients MUST send `token` + `character_id`;
    /// name/class are ignored when those are present (loaded from persist).
    Hello {
        #[serde(default)]
        name: String,
        #[serde(default)]
        class_id: String,
        /// Bearer session token from REST login/register.
        #[serde(default)]
        token: Option<String>,
        /// Durable character UUID (string form).
        #[serde(default)]
        character_id: Option<String>,
        /// Client protocol revision. Missing (old clients) deserializes as `None`.
        #[serde(default)]
        protocol_rev: Option<u32>,
        /// Client rewrite semver. Missing deserializes as `None`.
        #[serde(default)]
        rewrite_version: Option<String>,
    },
    Intent(PlayerIntent),
    Interact {
        target_id: EntityId,
        action: InteractAction,
    },
    PartyInvite {
        name: String,
    },
    PartyAccept,
    PartyLeave,
    Chat {
        channel: String,
        text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMsg {
    Welcome {
        player_id: EntityId,
        protocol_rev: u32,
    },
    Snapshot(Box<TickSnapshot>),
    Events {
        events: Vec<SimEvent>,
    },
    Error {
        message: String,
    },
    PartyUpdate {
        members: Vec<EntityId>,
    },
    Chat {
        channel: String,
        from: String,
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interact_action_roundtrip() {
        let actions = vec![
            InteractAction::Talk,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
            InteractAction::TurnInQuest {
                quest_id: "wolves_at_the_gate".into(),
                reward_choice: None,
            },
            InteractAction::AbandonQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
            InteractAction::ShareQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
            InteractAction::Buy {
                item_id: "travelers_ration".into(),
                count: 1,
            },
            InteractAction::Sell {
                bag_slot: 0,
                count: 1,
            },
            InteractAction::Equip { bag_slot: 0 },
            InteractAction::Unequip {
                equip_slot: EquipSlot::MainHand,
            },
            InteractAction::UseItem { bag_slot: 2 },
            InteractAction::LootCorpse { target_id: 3 },
            InteractAction::CloseVendor,
        ];
        for a in actions {
            let v = serde_json::to_value(&a).unwrap();
            let back: InteractAction = serde_json::from_value(v).unwrap();
            assert_eq!(format!("{back:?}"), format!("{a:?}"));
        }
    }

    #[test]
    fn npc_session_snapshot_defaults_when_omitted() {
        let json = serde_json::json!({
            "tick": 1,
            "player_id": 1,
            "entities": [],
            "progress": {
                "xp": 0, "xp_to_level": 100, "level": 1, "copper": 0
            }
        });
        let snap: TickSnapshot = serde_json::from_value(json).unwrap();
        assert!(snap.open_npc.is_none());
        assert_eq!(snap.hearth_ready_tick, 0);
        assert_eq!(snap.hearth_zone_id, "");
    }

    #[test]
    fn repair_and_hearth_actions_roundtrip() {
        for a in [
            InteractAction::RepairAll,
            InteractAction::Buyback { slot: 0 },
            InteractAction::TrainClass,
            InteractAction::BindHearth,
            InteractAction::UseHearthstone,
        ] {
            let v = serde_json::to_value(&a).unwrap();
            let back: InteractAction = serde_json::from_value(v).unwrap();
            assert_eq!(format!("{back:?}"), format!("{a:?}"));
        }
    }

    #[test]
    fn ws_hello_roundtrip() {
        let msg = WsClientMsg::Hello {
            name: "Ada".into(),
            class_id: "mage".into(),
            token: Some("tok".into()),
            character_id: Some("11111111-1111-1111-1111-111111111111".into()),
            protocol_rev: None,
            rewrite_version: None,
        };
        let s = serde_json::to_string(&msg).unwrap();
        let back: WsClientMsg = serde_json::from_str(&s).unwrap();
        match back {
            WsClientMsg::Hello {
                name,
                class_id,
                token,
                character_id,
                protocol_rev,
                rewrite_version,
            } => {
                assert_eq!(name, "Ada");
                assert_eq!(class_id, "mage");
                assert_eq!(token.as_deref(), Some("tok"));
                assert_eq!(
                    character_id.as_deref(),
                    Some("11111111-1111-1111-1111-111111111111")
                );
                assert!(protocol_rev.is_none());
                assert!(rewrite_version.is_none());
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn ws_hello_identity_roundtrip() {
        let msg = WsClientMsg::Hello {
            name: "Ada".into(),
            class_id: "mage".into(),
            token: Some("tok".into()),
            character_id: Some("11111111-1111-1111-1111-111111111111".into()),
            protocol_rev: Some(6),
            rewrite_version: Some("1.4.0".into()),
        };
        let s = serde_json::to_string(&msg).unwrap();
        assert!(s.contains("protocol_rev"));
        assert!(s.contains("rewrite_version"));
        let back: WsClientMsg = serde_json::from_str(&s).unwrap();
        match back {
            WsClientMsg::Hello {
                protocol_rev,
                rewrite_version,
                ..
            } => {
                assert_eq!(protocol_rev, Some(6));
                assert_eq!(rewrite_version.as_deref(), Some("1.4.0"));
            }
            _ => panic!("expected Hello"),
        }
    }

    fn minimal_tick_json() -> &'static str {
        r#"{
            "tick": 1,
            "player_id": 1,
            "entities": [],
            "progress": {
                "xp": 0,
                "xp_to_level": 100,
                "level": 1,
                "copper": 0
            },
            "target_id": null,
            "ability_ready": true,
            "ability_cooldown": 0.0
        }"#
    }

    #[test]
    fn tick_snapshot_old_json_defaults_new_fields() {
        let snap: TickSnapshot = serde_json::from_str(minimal_tick_json()).unwrap();
        assert!(snap.auras.is_empty());
        assert!(snap.cast.is_none());
        assert!(snap.ability_bar.is_empty());
        assert_eq!(snap.gcd, 0.0);
        assert!(!snap.auto_attack);
        assert!(!snap.is_dead);
        assert!(snap.party_id.is_none());
        assert!(snap.pending_loot.is_empty());
        assert_eq!(snap.bank_copper, 0);
        assert_eq!(snap.combo_points, 0);
        assert!(!snap.stealthed);
        assert!(snap.stance_id.is_empty());
        assert_eq!(snap.absorb, 0.0);
        assert_eq!(snap.protocol_rev, PROTOCOL_REV);
    }

    #[test]
    fn tick_snapshot_death_aura_party_roundtrip() {
        let snap = TickSnapshot {
            tick: 42,
            player_id: 7,
            entities: vec![],
            progress: PlayerProgress {
                xp: 10,
                xp_to_level: 100,
                level: 2,
                copper: 5,
                bag_item: None,
                class_id: "warrior".into(),
                resource_type: "rage".into(),
            },
            target_id: None,
            ability_ready: false,
            ability_cooldown: 0.5,
            protocol_rev: PROTOCOL_REV,
            inventory: vec![],
            equipment: EquipmentSnapshot::default(),
            quest_log: vec![],
            open_vendor: None,
            open_npc: None,
            ability_name: "Strike".into(),
            auras: vec![AuraSnapshot {
                id: "blessing".into(),
                remaining: 12.5,
                stacks: 2,
            }],
            cast: Some(CastSnapshot {
                ability_id: "fireball".into(),
                progress: 0.35,
            }),
            ability_bar: vec![AbilityBarSlot {
                slot: 1,
                ability_id: "heroic_strike".into(),
                name: "Heroic Strike".into(),
                known: true,
                ready: false,
                cooldown: 1.2,
            }],
            gcd: 0.4,
            auto_attack: true,
            is_dead: true,
            party_id: Some(3),
            zone_id: "eastbrook".into(),
            hearth_ready_tick: 0,
            hearth_zone_id: String::new(),
            talent_points: 2,
            talents: vec![TalentRankSnapshot {
                talent_id: "warrior_fury".into(),
                rank: 1,
            }],
            bank: vec![],
            mail: vec![],
            market: vec![],
            honor: 10,
            pvp_flagged: false,
            professions: vec![],
            loot_mode: Some("need_greed".into()),
            pending_loot: vec![PendingLootSnapshot {
                loot_id: 99,
                item_id: "wolf_fang".into(),
                copper: 5,
                rolled: false,
            }],
            bank_copper: 40,
            combo_points: 3,
            stealthed: true,
            stance_id: "battle".into(),
            absorb: 25.0,
            attack_power: 0.0,
            armor: 0.0,
            spell_power: 0.0,
            mail_postage: 0,
        };
        let s = serde_json::to_string(&snap).unwrap();
        let back: TickSnapshot = serde_json::from_str(&s).unwrap();
        assert_eq!(back.auras.len(), 1);
        assert_eq!(back.auras[0].id, "blessing");
        assert!((back.auras[0].remaining - 12.5).abs() < f32::EPSILON);
        assert_eq!(back.auras[0].stacks, 2);
        let cast = back.cast.expect("cast present");
        assert_eq!(cast.ability_id, "fireball");
        assert!((cast.progress - 0.35).abs() < f32::EPSILON);
        assert_eq!(back.ability_bar.len(), 1);
        assert_eq!(back.ability_bar[0].ability_id, "heroic_strike");
        assert!((back.gcd - 0.4).abs() < f32::EPSILON);
        assert!(back.auto_attack);
        assert!(back.is_dead);
        assert_eq!(back.party_id, Some(3));
        assert_eq!(back.zone_id, "eastbrook");
        assert_eq!(back.talent_points, 2);
        assert_eq!(back.honor, 10);
        assert_eq!(back.loot_mode.as_deref(), Some("need_greed"));
        assert_eq!(back.pending_loot.len(), 1);
        assert_eq!(back.bank_copper, 40);
        assert_eq!(back.combo_points, 3);
        assert!(back.stealthed);
        assert_eq!(back.stance_id, "battle");
        assert!((back.absorb - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sim_event_player_died_aura_applied_roundtrip() {
        let events = vec![
            SimEvent::PlayerDied { player: 9 },
            SimEvent::AuraApplied {
                player: 9,
                id: "regen".into(),
                remaining: 8.0,
                stacks: 1,
            },
            SimEvent::ProfessionDenied {
                player: 9,
                reason: ProfessionDeny::MissingTool,
            },
        ];
        for e in events {
            let v = serde_json::to_value(&e).unwrap();
            let back: SimEvent = serde_json::from_value(v).unwrap();
            assert_eq!(back, e);
        }
    }

    #[test]
    fn ability_slot_roundtrip_and_discriminants() {
        assert_eq!(AbilitySlot::Primary as u8, 1);
        assert_eq!(AbilitySlot::Slot2 as u8, 2);
        assert_eq!(AbilitySlot::Slot3 as u8, 3);
        assert_eq!(AbilitySlot::Slot4 as u8, 4);
        assert_eq!(AbilitySlot::Slot5 as u8, 5);
        for slot in [
            AbilitySlot::Primary,
            AbilitySlot::Slot2,
            AbilitySlot::Slot3,
            AbilitySlot::Slot4,
            AbilitySlot::Slot5,
        ] {
            let v = serde_json::to_value(slot).unwrap();
            let back: AbilitySlot = serde_json::from_value(v).unwrap();
            assert_eq!(back, slot);
        }
        // Old JSON still deserializes Primary.
        let old: AbilitySlot = serde_json::from_str("\"Primary\"").unwrap();
        assert_eq!(old, AbilitySlot::Primary);
    }

    #[test]
    fn player_intent_clear_target_defaults_false() {
        let intent: PlayerIntent = serde_json::from_str(
            r#"{"move_x":0.0,"move_z":0.0,"facing":0.0,"attack":false,"ability":null,"target_id":null}"#,
        )
        .unwrap();
        assert!(!intent.clear_target);
        let with = PlayerIntent {
            clear_target: true,
            ..Default::default()
        };
        let s = serde_json::to_string(&with).unwrap();
        let back: PlayerIntent = serde_json::from_str(&s).unwrap();
        assert!(back.clear_target);
    }

    #[test]
    fn party_chat_ws_msg_roundtrip() {
        let client_msgs = vec![
            WsClientMsg::PartyInvite { name: "Bob".into() },
            WsClientMsg::PartyAccept,
            WsClientMsg::PartyLeave,
            WsClientMsg::Chat {
                channel: "say".into(),
                text: "hello".into(),
            },
        ];
        for msg in client_msgs {
            let s = serde_json::to_string(&msg).unwrap();
            let back: WsClientMsg = serde_json::from_str(&s).unwrap();
            assert_eq!(format!("{back:?}"), format!("{msg:?}"));
        }

        let server_msgs = vec![
            WsServerMsg::PartyUpdate {
                members: vec![1, 2, 3],
            },
            WsServerMsg::Chat {
                channel: "say".into(),
                from: "Ada".into(),
                text: "hello".into(),
            },
        ];
        for msg in server_msgs {
            let s = serde_json::to_string(&msg).unwrap();
            let back: WsServerMsg = serde_json::from_str(&s).unwrap();
            assert_eq!(format!("{back:?}"), format!("{msg:?}"));
        }
    }

    #[test]
    fn interact_action_stub_roundtrip() {
        let actions = vec![
            InteractAction::ReleaseSpirit,
            InteractAction::TrainProfession {
                id: "mining".into(),
            },
            InteractAction::Gather { node_id: 42 },
            InteractAction::BankDeposit {
                bag_slot: 1,
                count: 3,
            },
            InteractAction::BankWithdraw {
                bank_slot: 0,
                count: 2,
            },
            InteractAction::BankDepositCopper { amount: 25 },
            InteractAction::BankWithdrawCopper { amount: 10 },
            InteractAction::SummonPet,
            InteractAction::DismissPet,
            InteractAction::LearnTalent {
                talent_id: "warrior_fury".into(),
            },
            InteractAction::RespecTalents,
            InteractAction::Craft {
                recipe_id: "minor_healing_salve".into(),
            },
            InteractAction::Skin { corpse_id: 9 },
            InteractAction::Disenchant { bag_slot: 2 },
            InteractAction::ApplyEnchant {
                bag_slot: 2,
                enchant_id: "weapon_minor_might".into(),
                confirm: true,
            },
            InteractAction::MailSend {
                to_name: "Bob".into(),
                copper: 50,
                bag_slot: Some(1),
                count: 1,
            },
            InteractAction::MailCollect { mail_id: 7 },
            InteractAction::MailReturn { mail_id: 9 },
            InteractAction::MarketList {
                bag_slot: 0,
                count: 1,
                price: 100,
            },
            InteractAction::MarketBuy { listing_id: 3 },
            InteractAction::MarketCancel { listing_id: 3 },
            InteractAction::DuelChallenge,
            InteractAction::DuelAccept,
            InteractAction::TogglePvp,
            InteractAction::EnterPortal {
                zone_id: "eastfen".into(),
            },
            InteractAction::EnterDungeon {
                dungeon_id: "eastbrook_crypt".into(),
            },
            InteractAction::EnterDelve {
                delve_id: "eastbrook_hollow".into(),
            },
            InteractAction::AdvanceDelve,
            InteractAction::LeaveInstance,
            InteractAction::LootNeed { loot_id: 9 },
            InteractAction::LootGreed { loot_id: 9 },
            InteractAction::LootPass { loot_id: 9 },
            InteractAction::SetLootMode {
                mode: "need_greed".into(),
            },
            InteractAction::ToggleStealth,
            InteractAction::CycleStance,
            InteractAction::ToggleForm,
            InteractAction::AbandonQuest {
                quest_id: "wolf_patrol".into(),
            },
            InteractAction::ShareQuest {
                quest_id: "wolf_patrol".into(),
            },
        ];
        for a in actions {
            let v = serde_json::to_value(&a).unwrap();
            let back: InteractAction = serde_json::from_value(v).unwrap();
            assert_eq!(format!("{back:?}"), format!("{a:?}"));
        }
    }

    #[test]
    fn turn_in_quest_reward_choice_defaults_none() {
        let v: InteractAction =
            serde_json::from_str(r#"{"type":"turn_in_quest","quest_id":"x"}"#).unwrap();
        assert_eq!(
            v,
            InteractAction::TurnInQuest {
                quest_id: "x".into(),
                reward_choice: None,
            }
        );
    }

    #[test]
    fn delve_events_roundtrip() {
        let events = [
            SimEvent::DelveRoomCleared {
                player: 7,
                delve_id: "eastbrook_hollow".into(),
                room: 1,
            },
            SimEvent::DelveCompleted {
                player: 7,
                delve_id: "eastbrook_hollow".into(),
                reward_copper: 75,
                reward_item: Some("eastbrook_greaves".into()),
            },
        ];

        for event in events {
            let value = serde_json::to_value(&event).unwrap();
            let back: SimEvent = serde_json::from_value(value).unwrap();
            assert_eq!(back, event);
        }
    }

    #[test]
    fn old_ws_hello_json_still_deserializes() {
        let json = r#"{"type":"hello","name":"Ada","class_id":"mage"}"#;
        let msg: WsClientMsg = serde_json::from_str(json).unwrap();
        match msg {
            WsClientMsg::Hello {
                name,
                class_id,
                token,
                character_id,
                protocol_rev,
                rewrite_version,
            } => {
                assert_eq!(name, "Ada");
                assert_eq!(class_id, "mage");
                assert!(token.is_none());
                assert!(character_id.is_none());
                assert!(protocol_rev.is_none());
                assert!(rewrite_version.is_none());
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn equipment_snapshot_omitted_jewelry_defaults() {
        let eq: EquipmentSnapshot = serde_json::from_str(
            r#"{"main_hand":"worn_sword","off_hand":null,"chest":"recruit_tunic"}"#,
        )
        .unwrap();
        assert_eq!(eq.main_hand.as_deref(), Some("worn_sword"));
        assert!(eq.neck.is_none());
        assert!(eq.finger.is_none());
        assert!(eq.finger2.is_none());
        assert!(eq.main_hand_enchant.is_none());
    }

    #[test]
    fn finger2_and_enchant_defaults() {
        let eq: EquipmentSnapshot =
            serde_json::from_str(r#"{"main_hand":null,"off_hand":null,"chest":null}"#).unwrap();
        assert!(eq.finger2.is_none());
        assert!(eq.main_hand_enchant.is_none());
        let slot: InvSlotSnapshot = serde_json::from_str(r#"{"item_id":"x","count":1}"#).unwrap();
        assert!(slot.enchant_id.is_none());
        assert_eq!(PROTOCOL_REV, 8);
    }

    #[test]
    fn unequip_finger2_roundtrip() {
        let a = InteractAction::Unequip {
            equip_slot: EquipSlot::Finger2,
        };
        let v = serde_json::to_value(&a).unwrap();
        let back: InteractAction = serde_json::from_value(v).unwrap();
        assert!(matches!(
            back,
            InteractAction::Unequip {
                equip_slot: EquipSlot::Finger2
            }
        ));
    }

    #[test]
    fn tick_snapshot_omitted_sheet_stats_default_zero() {
        let snap: TickSnapshot = serde_json::from_str(
            r#"{"tick":0,"player_id":1,"entities":[],"progress":{"xp":0,"xp_to_level":0,"level":1,"copper":0},"target_id":null,"ability_ready":false,"ability_cooldown":0.0}"#,
        )
        .unwrap();
        assert_eq!(snap.attack_power, 0.0);
        assert_eq!(snap.armor, 0.0);
        assert_eq!(snap.spell_power, 0.0);
        assert_eq!(snap.protocol_rev, PROTOCOL_REV);
        assert_eq!(PROTOCOL_REV, 8);
    }

    #[test]
    fn unequip_neck_roundtrip() {
        let a = InteractAction::Unequip {
            equip_slot: EquipSlot::Neck,
        };
        let s = serde_json::to_string(&a).unwrap();
        let back: InteractAction = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            InteractAction::Unequip {
                equip_slot: EquipSlot::Neck
            }
        ));
    }

    #[test]
    fn mail_return_roundtrip() {
        let a = InteractAction::MailReturn { mail_id: 9 };
        let back: InteractAction = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
        assert!(matches!(back, InteractAction::MailReturn { mail_id: 9 }));
    }

    #[test]
    fn mail_snapshot_omitted_instance_fields_default() {
        let mail: MailSnapshot = serde_json::from_str(
            r#"{"id":1,"from":"AH","subject":"Sold","copper":40,"item_count":0}"#,
        )
        .unwrap();
        assert!(mail.durability.is_none());
        assert!(mail.enchant_id.is_none());
        assert_eq!(mail.expires_tick, 0);
    }

    #[test]
    fn tick_snapshot_mail_postage_defaults_zero() {
        let snap: TickSnapshot = serde_json::from_str(minimal_tick_json()).unwrap();
        assert_eq!(snap.mail_postage, 0);
        assert_eq!(PROTOCOL_REV, 8);
    }
}
