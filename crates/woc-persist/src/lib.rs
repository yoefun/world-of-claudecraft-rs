//! Account auth and character save/load for World of ClaudeCraft.
//!
//! # Backends
//!
//! - **Memory** (always available): zero-config **dev default** when `DATABASE_URL`
//!   is unset. Accounts, characters, mail, and auction live only in this process.
//! - **Postgres** (`feature = "postgres"`, default): the **durable production
//!   path**. Used when `DATABASE_URL` is set to a non-empty Postgres URL.
//!
//! # Environment
//!
//! ```text
//! DATABASE_URL=postgres://woc:woc@127.0.0.1:5432/woc
//! ```
//!
//! `Persist::from_env()` picks Postgres when that variable is set, otherwise
//! memory. Migrations live in `crates/woc-persist/migrations/` (`001_init.sql`,
//! `002_realm_economy.sql`) and are applied on Postgres connect.
//!
//! Unit tests for hashing / JSON serialize always run. Postgres integration tests
//! skip automatically when `DATABASE_URL` is absent.

mod economy;
mod error;
mod memory;
mod models;
mod password;
#[cfg(feature = "postgres")]
pub mod postgres;
mod store;

pub use economy::{GuildDto, GuildMemberDto, MailDto, MarketListingDto, RealmEconomy};
pub use error::{PersistError, PersistResult};
pub use memory::MemoryStore;
pub use models::{
    equipment_from_json, equipment_to_json, inventory_from_json, inventory_to_json,
    quests_from_json, quests_to_json, Character, CharacterSave, CharacterSummary, EquipmentDto,
    InvStackDto, ProfessionSkillDto, QuestProgressDto, ReputationDto, TalentRankDto,
};
pub use password::{hash_password, verify_password};
pub use store::{validate_character_name, validate_username};

use models::CharacterSave as Save;
use uuid::Uuid;

/// Facade over memory or Postgres backends.
#[derive(Clone)]
pub enum Persist {
    Memory(std::sync::Arc<MemoryStore>),
    #[cfg(feature = "postgres")]
    Postgres(postgres::PostgresStore),
}

impl Persist {
    /// Open memory store, or Postgres when `DATABASE_URL` is set (postgres feature).
    pub async fn from_env() -> PersistResult<Self> {
        #[cfg(feature = "postgres")]
        {
            if let Ok(url) = std::env::var("DATABASE_URL") {
                if !url.is_empty() {
                    tracing::info!("woc-persist: using Postgres");
                    let store = postgres::PostgresStore::connect(&url).await?;
                    return Ok(Self::Postgres(store));
                }
            }
        }
        tracing::info!("woc-persist: using in-memory store (set DATABASE_URL for Postgres)");
        Ok(Self::Memory(std::sync::Arc::new(MemoryStore::new())))
    }

    pub fn memory() -> Self {
        Self::Memory(std::sync::Arc::new(MemoryStore::new()))
    }

    pub async fn register(&self, username: &str, password: &str) -> PersistResult<(Uuid, String)> {
        match self {
            Self::Memory(s) => s.register(username, password).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.register(username, password).await,
        }
    }

    pub async fn login(&self, username: &str, password: &str) -> PersistResult<(Uuid, String)> {
        match self {
            Self::Memory(s) => s.login(username, password).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.login(username, password).await,
        }
    }

    pub async fn account_id_for_token(&self, token: &str) -> PersistResult<Uuid> {
        match self {
            Self::Memory(s) => s.account_id_for_token(token).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.account_id_for_token(token).await,
        }
    }

    pub async fn list_characters(&self, account_id: Uuid) -> PersistResult<Vec<Character>> {
        match self {
            Self::Memory(s) => s.list_characters(account_id).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.list_characters(account_id).await,
        }
    }

    pub async fn create_character(
        &self,
        account_id: Uuid,
        name: &str,
        class_id: &str,
    ) -> PersistResult<Character> {
        match self {
            Self::Memory(s) => s.create_character(account_id, name, class_id).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.create_character(account_id, name, class_id).await,
        }
    }

    pub async fn get_character(&self, character_id: Uuid) -> PersistResult<Character> {
        match self {
            Self::Memory(s) => s.get_character(character_id).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.get_character(character_id).await,
        }
    }

    pub async fn delete_character(
        &self,
        account_id: Uuid,
        character_id: Uuid,
    ) -> PersistResult<()> {
        match self {
            Self::Memory(s) => s.delete_character(account_id, character_id).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.delete_character(account_id, character_id).await,
        }
    }

    pub async fn enter_character(
        &self,
        account_id: Uuid,
        character_id: Uuid,
    ) -> PersistResult<Character> {
        match self {
            Self::Memory(s) => s.enter_character(account_id, character_id).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.enter_character(account_id, character_id).await,
        }
    }

    /// Save helper used by the server tick / disconnect path.
    pub async fn save_character(&self, character_id: Uuid, save: Save) -> PersistResult<Character> {
        match self {
            Self::Memory(s) => s.save_character(character_id, save).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.save_character(character_id, save).await,
        }
    }

    pub async fn save_character_for_account(
        &self,
        account_id: Uuid,
        character_id: Uuid,
        save: Save,
    ) -> PersistResult<Character> {
        match self {
            Self::Memory(s) => {
                s.save_character_for_account(account_id, character_id, save)
                    .await
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                s.save_character_for_account(account_id, character_id, save)
                    .await
            }
        }
    }

    pub async fn load_character(&self, character_id: Uuid) -> PersistResult<Character> {
        self.get_character(character_id).await
    }

    pub async fn load_economy(&self) -> PersistResult<RealmEconomy> {
        match self {
            Self::Memory(s) => s.load_economy().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_economy().await,
        }
    }

    pub async fn save_economy(&self, economy: RealmEconomy) -> PersistResult<()> {
        match self {
            Self::Memory(s) => s.save_economy(economy).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.save_economy(economy).await,
        }
    }
}

#[cfg(test)]
mod serialize_tests {
    use super::*;

    #[test]
    fn equipment_dto_omitted_jewelry_defaults() {
        let eq: EquipmentDto = serde_json::from_str(r#"{"main_hand":"worn_sword"}"#).unwrap();
        assert_eq!(eq.main_hand.as_deref(), Some("worn_sword"));
        assert!(eq.neck.is_none());
        assert!(eq.finger.is_none());
        assert!(eq.finger2.is_none());
        assert!(eq.main_hand_enchant.is_none());
        assert!(eq.off_hand_enchant.is_none());
        assert!(eq.back.is_none());
    }

    #[test]
    fn inventory_equipment_quests_roundtrip() {
        let inv = vec![
            Some(InvStackDto {
                item_id: "bread".into(),
                count: 3,
                durability: None,
                enchant_id: None,
                quality: None,
                bound: false,
            }),
            None,
        ];
        let eq = EquipmentDto {
            chest: Some("cloth_tunic".into()),
            ..Default::default()
        };
        let quests = vec![QuestProgressDto {
            quest_id: "q1".into(),
            state: "ready".into(),
            counts: vec![2, 0],
            completed_tick: 0,
        }];
        let inv_s = inventory_to_json(&inv).unwrap();
        let eq_s = equipment_to_json(&eq).unwrap();
        let q_s = quests_to_json(&quests).unwrap();
        assert_eq!(inventory_from_json(&inv_s).unwrap(), inv);
        assert_eq!(equipment_from_json(&eq_s).unwrap(), eq);
        assert_eq!(quests_from_json(&q_s).unwrap(), quests);
    }
}
