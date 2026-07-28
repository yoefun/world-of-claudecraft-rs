//! Online character select / create (REST list → enter → WS Hello).

use bevy::prelude::*;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use woc_content::PlayerClass;

use crate::api::{self, CharacterResult, CharacterSummary, ListResult};
use crate::char_create::{CharName, SelectedClass};
use crate::{cleanup_ui, AppState, AuthSession, UiRoot};

#[derive(Resource)]
pub(crate) struct CharSelectForm {
    pub(crate) creating: bool,
    pub(crate) name: String,
    pub(crate) class: PlayerClass,
    pub(crate) cursor: usize,
    pub(crate) status: String,
    pub(crate) busy: bool,
}

impl Default for CharSelectForm {
    fn default() -> Self {
        Self {
            creating: false,
            name: "Aldric".into(),
            class: PlayerClass::Warrior,
            cursor: 0,
            status: String::new(),
            busy: false,
        }
    }
}

#[derive(Resource, Default)]
struct PendingList(Option<Mutex<Receiver<ListResult>>>);

#[derive(Resource, Default)]
struct PendingChar(Option<Mutex<Receiver<CharacterResult>>>);

#[derive(Component)]
struct SelectListLabel;

#[derive(Component)]
struct SelectStatusLabel;

#[derive(Component)]
struct SelectHintLabel;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<CharSelectForm>()
        .init_resource::<PendingList>()
        .init_resource::<PendingChar>()
        .add_systems(OnEnter(AppState::CharSelect), setup_char_select)
        .add_systems(OnExit(AppState::CharSelect), cleanup_ui)
        .add_systems(
            Update,
            (char_select_input, poll_list_result, poll_char_result)
                .chain()
                .run_if(in_state(AppState::CharSelect)),
        );
}

fn setup_char_select(
    mut commands: Commands,
    mut form: ResMut<CharSelectForm>,
    session: Res<AuthSession>,
    mut pending_list: ResMut<PendingList>,
) {
    form.creating = false;
    form.name = "Aldric".into();
    form.class = PlayerClass::Warrior;
    form.cursor = 0;
    form.busy = true;
    form.status = "Loading characters…".into();

    if let Some(token) = session.token.clone() {
        pending_list.0 = Some(Mutex::new(api::spawn_list_characters(token)));
    } else {
        form.busy = false;
        form.status = "Missing session token — Esc to login".into();
    }

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
            BackgroundColor(Color::srgba(0.04, 0.07, 0.1, 0.84)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Character Select"),
                TextFont::from_font_size(36.0),
                TextColor(Color::srgb(0.95, 0.86, 0.55)),
            ));
            p.spawn((
                SelectListLabel,
                Text::new(list_line(&form, &session)),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.85, 0.95, 0.85)),
            ));
            p.spawn((
                SelectStatusLabel,
                Text::new(form.status.clone()),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
            p.spawn((
                SelectHintLabel,
                Text::new(hint_line(&form)),
                TextFont::from_font_size(15.0),
                TextColor(Color::srgb(0.7, 0.75, 0.8)),
            ));
        });
}

fn list_line(form: &CharSelectForm, session: &AuthSession) -> String {
    if form.creating {
        return format!(
            "New: {}  [{}]",
            form.name,
            woc_content::class_def(form.class).name
        );
    }
    if session.characters.is_empty() {
        return "No characters — press N to create".into();
    }
    let c = &session.characters[form.cursor.min(session.characters.len() - 1)];
    format!(
        "[{}/{}] {}  {}  Lv{}",
        form.cursor + 1,
        session.characters.len(),
        c.name,
        c.class_id,
        c.level
    )
}

fn hint_line(form: &CharSelectForm) -> String {
    if form.creating {
        "Type name · ←/→ class · Enter create · Esc cancel".into()
    } else {
        "↑/↓ select · Enter enter world · N create · Esc login".into()
    }
}

