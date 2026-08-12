//! Title screen systems (Offline | Online mode picker).

use bevy::prelude::*;
use woc_version::footer;

use crate::menu_ui::{
    self, button_bundle, panel_node, spawn_screen_root, MenuBtnKind, BODY, GOLD, MUTED,
};
use crate::{cleanup_ui, AppState, PlayMode};

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Title), setup_title)
        .add_systems(OnExit(AppState::Title), cleanup_ui)
        .add_systems(
            Update,
            (
                menu_ui::menu_button_visuals,
                title_clicks,
                title_input,
                refresh_title_mode,
            )
                .chain()
                .run_if(in_state(AppState::Title)),
        );
}

#[derive(Component)]
struct ModeLabel;

#[derive(Component)]
struct OfflineBtn;

#[derive(Component)]
struct OnlineBtn;

#[derive(Component)]
struct ContinueBtn;

fn setup_title(mut commands: Commands, mode: Res<PlayMode>) {
    let root = spawn_screen_root(&mut commands);
    let (panel_n, panel_bg, panel_bd) = panel_node(480.0);
    commands.entity(root).with_children(|screen| {
        screen
            .spawn((panel_n, panel_bg, panel_bd))
            .with_children(|p| {
                p.spawn((
                    Text::new("World of ClaudeCraft"),
                    TextFont::from_font_size(40.0),
                    TextColor(GOLD),
                    Node {
                        align_self: AlignSelf::Center,
                        ..default()
                    },
                ));
                p.spawn((
                    Text::new("Rust rewrite · framework slice"),
                    TextFont::from_font_size(16.0),
                    TextColor(MUTED),
                    Node {
                        align_self: AlignSelf::Center,
                        ..default()
                    },
                ));
                p.spawn((
                    Text::new(footer()),
                    TextFont::from_font_size(13.0),
                    TextColor(MUTED),
                    Node {
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                ));

                p.spawn((
                    ModeLabel,
                    Text::new(mode_prompt(*mode)),
                    TextFont::from_font_size(18.0),
                    TextColor(BODY),
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
                    row.spawn((b, k, n, bg, bd, OfflineBtn))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("1  Offline"),
                                TextFont::from_font_size(16.0),
                                TextColor(BODY),
                            ));
                        });
                    let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Secondary);
                    row.spawn((b, k, n, bg, bd, OnlineBtn))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("2  Online"),
                                TextFont::from_font_size(16.0),
                                TextColor(BODY),
                            ));
                        });
                });

                p.spawn(Node {
                    width: Val::Percent(100.0),
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|row| {
                    let (b, k, n, bg, bd) = button_bundle(MenuBtnKind::Primary);
                    row.spawn((
                        b,
                        k,
                        Node {
                            width: Val::Percent(100.0),
                            ..n
                        },
                        bg,
                        bd,
                        ContinueBtn,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Continue"),
                            TextFont::from_font_size(16.0),
                            TextColor(BODY),
                        ));
                    });
                });

                p.spawn((
                    Text::new("1 / 2 or ←/→ choose mode · Enter / click Continue"),
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

fn mode_prompt(mode: PlayMode) -> String {
    match mode {
        PlayMode::Offline => "Mode: Offline  (local sim)".into(),
        PlayMode::Online => format!("Mode: Online  ({} → login)", crate::online::ONLINE_WS_URL),
    }
}

fn continue_from_title(mode: PlayMode, next: &mut NextState<AppState>) {
    match mode {
        PlayMode::Offline => next.set(AppState::CharCreate),
        PlayMode::Online => next.set(AppState::Login),
    }
}

fn title_clicks(
    mut mode: ResMut<PlayMode>,
    mut next: ResMut<NextState<AppState>>,
    offline: Query<&Interaction, (Changed<Interaction>, With<OfflineBtn>)>,
    online: Query<&Interaction, (Changed<Interaction>, With<OnlineBtn>)>,
    cont: Query<&Interaction, (Changed<Interaction>, With<ContinueBtn>)>,
) {
    for interaction in &offline {
        if *interaction == Interaction::Pressed {
            *mode = PlayMode::Offline;
        }
    }
    for interaction in &online {
        if *interaction == Interaction::Pressed {
            *mode = PlayMode::Online;
        }
    }
    for interaction in &cont {
        if *interaction == Interaction::Pressed {
            continue_from_title(*mode, &mut next);
        }
    }
}

fn title_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<PlayMode>,
    mut next: ResMut<NextState<AppState>>,
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
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        continue_from_title(*mode, &mut next);
    }
}

fn refresh_title_mode(mode: Res<PlayMode>, mut label: Query<&mut Text, With<ModeLabel>>) {
    if let Ok(mut text) = label.single_mut() {
        **text = mode_prompt(*mode);
    }
}
