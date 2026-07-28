//! WebSocket game host embedding `woc-sim`.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use woc_content::PlayerClass;
use woc_protocol::{
    EntityId, PlayerIntent, WorldHost, WsClientMsg, WsServerMsg, PROTOCOL_REV, TICK_RATE,
};
use woc_sim::Sim;

struct Realm {
    sim: Sim,
    intents: HashMap<EntityId, PlayerIntent>,
}

impl Realm {
    fn new() -> Self {
        Self {
            sim: Sim::new_eastbrook("World", PlayerClass::Warrior),
            intents: HashMap::new(),
        }
    }
}

struct Shared {
    realm: Mutex<Realm>,
    snapshots: broadcast::Sender<String>,
}

fn shared() -> Arc<Shared> {
    static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let (tx, _) = broadcast::channel(64);
            let shared = Arc::new(Shared {
                realm: Mutex::new(Realm::new()),
                snapshots: tx,
            });
            let tick_shared = shared.clone();
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(Duration::from_secs_f32(1.0 / TICK_RATE as f32));
                loop {
                    interval.tick().await;
                    let mut realm = tick_shared.realm.lock().await;
                    let intent = realm
                        .intents
                        .get(&realm.sim.player_id)
                        .copied()
                        .unwrap_or_default();
                    let (snap, events) = realm.sim.tick(intent);
                    drop(realm);
                    let _ = tick_shared.snapshots.send(
                        serde_json::to_string(&WsServerMsg::Snapshot(Box::new(snap)))
                            .unwrap_or_default(),
                    );
                    if !events.is_empty() {
                        let _ = tick_shared.snapshots.send(
                            serde_json::to_string(&WsServerMsg::Events { events })
                                .unwrap_or_default(),
                        );
                    }
                }
            });
            shared
        })
        .clone()
}

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let shared = shared();
    let (mut sender, mut receiver) = socket.split();
    let mut rx = shared.snapshots.subscribe();

    let mut player_id: Option<EntityId> = None;

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

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
            WsClientMsg::Hello { name, class_id } => {
                let class = PlayerClass::parse(&class_id).unwrap_or(PlayerClass::Warrior);
                let mut realm = shared.realm.lock().await;
                realm.sim = Sim::new_eastbrook(&name, class);
                let pid = realm.sim.player_id;
                realm.intents.insert(pid, PlayerIntent::default());
                player_id = Some(pid);
                let welcome = WsServerMsg::Welcome {
                    player_id: pid,
                    protocol_rev: PROTOCOL_REV,
                };
                let snap = WorldHost::snapshot_for(&realm.sim, pid);
                drop(realm);
                let _ = shared
                    .snapshots
                    .send(serde_json::to_string(&welcome).unwrap_or_default());
                let _ = shared.snapshots.send(
                    serde_json::to_string(&WsServerMsg::Snapshot(Box::new(snap)))
                        .unwrap_or_default(),
                );
            }
            WsClientMsg::Intent(intent) => {
                if let Some(pid) = player_id {
                    let mut realm = shared.realm.lock().await;
                    realm.intents.insert(pid, intent);
                    WorldHost::push_intent(&mut realm.sim, pid, intent);
                }
            }
            WsClientMsg::Interact { target_id, action } => {
                if let Some(pid) = player_id {
                    let mut realm = shared.realm.lock().await;
                    WorldHost::interact(&mut realm.sim, pid, target_id, action);
                }
            }
        }
    }

    send_task.abort();
    if let Some(pid) = player_id {
        let mut realm = shared.realm.lock().await;
        realm.intents.remove(&pid);
    }
}
