//! World spawn / visual entity setup and sync.

use bevy::prelude::*;
use std::collections::HashSet;
use woc_protocol::{
    EntityId, EntityKind, EntitySnapshot, PlayerIntent, SimEvent, TickSnapshot, WsClientMsg,
    WsServerMsg, DT,
};
use woc_sim::{
    terrain_height, water_bodies, water_level, zone_atmosphere, Sim, WORLD_MAX_X, WORLD_MAX_Z,
    WORLD_MIN_Z, WORLD_SEED,
};
use woc_version::{footer, Compat};

use crate::anim::{
    apply_limb_gait, death_root_rotation, family_uses_gait, sample_gait, GaitLimb, VisualMotion,
    REMOVE_FADE_SEC,
};
use crate::char_create::{CharName, SelectedClass};
use crate::hud::{
    ChromePanelKind, HudActionBarText, HudBagText, HudCastFill, HudCastPanel, HudCastText,
    HudCharPanel, HudCharText, HudChromePanel, HudChromeText, HudHpText, HudNetText,
    HudPartyFrames, HudPartyPanel, HudPartyText, HudQuestText, HudRoot, HudTargetText,
    HudToastText, HudVendorOffers, HudVendorPanel, HudVendorTitle, HudXpText,
};
use crate::map;
use crate::online;
use crate::visuals::{
    self, apply_alive_tint, ensure_target_ring, spawn_entity_visual, spawn_scene_props,
    sync_zone_atmosphere, ActiveAtmosphere, SceneProp, SimVisual, TargetRing, VisualPartMesh,
};
use crate::{AppState, GameHost, NetStatus, PlayMode};
use woc_sim::visual_spec;

#[derive(Component)]
struct TerrainMarker;

#[derive(Component)]
pub(crate) struct FollowCam;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<ActiveAtmosphere>()
        .add_systems(Startup, setup_camera_light)
        .add_systems(OnEnter(AppState::InWorld), setup_world)
        .add_systems(OnExit(AppState::InWorld), cleanup_world);
}

