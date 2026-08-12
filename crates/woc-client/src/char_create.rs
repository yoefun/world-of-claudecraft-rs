//! Character create / class select (offline path + shared class grid).

use bevy::prelude::*;
use woc_content::PlayerClass;

use crate::menu_ui::{
    self, button_bundle, class_detail_line, panel_node, spawn_class_grid, spawn_screen_root,
    ClassPickButton, MenuBtnKind, BODY, FIELD_FOCUS, GOLD, MUTED,
};
use crate::{cleanup_ui, AppState, PlayMode};

#[derive(Resource)]
pub(crate) struct CharName(pub(crate) String);

#[derive(Resource)]
pub(crate) struct SelectedClass(pub(crate) PlayerClass);

#[derive(Component)]
struct NameInputDisplay;

#[derive(Component)]
struct ClassDetailLabel;

#[derive(Component)]
struct CreateEnterBtn;

#[derive(Component)]
struct CreateBackBtn;

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(CharName("Aldric".into()))
        .insert_resource(SelectedClass(PlayerClass::Warrior))
        .add_systems(OnEnter(AppState::CharCreate), setup_char_create)
        .add_systems(OnExit(AppState::CharCreate), cleanup_ui)
        .add_systems(
            Update,
            (
                menu_ui::menu_button_visuals,
                char_create_clicks,
                char_create_input,
                refresh_char_create,
            )
                .chain()
                .run_if(in_state(AppState::CharCreate)),
        );
}

fn setup_char_create(
    mut commands: Commands,
    name: Res<CharName>,
    class: Res<SelectedClass>,
    mode: Res<PlayMode>,
) {
    let mode_hint = match *mode {
        PlayMode::Offline => "Offline · local Eastbrook sim".to_string(),
        PlayMode::Online => format!("Online · {}", crate::online::ONLINE_WS_URL),
    };

    let root = spawn_screen_root(&mut commands);
    let (panel_n, panel_bg, panel_bd) = panel_node(520.0);
    let panel = commands.spawn((panel_n, panel_bg, panel_bd)).id();
    commands.entity(root).add_child(panel);

    commands.entity(panel).with_children(|p| {
        p.spawn((
            Text::new("Create Character"),
            TextFont::from_font_size(34.0),
            TextColor(GOLD),
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
        ));
        p.spawn((
            Text::new(mode_hint),
            TextFont::from_font_size(14.0),
            TextColor(MUTED),
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
        ));

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
                NameInputDisplay,
                Text::new(format!("Name  {}_", name.0)),
                TextFont::from_font_size(18.0),
                TextColor(BODY),
            ));
        });
    });

    spawn_class_grid(&mut commands, panel, class.0);

    commands.entity(panel).with_children(|p| {
        p.spawn((
            ClassDetailLabel,
            Text::new(class_detail_line(class.0)),
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
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .with_children(|row| {
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Primary);
            row.spawn((b, k, n, bg, bd, CreateEnterBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Enter Eastbrook"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
            let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Secondary);
            row.spawn((b, k, n, bg, bd, CreateBackBtn))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Back"),
                        TextFont::from_font_size(16.0),
                        TextColor(BODY),
                    ));
                });
        });

        p.spawn((
            Text::new("Type a name · click class or ←/→ · Enter to play · Esc back"),
            TextFont::from_font_size(13.0),
            TextColor(MUTED),
            Node {
                align_self: AlignSelf::Center,
                ..default()
            },
        ));
    });
}

fn char_create_clicks(
    mut class: ResMut<SelectedClass>,
    mut next: ResMut<NextState<AppState>>,
    mut name: ResMut<CharName>,
    class_btns: Query<(&Interaction, &ClassPickButton), Changed<Interaction>>,
    enter_btn: Query<&Interaction, (Changed<Interaction>, With<CreateEnterBtn>)>,
    back_btn: Query<&Interaction, (Changed<Interaction>, With<CreateBackBtn>)>,
) {
    for (interaction, pick) in &class_btns {
        if *interaction == Interaction::Pressed {
            class.0 = pick.0;
        }
    }
    for interaction in &back_btn {
        if *interaction == Interaction::Pressed {
            next.set(AppState::Title);
            return;
        }
    }
    for interaction in &enter_btn {
        if *interaction == Interaction::Pressed {
            if name.0.trim().is_empty() {
                name.0 = "Aldric".into();
            }
            next.set(AppState::InWorld);
        }
    }
}

fn char_create_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut name: ResMut<CharName>,
    mut class: ResMut<SelectedClass>,
    mut next: ResMut<NextState<AppState>>,
    mut events: EventReader<bevy::input::keyboard::KeyboardInput>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
        keys.clear();
        return;
    }

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
}

fn refresh_char_create(
    name: Res<CharName>,
    class: Res<SelectedClass>,
    mut name_q: Query<&mut Text, With<NameInputDisplay>>,
    mut detail_q: Query<&mut Text, (With<ClassDetailLabel>, Without<NameInputDisplay>)>,
    class_btns: Query<(&ClassPickButton, &mut BackgroundColor, &Children)>,
    texts: Query<&mut TextColor, With<menu_ui::ClassPickLabel>>,
) {
    if let Ok(mut text) = name_q.single_mut() {
        **text = format!("Name  {}_", name.0);
    }
    if let Ok(mut text) = detail_q.single_mut() {
        **text = class_detail_line(class.0);
    }
    menu_ui::sync_class_pick_chrome(class.0, class_btns, texts);
}
