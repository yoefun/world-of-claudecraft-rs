//! Account auth and character save/load for World of ClaudeCraft.
//!
//! # Backends
//!
//! - **Memory** (always available): used when `DATABASE_URL` is unset.
//! - **Postgres** (`feature = "postgres"`, default): used when `DATABASE_URL` is set.
//!
//! # Environment
//!
//! ```text
//! DATABASE_URL=postgres://woc:woc@127.0.0.1:5432/woc
//! ```
//!
//! Migrations live in `migrations/001_init.sql` and are applied on Postgres connect.
//!
//! Unit tests for hashing / JSON serialize always run. Postgres integration tests
//! skip automatically when `DATABASE_URL` is absent.

mod error;
mod memory;
mod models;
mod password;
#[cfg(feature = "postgres")]
pub mod postgres;
mod store;

pub use error::{PersistError, PersistResult};
pub use memory::MemoryStore;
pub use models::{
    equipment_from_json, equipment_to_json, inventory_from_json, inventory_to_json,
    quests_from_json, quests_to_json, Character, CharacterSave, CharacterSummary, EquipmentDto,
    InvStackDto, ProfessionSkillDto, QuestProgressDto, TalentRankDto,
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

    pub async fn load_character(&self, character_id: Uuid) -> PersistResult<Character> {
        self.get_character(character_id).await
    }
}

#[cfg(test)]
mod serialize_tests {
    use super::*;

    #[test]
    fn inventory_equipment_quests_roundtrip() {
        let inv = vec![
            Some(InvStackDto {
                item_id: "bread".into(),
                count: 3,
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
        }];
        let inv_s = inventory_to_json(&inv).unwrap();
        let eq_s = equipment_to_json(&eq).unwrap();
        let q_s = quests_to_json(&quests).unwrap();
        assert_eq!(inventory_from_json(&inv_s).unwrap(), inv);
        assert_eq!(equipment_from_json(&eq_s).unwrap(), eq);
        assert_eq!(quests_from_json(&q_s).unwrap(), quests);
    }
}
