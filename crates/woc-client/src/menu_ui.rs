//! Shared visual language + helpers for title / login / character screens.

use bevy::prelude::*;
use woc_content::{class_def, PlayerClass, ResourceType};

/// Gold brand / headings.
pub(crate) const GOLD: Color = Color::srgb(0.95, 0.86, 0.55);
/// Soft body text.
pub(crate) const BODY: Color = Color::srgb(0.85, 0.9, 0.95);
/// Muted hints.
pub(crate) const MUTED: Color = Color::srgb(0.68, 0.74, 0.8);
/// Success / affirmative.
pub(crate) const OK: Color = Color::srgb(0.55, 0.88, 0.65);
/// Error / warning.
pub(crate) const ERR: Color = Color::srgb(0.95, 0.55, 0.5);
/// Busy / in-flight.
pub(crate) const BUSY: Color = Color::srgb(0.75, 0.82, 0.95);

pub(crate) const PANEL_BG: Color = Color::srgba(0.06, 0.09, 0.14, 0.94);
pub(crate) const FIELD_BG: Color = Color::srgba(0.08, 0.11, 0.16, 0.95);
pub(crate) const FIELD_FOCUS: Color = Color::srgba(0.12, 0.18, 0.26, 0.98);
pub(crate) const BTN_BG: Color = Color::srgba(0.14, 0.2, 0.28, 0.95);
pub(crate) const BTN_HOVER: Color = Color::srgba(0.2, 0.3, 0.4, 0.98);
pub(crate) const BTN_ACTIVE: Color = Color::srgba(0.28, 0.42, 0.32, 0.98);
pub(crate) const CLASS_IDLE: Color = Color::srgba(0.1, 0.13, 0.18, 0.92);
pub(crate) const CLASS_SELECTED: Color = Color::srgba(0.32, 0.28, 0.14, 0.98);
pub(crate) const ROW_IDLE: Color = Color::srgba(0.09, 0.12, 0.17, 0.9);
pub(crate) const ROW_SELECTED: Color = Color::srgba(0.18, 0.28, 0.22, 0.96);
pub(crate) const SCREEN_BG: Color = Color::srgba(0.04, 0.06, 0.1, 0.86);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClassPickButton(pub(crate) PlayerClass);

#[derive(Component)]
pub(crate) struct ClassPickLabel;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuBtnKind {
    Primary,
    Secondary,
    Danger,
}

/// Hover / press chrome for menu buttons (shared across pre-world screens).
pub(crate) fn menu_button_visuals(
    mut q: Query<
        (&Interaction, &MenuBtnKind, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, kind, mut bg) in &mut q {
        *bg = match (*interaction, *kind) {
            (Interaction::Pressed, MenuBtnKind::Danger) => {
                BackgroundColor(Color::srgba(0.55, 0.18, 0.16, 0.98))
            }
            (Interaction::Hovered, MenuBtnKind::Danger) => {
                BackgroundColor(Color::srgba(0.42, 0.16, 0.14, 0.96))
            }
            (Interaction::None, MenuBtnKind::Danger) => {
                BackgroundColor(Color::srgba(0.32, 0.12, 0.12, 0.94))
            }
            (Interaction::Pressed, MenuBtnKind::Primary) => BackgroundColor(BTN_ACTIVE),
            (Interaction::Hovered, MenuBtnKind::Primary) => BackgroundColor(BTN_HOVER),
            (Interaction::None, MenuBtnKind::Primary) => BackgroundColor(BTN_BG),
            (Interaction::Pressed, MenuBtnKind::Secondary) => {
                BackgroundColor(Color::srgba(0.22, 0.24, 0.3, 0.98))
            }
            (Interaction::Hovered, MenuBtnKind::Secondary) => {
                BackgroundColor(Color::srgba(0.16, 0.18, 0.24, 0.96))
            }
            (Interaction::None, MenuBtnKind::Secondary) => {
                BackgroundColor(Color::srgba(0.12, 0.14, 0.2, 0.94))
            }
        };
    }
}

