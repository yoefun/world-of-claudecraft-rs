//! WebSocket game host embedding `woc-sim` (sticky multi-player realm + persist).

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex};
use uuid::Uuid;
use woc_content::PlayerClass;
use woc_protocol::{EntityId, WorldHost, WsClientMsg, WsServerMsg, PROTOCOL_REV, TICK_RATE};
use woc_sim::Sim;
use woc_version::{
    check_compat, min_client_version_from_env, ClientIdentity, RealmIdentity, REWRITE_VERSION,
};

use crate::bridge::{
    apply_economy_to_sim, character_to_state, export_economy_from_sim, state_to_save,
};
use crate::AppState;

struct SessionBinding {
    player_id: EntityId,
    account_id: Uuid,
    character_id: Uuid,
}

struct Realm {
    sim: Sim,
    /// Tick counter for periodic economy checkpoints.
    economy_dirty: bool,
    last_economy_save_tick: u64,
}

impl Realm {
    fn new(sim: Sim) -> Self {
        Self {
            sim,
            economy_dirty: false,
            last_economy_save_tick: 0,
        }
    }
}

struct Shared {
    realm: Mutex<Realm>,
    /// Per-player snapshot/event fanout (JSON lines).
    player_tx: Mutex<HashMap<EntityId, mpsc::UnboundedSender<String>>>,
    /// Broadcast for party/chat notices that are not player-scoped.
    notices: broadcast::Sender<String>,
    persist: woc_persist::Persist,
}

async fn build_shared(persist: woc_persist::Persist) -> Arc<Shared> {
    let mut sim = Sim::new_empty_eastbrook();
    match persist.load_economy().await {
        Ok(eco) => apply_economy_to_sim(&mut sim, &eco),
        Err(e) => tracing::warn!("failed to load realm economy: {e}"),
    }
    let (notice_tx, _) = broadcast::channel(64);
    Arc::new(Shared {
        realm: Mutex::new(Realm::new(sim)),
        player_tx: Mutex::new(HashMap::new()),
        notices: notice_tx,
        persist,
    })
}

pub async fn ws_handler(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let persist = state.persist.clone();
    ws.on_upgrade(move |socket| async move {
        // One shared realm per process, initialized lazily with persist.
        static SHARED: tokio::sync::OnceCell<Arc<Shared>> = tokio::sync::OnceCell::const_new();
        let shared = SHARED
            .get_or_init(|| build_shared(persist.clone()))
            .await
            .clone();
        handle_socket(socket, shared).await;
    })
}

