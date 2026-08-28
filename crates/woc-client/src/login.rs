//! Online login / register screen (REST against `/api/login` and `/api/register`).

use bevy::prelude::*;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use crate::api::{self, AuthResult};
use crate::menu_ui::{
    self, button_bundle, panel_node, spawn_screen_root, status_color, MenuBtnKind, BODY, FIELD_BG,
    FIELD_FOCUS, GOLD, MUTED,
};
use crate::{cleanup_ui, AppState, AuthSession};

#[derive(Resource, Default)]
pub(crate) struct LoginForm {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) password_confirm: String,
    /// `false` = login, `true` = register.
    pub(crate) register_mode: bool,
    /// 0 = user, 1 = password, 2 = confirm (register only).
    pub(crate) focus: u8,
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
struct LoginConfirmLabel;

#[derive(Component)]
struct LoginConfirmRow;

#[derive(Component)]
struct LoginStatusLabel;

#[derive(Component)]
struct LoginSubmitLabel;

#[derive(Component)]
struct LoginModeLoginBtn;

#[derive(Component)]
struct LoginModeRegisterBtn;

#[derive(Component)]
struct LoginSubmitBtn;

#[derive(Component)]
struct LoginBackBtn;

#[derive(Component)]
struct LoginFieldUser;

#[derive(Component)]
struct LoginFieldPass;

#[derive(Component)]
struct LoginFieldConfirm;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<LoginForm>()
        .init_resource::<PendingAuth>()
        .add_systems(OnEnter(AppState::Login), setup_login)
        .add_systems(OnExit(AppState::Login), cleanup_ui)
        .add_systems(
            Update,
            (
                menu_ui::menu_button_visuals,
                login_clicks,
                login_input,
                poll_auth_result,
                refresh_login_chrome,
            )
                .chain()
                .run_if(in_state(AppState::Login)),
        );
}

fn setup_login(mut commands: Commands, mut form: ResMut<LoginForm>) {
    form.status = format!("Realm API {}", api::API_BASE);
    form.busy = false;
    form.focus = 0;
    if form.password_confirm.is_empty() {
        // keep typed fields across Esc→re-enter when possible
    }

    let root = spawn_screen_root(&mut commands);
    let (panel_n, panel_bg, panel_bd) = panel_node(460.0);

    commands.entity(root).with_children(|screen| {
        screen
            .spawn((panel_n, panel_bg, panel_bd))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Account"),
                    TextFont::from_font_size(34.0),
                    TextColor(GOLD),
                    Node {
                        align_self: AlignSelf::Center,
                        ..default()
                    },
                ));
                panel.spawn((
                    Text::new("Sign in to your realm, or create a new account"),
                    TextFont::from_font_size(15.0),
                    TextColor(MUTED),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                ));

                // Mode tabs
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Primary);
                        row.spawn((b, k, n, bg, bd, LoginModeLoginBtn))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new("Login"),
                                    TextFont::from_font_size(16.0),
                                    TextColor(BODY),
                                ));
                            });
                        let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Secondary);
                        row.spawn((b, k, n, bg, bd, LoginModeRegisterBtn))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new("Register"),
                                    TextFont::from_font_size(16.0),
                                    TextColor(BODY),
                                ));
                            });
                    });

                // Username field
                panel
                    .spawn((
                        Button,
                        LoginFieldUser,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(40.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(FIELD_FOCUS),
                        BorderColor(Color::srgba(0.45, 0.55, 0.7, 0.55)),
                    ))
                    .with_children(|f| {
                        f.spawn((
                            LoginUserLabel,
                            Text::new(user_line(&form)),
                            TextFont::from_font_size(18.0),
                            TextColor(BODY),
                        ));
                    });

                // Password field
                panel
                    .spawn((
                        Button,
                        LoginFieldPass,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(40.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(FIELD_BG),
                        BorderColor(Color::srgba(0.35, 0.4, 0.5, 0.4)),
                    ))
                    .with_children(|f| {
                        f.spawn((
                            LoginPassLabel,
                            Text::new(pass_line(&form)),
                            TextFont::from_font_size(18.0),
                            TextColor(BODY),
                        ));
                    });

                // Confirm (register only)
                panel
                    .spawn((
                        LoginConfirmRow,
                        Button,
                        LoginFieldConfirm,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(40.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            display: if form.register_mode {
                                Display::Flex
                            } else {
                                Display::None
                            },
                            ..default()
                        },
                        BackgroundColor(FIELD_BG),
                        BorderColor(Color::srgba(0.35, 0.4, 0.5, 0.4)),
                        Visibility::Inherited,
                    ))
                    .with_children(|f| {
                        f.spawn((
                            LoginConfirmLabel,
                            Text::new(confirm_line(&form)),
                            TextFont::from_font_size(18.0),
                            TextColor(BODY),
                        ));
                    });

                panel.spawn((
                    LoginStatusLabel,
                    Text::new(form.status.clone()),
                    TextFont::from_font_size(15.0),
                    TextColor(status_color(false, &form.status)),
                    Node {
                        align_self: AlignSelf::Center,
                        ..default()
                    },
                ));

                // Actions
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Primary);
                        row.spawn((b, k, n, bg, bd, LoginSubmitBtn))
                            .with_children(|btn| {
                                btn.spawn((
                                    LoginSubmitLabel,
                                    Text::new(submit_label(form.register_mode)),
                                    TextFont::from_font_size(16.0),
                                    TextColor(BODY),
                                ));
                            });
                        let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Secondary);
                        row.spawn((b, k, n, bg, bd, LoginBackBtn))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new("Back"),
                                    TextFont::from_font_size(16.0),
                                    TextColor(BODY),
                                ));
                            });
                    });

                panel.spawn((
                    Text::new("Tab fields · F2 mode · Enter submit · click also works"),
                    TextFont::from_font_size(13.0),
                    TextColor(MUTED),
                    Node {
                        align_self: AlignSelf::Center,
                        ..default()
                    },
                ));
            });
    });
}

