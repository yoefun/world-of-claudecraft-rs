//! Party roster and chat channels (say / party).

pub mod chat;
pub mod party;

pub use chat::{handle_chat, ChatEffect};
pub use party::{
    kill_credit_share, PartyEffect, PartyRoster, MAX_PARTY_SIZE, MIN_PARTY_SIZE,
};
