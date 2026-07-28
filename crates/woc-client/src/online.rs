//! Online WebSocket host for the Bevy client.
//!
//! # Architecture
//!
//! Bevy 0.16 does not own an async runtime suitable for `tokio-tungstenite` without
//! fighting the render/update loop. This MVP drives a **dedicated OS thread** with
//! sync [`tungstenite`] and bridges traffic through [`std::sync::mpsc`] channels:
//!
//! - Bevy → net: `WsClientMsg` (`Hello`, `Intent`, `Interact`)
//! - Net → Bevy: `WsServerMsg` (`Welcome`, `Snapshot`, `Events`, `Error`)
//!
//! The socket uses a short read timeout so the thread can also drain outbound intents
//! every ~16 ms without blocking the game forever.
//!
//! # How to run (server + online client)
//!
//! ```bash
//! # Terminal A — authoritative realm (auth + characters + WS)
//! cargo run -p woc-server
//!
//! # Terminal B — Bevy client in Online mode
//! cargo run -p woc-client
//! # Title: 2 Online → Login/Register → Character Select → Enter world (WS Hello)
//!
//! # Optional Terminal C — second client (same Online path) to see co-presence
//! cargo run -p woc-client
//! ```
//!
//! REST base: `http://127.0.0.1:8787/api/*`.  
//! Default WS URL: [`ONLINE_WS_URL`] (`ws://127.0.0.1:8787/ws/game`).

use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};
use woc_protocol::{WsClientMsg, WsServerMsg};

/// Configurable WebSocket endpoint for online play.
pub const ONLINE_WS_URL: &str = "ws://127.0.0.1:8787/ws/game";

/// Messages the Bevy side pushes to the net thread.
pub type OutboundTx = Sender<WsClientMsg>;
/// Messages the net thread pushes to Bevy.
pub type InboundRx = Receiver<WsServerMsg>;

/// Spawn the WebSocket IO thread and return channels for Bevy.
///
/// Immediately queues an authenticated `Hello` for the selected character.
pub fn spawn_online_session(
    token: String,
    character_id: uuid::Uuid,
) -> (OutboundTx, InboundRx, thread::JoinHandle<()>) {
    let (to_net_tx, to_net_rx) = mpsc::channel::<WsClientMsg>();
    let (from_net_tx, from_net_rx) = mpsc::channel::<WsServerMsg>();

    let hello = WsClientMsg::Hello {
        name: String::new(),
        class_id: String::new(),
        token: Some(token),
        character_id: Some(character_id.to_string()),
    };
    let _ = to_net_tx.send(hello);

    let handle = thread::Builder::new()
        .name("woc-ws".into())
        .spawn(move || ws_thread_main(ONLINE_WS_URL, to_net_rx, from_net_tx))
        .expect("spawn woc-ws thread");

    (to_net_tx, from_net_rx, handle)
}

fn ws_thread_main(url_str: &str, inbound: Receiver<WsClientMsg>, outbound: Sender<WsServerMsg>) {
    let (mut ws, _resp) = match connect(url_str) {
        Ok(pair) => pair,
        Err(e) => {
            let _ = outbound.send(WsServerMsg::Error {
                message: format!("connect failed ({url_str}): {e}"),
            });
            return;
        }
    };

    set_read_timeout(&mut ws, Duration::from_millis(16));

    loop {
        // Drain Bevy → server.
        loop {
            match inbound.try_recv() {
                Ok(msg) => {
                    let Ok(json) = serde_json::to_string(&msg) else {
                        continue;
                    };
                    if ws.send(Message::Text(json.into())).is_err() {
                        let _ = outbound.send(WsServerMsg::Error {
                            message: "send failed; socket closed".into(),
                        });
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    return;
                }
            }
        }

        // Poll server → Bevy.
        match ws.read() {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<WsServerMsg>(text.as_ref()) {
                    Ok(msg) => {
                        if outbound.send(msg).is_err() {
                            let _ = ws.close(None);
                            return;
                        }
                    }
                    Err(_) => {
                        // Ignore unknown / partial frames.
                    }
                }
            }
            Ok(Message::Ping(payload)) => {
                let _ = ws.send(Message::Pong(payload));
            }
            Ok(Message::Close(_)) | Ok(Message::Frame(_)) => {
                let _ = outbound.send(WsServerMsg::Error {
                    message: "server closed websocket".into(),
                });
                return;
            }
            Ok(Message::Binary(_)) | Ok(Message::Pong(_)) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => {
                let _ = outbound.send(WsServerMsg::Error {
                    message: "websocket read error".into(),
                });
                return;
            }
        }
    }
}

fn set_read_timeout(ws: &mut WebSocket<MaybeTlsStream<TcpStream>>, timeout: Duration) {
    match ws.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            let _ = stream.set_read_timeout(Some(timeout));
            let _ = stream.set_nonblocking(false);
        }
        #[allow(unused_variables)]
        other => {
            // No TLS in this MVP (`ws://` only); other variants are unused.
            let _ = other;
        }
    }
}