fn submit_label(register: bool) -> &'static str {
    if register {
        "Create account"
    } else {
        "Sign in"
    }
}

fn user_line(form: &LoginForm) -> String {
    let cursor = if form.focus == 0 { "_" } else { "" };
    format!("User  {}{}", form.username, cursor)
}

fn pass_line(form: &LoginForm) -> String {
    let masked: String = form.password.chars().map(|_| '•').collect();
    let cursor = if form.focus == 1 { "_" } else { "" };
    format!("Pass  {}{}", masked, cursor)
}

fn confirm_line(form: &LoginForm) -> String {
    let masked: String = form.password_confirm.chars().map(|_| '•').collect();
    let cursor = if form.focus == 2 { "_" } else { "" };
    format!("Again {}{}", masked, cursor)
}

fn max_focus(register: bool) -> u8 {
    if register {
        2
    } else {
        1
    }
}

fn login_clicks(
    mut form: ResMut<LoginForm>,
    mut next: ResMut<NextState<AppState>>,
    mut pending: ResMut<PendingAuth>,
    mode_login: Query<&Interaction, (Changed<Interaction>, With<LoginModeLoginBtn>)>,
    mode_reg: Query<&Interaction, (Changed<Interaction>, With<LoginModeRegisterBtn>)>,
    submit: Query<&Interaction, (Changed<Interaction>, With<LoginSubmitBtn>)>,
    back: Query<&Interaction, (Changed<Interaction>, With<LoginBackBtn>)>,
    field_user: Query<&Interaction, (Changed<Interaction>, With<LoginFieldUser>)>,
    field_pass: Query<&Interaction, (Changed<Interaction>, With<LoginFieldPass>)>,
    field_confirm: Query<&Interaction, (Changed<Interaction>, With<LoginFieldConfirm>)>,
) {
    if form.busy {
        return;
    }
    for interaction in &mode_login {
        if *interaction == Interaction::Pressed {
            form.register_mode = false;
            if form.focus > 1 {
                form.focus = 1;
            }
        }
    }
    for interaction in &mode_reg {
        if *interaction == Interaction::Pressed {
            form.register_mode = true;
        }
    }
    for interaction in &field_user {
        if *interaction == Interaction::Pressed {
            form.focus = 0;
        }
    }
    for interaction in &field_pass {
        if *interaction == Interaction::Pressed {
            form.focus = 1;
        }
    }
    for interaction in &field_confirm {
        if *interaction == Interaction::Pressed && form.register_mode {
            form.focus = 2;
        }
    }
    for interaction in &back {
        if *interaction == Interaction::Pressed {
            next.set(AppState::Title);
            return;
        }
    }
    for interaction in &submit {
        if *interaction == Interaction::Pressed {
            try_submit(&mut form, &mut pending);
        }
    }
}