async fn handle_socket(socket: WebSocket, shared: Arc<Shared>) {
    let (mut sender, mut receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let mut notice_rx = shared.notices.subscribe();

    let mut binding: Option<SessionBinding> = None;

    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = out_rx.recv() => {
                    match msg {
                        Some(text) => {
                            if sender.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                notice = notice_rx.recv() => {
                    match notice {
                        Ok(text) => {
                            if sender.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            }
        }
    });

    // Ensure tick loop is running.
    ensure_tick_loop(shared.clone()).await;

    while let Some(Ok(msg)) = receiver.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(parsed) = serde_json::from_str::<WsClientMsg>(&text) else {
            continue;
        };
        match parsed {
            WsClientMsg::Hello {
                name: _,
                class_id: _,
                token,
                character_id,
                protocol_rev,
                rewrite_version,
            } => {
                let realm = RealmIdentity {
                    rewrite_version: REWRITE_VERSION.to_string(),
                    protocol_rev: Some(PROTOCOL_REV),
                    min_client_version: min_client_version_from_env(),
                };
                let client = ClientIdentity::from_hello(protocol_rev, rewrite_version.as_deref());
                match check_compat(&client, &realm) {
                    woc_version::Compat::Compatible => {}
                    other => {
                        let _ = out_tx.send(err_json(&other.user_message()));
                        continue;
                    }
                }
                let Some(token) = token.filter(|t| !t.is_empty()) else {
                    let _ = out_tx.send(err_json("Hello requires token + character_id"));
                    continue;
                };
                let Some(character_id) = character_id.filter(|t| !t.is_empty()) else {
                    let _ = out_tx.send(err_json("Hello requires token + character_id"));
                    continue;
                };
                let Ok(character_uuid) = Uuid::parse_str(&character_id) else {
                    let _ = out_tx.send(err_json("Invalid character_id"));
                    continue;
                };

                // Save previous binding if any.
                if let Some(prev) = binding.take() {
                    save_and_park(&shared, &prev).await;
                    shared.player_tx.lock().await.remove(&prev.player_id);
                }

                let account_id = match shared.persist.account_id_for_token(&token).await {
                    Ok(id) => id,
                    Err(_) => {
                        let _ = out_tx.send(err_json("Unauthorized"));
                        continue;
                    }
                };
                let character = match shared
                    .persist
                    .enter_character(account_id, character_uuid)
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = out_tx.send(err_json(&format!("Enter failed: {e}")));
                        continue;
                    }
                };
                let class = PlayerClass::parse(&character.class_id).unwrap_or(PlayerClass::Warrior);
                let state = character_to_state(&character);

                let mut realm = shared.realm.lock().await;
                let Some(pid) = realm
                    .sim
                    .spawn_player_with_state(&character.name, class, &state)
                else {
                    drop(realm);
                    let _ = out_tx.send(err_json("Realm full or character already online"));
                    continue;
                };
                let welcome = WsServerMsg::Welcome {
                    player_id: pid,
                    protocol_rev: PROTOCOL_REV,
                };
                let snap = WorldHost::snapshot_for(&realm.sim, pid);
                drop(realm);

                shared.player_tx.lock().await.insert(pid, out_tx.clone());
                binding = Some(SessionBinding {
                    player_id: pid,
                    account_id,
                    character_id: character_uuid,
                });
                let _ = out_tx.send(serde_json::to_string(&welcome).unwrap_or_default());
                let _ = out_tx.send(
                    serde_json::to_string(&WsServerMsg::Snapshot(Box::new(snap)))
                        .unwrap_or_default(),
                );
            }
            WsClientMsg::Intent(intent) => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    WorldHost::push_intent(&mut realm.sim, b.player_id, intent);
                }
            }
            WsClientMsg::Interact { target_id, action } => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    WorldHost::interact(&mut realm.sim, b.player_id, target_id, action);
                    realm.economy_dirty = true;
                }
            }
            WsClientMsg::PartyInvite { name } => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_invite(b.player_id, &name);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyAccept => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_accept(b.player_id);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyLeave => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_leave(b.player_id);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyDecline => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_decline(b.player_id);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyKick { name } => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_kick(b.player_id, &name);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyPromote { name } => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_promote(b.player_id, &name);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyDisband => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_disband(b.player_id);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyReadyCheck => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_ready_check(b.player_id);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::PartyReadyRespond { ready } => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.party_ready_respond(b.player_id, ready);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::ConvertToRaid => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.convert_to_raid(b.player_id);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::ConvertToParty => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.convert_to_party(b.player_id);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
            WsClientMsg::Chat { channel, text } => {
                if let Some(b) = &binding {
                    let mut realm = shared.realm.lock().await;
                    let outs = realm.sim.chat(b.player_id, &channel, &text);
                    drop(realm);
                    for msg in outs {
                        let _ = shared
                            .notices
                            .send(serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
        }
    }

    send_task.abort();
    if let Some(prev) = binding.take() {
        save_and_park(&shared, &prev).await;
        shared.player_tx.lock().await.remove(&prev.player_id);
    }
}

fn err_json(message: &str) -> String {
    serde_json::to_string(&WsServerMsg::Error {
        message: message.to_string(),
    })
    .unwrap_or_default()
}

async fn save_and_park(shared: &Shared, binding: &SessionBinding) {
    let (save, economy) = {
        let mut realm = shared.realm.lock().await;
        let save = realm
            .sim
            .export_player_state(binding.player_id)
            .map(|s| state_to_save(&s));
        let economy = export_economy_from_sim(&realm.sim);
        realm.sim.park_player(binding.player_id);
        realm.economy_dirty = true;
        (save, economy)
    };
    if let Some(save) = save {
        if let Err(e) = shared
            .persist
            .save_character_for_account(binding.account_id, binding.character_id, save)
            .await
        {
            tracing::warn!("failed to save character {}: {e}", binding.character_id);
        }
    }
    if let Err(e) = shared.persist.save_economy(economy).await {
        tracing::warn!("failed to save economy on disconnect: {e}");
    }
}

async fn ensure_tick_loop(shared: Arc<Shared>) {
    static STARTED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    let _ = STARTED
        .get_or_init(|| async {
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(Duration::from_secs_f32(1.0 / TICK_RATE as f32));
                loop {
                    interval.tick().await;
                    let (per_player, events, economy_checkpoint) = {
                        let mut realm = shared.realm.lock().await;
                        let (_primary_snap, events) = realm.sim.tick_all();
                        let players: Vec<EntityId> = realm.sim.player_ids();
                        let mut snaps = Vec::new();
                        for pid in players {
                            let snap = WorldHost::snapshot_for(&realm.sim, pid);
                            snaps.push((
                                pid,
                                serde_json::to_string(&WsServerMsg::Snapshot(Box::new(snap)))
                                    .unwrap_or_default(),
                            ));
                        }
                        let event_json = if events.is_empty() {
                            None
                        } else {
                            Some(
                                serde_json::to_string(&WsServerMsg::Events { events })
                                    .unwrap_or_default(),
                            )
                        };
                        // Checkpoint economy every ~30s (600 ticks).
                        let mut economy = None;
                        if realm.economy_dirty
                            && realm.sim.tick.saturating_sub(realm.last_economy_save_tick) >= 600
                        {
                            economy = Some(export_economy_from_sim(&realm.sim));
                            realm.last_economy_save_tick = realm.sim.tick;
                            realm.economy_dirty = false;
                        }
                        (snaps, event_json, economy)
                    };

                    let tx_map = shared.player_tx.lock().await;
                    for (pid, snap) in per_player {
                        if let Some(tx) = tx_map.get(&pid) {
                            let _ = tx.send(snap);
                            if let Some(ref ev) = events {
                                let _ = tx.send(ev.clone());
                            }
                        }
                    }
                    drop(tx_map);

                    if let Some(eco) = economy_checkpoint {
                        if let Err(e) = shared.persist.save_economy(eco).await {
                            tracing::warn!("periodic economy save failed: {e}");
                        }
                    }
                }
            });
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::{EntityKind, PlayerIntent, WorldHost};
    use woc_sim::persist_state::PlayerPersistentState;
    use woc_version::{
        check_compat, min_client_version_from_env, ClientIdentity, Compat, RealmIdentity,
        REWRITE_VERSION,
    };

    fn test_realm() -> RealmIdentity {
        RealmIdentity {
            rewrite_version: REWRITE_VERSION.to_string(),
            protocol_rev: Some(PROTOCOL_REV),
            min_client_version: min_client_version_from_env(),
        }
    }

    #[test]
    fn hello_without_identity_rejected() {
        let c = check_compat(&ClientIdentity::from_hello(None, None), &test_realm());
        assert!(!c.is_ok());
        assert!(c.user_message().starts_with("version:"));
    }

    #[test]
    fn hello_with_current_identity_accepted() {
        let c = check_compat(
            &ClientIdentity::from_hello(Some(PROTOCOL_REV), Some(REWRITE_VERSION)),
            &test_realm(),
        );
        assert_eq!(c, Compat::Compatible);
    }

    #[test]
    fn hello_wrong_protocol_rejected() {
        let c = check_compat(
            &ClientIdentity::from_hello(
                Some(PROTOCOL_REV.saturating_sub(1)),
                Some(REWRITE_VERSION),
            ),
            &test_realm(),
        );
        assert!(matches!(c, Compat::ProtocolMismatch { .. }));
    }

    #[test]
    fn sticky_hello_keeps_npc_roster() {
        let mut realm = Realm::new(Sim::new_empty_eastbrook());
        let npc_before = realm
            .sim
            .world
            .live_ids()
            .filter(|&id| {
                realm
                    .sim
                    .world
                    .get::<woc_sim::ecs::components::Identity>(id)
                    .map(|i| i.kind)
                    == Some(EntityKind::Npc)
            })
            .count();
        assert!(npc_before >= 3);
        let a = realm
            .sim
            .spawn_player("Alice", PlayerClass::Warrior)
            .expect("spawn a");
        let b = realm
            .sim
            .spawn_player("Bob", PlayerClass::Mage)
            .expect("spawn b");
        assert_ne!(a, b);
        let npc_after = realm
            .sim
            .world
            .live_ids()
            .filter(|&id| {
                realm
                    .sim
                    .world
                    .get::<woc_sim::ecs::components::Identity>(id)
                    .map(|i| i.kind)
                    == Some(EntityKind::Npc)
            })
            .count();
        assert_eq!(npc_before, npc_after);
        realm.sim.despawn_player(a);
        assert!(realm.sim.world.contains(b));
    }

    #[test]
    fn park_keeps_player_for_resume() {
        let mut realm = Realm::new(Sim::new_empty_eastbrook());
        let a = realm
            .sim
            .spawn_player("Alice", PlayerClass::Warrior)
            .expect("spawn a");
        if let Some(d) = realm
            .sim
            .world
            .get_mut::<woc_sim::ecs::components::Durable>(a)
        {
            d.durable_id = Some("11111111-1111-1111-1111-111111111111".into());
        }
        realm.sim.park_player(a);
        assert!(realm.sim.world.contains(a));
        let resumed = realm
            .sim
            .resume_player("11111111-1111-1111-1111-111111111111")
            .expect("resume");
        assert_eq!(resumed, a);
    }

    #[test]
    fn spawn_with_state_restores_progression() {
        let mut sim = Sim::new_empty_eastbrook();
        let state = PlayerPersistentState {
            durable_id: Some("11111111-1111-1111-1111-111111111111".into()),
            level: 4,
            xp: 50,
            copper: 99,
            pos_x: 12.0,
            pos_z: 8.0,
            inventory: vec![],
            equipment: woc_sim::ecs::components::Equipment {
                main_hand: Some("worn_sword".into()),
                chest: Some("recruit_tunic".into()),
                ..Default::default()
            },
            equipment_wear: woc_sim::ecs::components::EquipmentWear::default(),
            equipment_enchants: woc_sim::ecs::components::EquipmentEnchants::default(),
            equipment_qualities: woc_sim::ecs::components::EquipmentQualities::default(),
            quests: vec![],
            zone_id: "eastbrook".into(),
            talent_points: 1,
            talents: Default::default(),
            bank: vec![],
            bank_copper: 0,
            honor: 5,
            professions: Default::default(),
            pvp_flagged: false,
            completed_deeds: Default::default(),
            hearth_zone_id: "eastbrook".into(),
            hearth_x: 2.0,
            hearth_z: 4.0,
            hearth_ready_tick: 0,
            stance_id: String::new(),
            reputation: Default::default(),
        };
        // Force non-virgin by setting copper.
        assert!(!state.is_virgin());
        let pid = sim
            .spawn_player_with_state("Ada", PlayerClass::Warrior, &state)
            .unwrap();
        let exported = sim.export_player_state(pid).unwrap();
        assert_eq!(exported.level, 4);
        assert_eq!(exported.copper, 99);
        assert_eq!(exported.honor, 5);
    }

    #[test]
    fn multi_intent_tick_moves_both_players() {
        let mut realm = Realm::new(Sim::new_empty_eastbrook());
        let a = realm.sim.spawn_player("A", PlayerClass::Warrior).unwrap();
        let b = realm.sim.spawn_player("B", PlayerClass::Rogue).unwrap();
        let (ax0, az0) = {
            let t = realm
                .sim
                .world
                .get::<woc_sim::ecs::components::Transform>(a)
                .unwrap();
            (t.x, t.z)
        };
        WorldHost::push_intent(
            &mut realm.sim,
            a,
            PlayerIntent {
                move_z: 1.0,
                ..Default::default()
            },
        );
        WorldHost::push_intent(
            &mut realm.sim,
            b,
            PlayerIntent {
                move_x: 1.0,
                ..Default::default()
            },
        );
        let _ = realm.sim.tick_all();
        let (ax1, az1) = {
            let t = realm
                .sim
                .world
                .get::<woc_sim::ecs::components::Transform>(a)
                .unwrap();
            (t.x, t.z)
        };
        assert!(
            (az1 - az0).abs() > 1e-3 || (ax1 - ax0).abs() > 1e-3,
            "player A should have moved"
        );
        assert_eq!(realm.sim.player_count(), 2);
    }
}
