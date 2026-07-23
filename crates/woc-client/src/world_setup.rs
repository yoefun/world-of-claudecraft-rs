//! World spawn / visual entity setup and sync.

use bevy::prelude::*;
use std::collections::HashSet;
use woc_protocol::{
    EntityId, EntityKind, EntitySnapshot, PlayerIntent, SimEvent, TickSnapshot, WsClientMsg,
    WsServerMsg, DT,
};
use woc_sim::{terrain_height, Sim, WORLD_HALF, WORLD_SEED};
use woc_version::footer;

use crate::char_create::{CharName, SelectedClass};
use crate::hud::{
    HudBagText, HudHpText, HudNetText, HudQuestText, HudRoot, HudTargetText, HudToastText,
    HudXpText,
};
use crate::online;
use crate::{AppState, GameHost, NetStatus, PlayMode};

#[derive(Component)]
pub(crate) struct SimVisual {
    pub(crate) id: EntityId,
}

#[derive(Component)]
struct TerrainMarker;

#[derive(Component)]
pub(crate) struct FollowCam;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, setup_camera_light)
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
    name: Res<CharName>,
    class: Res<SelectedClass>,
    play_mode: Res<PlayMode>,
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
            let (to_net, from_net, _handle) = online::spawn_online_session(
                name.0.trim().to_string(),
                class.0.as_str().to_string(),
            );
            GameHost {
                play_mode: PlayMode::Online,
                sim: None,
                snapshot: TickSnapshot::default(),
                accumulator: 0.0,
                pending_intent: PlayerIntent::default(),
                recent_toasts: vec![(
                    format!("Connecting to {}…", online::ONLINE_WS_URL),
                    4.0,
                )],
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

    let terrain_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.52, 0.28),
        perceptual_roughness: 0.95,
        ..default()
    });
    let step = 4.0;
    let half = WORLD_HALF;
    let mut x = -half;
    while x < half {
        let mut z = -half;
        while z < half {
            let y00 = terrain_height(x, z, WORLD_SEED);
            let y10 = terrain_height(x + step, z, WORLD_SEED);
            let y01 = terrain_height(x, z + step, WORLD_SEED);
            let y11 = terrain_height(x + step, z + step, WORLD_SEED);
            let y = (y00 + y10 + y01 + y11) * 0.25;
            commands.spawn((
                TerrainMarker,
                Mesh3d(meshes.add(Cuboid::new(step * 0.98, 0.35, step * 0.98))),
                MeshMaterial3d(terrain_mat.clone()),
                Transform::from_xyz(x + step * 0.5, y - 0.15, z + step * 0.5),
            ));
            z += step;
        }
        x += step;
    }

    spawn_visuals_from_entities(
        &mut commands,
        &mut meshes,
        &mut materials,
        &host.snapshot.entities,
    );
    commands.insert_resource(host);

    commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
        ))
        .with_children(|root| {
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
                        "LMB attack · 1 ability · E interact · B bags · L quests · RMB look · Esc",
                    ),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.7, 0.75, 0.8)),
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
        spawn_one_visual(commands, meshes, materials, e.id, e.kind, e.alive);
    }
}

fn spawn_one_visual(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    id: EntityId,
    kind: EntityKind,
    alive: bool,
) {
    let (mesh, color) = match kind {
        EntityKind::Player => (
            meshes.add(Capsule3d::new(0.35, 1.0)),
            Color::srgb(0.25, 0.45, 0.85),
        ),
        EntityKind::Mob => (
            meshes.add(Cuboid::new(0.9, 0.55, 1.3)),
            if alive {
                Color::srgb(0.45, 0.35, 0.28)
            } else {
                Color::srgb(0.2, 0.2, 0.2)
            },
        ),
        EntityKind::Npc => (
            meshes.add(Capsule3d::new(0.32, 0.95)),
            Color::srgb(0.55, 0.75, 0.45),
        ),
        EntityKind::Loot => (meshes.add(Sphere::new(0.25)), Color::srgb(0.9, 0.75, 0.2)),
    };
    commands.spawn((
        SimVisual { id },
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            ..default()
        })),
        Transform::default(),
    ));
}