fn try_submit(form: &mut LoginForm, pending: &mut PendingAuth) {
    let user = form.username.trim().to_string();
    let pass = form.password.clone();
    if user.is_empty() || pass.is_empty() {
        form.status = "Enter username and password".into();
        return;
    }
    if user.len() < 3 {
        form.status = "Username must be at least 3 characters".into();
        return;
    }
    if pass.len() < 6 {
        form.status = "Password must be at least 6 characters".into();
        return;
    }
    if form.register_mode && pass != form.password_confirm {
        form.status = "Passwords do not match — confirm again".into();
        form.focus = 2;
        return;
    }
    form.busy = true;
    form.status = if form.register_mode {
        "Creating account…".into()
    } else {
        "Signing in…".into()
    };
    let rx = if form.register_mode {
        api::spawn_register(user, pass)
    } else {
        api::spawn_login(user, pass)
    };
    pending.0 = Some(Mutex::new(rx));
}

fn login_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut form: ResMut<LoginForm>,
    mut pending: ResMut<PendingAuth>,
    mut next: ResMut<NextState<AppState>>,
    mut events: EventReader<bevy::input::keyboard::KeyboardInput>,
) {
    if form.busy {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
        keys.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let max = max_focus(form.register_mode);
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            form.focus = if form.focus == 0 { max } else { form.focus - 1 };
        } else {
            form.focus = if form.focus >= max { 0 } else { form.focus + 1 };
        }
    }
    if keys.just_pressed(KeyCode::F2) {
        form.register_mode = !form.register_mode;
        if !form.register_mode && form.focus > 1 {
            form.focus = 1;
        }
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
                let focus = form.focus;
                let register = form.register_mode;
                let ok = if focus == 0 {
                    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.'
                } else {
                    !ch.is_control() && !ch.is_whitespace()
                };
                if !ok {
                    continue;
                }
                let target = match focus {
                    1 => &mut form.password,
                    2 if register => &mut form.password_confirm,
                    _ => &mut form.username,
                };
                if target.len() < 32 {
                    target.push(ch);
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::Backspace) {
        match form.focus {
            1 => {
                form.password.pop();
            }
            2 if form.register_mode => {
                form.password_confirm.pop();
            }
            _ => {
                form.username.pop();
            }
        }
    }

    if keys.just_pressed(KeyCode::Enter) {
        try_submit(&mut form, &mut pending);
        keys.clear();
    }
}

fn poll_auth_result(
    mut form: ResMut<LoginForm>,
    mut pending: ResMut<PendingAuth>,
    mut session: ResMut<AuthSession>,
    mut next: ResMut<NextState<AppState>>,
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
            form.password.clear();
            form.password_confirm.clear();
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

fn refresh_login_chrome(
    form: Res<LoginForm>,
    mut texts: ParamSet<(
        Query<&mut Text, With<LoginUserLabel>>,
        Query<&mut Text, With<LoginPassLabel>>,
        Query<&mut Text, With<LoginConfirmLabel>>,
        Query<(&mut Text, &mut TextColor), With<LoginStatusLabel>>,
        Query<&mut Text, With<LoginSubmitLabel>>,
    )>,
    mut confirm_row: Query<&mut Node, With<LoginConfirmRow>>,
    mut fields: ParamSet<(
        Query<&mut BackgroundColor, With<LoginFieldUser>>,
        Query<&mut BackgroundColor, With<LoginFieldPass>>,
        Query<&mut BackgroundColor, With<LoginFieldConfirm>>,
    )>,
) {
    if let Ok(mut text) = texts.p0().single_mut() {
        **text = user_line(&form);
    }
    if let Ok(mut text) = texts.p1().single_mut() {
        **text = pass_line(&form);
    }
    if let Ok(mut text) = texts.p2().single_mut() {
        **text = confirm_line(&form);
    }
    if let Ok((mut text, mut color)) = texts.p3().single_mut() {
        **text = form.status.clone();
        *color = TextColor(status_color(form.busy, &form.status));
    }
    if let Ok(mut text) = texts.p4().single_mut() {
        **text = submit_label(form.register_mode).into();
    }
    if let Ok(mut node) = confirm_row.single_mut() {
        node.display = if form.register_mode {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut bg) = fields.p0().single_mut() {
        *bg = BackgroundColor(if form.focus == 0 {
            FIELD_FOCUS
        } else {
            FIELD_BG
        });
    }
    if let Ok(mut bg) = fields.p1().single_mut() {
        *bg = BackgroundColor(if form.focus == 1 {
            FIELD_FOCUS
        } else {
            FIELD_BG
        });
    }
    if let Ok(mut bg) = fields.p2().single_mut() {
        *bg = BackgroundColor(if form.focus == 2 {
            FIELD_FOCUS
        } else {
            FIELD_BG
        });
    }
}
