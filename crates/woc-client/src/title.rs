//! Title screen systems.

use bevy::prelude::*;
use woc_version::footer;

use crate::{cleanup_ui, AppState, UiRoot};

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Title), setup_title)
        .add_systems(OnExit(AppState::Title), cleanup_ui)
        .add_systems(Update, title_input.run_if(in_state(AppState::Title)));
}

fn setup_title(mut commands: Commands) {
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
                Text::new("Press Enter to create a character"),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.9, 0.92, 0.85)),
            ));
        });
}

fn title_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        next.set(AppState::CharCreate);
    }
}