fn setup_camera_light(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 12.0, 18.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        FollowCam,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.6, 0.0)),
    ));
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<AmbientLight>,
    mut atmo: ResMut<ActiveAtmosphere>,
    mut images: ResMut<Assets<Image>>,
    name: Res<CharName>,
    class: Res<SelectedClass>,
    play_mode: Res<PlayMode>,
    session: Res<crate::AuthSession>,
) {
    let host = match *play_mode {
        PlayMode::Offline => {
            let sim = Sim::new_eastbrook(name.0.trim(), class.0);
            let snapshot = sim.snapshot();
            GameHost {
                play_mode: PlayMode::Offline,
                sim: Some(sim),
                snapshot,
                accumulator: 0.0,
                pending_intent: PlayerIntent::default(),
                recent_toasts: Vec::new(),
                look_yaw: 0.0,
                look_pitch: -0.35,
                cursor_grabbed: false,
                net_status: NetStatus::Idle,
                to_net: None,
                from_net: None,
                local_auto_attack: false,
            }
        }
        PlayMode::Online => {
            let token = session.token.clone().unwrap_or_default();
            let character_id = session.selected.unwrap_or_else(uuid::Uuid::nil);
            let (to_net, from_net, _handle) = online::spawn_online_session(token, character_id);
            GameHost {
                play_mode: PlayMode::Online,
                sim: None,
                snapshot: TickSnapshot::default(),
                accumulator: 0.0,
                pending_intent: PlayerIntent::default(),
                recent_toasts: vec![(format!("Connecting to {}…", online::ONLINE_WS_URL), 4.0)],
                look_yaw: 0.0,
                look_pitch: -0.35,
                cursor_grabbed: false,
                net_status: NetStatus::Connecting,
                to_net: Some(to_net),
                from_net: Some(std::sync::Mutex::new(from_net)),
                local_auto_attack: false,
            }
        }
    };

    // Seed sky from starting zone (eastbrook offline; online waits for first snapshot).
    sync_zone_atmosphere(
        host.snapshot.zone_id.as_str(),
        &mut atmo,
        &mut clear,
        &mut ambient,
    );

    let eastbrook = zone_atmosphere("eastbrook");
    let marsh = zone_atmosphere("eastfen");
    let peaks = zone_atmosphere("thornpeak");
    let terrain_vale = materials.add(StandardMaterial {
        base_color: Color::srgb(
            eastbrook.terrain[0],
            eastbrook.terrain[1],
            eastbrook.terrain[2],
        ),
        perceptual_roughness: 0.95,
        ..default()
    });
    let terrain_marsh = materials.add(StandardMaterial {
        base_color: Color::srgb(marsh.terrain[0], marsh.terrain[1], marsh.terrain[2]),
        perceptual_roughness: 0.95,
        ..default()
    });
    let terrain_peaks = materials.add(StandardMaterial {
        base_color: Color::srgb(peaks.terrain[0], peaks.terrain[1], peaks.terrain[2]),
        perceptual_roughness: 0.95,
        ..default()
    });
    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(
            eastbrook.water[0],
            eastbrook.water[1],
            eastbrook.water[2],
            eastbrook.water[3],
        ),
        perceptual_roughness: 0.2,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    // Chunked height samples across the continuous strip (step ~8 yd).
    let step = 8.0;
    let mut x = -WORLD_MAX_X;
    while x < WORLD_MAX_X {
        let mut z = WORLD_MIN_Z;
        while z < WORLD_MAX_Z {
            let y00 = terrain_height(x, z, WORLD_SEED);
            let y10 = terrain_height(x + step, z, WORLD_SEED);
            let y01 = terrain_height(x, z + step, WORLD_SEED);
            let y11 = terrain_height(x + step, z + step, WORLD_SEED);
            let y = (y00 + y10 + y01 + y11) * 0.25;
            let mid_z = z + step * 0.5;
            let terrain_mat = if mid_z < 180.0 {
                terrain_vale.clone()
            } else if mid_z < 540.0 {
                terrain_marsh.clone()
            } else {
                terrain_peaks.clone()
            };
            commands.spawn((
                TerrainMarker,
                Mesh3d(meshes.add(Cuboid::new(step * 0.98, 0.45, step * 0.98))),
                MeshMaterial3d(terrain_mat),
                Transform::from_xyz(x + step * 0.5, y - 0.2, z + step * 0.5),
            ));
            z += step;
        }
        x += step;
    }
    for (wx, wz, radius) in water_bodies() {
        let y = water_level();
        commands.spawn((
            TerrainMarker,
            Mesh3d(meshes.add(Cylinder::new(radius.max(1.0), 0.25))),
            MeshMaterial3d(water_mat.clone()),
            Transform::from_xyz(wx, y, wz),
        ));
    }

    spawn_scene_props(&mut commands, &mut meshes, &mut materials);

    spawn_visuals_from_entities(
        &mut commands,
        &mut meshes,
        &mut materials,
        &host.snapshot.entities,
    );

    let npc_n = host
        .snapshot
        .entities
        .iter()
        .filter(|e| e.kind == EntityKind::Npc)
        .count();
    let mob_n = host
        .snapshot
        .entities
        .iter()
        .filter(|e| e.kind == EntityKind::Mob)
        .count();
    let herb_n = host
        .snapshot
        .entities
        .iter()
        .filter(|e| {
            e.kind == EntityKind::Loot
                && e.template_id
                    .as_deref()
                    .is_some_and(|t| woc_content::gather_node(t).is_some())
        })
        .count();
    // Mutate after count: toast on local host before insert.
    let mut host = host;
    if npc_n + mob_n > 0 {
        host.recent_toasts.push((
            format!("Scene loaded · {npc_n} NPCs · {mob_n} foes · {herb_n} herbs"),
            3.5,
        ));
    }
    commands.insert_resource(host);

    let (minimap, world_map) = map::create_map_textures(&mut commands, &mut images);

    commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            map::spawn_map_chrome(root, minimap, world_map);

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.06, 0.45)),
            ))
            .with_children(|top| {
                top.spawn((
                    HudNetText,
                    Text::new(""),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.85, 0.95)),
                ));
                top.spawn((
                    HudHpText,
                    Text::new("HP --"),
                    TextFont::from_font_size(18.0),
                    TextColor(Color::srgb(0.9, 0.35, 0.3)),
                ));
                top.spawn((
                    HudXpText,
                    Text::new("XP --"),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.95, 0.86, 0.45)),
                ));
                top.spawn((
                    HudTargetText,
                    Text::new("Target: none"),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.85, 0.9, 0.95)),
                ));
                top.spawn((
                    HudPartyFrames,
                    Text::new(""),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.55, 0.85, 0.7)),
                ));
                top.spawn((
                    HudQuestText,
                    Text::new("Quest: —"),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.95, 0.9, 0.55)),
                ));
                top.spawn((
                    HudBagText,
                    Text::new(""),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.8, 0.85, 0.7)),
                ));
                top.spawn((
                    Text::new(
                        "LMB/F attack · Tab target · G invite · P party · O accept · J guild · 1–5 abilities · T pet · E interact/loot · B bags (Q/F) · L quests · C sheet (rep) · N talents · K bank (G/H/J/Y) · I mail (Enter To, S/Y send, 1–9/P collect, X return) · M map · U market (L/O/X) · [ ] loot mode · RMB look · Esc clear",
                    ),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
                ));
            });

            // Character sheet overlay (toggle C)
            root.spawn((
                HudCharPanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    top: Val::Px(200.0),
                    width: Val::Px(300.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.06, 0.1, 0.88)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    HudCharText,
                    Text::new(""),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.9, 0.92, 0.85)),
                ));
            });

            // Party roster overlay (toggle P)
            root.spawn((
                HudPartyPanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    top: Val::Px(180.0),
                    width: Val::Px(280.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.08, 0.06, 0.88)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    HudPartyText,
                    Text::new(""),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.85, 0.95, 0.88)),
                ));
            });

            // Snapshot-backed progression / economy panels (N/K/M/U).
            root.spawn((Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                right: Val::Px(12.0),
                top: Val::Px(200.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                ..default()
            },))
                .with_children(|row| {
                    row.spawn((
                        HudChromePanel(ChromePanelKind::Talents),
                        Visibility::Hidden,
                        Node {
                            width: Val::Px(300.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.06, 0.12, 0.92)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            HudChromeText(ChromePanelKind::Talents),
                            Text::new(""),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgb(0.9, 0.86, 0.98)),
                        ));
                    });
                    row.spawn((
                        HudChromePanel(ChromePanelKind::Bank),
                        Visibility::Hidden,
                        Node {
                            width: Val::Px(300.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.08, 0.05, 0.92)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            HudChromeText(ChromePanelKind::Bank),
                            Text::new(""),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgb(0.95, 0.9, 0.7)),
                        ));
                    });
                    row.spawn((
                        HudChromePanel(ChromePanelKind::Mail),
                        Visibility::Hidden,
                        Node {
                            width: Val::Px(300.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.08, 0.11, 0.92)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            HudChromeText(ChromePanelKind::Mail),
                            Text::new(""),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgb(0.78, 0.9, 0.98)),
                        ));
                    });
                    row.spawn((
                        HudChromePanel(ChromePanelKind::Market),
                        Visibility::Hidden,
                        Node {
                            width: Val::Px(300.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.1, 0.08, 0.92)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            HudChromeText(ChromePanelKind::Market),
                            Text::new(""),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgb(0.82, 0.96, 0.8)),
                        ));
                    });
                    row.spawn((
                        HudChromePanel(ChromePanelKind::Guild),
                        Visibility::Hidden,
                        Node {
                            width: Val::Px(300.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.06, 0.12, 0.92)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            HudChromeText(ChromePanelKind::Guild),
                            Text::new(""),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgb(0.92, 0.82, 0.98)),
                        ));
                    });
                    row.spawn((
                        HudChromePanel(ChromePanelKind::Friends),
                        Visibility::Hidden,
                        Node {
                            width: Val::Px(300.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.10, 0.12, 0.92)),
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            HudChromeText(ChromePanelKind::Friends),
                            Text::new(""),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgb(0.78, 0.92, 0.98)),
                        ));
                    });
                });

            // Vendor panel (visible when open_vendor is Some)
            root.spawn((
                HudVendorPanel,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(12.0),
                    top: Val::Px(160.0),
                    width: Val::Px(320.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.1, 0.08, 0.9)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    HudVendorTitle,
                    Text::new("Vendor"),
                    TextFont::from_font_size(18.0),
                    TextColor(Color::srgb(0.85, 0.95, 0.75)),
                ));
                panel.spawn((
                    HudVendorOffers,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },
                ));
            });

            root.spawn((Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            },))
                .with_children(|bot| {
                    // Cast bar
                    bot.spawn((
                        HudCastPanel,
                        Visibility::Hidden,
                        Node {
                            width: Val::Px(360.0),
                            height: Val::Px(28.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.75)),
                    ))
                    .with_children(|cast| {
                        cast.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(10.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.15, 0.15, 0.2, 0.9)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                HudCastFill,
                                Node {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.35, 0.55, 0.95)),
                            ));
                        });
                        cast.spawn((
                            HudCastText,
                            Text::new(""),
                            TextFont::from_font_size(14.0),
                            TextColor(Color::srgb(0.85, 0.9, 1.0)),
                        ));
                    });

                    // Action bar
                    bot.spawn((
                        Node {
                            width: Val::Px(920.0),
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.8)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            HudActionBarText,
                            Text::new("[1] Ability   [2] —   [3] —   [4] —   [5] —"),
                            TextFont::from_font_size(15.0),
                            TextColor(Color::srgb(0.9, 0.88, 0.7)),
                        ));
                    });

                    bot.spawn((
                        HudToastText,
                        Text::new(""),
                        TextFont::from_font_size(20.0),
                        TextColor(Color::srgb(0.95, 0.9, 0.6)),
                    ));
                    bot.spawn((
                        Text::new(footer()),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgb(0.65, 0.7, 0.75)),
                    ));
                });
        });
}

