//! World-space nameplates projected to screen UI for nearby NPCs / mobs / herbs.

use bevy::prelude::*;
use woc_content::npc;
use woc_protocol::{EntityId, EntityKind};
use woc_sim::visual_spec;

use crate::visuals::SimVisual;
use crate::{AppState, GameHost};

const NAMEPLATE_RANGE: f32 = 42.0;
const MAX_PLATES: usize = 24;

#[derive(Component)]
struct NameplateUi {
    entity_id: EntityId,
}

#[derive(Component)]
struct NameplateRoot;

#[derive(Component)]
struct NameplateLabel;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::InWorld), setup_nameplates)
        .add_systems(OnExit(AppState::InWorld), cleanup_nameplates)
        .add_systems(Update, sync_nameplates.run_if(in_state(AppState::InWorld)));
}

fn setup_nameplates(mut commands: Commands) {
    commands
        .spawn((
            NameplateRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            // Pass clicks through to the 3D world / HUD.
            ZIndex(5),
        ))
        .with_children(|root| {
            for _ in 0..MAX_PLATES {
                root.spawn((
                    NameplateUi {
                        entity_id: u32::MAX,
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-1000.0),
                        top: Val::Px(-1000.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.04, 0.06, 0.55)),
                    Visibility::Hidden,
                ))
                .with_children(|plate| {
                    plate.spawn((
                        NameplateLabel,
                        Text::new(""),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgb(0.92, 0.94, 0.88)),
                    ));
                });
            }
        });
}

fn cleanup_nameplates(mut commands: Commands, q: Query<Entity, With<NameplateRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn sync_nameplates(
    host: Res<GameHost>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    windows: Query<&Window>,
    visuals: Query<(&SimVisual, &GlobalTransform)>,
    mut plates: Query<(&mut NameplateUi, &mut Node, &mut Visibility, &Children)>,
    mut labels: Query<(&mut Text, &mut TextColor), With<NameplateLabel>>,
) {
    let Ok((camera, cam_tf)) = cameras.single() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(player) = host.player_snap() else {
        hide_all(&mut plates);
        return;
    };

    let mut candidates: Vec<(EntityId, f32, String, Color, f32, f32, f32)> = Vec::new();
    for e in &host.snapshot.entities {
        if e.id == host.snapshot.player_id {
            continue;
        }
        if !matches!(
            e.kind,
            EntityKind::Npc | EntityKind::Mob | EntityKind::Pet | EntityKind::Loot
        ) {
            continue;
        }
        if !e.alive && e.kind != EntityKind::Mob && e.kind != EntityKind::Npc {
            continue;
        }
        let dx = e.x - player.x;
        let dz = e.z - player.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > NAMEPLATE_RANGE {
            continue;
        }
        let spec = visual_spec(e.kind, e.template_id.as_deref());
        let label = format_label(e.kind, &e.name, e.level, e.template_id.as_deref(), e.alive);
        let color = label_color(e.kind, e.template_id.as_deref(), e.alive);
        candidates.push((e.id, dist, label, color, e.x, e.y + spec.label_height, e.z));
    }
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(MAX_PLATES);

    let vis_pos: std::collections::HashMap<EntityId, Vec3> = visuals
        .iter()
        .map(|(v, gt)| (v.id, gt.translation()))
        .collect();

    let mut plate_iter = plates.iter_mut();
    for (id, _dist, label, color, x, y, z) in &candidates {
        let Some((mut ui, mut node, mut vis, children)) = plate_iter.next() else {
            break;
        };
        let world = vis_pos
            .get(id)
            .copied()
            .map(|p| {
                let spec_y = host
                    .snapshot
                    .entities
                    .iter()
                    .find(|e| e.id == *id)
                    .map(|e| visual_spec(e.kind, e.template_id.as_deref()).label_height)
                    .unwrap_or(2.0);
                Vec3::new(p.x, p.y + spec_y, p.z)
            })
            .unwrap_or(Vec3::new(*x, *y, *z));

        let Ok(viewport) = camera.world_to_viewport(cam_tf, world) else {
            *vis = Visibility::Hidden;
            ui.entity_id = u32::MAX;
            continue;
        };
        if viewport.x < 0.0
            || viewport.y < 0.0
            || viewport.x > window.width()
            || viewport.y > window.height()
        {
            *vis = Visibility::Hidden;
            ui.entity_id = u32::MAX;
            continue;
        }

        ui.entity_id = *id;
        *vis = Visibility::Visible;
        node.left = Val::Px(viewport.x - 40.0);
        node.top = Val::Px(viewport.y - 18.0);
        if let Some(child) = children.first() {
            if let Ok((mut text, mut text_color)) = labels.get_mut(*child) {
                **text = label.clone();
                *text_color = TextColor(*color);
            }
        }
    }
    for (mut ui, mut node, mut vis, _) in plate_iter {
        ui.entity_id = u32::MAX;
        *vis = Visibility::Hidden;
        node.left = Val::Px(-1000.0);
        node.top = Val::Px(-1000.0);
    }
}

fn hide_all(plates: &mut Query<(&mut NameplateUi, &mut Node, &mut Visibility, &Children)>) {
    for (mut ui, mut node, mut vis, _) in plates.iter_mut() {
        ui.entity_id = u32::MAX;
        *vis = Visibility::Hidden;
        node.left = Val::Px(-1000.0);
        node.top = Val::Px(-1000.0);
    }
}

fn format_label(
    kind: EntityKind,
    name: &str,
    level: u32,
    template_id: Option<&str>,
    alive: bool,
) -> String {
    let corpse = if alive { "" } else { " (dead)" };
    match kind {
        EntityKind::Npc => {
            let role = npc(template_id.unwrap_or(""))
                .map(|n| {
                    if n.is_quest_giver && n.is_vendor {
                        " [!][$]"
                    } else if n.is_quest_giver {
                        " [!]"
                    } else if n.is_vendor {
                        " [$]"
                    } else {
                        ""
                    }
                })
                .unwrap_or("");
            format!("{name}{role}")
        }
        EntityKind::Mob => format!("Lv{level} {name}{corpse}"),
        EntityKind::Pet => format!("{name}"),
        EntityKind::Loot => {
            if template_id.is_some_and(|t| woc_content::gather_node(t).is_some()) {
                format!("✦ {name}")
            } else {
                format!("Loot")
            }
        }
        EntityKind::Player => name.to_string(),
    }
}

fn label_color(kind: EntityKind, template_id: Option<&str>, alive: bool) -> Color {
    if !alive {
        return Color::srgb(0.55, 0.55, 0.55);
    }
    match kind {
        EntityKind::Npc => {
            if npc(template_id.unwrap_or("")).is_some_and(|n| n.is_quest_giver) {
                Color::srgb(0.95, 0.85, 0.35)
            } else if npc(template_id.unwrap_or("")).is_some_and(|n| n.is_vendor) {
                Color::srgb(0.45, 0.85, 0.55)
            } else {
                Color::srgb(0.75, 0.90, 0.70)
            }
        }
        EntityKind::Mob => Color::srgb(0.95, 0.55, 0.45),
        EntityKind::Pet => Color::srgb(0.55, 0.75, 0.95),
        EntityKind::Loot => Color::srgb(0.95, 0.85, 0.40),
        EntityKind::Player => Color::srgb(0.70, 0.80, 0.95),
    }
}
