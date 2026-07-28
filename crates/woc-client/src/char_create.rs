//! Character create / class select.

use bevy::prelude::*;
use woc_content::PlayerClass;

use crate::{cleanup_ui, AppState, UiRoot};

#[derive(Resource)]
pub(crate) struct CharName(pub(crate) String);

#[derive(Resource)]
pub(crate) struct SelectedClass(pub(crate) PlayerClass);

#[derive(Component)]
struct NameInputDisplay;

#[derive(Component)]
struct ClassLabel;

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(CharName("Aldric".into()))
        .insert_resource(SelectedClass(PlayerClass::Warrior))
        .add_systems(OnEnter(AppState::CharCreate), setup_char_create)
        .add_systems(OnExit(AppState::CharCreate), cleanup_ui)
        .add_systems(
            Update,
            char_create_input.run_if(in_state(AppState::CharCreate)),
        );
}

fn setup_char_create(mut commands: Commands, name: Res<CharName>, class: Res<SelectedClass>) {
    let label = format!("Name: {}", name.0);
    let class_label = format!(
        "Class: {}  (Left/Right to change)",
        woc_content::class_def(class.0).name
    );
    commands
        .spawn((
            UiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.1, 0.8)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Create Character"),
                TextFont::from_font_size(36.0),
                TextColor(Color::srgb(0.95, 0.86, 0.55)),
            ));
            p.spawn((
                NameInputDisplay,
                Text::new(label),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.85, 0.95, 0.85)),
            ));
            p.spawn((
                ClassLabel,
                Text::new(class_label),
                TextFont::from_font_size(18.0),
                TextColor(Color::WHITE),
            ));
            p.spawn((
                Text::new("Type a name · ←/→ class · Enter to enter Eastbrook"),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
        });
}

fn char_create_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut name: ResMut<CharName>,
    mut class: ResMut<SelectedClass>,
    mut next: ResMut<NextState<AppState>>,
    mut q: Query<&mut Text, With<NameInputDisplay>>,
    mut cq: Query<&mut Text, (With<ClassLabel>, Without<NameInputDisplay>)>,
    mut events: EventReader<bevy::input::keyboard::KeyboardInput>,
) {
    use bevy::input::ButtonState;
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if let Some(text) = &ev.text {
            for ch in text.chars() {
                if (ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ' ')
                    && name.0.len() < 16
                {
                    name.0.push(ch);
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::Backspace) {
        name.0.pop();
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) {
        let idx = PlayerClass::ALL
            .iter()
            .position(|c| *c == class.0)
            .unwrap_or(0);
        let next_idx = if keys.just_pressed(KeyCode::ArrowRight) {
            (idx + 1) % PlayerClass::ALL.len()
        } else {
            (idx + PlayerClass::ALL.len() - 1) % PlayerClass::ALL.len()
        };
        class.0 = PlayerClass::ALL[next_idx];
    }
    if keys.just_pressed(KeyCode::Enter) {
        if name.0.trim().is_empty() {
            name.0 = "Aldric".into();
        }
        next.set(AppState::InWorld);
        keys.clear();
    }
    if let Ok(mut text) = q.single_mut() {
        **text = format!("Name: {}", name.0);
    }
    if let Ok(mut text) = cq.single_mut() {
        **text = format!(
            "Class: {}  (Left/Right to change)",
            woc_content::class_def(class.0).name
        );
    }
}
