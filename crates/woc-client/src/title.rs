//! Title screen systems (Offline | Online mode picker).

use bevy::prelude::*;
use woc_version::footer;

use crate::{cleanup_ui, AppState, PlayMode, UiRoot};

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Title), setup_title)
        .add_systems(OnExit(AppState::Title), cleanup_ui)
        .add_systems(Update, title_input.run_if(in_state(AppState::Title)));
}

#[derive(Component)]
struct ModeLabel;

fn setup_title(mut commands: Commands, mode: Res<PlayMode>) {
    let mode_line = mode_prompt(*mode);
    commands
        .spawn((
            UiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.72)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("World of ClaudeCraft"),
                TextFont::from_font_size(48.0),
                TextColor(Color::srgb(0.95, 0.86, 0.55)),
            ));
            p.spawn((
                Text::new("Rust rewrite · framework slice"),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.85, 0.9, 0.95)),
            ));
            p.spawn((
                Text::new(footer()),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.7, 0.75, 0.8)),
            ));
            p.spawn((
                ModeLabel,
                Text::new(mode_line),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.9, 0.92, 0.85)),
            ));
            p.spawn((
                Text::new("1 Offline · 2 Online · ←/→ · Enter to continue"),
                TextFont::from_font_size(18.0),
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
        });
}

fn mode_prompt(mode: PlayMode) -> String {
    match mode {
        PlayMode::Offline => "Mode: Offline  (local sim)".into(),
        PlayMode::Online => format!(
            "Mode: Online  ({} → login)",
            crate::online::ONLINE_WS_URL
        ),
    }
}

fn title_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<PlayMode>,
    mut next: ResMut<NextState<AppState>>,
    mut label: Query<&mut Text, With<ModeLabel>>,
) {
    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        *mode = PlayMode::Offline;
    }
    if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        *mode = PlayMode::Online;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) {
        *mode = match *mode {
            PlayMode::Offline => PlayMode::Online,
            PlayMode::Online => PlayMode::Offline,
        };
    }
    if let Ok(mut text) = label.single_mut() {
        **text = mode_prompt(*mode);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        match *mode {
            PlayMode::Offline => next.set(AppState::CharCreate),
            PlayMode::Online => next.set(AppState::Login),
        }
    }
}
