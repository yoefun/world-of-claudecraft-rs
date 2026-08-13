//! Bevy host (offline sim embed + online WS).

mod anim;
mod api;
mod char_create;
mod char_select;
mod hud;
mod input;
mod login;
mod map;
mod menu_ui;
mod nameplates;
mod online;
mod title;
mod visuals;
mod world_setup;

use bevy::prelude::*;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use uuid::Uuid;
use woc_protocol::{
    EntityId, EntitySnapshot, InteractAction, PlayerIntent, TickSnapshot, WsClientMsg,
};
use woc_sim::Sim;
use woc_version::{footer, VersionInfo};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: footer(),
                resolution: (1280.0_f32, 720.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .init_resource::<PlayMode>()
        .init_resource::<AuthSession>()
        .init_resource::<RealmCompat>()
        .insert_resource(ClearColor(Color::srgb(0.45, 0.62, 0.78)))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.92, 0.94, 0.88),
            brightness: 350.0,
            ..default()
        })
        .add_plugins((
            title::plugin,
            login::plugin,
            char_create::plugin,
            char_select::plugin,
            world_setup::plugin,
            hud::plugin,
            nameplates::plugin,
            map::plugin,
        ))
        .add_systems(
            Update,
            (
                input::grab_cursor,
                input::camera_look,
                input::collect_intent,
                input::handle_interact_keys,
                world_setup::sim_fixed_step,
                world_setup::sync_visuals,
                hud::update_hud,
                hud::update_chrome_panels,
                hud::sync_vendor_panel,
                hud::npc_session_clicks,
                hud::vendor_ungrab_cursor,
                hud::toast_fade,
            )
                .chain()
                .run_if(in_state(AppState::InWorld)),
        )
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) enum AppState {
    #[default]
    Title,
    /// Online: register / login against REST auth.
    Login,
    /// Offline: local name/class pick.
    CharCreate,
    /// Online: list / create / enter character via REST.
    CharSelect,
    InWorld,
}

/// Bearer session + character roster after online login.
#[derive(Resource, Debug, Clone, Default)]
pub(crate) struct AuthSession {
    pub(crate) token: Option<String>,
    pub(crate) account_id: Option<Uuid>,
    pub(crate) characters: Vec<api::CharacterSummary>,
    pub(crate) selected: Option<Uuid>,
}

/// Title / session mode: local sim vs WebSocket server.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PlayMode {
    #[default]
    Offline,
    Online,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum RealmCompatState {
    #[default]
    Idle,
    Checking,
    Compatible {
        realm_rewrite: String,
        protocol_rev: u32,
    },
    Incompatible {
        message: String,
    },
    Unreachable {
        message: String,
    },
}

#[derive(Resource, Default)]
pub(crate) struct RealmCompat {
    pub(crate) state: RealmCompatState,
    pub(crate) pending: Option<Mutex<Receiver<Result<VersionInfo, String>>>>,
    pub(crate) update_manifest_url: String,
}

impl RealmCompat {
    pub(crate) fn status_line(&self) -> String {
        match &self.state {
            RealmCompatState::Idle => "Online: not checked".into(),
            RealmCompatState::Checking => "Online: checking realm version…".into(),
            RealmCompatState::Compatible {
                realm_rewrite,
                protocol_rev,
            } => format!("Online: compatible · realm {realm_rewrite} · proto {protocol_rev}"),
            RealmCompatState::Incompatible { message }
            | RealmCompatState::Unreachable { message } => message.clone(),
        }
    }

    pub(crate) fn begin_probe(&mut self) {
        if matches!(self.state, RealmCompatState::Checking) {
            return;
        }
        self.state = RealmCompatState::Checking;
        self.pending = Some(Mutex::new(crate::api::spawn_fetch_version()));
    }

    pub(crate) fn poll(&mut self) {
        let Some(rx) = self.pending.as_ref() else {
            return;
        };
        let recv = rx.lock().expect("version receiver mutex").try_recv();
        match recv {
            Ok(Ok(info)) => {
                self.pending = None;
                self.update_manifest_url = info.update_manifest_url.clone();
                let client = woc_version::ClientIdentity {
                    rewrite_version: woc_version::REWRITE_VERSION.to_string(),
                    protocol_rev: woc_protocol::PROTOCOL_REV,
                };
                match woc_version::check_compat(&client, &info.realm_identity()) {
                    woc_version::Compat::Compatible => {
                        self.state = RealmCompatState::Compatible {
                            realm_rewrite: info.rewrite_version,
                            protocol_rev: info.protocol_rev,
                        };
                    }
                    other => {
                        self.state = RealmCompatState::Incompatible {
                            message: other.user_message(),
                        };
                    }
                }
            }
            Ok(Err(message)) => {
                self.pending = None;
                self.state = RealmCompatState::Unreachable {
                    message: format!("version: unreachable ({message})"),
                };
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                self.state = RealmCompatState::Unreachable {
                    message: "version: unreachable (version thread disconnected)".into(),
                };
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NetStatus {
    Idle,
    Connecting,
    Connected { player_id: EntityId },
    Error(String),
}

#[derive(Component)]
pub(crate) struct UiRoot;

/// Shared in-world host: offline embeds `Sim`; online applies WS snapshots.
#[derive(Resource)]
pub(crate) struct GameHost {
    pub(crate) play_mode: PlayMode,
    /// Local sim when [`PlayMode::Offline`].
    pub(crate) sim: Option<Sim>,
    /// Authoritative view used by HUD / visuals (both modes).
    pub(crate) snapshot: TickSnapshot,
    pub(crate) accumulator: f32,
    pub(crate) pending_intent: PlayerIntent,
    pub(crate) recent_toasts: Vec<(String, f32)>,
    pub(crate) look_yaw: f32,
    pub(crate) look_pitch: f32,
    pub(crate) cursor_grabbed: bool,
    pub(crate) net_status: NetStatus,
    /// Online: Bevy → WS thread.
    pub(crate) to_net: Option<std::sync::mpsc::Sender<WsClientMsg>>,
    /// Online: WS thread → Bevy (`Mutex` so `GameHost: Sync` for Bevy resources).
    pub(crate) from_net: Option<Mutex<std::sync::mpsc::Receiver<woc_protocol::WsServerMsg>>>,
    /// Sticky attack for intent building when the snapshot has no auto_attack flag.
    pub(crate) local_auto_attack: bool,
}

impl GameHost {
    pub(crate) fn is_online(&self) -> bool {
        matches!(self.play_mode, PlayMode::Online)
    }

    pub(crate) fn player_snap(&self) -> Option<&EntitySnapshot> {
        self.snapshot
            .entities
            .iter()
            .find(|e| e.id == self.snapshot.player_id)
    }

    pub(crate) fn interact(&mut self, target_id: EntityId, action: InteractAction) {
        match self.play_mode {
            PlayMode::Offline => {
                if let Some(sim) = self.sim.as_mut() {
                    sim.interact(target_id, action);
                }
            }
            PlayMode::Online => {
                if let Some(tx) = &self.to_net {
                    let _ = tx.send(WsClientMsg::Interact { target_id, action });
                }
            }
        }
    }
}

pub(crate) fn cleanup_ui(mut commands: Commands, q: Query<Entity, With<UiRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
