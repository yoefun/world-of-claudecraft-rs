//! Online login / register screen (REST against `/api/login` and `/api/register`).

use bevy::prelude::*;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;

use crate::api::{self, AuthResult};
use crate::{cleanup_ui, AppState, AuthSession, UiRoot};

#[derive(Resource, Default)]
pub(crate) struct LoginForm {
    pub(crate) username: String,
    pub(crate) password: String,
    /// `false` = login, `true` = register.
    pub(crate) register_mode: bool,
    pub(crate) editing_password: bool,
    pub(crate) status: String,
    pub(crate) busy: bool,
}

#[derive(Resource, Default)]
struct PendingAuth(Option<Mutex<Receiver<AuthResult>>>);

#[derive(Component)]
struct LoginUserLabel;

#[derive(Component)]
struct LoginPassLabel;

#[derive(Component)]
struct LoginModeLabel;

#[derive(Component)]
struct LoginStatusLabel;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<LoginForm>()
        .init_resource::<PendingAuth>()
        .add_systems(OnEnter(AppState::Login), setup_login)
        .add_systems(OnExit(AppState::Login), cleanup_ui)
        .add_systems(
            Update,
            (login_input, poll_auth_result)
                .chain()
                .run_if(in_state(AppState::Login)),
        );
}

fn setup_login(mut commands: Commands, mut form: ResMut<LoginForm>) {
    form.status = format!("API {}", api::API_BASE);
    form.busy = false;
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
            BackgroundColor(Color::srgba(0.04, 0.06, 0.1, 0.82)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Account"),
                TextFont::from_font_size(36.0),
                TextColor(Color::srgb(0.95, 0.86, 0.55)),
            ));
            p.spawn((
                LoginModeLabel,
                Text::new(mode_line(form.register_mode)),
                TextFont::from_font_size(18.0),
                TextColor(Color::srgb(0.85, 0.9, 0.95)),
            ));
            p.spawn((
                LoginUserLabel,
                Text::new(user_line(&form)),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.85, 0.95, 0.85)),
            ));
            p.spawn((
                LoginPassLabel,
                Text::new(pass_line(&form)),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.85, 0.95, 0.85)),
            ));
            p.spawn((
                LoginStatusLabel,
                Text::new(form.status.clone()),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
            p.spawn((
                Text::new("Tab field · F2 login/register · Enter submit · Esc title"),
                TextFont::from_font_size(15.0),
                TextColor(Color::srgb(0.7, 0.75, 0.8)),
            ));
        });
}

fn mode_line(register: bool) -> String {
    if register {
        "Mode: Register  (F2 to switch)".into()
    } else {
        "Mode: Login  (F2 to switch)".into()
    }
}

fn user_line(form: &LoginForm) -> String {
    let cursor = if !form.editing_password { "_" } else { "" };
    format!("User: {}{}", form.username, cursor)
}

fn pass_line(form: &LoginForm) -> String {
    let masked: String = form.password.chars().map(|_| '*').collect();
    let cursor = if form.editing_password { "_" } else { "" };
    format!("Pass: {}{}", masked, cursor)
}

fn login_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut form: ResMut<LoginForm>,
    mut pending: ResMut<PendingAuth>,
    mut next: ResMut<NextState<AppState>>,
    mut events: EventReader<bevy::input::keyboard::KeyboardInput>,
    mut user_q: Query<&mut Text, With<LoginUserLabel>>,
    mut pass_q: Query<&mut Text, (With<LoginPassLabel>, Without<LoginUserLabel>)>,
    mut mode_q: Query<
        &mut Text,
        (
            With<LoginModeLabel>,
            Without<LoginUserLabel>,
            Without<LoginPassLabel>,
        ),
    >,
    mut status_q: Query<
        &mut Text,
        (
            With<LoginStatusLabel>,
            Without<LoginUserLabel>,
            Without<LoginPassLabel>,
            Without<LoginModeLabel>,
        ),
    >,
) {
    if form.busy {
        refresh_labels(&form, &mut user_q, &mut pass_q, &mut mode_q, &mut status_q);
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
        keys.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        form.editing_password = !form.editing_password;
    }
    if keys.just_pressed(KeyCode::F2) {
        form.register_mode = !form.register_mode;
    }

    use bevy::input::ButtonState;
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if let Some(text) = &ev.text {
            for ch in text.chars() {
                if ch.is_control() {
                    continue;
                }
                let target = if form.editing_password {
                    &mut form.password
                } else {
                    &mut form.username
                };
                if (ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
                    && target.len() < 24
                {
                    target.push(ch);
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::Backspace) {
        if form.editing_password {
            form.password.pop();
        } else {
            form.username.pop();
        }
    }

    if keys.just_pressed(KeyCode::Enter) {
        let user = form.username.trim().to_string();
        let pass = form.password.clone();
        if user.is_empty() || pass.is_empty() {
            form.status = "Enter username and password".into();
        } else {
            form.busy = true;
            form.status = if form.register_mode {
                "Registering…".into()
            } else {
                "Logging in…".into()
            };
            let rx = if form.register_mode {
                api::spawn_register(user, pass)
            } else {
                api::spawn_login(user, pass)
            };
            pending.0 = Some(Mutex::new(rx));
        }
        keys.clear();
    }

    refresh_labels(&form, &mut user_q, &mut pass_q, &mut mode_q, &mut status_q);
}

fn poll_auth_result(
    mut form: ResMut<LoginForm>,
    mut pending: ResMut<PendingAuth>,
    mut session: ResMut<AuthSession>,
    mut next: ResMut<NextState<AppState>>,
    mut status_q: Query<&mut Text, With<LoginStatusLabel>>,
) {
    let Some(mutex) = pending.0.as_ref() else {
        return;
    };
    let Ok(guard) = mutex.lock() else {
        return;
    };
    match guard.try_recv() {
        Ok(AuthResult::Ok(auth)) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.status = "Authenticated".into();
            session.token = Some(auth.token);
            session.account_id = Some(auth.account_id);
            session.characters.clear();
            session.selected = None;
            next.set(AppState::CharSelect);
        }
        Ok(AuthResult::Err(msg)) => {
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
            form.status = "Auth thread disconnected".into();
        }
    }
}

fn refresh_labels(
    form: &LoginForm,
    user_q: &mut Query<&mut Text, With<LoginUserLabel>>,
    pass_q: &mut Query<&mut Text, (With<LoginPassLabel>, Without<LoginUserLabel>)>,
    mode_q: &mut Query<
        &mut Text,
        (
            With<LoginModeLabel>,
            Without<LoginUserLabel>,
            Without<LoginPassLabel>,
        ),
    >,
    status_q: &mut Query<
        &mut Text,
        (
            With<LoginStatusLabel>,
            Without<LoginUserLabel>,
            Without<LoginPassLabel>,
            Without<LoginModeLabel>,
        ),
    >,
) {
    if let Ok(mut text) = user_q.single_mut() {
        **text = user_line(form);
    }
    if let Ok(mut text) = pass_q.single_mut() {
        **text = pass_line(form);
    }
    if let Ok(mut text) = mode_q.single_mut() {
        **text = mode_line(form.register_mode);
    }
    if let Ok(mut text) = status_q.single_mut() {
        **text = form.status.clone();
    }
}
