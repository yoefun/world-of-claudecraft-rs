//! In-memory persist backend (default when `DATABASE_URL` is unset).

use crate::error::{PersistError, PersistResult};
use crate::models::{Character, CharacterSave, EquipmentDto};
use crate::password::{hash_password, verify_password};
use crate::store::{validate_character_name, validate_username};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Default)]
struct Inner {
    accounts_by_name: HashMap<String, Uuid>,
    accounts: HashMap<Uuid, (String, String)>, // id -> (username, hash)
    sessions: HashMap<String, Uuid>,
    characters: HashMap<Uuid, Character>,
    economy: crate::economy::RealmEconomy,
}

#[derive(Debug, Default)]
pub struct MemoryStore {
    inner: Mutex<Inner>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, username: &str, password: &str) -> PersistResult<(Uuid, String)> {
        validate_username(username)?;
        let username = username.trim().to_string();
        let password_hash = hash_password(password)?;
        let mut g = self.inner.lock().expect("memory store lock");
        if g.accounts_by_name.contains_key(&username) {
            return Err(PersistError::UsernameTaken);
        }
        let id = Uuid::new_v4();
        g.accounts_by_name.insert(username.clone(), id);
        g.accounts.insert(id, (username, password_hash));
        let token = mint_token(&mut g, id);
        Ok((id, token))
    }

    pub async fn login(&self, username: &str, password: &str) -> PersistResult<(Uuid, String)> {
        let username = username.trim().to_string();
        let (id, hash) = {
            let g = self.inner.lock().expect("memory store lock");
            let id = *g
                .accounts_by_name
                .get(&username)
                .ok_or(PersistError::InvalidCredentials)?;
            let (_, hash) = g
                .accounts
                .get(&id)
                .ok_or(PersistError::InvalidCredentials)?;
            (id, hash.clone())
        };
        if !verify_password(password, &hash)? {
            return Err(PersistError::InvalidCredentials);
        }
        let mut g = self.inner.lock().expect("memory store lock");
        let token = mint_token(&mut g, id);
        Ok((id, token))
    }

    pub async fn account_id_for_token(&self, token: &str) -> PersistResult<Uuid> {
        let g = self.inner.lock().expect("memory store lock");
        g.sessions
            .get(token)
            .copied()
            .ok_or(PersistError::Unauthorized)
    }

    pub async fn list_characters(&self, account_id: Uuid) -> PersistResult<Vec<Character>> {
        let g = self.inner.lock().expect("memory store lock");
        let mut out: Vec<_> = g
            .characters
            .values()
            .filter(|c| c.account_id == account_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub async fn create_character(
        &self,
        account_id: Uuid,
        name: &str,
        class_id: &str,
    ) -> PersistResult<Character> {
        validate_character_name(name)?;
        if class_id.trim().is_empty() {
            return Err(PersistError::InvalidInput("class_id required".into()));
        }
        let name = name.trim().to_string();
        let mut g = self.inner.lock().expect("memory store lock");
        if !g.accounts.contains_key(&account_id) {
            return Err(PersistError::Unauthorized);
        }
        if g.characters
            .values()
            .any(|c| c.account_id == account_id && c.name.eq_ignore_ascii_case(&name))
        {
            return Err(PersistError::CharacterNameTaken);
        }
        let character = Character {
            id: Uuid::new_v4(),
            account_id,
            name,
            class_id: class_id.trim().to_string(),
            level: 1,
            xp: 0,
            copper: 0,
            pos_x: 0.0,
            pos_z: 0.0,
            inventory: Vec::new(),
            equipment: EquipmentDto::default(),
            quests: Vec::new(),
            zone_id: "eastbrook".into(),
            talent_points: 0,
            talents: Vec::new(),
            bank: Vec::new(),
            bank_copper: 0,
            honor: 0,
            professions: Vec::new(),
            pvp_flagged: false,
            completed_deeds: Vec::new(),
            hearth_zone_id: "eastbrook".into(),
            hearth_x: 2.0,
            hearth_z: 4.0,
            hearth_ready_tick: 0,
            stance_id: String::new(),
            riding_rank: 0,
            known_mounts: Vec::new(),
            last_mount: String::new(),
        };
        g.characters.insert(character.id, character.clone());
        Ok(character)
    }

    pub async fn get_character(&self, character_id: Uuid) -> PersistResult<Character> {
        let g = self.inner.lock().expect("memory store lock");
        g.characters
            .get(&character_id)
            .cloned()
            .ok_or(PersistError::CharacterNotFound)
    }

    pub async fn delete_character(
        &self,
        account_id: Uuid,
        character_id: Uuid,
    ) -> PersistResult<()> {
        let mut g = self.inner.lock().expect("memory store lock");
        let Some(c) = g.characters.get(&character_id) else {
            return Err(PersistError::CharacterNotFound);
        };
        if c.account_id != account_id {
            return Err(PersistError::Forbidden);
        }
        g.characters.remove(&character_id);
        Ok(())
    }

    pub async fn enter_character(
        &self,
        account_id: Uuid,
        character_id: Uuid,
    ) -> PersistResult<Character> {
        let c = self.get_character(character_id).await?;
        if c.account_id != account_id {
            return Err(PersistError::Forbidden);
        }
        Ok(c)
    }

    pub async fn save_character(
        &self,
        character_id: Uuid,
        save: CharacterSave,
    ) -> PersistResult<Character> {
        let mut g = self.inner.lock().expect("memory store lock");
        let c = g
            .characters
            .get_mut(&character_id)
            .ok_or(PersistError::CharacterNotFound)?;
        c.apply_save(save);
        Ok(c.clone())
    }

    pub async fn save_character_for_account(
        &self,
        account_id: Uuid,
        character_id: Uuid,
        save: CharacterSave,
    ) -> PersistResult<Character> {
        let mut g = self.inner.lock().expect("memory store lock");
        let c = g
            .characters
            .get_mut(&character_id)
            .ok_or(PersistError::CharacterNotFound)?;
        if c.account_id != account_id {
            return Err(PersistError::Forbidden);
        }
        c.apply_save(save);
        Ok(c.clone())
    }

    pub async fn load_economy(&self) -> PersistResult<crate::economy::RealmEconomy> {
        let g = self.inner.lock().expect("memory store lock");
        Ok(g.economy.clone())
    }

    pub async fn save_economy(&self, economy: crate::economy::RealmEconomy) -> PersistResult<()> {
        let mut g = self.inner.lock().expect("memory store lock");
        g.economy = economy;
        Ok(())
    }

    pub async fn find_character_by_name(&self, name: &str) -> PersistResult<Option<Character>> {
        let g = self.inner.lock().expect("memory store lock");
        Ok(g.characters
            .values()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .cloned())
    }
}

fn mint_token(inner: &mut Inner, account_id: Uuid) -> String {
    let token = format!("mem_{}", Uuid::new_v4());
    inner.sessions.insert(token.clone(), account_id);
    token
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{InvStackDto, ProfessionSkillDto, QuestProgressDto, TalentRankDto};

    #[tokio::test]
    async fn register_login_and_character_roundtrip() {
        let store = MemoryStore::new();
        let (aid, token) = store.register("hero_one", "secret1").await.unwrap();
        assert_eq!(store.account_id_for_token(&token).await.unwrap(), aid);

        let (aid2, token2) = store.login("hero_one", "secret1").await.unwrap();
        assert_eq!(aid, aid2);
        assert_eq!(store.account_id_for_token(&token2).await.unwrap(), aid);

        let c = store
            .create_character(aid, "Aldric", "warrior")
            .await
            .unwrap();
        assert_eq!(c.level, 1);

        let save = CharacterSave {
            level: 3,
            xp: 450,
            copper: 12,
            pos_x: 10.5,
            pos_z: -2.0,
            inventory: vec![Some(InvStackDto {
                item_id: "wolf_pelt".into(),
                count: 2,
                durability: None,
                enchant_id: None,
            })],
            equipment: EquipmentDto {
                main_hand: Some("rusty_sword".into()),
                ..Default::default()
            },
            quests: vec![QuestProgressDto {
                quest_id: "wolves_at_the_gate".into(),
                state: "active".into(),
                counts: vec![1],
                completed_tick: 0,
            }],
            zone_id: "eastfen".into(),
            talent_points: 2,
            talents: vec![TalentRankDto {
                talent_id: "shield_mastery".into(),
                rank: 2,
            }],
            bank: vec![Some(InvStackDto {
                item_id: "silverleaf".into(),
                count: 8,
                durability: None,
                enchant_id: None,
            })],
            bank_copper: 0,
            honor: 125,
            professions: vec![ProfessionSkillDto {
                id: "herbalism".into(),
                skill: 42,
            }],
            pvp_flagged: true,
            completed_deeds: vec!["eastfen_mire_terror".into()],
            hearth_zone_id: "eastfen".into(),
            hearth_x: 12.0,
            hearth_z: 34.0,
            hearth_ready_tick: 77,
            stance_id: String::new(),
            riding_rank: 0,
            known_mounts: Vec::new(),
            last_mount: String::new(),
        };
        let saved = store.save_character(c.id, save.clone()).await.unwrap();
        assert_eq!(saved.to_save(), save);

        let entered = store.enter_character(aid, c.id).await.unwrap();
        assert_eq!(entered.xp, 450);
        assert_eq!(store.list_characters(aid).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn duplicate_username_rejected() {
        let store = MemoryStore::new();
        store.register("dup_user", "secret1").await.unwrap();
        let err = store.register("dup_user", "secret1").await.unwrap_err();
        assert!(matches!(err, PersistError::UsernameTaken));
    }
}
