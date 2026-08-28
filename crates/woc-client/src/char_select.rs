//! Online character select / create (REST list → enter → WS Hello).

use bevy::prelude::*;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use uuid::Uuid;
use woc_content::PlayerClass;

use crate::api::{self, CharacterResult, CharacterSummary, ListResult};
use crate::char_create::{CharName, SelectedClass};
use crate::menu_ui::{
    self, button_bundle, class_detail_line, panel_node, spawn_class_grid, spawn_screen_root,
    status_color, ClassPickButton, MenuBtnKind, BODY, FIELD_FOCUS, GOLD, MUTED, ROW_IDLE,
    ROW_SELECTED,
};
use crate::{cleanup_ui, AppState, AuthSession};

#[derive(Resource)]
pub(crate) struct CharSelectForm {
    pub(crate) creating: bool,
    pub(crate) name: String,
    pub(crate) class: PlayerClass,
    pub(crate) cursor: usize,
    pub(crate) status: String,
    pub(crate) busy: bool,
    /// When set, next Delete confirm removes this character.
    pub(crate) delete_armed: Option<Uuid>,
    /// Character id currently being deleted (in-flight).
    pub(crate) deleting: Option<Uuid>,
    /// Dirty flag so roster rows rebuild after list/create/delete.
    pub(crate) roster_dirty: bool,
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
            delete_armed: None,
            deleting: None,
            roster_dirty: true,
        }
    }
}

#[derive(Resource, Default)]
struct PendingList(Option<Mutex<Receiver<ListResult>>>);

#[derive(Resource, Default)]
struct PendingChar(Option<Mutex<Receiver<CharacterResult>>>);

#[derive(Resource, Default)]
struct PendingDelete(Option<Mutex<Receiver<Result<(), String>>>>);

#[derive(Component)]
struct SelectStatusLabel;

#[derive(Component)]
struct SelectHintLabel;

#[derive(Component)]
struct RosterRoot;

#[derive(Component)]
struct CreatePanel;

#[derive(Component)]
struct CreateNameLabel;

#[derive(Component)]
struct CreateDetailLabel;

#[derive(Component)]
struct CharRowButton {
    index: usize,
}

#[derive(Component)]
struct SelectEnterBtn;

#[derive(Component)]
struct SelectCreateBtn;

#[derive(Component)]
struct SelectDeleteBtn;

#[derive(Component)]
struct SelectBackBtn;

#[derive(Component)]
struct CreateSubmitBtn;

#[derive(Component)]
struct CreateCancelBtn;