/// Refresh class-tile chrome when the selected class changes.
pub(crate) fn sync_class_pick_chrome(
    selected: PlayerClass,
    mut q: Query<(&ClassPickButton, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut TextColor, With<ClassPickLabel>>,
) {
    for (btn, mut bg, children) in &mut q {
        let on = btn.0 == selected;
        *bg = BackgroundColor(if on { CLASS_SELECTED } else { CLASS_IDLE });
        for child in children.iter() {
            if let Ok(mut color) = texts.get_mut(child) {
                *color = TextColor(if on { GOLD } else { BODY });
            }
        }
    }
}

pub(crate) fn resource_label(rt: ResourceType) -> &'static str {
    match rt {
        ResourceType::Rage => "Rage",
        ResourceType::Mana => "Mana",
        ResourceType::Energy => "Energy",
    }
}

pub(crate) fn class_detail_line(class: PlayerClass) -> String {
    let def = class_def(class);
    format!(
        "{} · {} · HP {:.0} · {} {:.0} · {}",
        def.name,
        resource_label(def.resource_type),
        def.base_hp,
        resource_label(def.resource_type),
        def.resource_max,
        def.primary_ability.replace('_', " ")
    )
}

/// Full-screen column root used by pre-world menus.
pub(crate) fn spawn_screen_root(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            crate::UiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(24.0)),
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(SCREEN_BG),
        ))
        .id()
}

pub(crate) fn panel_node(width: f32) -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            width: Val::Px(width),
            max_width: Val::Percent(94.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            padding: UiRect::axes(Val::Px(28.0), Val::Px(22.0)),
            row_gap: Val::Px(12.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
        BorderColor(Color::srgba(0.35, 0.4, 0.48, 0.55)),
    )
}

pub(crate) fn button_bundle(
    kind: MenuBtnKind,
) -> (Button, MenuBtnKind, Node, BackgroundColor, BorderColor) {
    let bg = match kind {
        MenuBtnKind::Primary => BTN_BG,
        MenuBtnKind::Secondary => Color::srgba(0.12, 0.14, 0.2, 0.94),
        MenuBtnKind::Danger => Color::srgba(0.32, 0.12, 0.12, 0.94),
    };
    (
        Button,
        kind,
        Node {
            min_height: Val::Px(36.0),
            padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(bg),
        BorderColor(Color::srgba(0.4, 0.45, 0.55, 0.45)),
    )
}

/// Spawn a 3×3 class grid as a child of `parent`.
pub(crate) fn spawn_class_grid(commands: &mut Commands, parent: Entity, selected: PlayerClass) {
    commands.entity(parent).with_children(|root| {
        root.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            align_items: AlignItems::Stretch,
            ..default()
        })
        .with_children(|grid| {
            for row in 0..3 {
                grid.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|row_node| {
                    for col in 0..3 {
                        let class = PlayerClass::ALL[row * 3 + col];
                        let on = class == selected;
                        let name = class_def(class).name;
                        row_node
                            .spawn((
                                Button,
                                ClassPickButton(class),
                                Node {
                                    flex_grow: 1.0,
                                    min_height: Val::Px(40.0),
                                    padding: UiRect::axes(Val::Px(6.0), Val::Px(8.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(if on { CLASS_SELECTED } else { CLASS_IDLE }),
                                BorderColor(Color::srgba(0.4, 0.42, 0.5, 0.4)),
                            ))
                            .with_children(|cell| {
                                cell.spawn((
                                    ClassPickLabel,
                                    Text::new(name),
                                    TextFont::from_font_size(15.0),
                                    TextColor(if on { GOLD } else { BODY }),
                                ));
                            });
                    }
                });
            }
        });
    });
}

pub(crate) fn status_color(busy: bool, text: &str) -> Color {
    if busy {
        return BUSY;
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("fail")
        || lower.contains("error")
        || lower.contains("invalid")
        || lower.contains("missing")
        || lower.contains("required")
        || lower.contains("taken")
        || lower.contains("mismatch")
        || lower.starts_with("version:")
        || lower.contains("outdated")
        || lower.contains("unreachable")
        || lower.contains("short")
        || lower.starts_with("http")
        || lower.contains("disconnect")
        || lower.contains("enter username")
        || lower.contains("password must")
        || lower.contains("confirm")
    {
        ERR
    } else if lower.contains("created")
        || lower.contains("authenticated")
        || lower.contains("ready")
        || lower.contains("character(s)")
        || lower.contains("deleted")
    {
        OK
    } else {
        MUTED
    }
}