fn spawn_visuals_from_entities(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    entities: &[EntitySnapshot],
) {
    for e in entities {
        if !e.alive && e.kind != EntityKind::Mob && e.kind != EntityKind::Npc {
            continue;
        }
        spawn_entity_visual(commands, meshes, materials, e);
    }
}

fn cleanup_world(
    mut commands: Commands,
    visuals: Query<
        Entity,
        Or<(
            With<SimVisual>,
            With<TerrainMarker>,
            With<SceneProp>,
            With<TargetRing>,
            With<HudRoot>,
        )>,
    >,
    mut atmo: ResMut<ActiveAtmosphere>,
) {
    for e in &visuals {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<GameHost>();
    atmo.zone_tag.clear();
    commands.remove_resource::<map::MapTextures>();
}
fn push_events_toasts(host: &mut GameHost, events: &[SimEvent]) {
    let pid = host.snapshot.player_id;
    for ev in events {
        match ev {
            SimEvent::LevelUp { level, .. } => {
                host.recent_toasts
                    .push((format!("Level up! You are now {level}."), 3.0));
            }
            SimEvent::Toast { message } => host.recent_toasts.push((message.clone(), 3.0)),
            SimEvent::Damage {
                source,
                target,
                amount,
                ability,
            } => {
                // Ability hits outgoing + any damage taken (skip spammy auto swings out).
                if *target == pid {
                    let label = ability
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("hit");
                    host.recent_toasts
                        .push((format!("Took {label} for {:.0}", amount), 1.2));
                } else if *source == pid {
                    if let Some(abil) = ability.as_deref().filter(|s| !s.is_empty()) {
                        host.recent_toasts
                            .push((format!("{abil} hits for {:.0}", amount), 1.2));
                    }
                }
            }
            SimEvent::Kill { victim_name, .. } => {
                host.recent_toasts
                    .push((format!("Slain: {victim_name}"), 2.5));
            }
            SimEvent::Loot { copper, item, .. } => {
                let mut msg = format!("Looted {copper} copper");
                if let Some(it) = item {
                    msg.push_str(&format!(" · {it}"));
                }
                host.recent_toasts.push((msg, 2.5));
            }
            SimEvent::QuestAccepted { quest_id, .. } => {
                host.recent_toasts
                    .push((format!("Quest accepted: {quest_id}"), 3.0));
            }
            SimEvent::QuestCompleted { quest_id, .. } => {
                host.recent_toasts
                    .push((format!("Quest complete: {quest_id}"), 3.0));
            }
            SimEvent::QuestProgress { text, .. } => {
                host.recent_toasts.push((text.clone(), 2.0));
            }
            SimEvent::NpcDialog { text, .. } => {
                host.recent_toasts.push((text.clone(), 3.0));
            }
            SimEvent::AuraApplied { id, remaining, .. } => {
                host.recent_toasts
                    .push((format!("Aura: {id} ({remaining:.0}s)"), 1.5));
            }
            SimEvent::TalentLearned {
                talent_id, rank, ..
            } => {
                host.recent_toasts
                    .push((format!("Talent learned: {talent_id} rank {rank}"), 2.5));
            }
            SimEvent::TalentRespec { .. } => {
                host.recent_toasts
                    .push(("Talents reset — points refunded.".into(), 2.5));
            }
            SimEvent::ProfessionDenied { reason, .. } => {
                host.recent_toasts
                    .push((format!("profession_denied:{reason:?}"), 2.0));
            }
            _ => {}
        }
    }
}

fn apply_online_messages(host: &mut GameHost) -> bool {
    let mut pending = Vec::new();
    if let Some(rx_mutex) = host.from_net.as_ref() {
        if let Ok(rx) = rx_mutex.lock() {
            while let Ok(msg) = rx.try_recv() {
                pending.push(msg);
            }
        }
    }
    let mut kick = false;
    for msg in pending {
        match msg {
            WsServerMsg::Welcome {
                player_id,
                protocol_rev,
            } => {
                if protocol_rev != woc_protocol::PROTOCOL_REV {
                    let message = Compat::ProtocolMismatch {
                        client_rev: woc_protocol::PROTOCOL_REV,
                        realm_rev: protocol_rev,
                    }
                    .user_message();
                    host.net_status = NetStatus::Error(message.clone());
                    host.recent_toasts.push((message, 5.0));
                    kick = true;
                    continue;
                }
                host.net_status = NetStatus::Connected { player_id };
                host.snapshot.player_id = player_id;
                host.recent_toasts.push((
                    format!("Welcome · player #{player_id} · proto {protocol_rev}"),
                    3.0,
                ));
            }
            WsServerMsg::Snapshot(snap) => {
                host.snapshot = *snap;
                if matches!(host.net_status, NetStatus::Connecting) {
                    host.net_status = NetStatus::Connected {
                        player_id: host.snapshot.player_id,
                    };
                }
            }
            WsServerMsg::Events { events } => {
                push_events_toasts(host, &events);
            }
            WsServerMsg::Error { message } => {
                host.net_status = NetStatus::Error(message.clone());
                host.recent_toasts.push((message.clone(), 5.0));
                if message.starts_with("version:") {
                    kick = true;
                }
            }
            WsServerMsg::Chat {
                channel,
                from,
                text,
            } => {
                host.recent_toasts
                    .push((format!("[{channel}] {from}: {text}"), 4.0));
            }
            WsServerMsg::PartyUpdate { .. } => {}
        }
    }
    kick
}

pub(crate) fn sim_fixed_step(
    time: Res<Time>,
    mut host: ResMut<GameHost>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: Query<(Entity, &SimVisual, &mut VisualMotion)>,
) {
    if host.is_online() {
        if apply_online_messages(&mut host) {
            next.set(AppState::Title);
            return;
        }
        host.accumulator += time.delta_secs();
        let step = DT;
        while host.accumulator >= step {
            host.accumulator -= step;
            if let Some(tx) = &host.to_net {
                let intent = host.pending_intent;
                let _ = tx.send(WsClientMsg::Intent(intent));
            }
            host.pending_intent.ability = None;
            host.pending_intent.clear_target = false;
        }
    } else {
        host.accumulator += time.delta_secs();
        let step = DT;
        let mut events_all = Vec::new();
        while host.accumulator >= step {
            host.accumulator -= step;
            let intent = host.pending_intent;
            if let Some(sim) = host.sim.as_mut() {
                let (_snap, events) = sim.tick(intent);
                events_all.extend(events);
            }
            host.pending_intent.ability = None;
            host.pending_intent.clear_target = false;
        }
        push_events_toasts(&mut host, &events_all);
        if let Some(sim) = host.sim.as_ref() {
            host.snapshot = sim.snapshot();
        }
    }

    let known: HashSet<EntityId> = visuals.iter().map(|(_, v, _)| v.id).collect();
    let snap_ids: HashSet<EntityId> = host.snapshot.entities.iter().map(|e| e.id).collect();
    let dt = time.delta_secs();

    // Spawn any snapshot entity we don't yet have a visual for (including corpses / loot / herbs).
    for e in &host.snapshot.entities {
        if known.contains(&e.id) {
            continue;
        }
        let should_spawn = e.alive
            || matches!(e.kind, EntityKind::Mob | EntityKind::Npc)
            || (e.kind == EntityKind::Loot && e.alive);
        if should_spawn {
            spawn_entity_visual(&mut commands, &mut meshes, &mut materials, e);
        }
    }

    // Soft-remove visuals that left the snapshot (picked loot, dismissed pets, disconnects).
    let mut despawn = Vec::new();
    for (entity, vis, mut motion) in &mut visuals {
        if snap_ids.contains(&vis.id) {
            // Re-appeared or still present — cancel any in-flight fade.
            motion.remove_timer = None;
            continue;
        }
        if motion.remove_timer.is_none() {
            motion.remove_timer = Some(REMOVE_FADE_SEC);
        }
        if let Some(timer) = motion.remove_timer.as_mut() {
            *timer -= dt;
            if *timer <= 0.0 {
                despawn.push(entity);
            }
        }
    }
    for entity in despawn {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn sync_visuals(
    time: Res<Time>,
    host: Res<GameHost>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: Query<(
        Entity,
        &mut SimVisual,
        &mut VisualMotion,
        &mut Transform,
        &mut Visibility,
        Option<&Children>,
    )>,
    mut limbs: Query<
        (&GaitLimb, &mut Transform),
        (Without<SimVisual>, Without<FollowCam>, Without<TargetRing>),
    >,
    part_mats: Query<&MeshMaterial3d<StandardMaterial>, With<VisualPartMesh>>,
    mut cam: Query<
        &mut Transform,
        (
            With<FollowCam>,
            Without<SimVisual>,
            Without<TargetRing>,
            Without<GaitLimb>,
        ),
    >,
    ring_q: Query<Entity, With<TargetRing>>,
    mut ring_tf: Query<
        (&mut Transform, &mut Visibility),
        (
            With<TargetRing>,
            Without<SimVisual>,
            Without<FollowCam>,
            Without<GaitLimb>,
        ),
    >,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<AmbientLight>,
    mut atmo: ResMut<ActiveAtmosphere>,
) {
    sync_zone_atmosphere(
        host.snapshot.zone_id.as_str(),
        &mut atmo,
        &mut clear,
        &mut ambient,
    );

    let bob_t = time.elapsed_secs();
    let dt = time.delta_secs();
    let player = host.player_snap().map(|p| (p.x, p.y, p.z));
    for (entity, mut vis, mut motion, mut tf, mut visibility, children) in &mut visuals {
        // Fade-out scale while pending removal (entity already gone from snapshot).
        if let Some(timer) = motion.remove_timer {
            let t = (timer / REMOVE_FADE_SEC).clamp(0.0, 1.0);
            tf.scale = Vec3::splat(t);
            *visibility = Visibility::Visible;
            continue;
        }

        if let Some(e) = host.snapshot.entities.iter().find(|ent| ent.id == vis.id) {
            visuals::respawn_parts_if_needed(
                &mut commands,
                &mut meshes,
                &mut materials,
                entity,
                &mut vis,
                e,
                children,
            );

            let spec = visual_spec(e.kind, e.template_id.as_deref());
            let (pose, _speed) = if e.alive && e.on_ground && !e.flying && !e.swimming {
                sample_gait(&mut motion, e.x, e.z, e.yaw, dt)
            } else {
                (woc_sim::WalkPose::Idle, 0.0)
            };

            let bob = if vis.bob && e.alive {
                (bob_t * 2.4 + e.id as f32 * 0.7).sin() * 0.12
            } else if e.alive && e.swimming {
                (bob_t * 3.0 + e.id as f32).sin() * 0.08
            } else if e.alive && e.flying {
                (bob_t * 1.6 + e.id as f32 * 0.3).sin() * 0.1
            } else if e.alive && family_uses_gait(spec.family) && pose != woc_sim::WalkPose::Idle {
                (motion.cycle * 2.0).sin().abs() * 0.06
            } else {
                0.0
            };

            if e.alive {
                let mut pitch = 0.0_f32;
                if e.flying {
                    pitch = -0.18;
                } else if e.swimming {
                    pitch = 0.22;
                } else if !e.on_ground {
                    pitch = -0.08;
                }
                tf.translation = Vec3::new(e.x, e.y + bob, e.z);
                tf.rotation = Quat::from_euler(EulerRot::YXZ, e.yaw, pitch, 0.0);
                tf.scale = Vec3::ONE;
                *visibility = Visibility::Visible;
            } else if matches!(
                e.kind,
                EntityKind::Mob | EntityKind::Npc | EntityKind::Player
            ) {
                // Corpse pose: tip onto the side; keep clickable for loot.
                tf.translation = Vec3::new(e.x, e.y + 0.15, e.z);
                tf.rotation = death_root_rotation(e.yaw);
                tf.scale = Vec3::ONE;
                *visibility = Visibility::Visible;
            } else {
                *visibility = Visibility::Hidden;
                tf.scale = Vec3::ZERO;
            }

            if e.alive && family_uses_gait(spec.family) {
                if let Some(children) = children {
                    for child in children.iter() {
                        if let Ok((limb, mut limb_tf)) = limbs.get_mut(child) {
                            apply_limb_gait(
                                limb.role,
                                limb.rest_translation,
                                pose,
                                motion.cycle,
                                &mut limb_tf,
                            );
                        }
                    }
                }
            } else if !e.alive {
                // Reset limbs when dead so the tipped root looks clean.
                if let Some(children) = children {
                    for child in children.iter() {
                        if let Ok((limb, mut limb_tf)) = limbs.get_mut(child) {
                            limb_tf.translation = limb.rest_translation;
                            limb_tf.rotation = Quat::IDENTITY;
                        }
                    }
                }
            }

            if matches!(e.kind, EntityKind::Mob | EntityKind::Npc | EntityKind::Loot) {
                if let Some(children) = children {
                    let handles = children
                        .iter()
                        .filter_map(|c| part_mats.get(c).ok().map(|m| m.0.clone()));
                    apply_alive_tint(
                        &mut materials,
                        handles,
                        e.kind,
                        e.alive,
                        e.template_id.as_deref(),
                    );
                }
            }
        } else {
            tf.scale = Vec3::ZERO;
        }
    }

    // Target ground ring.
    let ring_entity = ensure_target_ring(&mut commands, &mut meshes, &mut materials, &ring_q);
    let _ = ring_entity;
    if let Ok((mut rtf, mut rvis)) = ring_tf.single_mut() {
        if let Some(tid) = host.snapshot.target_id {
            if let Some(t) = host.snapshot.entities.iter().find(|e| e.id == tid) {
                rtf.translation = Vec3::new(t.x, t.y + 0.05, t.z);
                *rvis = Visibility::Visible;
            } else {
                *rvis = Visibility::Hidden;
            }
        } else {
            *rvis = Visibility::Hidden;
        }
    }

    if let (Some((px, py, pz)), Ok(mut ctf)) = (player, cam.single_mut()) {
        let yaw = host.look_yaw;
        let pitch = host.look_pitch;
        let dist = 10.0;
        let offset = Vec3::new(
            yaw.sin() * pitch.cos() * dist,
            (-pitch.sin()) * dist + 2.0,
            yaw.cos() * pitch.cos() * dist,
        );
        let target = Vec3::new(px, py + 1.2, pz);
        ctf.translation = target + offset;
        ctf.look_at(target, Vec3::Y);
    }
}
