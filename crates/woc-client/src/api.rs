//! Blocking REST client for auth / characters (`http://127.0.0.1:8787/api/*`).
//!
//! HTTP runs on a dedicated OS thread (sync [`ureq`]) so Bevy’s update loop stays free.
//! Results come back over [`std::sync::mpsc`].

use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// HTTP API base (same host as the WS game server).
pub const API_BASE: &str = "http://127.0.0.1:8787";

#[derive(Debug, Clone, Serialize)]
struct AuthRequest {
    username: String,
    password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub account_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
struct CreateCharacterRequest {
    name: String,
    class_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CharacterSummary {
    pub id: Uuid,
    pub name: String,
    pub class_id: String,
    pub level: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct CharacterListResponse {
    characters: Vec<CharacterSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Character {
    pub id: Uuid,
    pub name: String,
    pub class_id: String,
    pub level: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct EnterResponse {
    character: Character,
}

#[derive(Debug, Clone, Deserialize)]
struct ErrorBody {
    error: String,
}

/// Outcome of an auth call (register or login).
#[derive(Debug, Clone)]
pub enum AuthResult {
    Ok(AuthResponse),
    Err(String),
}

/// Outcome of listing characters.
#[derive(Debug, Clone)]
pub enum ListResult {
    Ok(Vec<CharacterSummary>),
    Err(String),
}

/// Outcome of create / enter character.
#[derive(Debug, Clone)]
pub enum CharacterResult {
    Ok(Character),
    Err(String),
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(3))
        .timeout_read(Duration::from_secs(8))
        .build()
}

fn read_error(resp: ureq::Response) -> String {
    let status = resp.status();
    match resp.into_json::<ErrorBody>() {
        Ok(body) if !body.error.is_empty() => format!("HTTP {status}: {}", body.error),
        _ => format!("HTTP {status}"),
    }
}

fn post_auth(path: &str, username: String, password: String) -> AuthResult {
    let url = format!("{API_BASE}{path}");
    let body = AuthRequest { username, password };
    match agent().post(&url).send_json(ureq::json!(body)) {
        Ok(resp) => match resp.into_json::<AuthResponse>() {
            Ok(auth) => AuthResult::Ok(auth),
            Err(e) => AuthResult::Err(format!("bad auth response: {e}")),
        },
        Err(ureq::Error::Status(code, resp)) => {
            AuthResult::Err(format!("{} ({})", read_error(resp), code))
        }
        Err(e) => AuthResult::Err(format!("request failed: {e}")),
    }
}

fn list_characters_blocking(token: &str) -> ListResult {
    let url = format!("{API_BASE}/api/characters");
    match agent()
        .get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .call()
    {
        Ok(resp) => match resp.into_json::<CharacterListResponse>() {
            Ok(list) => ListResult::Ok(list.characters),
            Err(e) => ListResult::Err(format!("bad character list: {e}")),
        },
        Err(ureq::Error::Status(_, resp)) => ListResult::Err(read_error(resp)),
        Err(e) => ListResult::Err(format!("request failed: {e}")),
    }
}

fn create_character_blocking(token: &str, name: String, class_id: String) -> CharacterResult {
    let url = format!("{API_BASE}/api/characters");
    let body = CreateCharacterRequest { name, class_id };
    match agent()
        .post(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(ureq::json!(body))
    {
        Ok(resp) => match resp.into_json::<Character>() {
            Ok(c) => CharacterResult::Ok(c),
            Err(e) => CharacterResult::Err(format!("bad create response: {e}")),
        },
        Err(ureq::Error::Status(_, resp)) => CharacterResult::Err(read_error(resp)),
        Err(e) => CharacterResult::Err(format!("request failed: {e}")),
    }
}

fn enter_character_blocking(token: &str, id: Uuid) -> CharacterResult {
    let url = format!("{API_BASE}/api/characters/{id}/enter");
    match agent()
        .post(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .call()
    {
        Ok(resp) => match resp.into_json::<EnterResponse>() {
            Ok(enter) => CharacterResult::Ok(enter.character),
            Err(e) => CharacterResult::Err(format!("bad enter response: {e}")),
        },
        Err(ureq::Error::Status(_, resp)) => CharacterResult::Err(read_error(resp)),
        Err(e) => CharacterResult::Err(format!("request failed: {e}")),
    }
}

/// Spawn a thread that registers a new account.
pub fn spawn_register(username: String, password: String) -> Receiver<AuthResult> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("woc-api-register".into())
        .spawn(move || {
            let _ = tx.send(post_auth("/api/register", username, password));
        })
        .expect("spawn api register");
    rx
}

/// Spawn a thread that logs in.
pub fn spawn_login(username: String, password: String) -> Receiver<AuthResult> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("woc-api-login".into())
        .spawn(move || {
            let _ = tx.send(post_auth("/api/login", username, password));
        })
        .expect("spawn api login");
    rx
}

/// Spawn a thread that lists characters for `token`.
pub fn spawn_list_characters(token: String) -> Receiver<ListResult> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("woc-api-list".into())
        .spawn(move || {
            let _ = tx.send(list_characters_blocking(&token));
        })
        .expect("spawn api list");
    rx
}

/// Spawn a thread that creates a character.
pub fn spawn_create_character(
    token: String,
    name: String,
    class_id: String,
) -> Receiver<CharacterResult> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("woc-api-create".into())
        .spawn(move || {
            let _ = tx.send(create_character_blocking(&token, name, class_id));
        })
        .expect("spawn api create");
    rx
}

/// Spawn a thread that enters a character (loads save for session).
pub fn spawn_enter_character(token: String, id: Uuid) -> Receiver<CharacterResult> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("woc-api-enter".into())
        .spawn(move || {
            let _ = tx.send(enter_character_blocking(&token, id));
        })
        .expect("spawn api enter");
    rx
}

fn delete_character_blocking(token: &str, id: Uuid) -> Result<(), String> {
    let url = format!("{API_BASE}/api/characters/{id}");
    match agent()
        .delete(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .call()
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(_, resp)) => Err(read_error(resp)),
        Err(e) => Err(format!("request failed: {e}")),
    }
}

/// Spawn a thread that deletes a character.
pub fn spawn_delete_character(token: String, id: Uuid) -> Receiver<Result<(), String>> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("woc-api-delete".into())
        .spawn(move || {
            let _ = tx.send(delete_character_blocking(&token, id));
        })
        .expect("spawn api delete");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_response_deserializes() {
        let json = r#"{"token":"abc","account_id":"11111111-1111-1111-1111-111111111111"}"#;
        let auth: AuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(auth.token, "abc");
    }

    #[test]
    fn character_list_deserializes() {
        let json = r#"{
            "characters": [
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "name": "Aldric",
                    "class_id": "warrior",
                    "level": 1
                }
            ]
        }"#;
        let list: CharacterListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(list.characters.len(), 1);
        assert_eq!(list.characters[0].name, "Aldric");
    }
}
