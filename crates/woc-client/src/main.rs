//! Bevy offline host for the framework slice.

mod char_create;
mod hud;
mod input;
mod title;
mod world_setup;

use bevy::prelude::*;
use woc_protocol::PlayerIntent;
use woc_sim::Sim;
use woc_version::footer;

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
        .insert_resource(ClearColor(Color::srgb(0.45, 0.62, 0.78)))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.92, 0.94, 0.88),
            brightness: 350.0,
            ..default()
        })
        .add_plugins((
            title::plugin,
            char_create::plugin,
            world_setup::plugin,
            hud::plugin,
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
    CharCreate,
    InWorld,
}

#[derive(Component)]
pub(crate) struct UiRoot;

#[derive(Resource)]
pub(crate) struct OfflineHost {
    pub(crate) sim: Sim,
    pub(crate) accumulator: f32,
    pub(crate) pending_intent: PlayerIntent,
    pub(crate) recent_toasts: Vec<(String, f32)>,
    pub(crate) look_yaw: f32,
    pub(crate) look_pitch: f32,
    pub(crate) cursor_grabbed: bool,
}

pub(crate) fn cleanup_ui(mut commands: Commands, q: Query<Entity, With<UiRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
