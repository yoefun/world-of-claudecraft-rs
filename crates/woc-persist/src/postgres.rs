//! Postgres persist backend (feature = `postgres`).
//!
//! Requires `DATABASE_URL`, e.g. `postgres://woc:woc@127.0.0.1:5432/woc`.

use crate::error::{PersistError, PersistResult};
use crate::models::{
    completion_from_json, completion_to_json, equipment_from_json, equipment_to_json,
    inventory_from_json, inventory_to_json, Character, CharacterSave, EquipmentDto,
};
use crate::password::{hash_password, verify_password};
use crate::store::{validate_character_name, validate_username};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

const MIGRATION_SQL: &str = include_str!("../migrations/001_init.sql");
const MIGRATION_SQL_002: &str = include_str!("../migrations/002_realm_economy.sql");

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    /// Connect and run migrations. Pass the connection URL explicitly.
    pub async fn connect(database_url: &str) -> PersistResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn migrate(&self) -> PersistResult<()> {
        for stmt in split_sql(MIGRATION_SQL)
            .into_iter()
            .chain(split_sql(MIGRATION_SQL_002))
        {
            sqlx::query(&stmt).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn register(&self, username: &str, password: &str) -> PersistResult<(Uuid, String)> {
        validate_username(username)?;
        let username = username.trim().to_string();
        let password_hash = hash_password(password)?;
        let id = Uuid::new_v4();
        let res =
            sqlx::query("INSERT INTO accounts (id, username, password_hash) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(&username)
                .bind(&password_hash)
                .execute(&self.pool)
                .await;
        match res {
            Ok(_) => {}
            Err(sqlx::Error::Database(db)) if db.constraint() == Some("accounts_username_key") => {
                return Err(PersistError::UsernameTaken);
            }
            Err(e) => return Err(e.into()),
        }
        let token = self.mint_session(id).await?;
        Ok((id, token))
    }

    pub async fn login(&self, username: &str, password: &str) -> PersistResult<(Uuid, String)> {
        let username = username.trim().to_string();
        let row = sqlx::query("SELECT id, password_hash FROM accounts WHERE username = $1")
            .bind(&username)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(PersistError::InvalidCredentials)?;
        let id: Uuid = row.get("id");
        let hash: String = row.get("password_hash");
        if !verify_password(password, &hash)? {
            return Err(PersistError::InvalidCredentials);
        }
        let token = self.mint_session(id).await?;
        Ok((id, token))
    }

    pub async fn account_id_for_token(&self, token: &str) -> PersistResult<Uuid> {
        let row = sqlx::query("SELECT account_id FROM sessions WHERE token = $1")
            .bind(token)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(PersistError::Unauthorized)?;
        Ok(row.get("account_id"))
    }

    pub async fn list_characters(&self, account_id: Uuid) -> PersistResult<Vec<Character>> {
        let rows = sqlx::query(
            r#"
            SELECT id, account_id, name, class_id, level, xp, copper, pos_x, pos_z,
                   inventory_json::text, equipment_json::text, quests_json::text
            FROM characters
            WHERE account_id = $1
            ORDER BY name
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(row_to_character).collect()
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
        let class_id = class_id.trim().to_string();
        let id = Uuid::new_v4();
        let inventory = inventory_to_json(&[])?;
        let equipment = equipment_to_json(&EquipmentDto::default())?;
        let completion = completion_to_json(&CharacterSave {
            level: 1,
            ..Default::default()
        })?;
        let res = sqlx::query(
            r#"
            INSERT INTO characters (
                id, account_id, name, class_id, level, xp, copper, pos_x, pos_z,
                inventory_json, equipment_json, quests_json
            ) VALUES (
                $1, $2, $3, $4, 1, 0, 0, 0, 0,
                $5::jsonb, $6::jsonb, $7::jsonb
            )
            "#,
        )
        .bind(id)
        .bind(account_id)
        .bind(&name)
        .bind(&class_id)
        .bind(&inventory)
        .bind(&equipment)
        .bind(&completion)
        .execute(&self.pool)
        .await;
        match res {
            Ok(_) => {}
            Err(sqlx::Error::Database(db))
                if db.constraint() == Some("characters_account_id_name_key") =>
            {
                return Err(PersistError::CharacterNameTaken);
            }
            Err(e) => return Err(e.into()),
        }
        self.get_character(id).await
    }

    pub async fn get_character(&self, character_id: Uuid) -> PersistResult<Character> {
        let row = sqlx::query(
            r#"
            SELECT id, account_id, name, class_id, level, xp, copper, pos_x, pos_z,
                   inventory_json::text, equipment_json::text, quests_json::text
            FROM characters
            WHERE id = $1
            "#,
        )
        .bind(character_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PersistError::CharacterNotFound)?;
        row_to_character(row)
    }

    pub async fn delete_character(
        &self,
        account_id: Uuid,
        character_id: Uuid,
    ) -> PersistResult<()> {
        let res = sqlx::query("DELETE FROM characters WHERE id = $1 AND account_id = $2")
            .bind(character_id)
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            if self.get_character(character_id).await.is_ok() {
                return Err(PersistError::Forbidden);
            }
            return Err(PersistError::CharacterNotFound);
        }
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
        let inventory = inventory_to_json(&save.inventory)?;
        let equipment = equipment_to_json(&save.equipment)?;
        let completion = completion_to_json(&save)?;
        let res = sqlx::query(
            r#"
            UPDATE characters SET
                level = $2,
                xp = $3,
                copper = $4,
                pos_x = $5,
                pos_z = $6,
                inventory_json = $7::jsonb,
                equipment_json = $8::jsonb,
                quests_json = $9::jsonb,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(character_id)
        .bind(save.level as i32)
        .bind(save.xp as i32)
        .bind(save.copper as i32)
        .bind(save.pos_x)
        .bind(save.pos_z)
        .bind(&inventory)
        .bind(&equipment)
        .bind(&completion)
        .execute(&self.pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(PersistError::CharacterNotFound);
        }
        self.get_character(character_id).await
    }

    pub async fn save_character_for_account(
        &self,
        account_id: Uuid,
        character_id: Uuid,
        save: CharacterSave,
    ) -> PersistResult<Character> {
        let c = self.get_character(character_id).await?;
        if c.account_id != account_id {
            return Err(PersistError::Forbidden);
        }
        self.save_character(character_id, save).await
    }

    pub async fn load_economy(&self) -> PersistResult<crate::economy::RealmEconomy> {
        let row = sqlx::query("SELECT data::text AS data FROM realm_economy WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => {
                let data: String = r.get("data");
                Ok(crate::economy::economy_from_json(&data)?)
            }
            None => Ok(crate::economy::RealmEconomy::default()),
        }
    }

    pub async fn save_economy(&self, economy: crate::economy::RealmEconomy) -> PersistResult<()> {
        let data = crate::economy::economy_to_json(&economy)?;
        sqlx::query(
            r#"
            INSERT INTO realm_economy (id, data) VALUES (1, $1::jsonb)
            ON CONFLICT (id) DO UPDATE SET data = EXCLUDED.data, updated_at = NOW()
            "#,
        )
        .bind(&data)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mint_session(&self, account_id: Uuid) -> PersistResult<String> {
        let token = format!("pg_{}", Uuid::new_v4());
        sqlx::query("INSERT INTO sessions (token, account_id) VALUES ($1, $2)")
            .bind(&token)
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(token)
    }
}

fn row_to_character(row: sqlx::postgres::PgRow) -> PersistResult<Character> {
    let inventory_json: String = row.get("inventory_json");
    let equipment_json: String = row.get("equipment_json");
    let completion_json: String = row.get("quests_json");
    let completion = completion_from_json(&completion_json)?;
    Ok(Character {
        id: row.get("id"),
        account_id: row.get("account_id"),
        name: row.get("name"),
        class_id: row.get("class_id"),
        level: row.get::<i32, _>("level") as u32,
        xp: row.get::<i32, _>("xp") as u32,
        copper: row.get::<i32, _>("copper") as u32,
        pos_x: row.get("pos_x"),
        pos_z: row.get("pos_z"),
        inventory: inventory_from_json(&inventory_json)?,
        equipment: equipment_from_json(&equipment_json)?,
        quests: completion.quests,
        zone_id: completion.zone_id,
        talent_points: completion.talent_points,
        talents: completion.talents,
        bank: completion.bank,
        bank_copper: completion.bank_copper,
        honor: completion.honor,
        professions: completion.professions,
        pvp_flagged: completion.pvp_flagged,
        completed_deeds: completion.completed_deeds,
    })
}

fn split_sql(sql: &str) -> Vec<String> {
    sql.split(';')
        .filter_map(|chunk| {
            let without_line_comments: String = chunk
                .lines()
                .filter(|line| !line.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");
            let trimmed = without_line_comments.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .collect()
}

/// Run Postgres integration tests only when `DATABASE_URL` is set.
#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    use crate::models::{InvStackDto, ProfessionSkillDto, TalentRankDto};

    fn database_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty())
    }

    #[tokio::test]
    async fn postgres_register_and_save_when_db_present() {
        let Some(url) = database_url() else {
            eprintln!("skipping postgres test: DATABASE_URL unset");
            return;
        };
        let store = PostgresStore::connect(&url).await.expect("connect+migrate");
        let user = format!("t_{}", &Uuid::new_v4().to_string()[..8]);
        let (aid, token) = store.register(&user, "secret1").await.expect("register");
        assert_eq!(store.account_id_for_token(&token).await.unwrap(), aid);
        let c = store
            .create_character(aid, "TestChar", "mage")
            .await
            .unwrap();
        let save = CharacterSave {
            level: 2,
            xp: 100,
            copper: 5,
            pos_x: 1.0,
            pos_z: 2.0,
            zone_id: "eastfen".into(),
            talent_points: 2,
            talents: vec![TalentRankDto {
                talent_id: "arcane_focus".into(),
                rank: 2,
            }],
            bank: vec![Some(InvStackDto {
                item_id: "silverleaf".into(),
                count: 8,
            })],
            bank_copper: 0,
            honor: 125,
            professions: vec![ProfessionSkillDto {
                id: "herbalism".into(),
                skill: 42,
            }],
            pvp_flagged: true,
            ..Default::default()
        };
        let saved = store.save_character(c.id, save.clone()).await.unwrap();
        assert_eq!(saved.to_save(), save);
        store.delete_character(aid, c.id).await.unwrap();
    }
}