fn char_select_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut form: ResMut<CharSelectForm>,
    mut session: ResMut<AuthSession>,
    mut pending_char: ResMut<PendingChar>,
    mut next: ResMut<NextState<AppState>>,
    mut events: EventReader<bevy::input::keyboard::KeyboardInput>,
    mut list_q: Query<&mut Text, With<SelectListLabel>>,
    mut status_q: Query<&mut Text, (With<SelectStatusLabel>, Without<SelectListLabel>)>,
    mut hint_q: Query<
        &mut Text,
        (
            With<SelectHintLabel>,
            Without<SelectListLabel>,
            Without<SelectStatusLabel>,
        ),
    >,
) {
    if form.busy {
        refresh(&form, &session, &mut list_q, &mut status_q, &mut hint_q);
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if form.creating {
            form.creating = false;
            form.status = "Select a character".into();
        } else {
            session.token = None;
            session.account_id = None;
            session.characters.clear();
            next.set(AppState::Login);
            keys.clear();
            return;
        }
    }

    if form.creating {
        use bevy::input::ButtonState;
        for ev in events.read() {
            if ev.state != ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &ev.text {
                for ch in text.chars() {
                    if (ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ' ')
                        && form.name.len() < 16
                    {
                        form.name.push(ch);
                    }
                }
            }
        }
        if keys.just_pressed(KeyCode::Backspace) {
            form.name.pop();
        }
        if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) {
            let idx = PlayerClass::ALL
                .iter()
                .position(|c| *c == form.class)
                .unwrap_or(0);
            let next_idx = if keys.just_pressed(KeyCode::ArrowRight) {
                (idx + 1) % PlayerClass::ALL.len()
            } else {
                (idx + PlayerClass::ALL.len() - 1) % PlayerClass::ALL.len()
            };
            form.class = PlayerClass::ALL[next_idx];
        }
        if keys.just_pressed(KeyCode::Enter) {
            let Some(token) = session.token.clone() else {
                form.status = "Missing token".into();
                refresh(&form, &session, &mut list_q, &mut status_q, &mut hint_q);
                return;
            };
            let name = form.name.trim().to_string();
            if name.is_empty() {
                form.status = "Name required".into();
            } else {
                form.busy = true;
                form.status = "Creating…".into();
                pending_char.0 = Some(Mutex::new(api::spawn_create_character(
                    token,
                    name,
                    form.class.as_str().to_string(),
                )));
            }
            keys.clear();
        }
    } else {
        if keys.just_pressed(KeyCode::KeyN) {
            form.creating = true;
            form.status = "Create character".into();
        }
        if !session.characters.is_empty() {
            if keys.just_pressed(KeyCode::ArrowUp) {
                form.cursor = form.cursor.saturating_sub(1);
            }
            if keys.just_pressed(KeyCode::ArrowDown) {
                form.cursor = (form.cursor + 1).min(session.characters.len() - 1);
            }
            if keys.just_pressed(KeyCode::Enter) {
                let Some(token) = session.token.clone() else {
                    form.status = "Missing token".into();
                    refresh(&form, &session, &mut list_q, &mut status_q, &mut hint_q);
                    return;
                };
                let id = session.characters[form.cursor].id;
                session.selected = Some(id);
                form.busy = true;
                form.status = "Entering world…".into();
                pending_char.0 = Some(Mutex::new(api::spawn_enter_character(token, id)));
                keys.clear();
            }
        }
    }

    refresh(&form, &session, &mut list_q, &mut status_q, &mut hint_q);
}

fn poll_list_result(
    mut form: ResMut<CharSelectForm>,
    mut pending: ResMut<PendingList>,
    mut session: ResMut<AuthSession>,
    mut list_q: Query<&mut Text, With<SelectListLabel>>,
    mut status_q: Query<&mut Text, (With<SelectStatusLabel>, Without<SelectListLabel>)>,
) {
    let Some(mutex) = pending.0.as_ref() else {
        return;
    };
    let Ok(guard) = mutex.lock() else {
        return;
    };
    match guard.try_recv() {
        Ok(ListResult::Ok(chars)) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.cursor = 0;
            session.characters = chars;
            form.status = if session.characters.is_empty() {
                "No characters yet".into()
            } else {
                format!("{} character(s)", session.characters.len())
            };
            if let Ok(mut text) = list_q.single_mut() {
                **text = list_line(&form, &session);
            }
            if let Ok(mut text) = status_q.single_mut() {
                **text = form.status.clone();
            }
        }
        Ok(ListResult::Err(msg)) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.status = msg;
            if let Ok(mut text) = status_q.single_mut() {
                **text = form.status.clone();
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.status = "List thread disconnected".into();
        }
    }
}

fn poll_char_result(
    mut form: ResMut<CharSelectForm>,
    mut pending: ResMut<PendingChar>,
    mut session: ResMut<AuthSession>,
    mut name: ResMut<CharName>,
    mut class: ResMut<SelectedClass>,
    mut next: ResMut<NextState<AppState>>,
    mut list_q: Query<&mut Text, With<SelectListLabel>>,
    mut status_q: Query<&mut Text, (With<SelectStatusLabel>, Without<SelectListLabel>)>,
) {
    let Some(mutex) = pending.0.as_ref() else {
        return;
    };
    let Ok(guard) = mutex.lock() else {
        return;
    };
    match guard.try_recv() {
        Ok(CharacterResult::Ok(c)) => {
            drop(guard);
            pending.0 = None;
            if form.creating {
                form.creating = false;
                session.characters.push(CharacterSummary {
                    id: c.id,
                    name: c.name.clone(),
                    class_id: c.class_id.clone(),
                    level: c.level,
                });
                form.cursor = session.characters.len().saturating_sub(1);
                form.busy = false;
                form.status = format!("Created {} — Enter to play", c.name);
                if let Ok(mut text) = list_q.single_mut() {
                    **text = list_line(&form, &session);
                }
                if let Ok(mut text) = status_q.single_mut() {
                    **text = form.status.clone();
                }
            } else {
                // Enter world path.
                form.busy = false;
                session.selected = Some(c.id);
                name.0 = c.name;
                class.0 = PlayerClass::ALL
                    .iter()
                    .copied()
                    .find(|pc| pc.as_str() == c.class_id)
                    .unwrap_or(PlayerClass::Warrior);
                next.set(AppState::InWorld);
            }
        }
        Ok(CharacterResult::Err(msg)) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.status = msg;
            if let Ok(mut text) = status_q.single_mut() {
                **text = form.status.clone();
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.status = "Character request disconnected".into();
        }
    }
}

fn refresh(
    form: &CharSelectForm,
    session: &AuthSession,
    list_q: &mut Query<&mut Text, With<SelectListLabel>>,
    status_q: &mut Query<&mut Text, (With<SelectStatusLabel>, Without<SelectListLabel>)>,
    hint_q: &mut Query<
        &mut Text,
        (
            With<SelectHintLabel>,
            Without<SelectListLabel>,
            Without<SelectStatusLabel>,
        ),
    >,
) {
    if let Ok(mut text) = list_q.single_mut() {
        **text = list_line(form, session);
    }
    if let Ok(mut text) = status_q.single_mut() {
        **text = form.status.clone();
    }
    if let Ok(mut text) = hint_q.single_mut() {
        **text = hint_line(form);
    }
}
