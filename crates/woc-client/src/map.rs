//! Minimap (always-on) + world map window (toggle M).
//!
//! Terrain / marker painting lives in `woc_sim::map_view`; this module owns Bevy
//! UI chrome and the refresh cadence.

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use woc_content::{npc, zone_by_id, ZONES};
use woc_protocol::{EntityId, EntityKind, TickSnapshot};
use woc_sim::quests::npc_quest_offers;
use woc_sim::{
    paint_map_frame, paint_player_arrow, region_for_zone, static_markers_for_region,
    world_to_pixel, MapMarker, MapMarkerKind, MapRegion, WORLD_SEED,
};

use crate::hud::UiFlags;
use crate::{AppState, GameHost};

pub(crate) const MINIMAP_SIZE: u32 = 162;
pub(crate) const MINIMAP_HALF_SPAN: f32 = 48.0;
pub(crate) const WORLD_MAP_WIDTH: u32 = 420;
const MINIMAP_REFRESH_S: f32 = 0.1;
const WORLD_MAP_REFRESH_S: f32 = 0.25;

#[derive(Component)]
pub(crate) struct MinimapImage;

#[derive(Component)]
pub(crate) struct MinimapZoneText;

#[derive(Component)]
pub(crate) struct MinimapCoordText;

#[derive(Component)]
pub(crate) struct WorldMapPanel;

#[derive(Component)]
pub(crate) struct WorldMapImage;

#[derive(Component)]
pub(crate) struct WorldMapTitleText;

#[derive(Component)]
pub(crate) struct WorldMapLegendText;

#[derive(Resource)]
pub(crate) struct MapTextures {
    pub(crate) minimap: Handle<Image>,
    pub(crate) world_map: Handle<Image>,
    minimap_acc: f32,
    world_acc: f32,
    last_world_zone: String,
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (update_minimap, update_world_map_panel)
            .chain()
            .run_if(in_state(AppState::InWorld)),
    );
}

/// Allocate blank map textures and register [`MapTextures`].
pub(crate) fn create_map_textures(
    commands: &mut Commands,
    images: &mut Assets<Image>,
) -> (Handle<Image>, Handle<Image>) {
    let minimap = images.add(blank_rgba_image(MINIMAP_SIZE, MINIMAP_SIZE));
    let world_h = world_map_height_for_zone("eastbrook", 0.0);
    let world_map = images.add(blank_rgba_image(WORLD_MAP_WIDTH, world_h));
    commands.insert_resource(MapTextures {
        minimap: minimap.clone(),
        world_map: world_map.clone(),
        minimap_acc: MINIMAP_REFRESH_S,
        world_acc: WORLD_MAP_REFRESH_S,
        last_world_zone: String::new(),
    });
    (minimap, world_map)
}

/// Spawn minimap + world-map chrome under the in-world HUD root.
pub(crate) fn spawn_map_chrome(
    parent: &mut ChildSpawnerCommands<'_>,
    minimap: Handle<Image>,
    world_map: Handle<Image>,
) {
    let world_h = world_map_height_for_zone("eastbrook", 0.0);

    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(MINIMAP_SIZE as f32 + 16.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.82)),
            BorderColor(Color::srgb(0.72, 0.58, 0.28)),
        ))
        .with_children(|cluster| {
            cluster.spawn((
                MinimapZoneText,
                Text::new("Zone"),
                TextFont::from_font_size(14.0),
                TextColor(Color::srgb(0.92, 0.86, 0.62)),
            ));
            cluster.spawn((
                MinimapImage,
                ImageNode::new(minimap),
                Node {
                    width: Val::Px(MINIMAP_SIZE as f32),
                    height: Val::Px(MINIMAP_SIZE as f32),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor(Color::srgb(0.78, 0.62, 0.32)),
                BorderRadius::MAX,
            ));
            cluster.spawn((
                MinimapCoordText,
                Text::new("0.0, 0.0"),
                TextFont::from_font_size(12.0),
                TextColor(Color::srgb(0.78, 0.82, 0.72)),
            ));
            cluster.spawn((
                Text::new("M world map"),
                TextFont::from_font_size(11.0),
                TextColor(Color::srgb(0.62, 0.66, 0.7)),
            ));
        });

    parent
        .spawn((
            WorldMapPanel,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.55)),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(WORLD_MAP_WIDTH as f32 + 48.0),
                        max_height: Val::Percent(92.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.07, 0.11, 0.94)),
                    BorderColor(Color::srgb(0.72, 0.58, 0.28)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        WorldMapTitleText,
                        Text::new("World Map"),
                        TextFont::from_font_size(20.0),
                        TextColor(Color::srgb(0.95, 0.88, 0.62)),
                    ));
                    panel.spawn((
                        WorldMapImage,
                        ImageNode::new(world_map),
                        Node {
                            width: Val::Px(WORLD_MAP_WIDTH as f32),
                            height: Val::Px(world_h as f32),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor(Color::srgb(0.55, 0.48, 0.3)),
                    ));
                    panel.spawn((
                        WorldMapLegendText,
                        Text::new(""),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgb(0.85, 0.88, 0.78)),
                    ));
                    panel.spawn((
                        Text::new("M / Esc close · gold hub · violet portal · yellow/green quest"),
                        TextFont::from_font_size(12.0),
                        TextColor(Color::srgb(0.65, 0.7, 0.75)),
                    ));
                });
        });
}