#[derive(Component)]
struct SelectModeActions;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<CharSelectForm>()
        .init_resource::<PendingList>()
        .init_resource::<PendingChar>()
        .init_resource::<PendingDelete>()
        .add_systems(OnEnter(AppState::CharSelect), setup_char_select)
        .add_systems(OnExit(AppState::CharSelect), cleanup_ui)
        .add_systems(
            Update,
            (
                menu_ui::menu_button_visuals,
                char_select_clicks,
                char_select_input,
                poll_list_result,
                poll_char_result,
                poll_delete_result,
                rebuild_roster_if_needed,
                refresh_char_select_chrome,
            )
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
    form.deleting = None;
    form.busy = true;
    form.delete_armed = None;
    form.roster_dirty = true;
    form.status = "Loading characters…".into();

    if let Some(token) = session.token.clone() {
        pending_list.0 = Some(Mutex::new(api::spawn_list_characters(token)));
    } else {
        form.busy = false;
        form.status = "Missing session token — go back to login".into();
    }

    let root = spawn_screen_root(&mut commands);
    let (panel_n, panel_bg, panel_bd) = panel_node(560.0);
    let panel = commands.spawn((panel_n, panel_bg, panel_bd)).id();
    commands.entity(root).add_child(panel);

    let title = commands
        .spawn((
            Text::new("Character Select"),
            TextFont::from_font_size(34.0),
            TextColor(GOLD),
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
        ))
        .id();
    commands.entity(panel).add_child(title);

    let roster = commands
        .spawn((
            RosterRoot,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(120.0),
                max_height: Val::Px(220.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                align_items: AlignItems::Stretch,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(panel).add_child(roster);

    let create_panel = commands
        .spawn((
            CreatePanel,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                display: Display::None,
                ..default()
            },
        ))
        .id();
    commands.entity(panel).add_child(create_panel);

    commands.entity(create_panel).with_children(|p| {
        p.spawn((
            Button,
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
                CreateNameLabel,
                Text::new(format!("Name  {}_", form.name)),
                TextFont::from_font_size(18.0),
                TextColor(BODY),
            ));
        });
    });

    spawn_class_grid(&mut commands, create_panel, form.class);

    commands.entity(create_panel).with_children(|p| {
        p.spawn((
            CreateDetailLabel,
            Text::new(class_detail_line(form.class)),
            TextFont::from_font_size(14.0),
            TextColor(MUTED),
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
        ));
        p.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Primary);
            row.spawn((b, k, n, bg, bd, CreateSubmitBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Create"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Secondary);
            row.spawn((b, k, n, bg, bd, CreateCancelBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Cancel"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
        });
    });

    commands.entity(panel).with_children(|p| {
        p.spawn((
            SelectStatusLabel,
            Text::new(form.status.clone()),
            TextFont::from_font_size(15.0),
            TextColor(status_color(true, &form.status)),
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
        ));

        p.spawn((
            SelectModeActions,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|row| {
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Primary);
            row.spawn((b, k, n, bg, bd, SelectEnterBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Enter world"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Secondary);
            row.spawn((b, k, n, bg, bd, SelectCreateBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("New character"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Danger);
            row.spawn((b, k, n, bg, bd, SelectDeleteBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Delete"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Secondary);
            row.spawn((b, k, n, bg, bd, SelectBackBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Logout"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
        });

        p.spawn((
            SelectHintLabel,
            Text::new(hint_line(&form)),
            TextFont::from_font_size(13.0),
            TextColor(MUTED),
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
        ));
    });
}

fn hint_line(form: &CharSelectForm) -> String {
    if form.creating {
        "Type name · click class or ←/→ · Create · Esc cancel".into()
    } else if form.delete_armed.is_some() {
        "Delete armed — press Delete again to confirm, Esc to cancel".into()
    } else {
        "Click or ↑/↓ select · Enter world · N create · D delete · Esc logout".into()
    }
}

fn start_create(form: &mut CharSelectForm) {
    form.creating = true;
    form.delete_armed = None;
    form.status = "Name your character and pick a class".into();
}

fn cancel_create(form: &mut CharSelectForm) {
    form.creating = false;
    form.status = "Select a character".into();
}

fn logout(session: &mut AuthSession, next: &mut NextState<AppState>) {
    session.token = None;
    session.account_id = None;
    session.characters.clear();
    session.selected = None;
    next.set(AppState::Login);
}

fn enter_selected(
    form: &mut CharSelectForm,
    session: &mut AuthSession,
    pending_char: &mut PendingChar,
) {
    if session.characters.is_empty() {
        form.status = "No characters — create one first".into();
        return;
    }
    let Some(token) = session.token.clone() else {
        form.status = "Missing token".into();
        return;
    };
    let idx = form.cursor.min(session.characters.len() - 1);
    let id = session.characters[idx].id;
    session.selected = Some(id);
    form.busy = true;
    form.delete_armed = None;
    form.status = "Entering world…".into();
    pending_char.0 = Some(Mutex::new(api::spawn_enter_character(token, id)));
}

fn create_character(
    form: &mut CharSelectForm,
    session: &AuthSession,
    pending_char: &mut PendingChar,
) {
    let Some(token) = session.token.clone() else {
        form.status = "Missing token".into();
        return;
    };
    let name = form.name.trim().to_string();
    if name.is_empty() {
        form.status = "Name required".into();
        return;
    }
    form.busy = true;
    form.status = "Creating…".into();
    pending_char.0 = Some(Mutex::new(api::spawn_create_character(
        token,
        name,
        form.class.as_str().to_string(),
    )));
}

fn arm_or_delete(
    form: &mut CharSelectForm,
    session: &AuthSession,
    pending_delete: &mut PendingDelete,
) {
    if session.characters.is_empty() {
        form.status = "Nothing to delete".into();
        return;
    }
    let idx = form.cursor.min(session.characters.len() - 1);
    let id = session.characters[idx].id;
    let name = session.characters[idx].name.clone();
    if form.delete_armed == Some(id) {
        let Some(token) = session.token.clone() else {
            form.status = "Missing token".into();
            return;
        };
        form.busy = true;
        form.deleting = Some(id);
        form.delete_armed = None;
        form.status = format!("Deleting {name}…");
        pending_delete.0 = Some(Mutex::new(api::spawn_delete_character(token, id)));
    } else {
        form.delete_armed = Some(id);
        form.status = format!("Delete {name}? Press Delete again to confirm");
    }
}

fn char_select_clicks(
    mut form: ResMut<CharSelectForm>,
    mut session: ResMut<AuthSession>,
    mut pending_char: ResMut<PendingChar>,
    mut pending_delete: ResMut<PendingDelete>,
    mut next: ResMut<NextState<AppState>>,
    rows: Query<(&Interaction, &CharRowButton), Changed<Interaction>>,
    class_btns: Query<(&Interaction, &ClassPickButton), Changed<Interaction>>,
    enter_btn: Query<&Interaction, (Changed<Interaction>, With<SelectEnterBtn>)>,
    create_btn: Query<&Interaction, (Changed<Interaction>, With<SelectCreateBtn>)>,
    delete_btn: Query<&Interaction, (Changed<Interaction>, With<SelectDeleteBtn>)>,
    back_btn: Query<&Interaction, (Changed<Interaction>, With<SelectBackBtn>)>,
    create_submit: Query<&Interaction, (Changed<Interaction>, With<CreateSubmitBtn>)>,
    create_cancel: Query<&Interaction, (Changed<Interaction>, With<CreateCancelBtn>)>,
) {
    if form.busy {
        return;
    }

    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed && !form.creating {
            form.cursor = row.index;
            form.delete_armed = None;
        }
    }
    for (interaction, pick) in &class_btns {
        if *interaction == Interaction::Pressed && form.creating {
            form.class = pick.0;
        }
    }
    for interaction in &enter_btn {
        if *interaction == Interaction::Pressed && !form.creating {
            enter_selected(&mut form, &mut session, &mut pending_char);
        }
    }
    for interaction in &create_btn {
        if *interaction == Interaction::Pressed && !form.creating {
            start_create(&mut form);
        }
    }
    for interaction in &delete_btn {
        if *interaction == Interaction::Pressed && !form.creating {
            arm_or_delete(&mut form, &session, &mut pending_delete);
        }
    }
    for interaction in &back_btn {
        if *interaction == Interaction::Pressed && !form.creating {
            logout(&mut session, &mut next);
            return;
        }
    }
    for interaction in &create_submit {
        if *interaction == Interaction::Pressed && form.creating {
            create_character(&mut form, &session, &mut pending_char);
        }
    }
    for interaction in &create_cancel {
        if *interaction == Interaction::Pressed && form.creating {
            cancel_create(&mut form);
        }
    }
}

fn char_select_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut form: ResMut<CharSelectForm>,
    mut session: ResMut<AuthSession>,
    mut pending_char: ResMut<PendingChar>,
    mut pending_delete: ResMut<PendingDelete>,
    mut next: ResMut<NextState<AppState>>,
    mut events: EventReader<bevy::input::keyboard::KeyboardInput>,
) {
    if form.busy {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if form.creating {
            cancel_create(&mut form);
        } else if form.delete_armed.is_some() {
            form.delete_armed = None;
            form.status = "Delete cancelled".into();
        } else {
            logout(&mut session, &mut next);
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
            create_character(&mut form, &session, &mut pending_char);
            keys.clear();
        }
    } else {
        // Drain text events so they don't leak into other screens.
        events.clear();
        if keys.just_pressed(KeyCode::KeyN) {
            start_create(&mut form);
        }
        if keys.just_pressed(KeyCode::KeyD) || keys.just_pressed(KeyCode::Delete) {
            arm_or_delete(&mut form, &session, &mut pending_delete);
        }
        if !session.characters.is_empty() {
            if keys.just_pressed(KeyCode::ArrowUp) {
                form.cursor = form.cursor.saturating_sub(1);
                form.delete_armed = None;
            }
            if keys.just_pressed(KeyCode::ArrowDown) {
                form.cursor = (form.cursor + 1).min(session.characters.len() - 1);
                form.delete_armed = None;
            }
            if keys.just_pressed(KeyCode::Enter) {
                enter_selected(&mut form, &mut session, &mut pending_char);
                keys.clear();
            }
        }
    }
}

fn poll_list_result(
    mut form: ResMut<CharSelectForm>,
    mut pending: ResMut<PendingList>,
    mut session: ResMut<AuthSession>,
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
            form.roster_dirty = true;
            form.status = if session.characters.is_empty() {
                "No characters yet — create one".into()
            } else {
                format!("{} character(s) ready", session.characters.len())
            };
        }
        Ok(ListResult::Err(msg)) => {
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
                form.roster_dirty = true;
                form.busy = false;
                form.status = format!("Created {} — Enter world to play", c.name);
            } else {
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

fn poll_delete_result(
    mut form: ResMut<CharSelectForm>,
    mut pending: ResMut<PendingDelete>,
    mut session: ResMut<AuthSession>,
) {
    let Some(mutex) = pending.0.as_ref() else {
        return;
    };
    let Ok(guard) = mutex.lock() else {
        return;
    };
    match guard.try_recv() {
        Ok(Ok(())) => {
            drop(guard);
            pending.0 = None;
            if let Some(id) = form.deleting.take() {
                session.characters.retain(|c| c.id != id);
            }
            if form.cursor >= session.characters.len() && form.cursor > 0 {
                form.cursor = session.characters.len() - 1;
            }
            form.roster_dirty = true;
            form.busy = false;
            form.status = if session.characters.is_empty() {
                "Character deleted — roster empty".into()
            } else {
                "Character deleted".into()
            };
        }
        Ok(Err(msg)) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.deleting = None;
            form.delete_armed = None;
            form.status = msg;
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            drop(guard);
            pending.0 = None;
            form.busy = false;
            form.deleting = None;
            form.status = "Delete thread disconnected".into();
        }
    }
}

fn rebuild_roster_if_needed(
    mut commands: Commands,
    mut form: ResMut<CharSelectForm>,
    session: Res<AuthSession>,
    roster_q: Query<Entity, With<RosterRoot>>,
) {
    if !form.roster_dirty {
        return;
    }
    form.roster_dirty = false;
    let Ok(roster) = roster_q.single() else {
        return;
    };
    commands.entity(roster).despawn_related::<Children>();

    if session.characters.is_empty() {
        commands.entity(roster).with_children(|p| {
            p.spawn((
                Text::new("No characters yet"),
                TextFont::from_font_size(16.0),
                TextColor(MUTED),
                Node {
                    padding: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
            ));
        });
        return;
    }

    let cursor = form.cursor.min(session.characters.len() - 1);
    let chars: Vec<_> = session.characters.clone();
    commands.entity(roster).with_children(|p| {
        for (i, c) in chars.iter().enumerate() {
            let selected = i == cursor;
            let label = format!("{}  ·  {}  ·  Lv{}", c.name, c.class_id, c.level);
            p.spawn((
                Button,
                CharRowButton { index: i },
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(36.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if selected { ROW_SELECTED } else { ROW_IDLE }),
                BorderColor(Color::srgba(0.35, 0.4, 0.48, 0.4)),
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(label),
                    TextFont::from_font_size(16.0),
                    TextColor(if selected { GOLD } else { BODY }),
                ));
            });
        }
    });
}

fn refresh_char_select_chrome(
    form: Res<CharSelectForm>,
    session: Res<AuthSession>,
    mut status_q: Query<
        (&mut Text, &mut TextColor),
        (With<SelectStatusLabel>, Without<menu_ui::ClassPickLabel>),
    >,
    mut hint_q: Query<
        &mut Text,
        (
            With<SelectHintLabel>,
            Without<SelectStatusLabel>,
            Without<CreateNameLabel>,
        ),
    >,
    mut create_name_q: Query<&mut Text, (With<CreateNameLabel>, Without<SelectStatusLabel>)>,
    mut create_detail_q: Query<
        &mut Text,
        (
            With<CreateDetailLabel>,
            Without<SelectStatusLabel>,
            Without<CreateNameLabel>,
            Without<SelectHintLabel>,
        ),
    >,
    mut create_panel: Query<&mut Node, (With<CreatePanel>, Without<SelectModeActions>)>,
    mut select_actions: Query<&mut Node, (With<SelectModeActions>, Without<CreatePanel>)>,
    mut roster: Query<
        &mut Node,
        (
            With<RosterRoot>,
            Without<CreatePanel>,
            Without<SelectModeActions>,
        ),
    >,
    class_btns: Query<(&ClassPickButton, &mut BackgroundColor, &Children)>,
    texts: Query<&mut TextColor, With<menu_ui::ClassPickLabel>>,
    mut rows: Query<(&CharRowButton, &mut BackgroundColor, &Children), Without<ClassPickButton>>,
    mut row_texts: Query<
        &mut TextColor,
        (Without<menu_ui::ClassPickLabel>, Without<SelectStatusLabel>),
    >,
) {
    if let Ok((mut text, mut color)) = status_q.single_mut() {
        **text = form.status.clone();
        *color = TextColor(status_color(form.busy, &form.status));
    }
    if let Ok(mut text) = hint_q.single_mut() {
        **text = hint_line(&form);
    }
    if let Ok(mut text) = create_name_q.single_mut() {
        **text = format!("Name  {}_", form.name);
    }
    if let Ok(mut text) = create_detail_q.single_mut() {
        **text = class_detail_line(form.class);
    }
    if let Ok(mut node) = create_panel.single_mut() {
        node.display = if form.creating {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = select_actions.single_mut() {
        node.display = if form.creating {
            Display::None
        } else {
            Display::Flex
        };
    }
    if let Ok(mut node) = roster.single_mut() {
        node.display = if form.creating {
            Display::None
        } else {
            Display::Flex
        };
    }

    if form.creating {
        menu_ui::sync_class_pick_chrome(form.class, class_btns, texts);
    } else if !session.characters.is_empty() {
        let cursor = form.cursor.min(session.characters.len() - 1);
        for (row, mut bg, children) in &mut rows {
            let on = row.index == cursor;
            *bg = BackgroundColor(if on { ROW_SELECTED } else { ROW_IDLE });
            for child in children.iter() {
                if let Ok(mut color) = row_texts.get_mut(child) {
                    *color = TextColor(if on { GOLD } else { BODY });
                }
            }
        }
    }
}
