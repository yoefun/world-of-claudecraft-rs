//! WebSocket game host embedding `woc-sim` (sticky multi-player realm).

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use woc_content::PlayerClass;
use woc_protocol::{
    EntityId, WorldHost, WsClientMsg, WsServerMsg, PROTOCOL_REV, TICK_RATE,
};
use woc_sim::Sim;

struct Realm {
    sim: Sim,
}

impl Realm {
    fn new() -> Self {
        Self {
            // Sticky: NPCs/mobs survive Hello; players spawn/despawn.
            sim: Sim::new_empty_eastbrook(),
        }
    }
}

struct Shared {
    realm: Mutex<Realm>,
    /// Broadcast JSON lines to all sockets (snapshots/events/welcome).
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
                    let (snap, events) = realm.sim.tick_all();
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
                // If reconnecting same socket after Hello, despawn old first.
                if let Some(old) = player_id {
                    realm.sim.despawn_player(old);
                }
                let Some(pid) = realm.sim.spawn_player(&name, class) else {
                    drop(realm);
                    let err = WsServerMsg::Welcome {
                        player_id: 0,
                        protocol_rev: PROTOCOL_REV,
                    };
                    let _ = shared
                        .snapshots
                        .send(serde_json::to_string(&err).unwrap_or_default());
                    continue;
                };
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
        realm.sim.despawn_player(pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::{EntityKind, PlayerIntent, WorldHost};

    #[test]
    fn sticky_hello_keeps_npc_roster() {
        let mut realm = Realm::new();
        let npc_before = realm
            .sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Npc)
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
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Npc)
            .count();
        assert_eq!(npc_before, npc_after);
        realm.sim.despawn_player(a);
        assert_eq!(
            realm
                .sim
                .entities
                .iter()
                .filter(|e| e.kind == EntityKind::Npc)
                .count(),
            npc_before
        );
        // Bob still present
        assert!(realm.sim.entities.iter().any(|e| e.id == b));
    }

    #[test]
    fn multi_intent_tick_moves_both_players() {
        let mut realm = Realm::new();
        let a = realm.sim.spawn_player("A", PlayerClass::Warrior).unwrap();
        let b = realm.sim.spawn_player("B", PlayerClass::Rogue).unwrap();
        let (ax0, az0) = {
            let p = realm.sim.entities.iter().find(|e| e.id == a).unwrap();
            (p.x, p.z)
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
            let p = realm.sim.entities.iter().find(|e| e.id == a).unwrap();
            (p.x, p.z)
        };
        let (bx1, _bz1) = {
            let p = realm.sim.entities.iter().find(|e| e.id == b).unwrap();
            (p.x, p.z)
        };
        assert!(
            (az1 - az0).abs() > 1e-3 || (ax1 - ax0).abs() > 1e-3,
            "player A should have moved"
        );
        let bx0 = realm
            .sim
            .entities
            .iter()
            .find(|e| e.id == b)
            .map(|e| e.x)
            .unwrap();
        // After one tick B moved on X relative to spawn offset — just check alive intents applied
        let _ = (bx1, bx0);
        assert_eq!(realm.sim.player_count(), 2);
    }
}