fn blank_rgba_image(width: u32, height: u32) -> Image {
    let data = vec![0u8; (width * height * 4) as usize];
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn world_map_height_for_zone(zone_id: &str, player_z: f32) -> u32 {
    let region = region_for_zone(zone_id, player_z);
    let aspect = region.span_z() / region.span_x();
    ((WORLD_MAP_WIDTH as f32) * aspect)
        .round()
        .clamp(180.0, 720.0) as u32
}

fn ensure_world_map_size(
    images: &mut Assets<Image>,
    textures: &mut MapTextures,
    zone_id: &str,
    player_z: f32,
) -> u32 {
    let height = world_map_height_for_zone(zone_id, player_z);
    let needs_resize = textures.last_world_zone != zone_id
        || images
            .get(&textures.world_map)
            .map(|img| img.texture_descriptor.size.height != height)
            .unwrap_or(true);
    if needs_resize {
        textures.world_map = images.add(blank_rgba_image(WORLD_MAP_WIDTH, height));
        textures.last_world_zone = zone_id.to_string();
    }
    height
}

pub(crate) fn update_minimap(
    time: Res<Time>,
    host: Res<GameHost>,
    mut textures: Option<ResMut<MapTextures>>,
    mut images: ResMut<Assets<Image>>,
    mut zone_text: Query<&mut Text, With<MinimapZoneText>>,
    mut coord_text: Query<&mut Text, (With<MinimapCoordText>, Without<MinimapZoneText>)>,
) {
    let Some(ref mut textures) = textures else {
        return;
    };
    textures.minimap_acc += time.delta_secs();
    if textures.minimap_acc < MINIMAP_REFRESH_S {
        return;
    }
    textures.minimap_acc = 0.0;

    let snap = &host.snapshot;
    let Some(player) = snap.entities.iter().find(|e| e.id == snap.player_id) else {
        return;
    };

    let region = MapRegion::around(player.x, player.z, MINIMAP_HALF_SPAN);
    let markers = collect_dynamic_markers(snap, region, player.id);
    let mut data = vec![0u8; (MINIMAP_SIZE * MINIMAP_SIZE * 4) as usize];
    paint_map_frame(
        &mut data,
        MINIMAP_SIZE,
        MINIMAP_SIZE,
        region,
        WORLD_SEED,
        &markers,
        true,
    );
    let (mx, my) = world_to_pixel(player.x, player.z, region, MINIMAP_SIZE, MINIMAP_SIZE);
    paint_player_arrow(&mut data, MINIMAP_SIZE, MINIMAP_SIZE, mx, my, player.yaw);

    if let Some(image) = images.get_mut(&textures.minimap) {
        if let Some(buf) = image.data.as_mut() {
            buf.copy_from_slice(&data);
        }
    }

    let zone_label = display_zone_name(snap);
    if let Ok(mut text) = zone_text.single_mut() {
        **text = zone_label;
    }
    if let Ok(mut text) = coord_text.single_mut() {
        **text = format!("{:.0}, {:.0}", player.x, player.z);
    }
}

pub(crate) fn update_world_map_panel(
    time: Res<Time>,
    host: Res<GameHost>,
    ui: Res<UiFlags>,
    mut textures: Option<ResMut<MapTextures>>,
    mut images: ResMut<Assets<Image>>,
    mut panel: Query<&mut Visibility, With<WorldMapPanel>>,
    mut title: Query<&mut Text, With<WorldMapTitleText>>,
    mut legend: Query<&mut Text, (With<WorldMapLegendText>, Without<WorldMapTitleText>)>,
    mut map_image: Query<&mut ImageNode, With<WorldMapImage>>,
    mut map_node: Query<&mut Node, (With<WorldMapImage>, Without<WorldMapPanel>)>,
) {
    for mut visibility in &mut panel {
        *visibility = if ui.show_map {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !ui.show_map {
        return;
    }
    let Some(ref mut textures) = textures else {
        return;
    };

    textures.world_acc += time.delta_secs();
    let force = textures.last_world_zone != host.snapshot.zone_id;
    if !force && textures.world_acc < WORLD_MAP_REFRESH_S {
        return;
    }
    textures.world_acc = 0.0;

    let snap = &host.snapshot;
    let Some(player) = snap.entities.iter().find(|e| e.id == snap.player_id) else {
        return;
    };

    let height = ensure_world_map_size(&mut images, textures, &snap.zone_id, player.z);
    if let Ok(mut node) = map_image.single_mut() {
        if node.image != textures.world_map {
            node.image = textures.world_map.clone();
        }
    }
    if let Ok(mut node) = map_node.single_mut() {
        node.height = Val::Px(height as f32);
    }

    let region = region_for_zone(&snap.zone_id, player.z);
    let mut markers = static_markers_for_region(region);
    markers.extend(collect_dynamic_markers(snap, region, player.id));

    let mut data = vec![0u8; (WORLD_MAP_WIDTH * height * 4) as usize];
    paint_map_frame(
        &mut data,
        WORLD_MAP_WIDTH,
        height,
        region,
        WORLD_SEED,
        &markers,
        false,
    );
    let (mx, my) = world_to_pixel(player.x, player.z, region, WORLD_MAP_WIDTH, height);
    paint_player_arrow(&mut data, WORLD_MAP_WIDTH, height, mx, my, player.yaw);

    if let Some(image) = images.get_mut(&textures.world_map) {
        if let Some(buf) = image.data.as_mut() {
            if buf.len() == data.len() {
                buf.copy_from_slice(&data);
            }
        }
    }

    let zone_label = display_zone_name(snap);
    if let Ok(mut text) = title.single_mut() {
        **text = format!("World Map — {zone_label}");
    }
    if let Ok(mut text) = legend.single_mut() {
        **text = legend_text(&markers, player.x, player.z);
    }
}

fn display_zone_name(snap: &TickSnapshot) -> String {
    if snap.zone_id.is_empty() {
        return "Unknown".into();
    }
    if let Some(band) = zone_by_id(&snap.zone_id) {
        return band.name.to_string();
    }
    for band in ZONES {
        if snap.zone_id == band.id {
            return band.name.to_string();
        }
    }
    snap.zone_id.clone()
}

fn legend_text(markers: &[MapMarker], px: f32, pz: f32) -> String {
    let mut lines = vec![format!("You: ({px:.0}, {pz:.0})")];
    let mut hubs = 0usize;
    let mut portals = 0usize;
    let mut quests = 0usize;
    for m in markers {
        match m.kind {
            MapMarkerKind::Hub if hubs < 4 => {
                lines.push(format!("Hub · {} ({:.0}, {:.0})", m.label, m.x, m.z));
                hubs += 1;
            }
            MapMarkerKind::Portal if portals < 4 => {
                lines.push(format!("Portal · {} ({:.0}, {:.0})", m.label, m.x, m.z));
                portals += 1;
            }
            MapMarkerKind::QuestAvailable | MapMarkerKind::QuestReady if quests < 6 => {
                let tag = if m.kind == MapMarkerKind::QuestReady {
                    "?"
                } else {
                    "!"
                };
                lines.push(format!("{tag} {} ({:.0}, {:.0})", m.label, m.x, m.z));
                quests += 1;
            }
            _ => {}
        }
    }
    lines.join("\n")
}

fn collect_dynamic_markers(
    snap: &TickSnapshot,
    region: MapRegion,
    player_id: EntityId,
) -> Vec<MapMarker> {
    let mut out = Vec::new();
    for entity in &snap.entities {
        if !region.contains(entity.x, entity.z) {
            continue;
        }
        match entity.kind {
            EntityKind::Player if entity.id == player_id => {}
            EntityKind::Player => out.push(MapMarker {
                x: entity.x,
                z: entity.z,
                kind: MapMarkerKind::Ally,
                label: entity.name.clone(),
            }),
            EntityKind::Npc => {
                let quest_kind = npc_quest_marker(snap, entity.template_id.as_deref());
                out.push(MapMarker {
                    x: entity.x,
                    z: entity.z,
                    kind: quest_kind.unwrap_or(MapMarkerKind::Npc),
                    label: entity.name.clone(),
                });
            }
            EntityKind::Mob if entity.alive => out.push(MapMarker {
                x: entity.x,
                z: entity.z,
                kind: MapMarkerKind::Mob,
                label: entity.name.clone(),
            }),
            _ => {}
        }
    }
    out
}

fn npc_quest_marker(snap: &TickSnapshot, template_id: Option<&str>) -> Option<MapMarkerKind> {
    let template_id = template_id?;
    let def = npc(template_id)?;
    if !def.is_quest_giver {
        return None;
    }
    let offers = npc_quest_offers(template_id, &snap.quest_log);
    if !offers.turn_in.is_empty() {
        return Some(MapMarkerKind::QuestReady);
    }
    if !offers.accept.is_empty() {
        Some(MapMarkerKind::QuestAvailable)
    } else {
        Some(MapMarkerKind::Npc)
    }
}

#[cfg(test)]
mod tests {
    use super::npc_quest_marker;
    use woc_protocol::{QuestLogEntry, TickSnapshot};
    use woc_sim::map_view::MapMarkerKind;

    #[test]
    fn alden_is_plain_until_report_completed() {
        let snap = TickSnapshot::default();
        assert_eq!(
            npc_quest_marker(&snap, Some("captain_alden")),
            Some(MapMarkerKind::Npc)
        );

        let mut snap = TickSnapshot::default();
        snap.quest_log.push(QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "completed".into(),
            counts: vec![1],
        });
        assert_eq!(
            npc_quest_marker(&snap, Some("captain_alden")),
            Some(MapMarkerKind::QuestAvailable)
        );
    }
}
