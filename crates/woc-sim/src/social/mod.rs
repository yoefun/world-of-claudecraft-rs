//! Party roster and chat channels (say / party / raid).

pub mod chat;
pub mod friends;
pub mod guild;
pub mod loot;
pub mod party;

pub use chat::{handle_chat, ChatEffect};
pub use friends::{FriendEntry, FriendRoster, SocialBook, SocialDelivery, SocialEffect};
pub use guild::{GuildDelivery, GuildEffect, GuildRank, GuildRoster};
pub use loot::{LootMode, LootRules, RollChoice};
pub use party::{
    group_xp, kill_credit_share, GroupKind, PartyEffect, PartyRoster, INVITE_TTL_TICKS,
    MAX_PARTY_SIZE, MAX_RAID_SIZE, MIN_PARTY_SIZE, READY_CHECK_TTL_TICKS,
};