fn cleanup_world(
    mut commands: Commands,
    visuals: Query<Entity, Or<(With<SimVisual>, With<TerrainMarker>, With<HudRoot>)>>,
) {
    for e in &visuals {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<GameHost>();
}

fn push_events_toasts(host: &mut GameHost, events: &[SimEvent]) {
    for ev in events {
        match ev {
            SimEvent::LevelUp { level, .. } => {
                host.recent_toasts
                    .push((format!("Level up! You are now {level}."), 3.0));
            }
            SimEvent::Toast { message } => host.recent_toasts.push((message.clone(), 3.0)),
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
            _ => {}
        }
    }
}

fn apply_online_messages(host: &mut GameHost) {
    let mut pending = Vec::new();
    if let Some(rx_mutex) = host.from_net.as_ref() {
        if let Ok(rx) = rx_mutex.lock() {
            while let Ok(msg) = rx.try_recv() {
                pending.push(msg);
            }
        }
    }
    for msg in pending {
        match msg {
            WsServerMsg::Welcome {
                player_id,
                protocol_rev,
            } => {
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
                host.recent_toasts.push((message, 5.0));
            }
        }
    }
}

pub(crate) fn sim_fixed_step(
    time: Res<Time>,
    mut host: ResMut<GameHost>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    visuals: Query<&SimVisual>,
) {
    if host.is_online() {
        apply_online_messages(&mut host);
        host.accumulator += time.delta_secs();
        let step = DT;
        while host.accumulator >= step {
            host.accumulator -= step;
            if let Some(tx) = &host.to_net {
                let intent = host.pending_intent;
                let _ = tx.send(WsClientMsg::Intent(intent));
            }
            host.pending_intent.ability = None;
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
        }
        push_events_toasts(&mut host, &events_all);
        if let Some(sim) = host.sim.as_ref() {
            host.snapshot = sim.snapshot();
        }
    }

    let known: HashSet<EntityId> = visuals.iter().map(|v| v.id).collect();
    for e in &host.snapshot.entities {
        if e.alive && !known.contains(&e.id) {
            spawn_one_visual(
                &mut commands,
                &mut meshes,
                &mut materials,
                e.id,
                e.kind,
                e.alive,
            );
        }
    }
}

pub(crate) fn sync_visuals(
    host: Res<GameHost>,
    mut visuals: Query<(
        &SimVisual,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cam: Query<&mut Transform, (With<FollowCam>, Without<SimVisual>)>,
) {
    let player = host.player_snap().map(|p| (p.x, p.y, p.z));
    for (vis, mut tf, mat_h) in &mut visuals {
        if let Some(e) = host.snapshot.entities.iter().find(|e| e.id == vis.id) {
            let y_off = match e.kind {
                EntityKind::Player => 0.9,
                EntityKind::Npc => 0.9,
                EntityKind::Mob => 0.3,
                EntityKind::Loot => 0.25,
            };
            if e.alive || e.kind == EntityKind::Mob || e.kind == EntityKind::Npc {
                tf.translation = Vec3::new(e.x, e.y + y_off, e.z);
                tf.rotation = Quat::from_rotation_y(e.yaw);
                tf.scale = Vec3::ONE;
            } else {
                tf.scale = Vec3::ZERO;
            }
            if e.kind == EntityKind::Mob {
                if let Some(mat) = materials.get_mut(&mat_h.0) {
                    mat.base_color = if e.alive {
                        Color::srgb(0.45, 0.35, 0.28)
                    } else {
                        Color::srgb(0.15, 0.15, 0.15)
                    };
                }
            }
        } else {
            // Entity left the snapshot (despawned remote player, etc.).
            tf.scale = Vec3::ZERO;
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
