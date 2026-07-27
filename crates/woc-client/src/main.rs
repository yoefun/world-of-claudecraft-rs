//! Bevy offline host for the combat slice.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use std::collections::HashSet;
use woc_protocol::{AbilitySlot, EntityId, EntityKind, PlayerIntent, SimEvent, DT};
use woc_sim::{terrain_height, Sim, WORLD_HALF, WORLD_SEED};
use woc_version::footer;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: footer(),
                resolution: (1280.0_f32, 720.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .insert_resource(ClearColor(Color::srgb(0.45, 0.62, 0.78)))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.92, 0.94, 0.88),
            brightness: 350.0,
            ..default()
        })
        .insert_resource(CharName("Aldric".into()))
        .add_systems(Startup, setup_camera_light)
        .add_systems(OnEnter(AppState::Title), setup_title)
        .add_systems(OnExit(AppState::Title), cleanup_ui)
        .add_systems(Update, title_input.run_if(in_state(AppState::Title)))
        .add_systems(OnEnter(AppState::CharCreate), setup_char_create)
        .add_systems(OnExit(AppState::CharCreate), cleanup_ui)
        .add_systems(
            Update,
            char_create_input.run_if(in_state(AppState::CharCreate)),
        )
        .add_systems(OnEnter(AppState::InWorld), setup_world)
        .add_systems(OnExit(AppState::InWorld), cleanup_world)
        .add_systems(
            Update,
            (
                grab_cursor,
                camera_look,
                collect_intent,
                sim_fixed_step,
                sync_visuals,
                update_hud,
                toast_fade,
            )
                .chain()
                .run_if(in_state(AppState::InWorld)),
        )
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    Title,
    CharCreate,
    InWorld,
}

#[derive(Component)]
struct UiRoot;

#[derive(Component)]
struct HudRoot;

#[derive(Component)]
struct HudHpText;

#[derive(Component)]
struct HudXpText;

#[derive(Component)]
struct HudTargetText;

#[derive(Component)]
struct HudToastText;

#[derive(Component)]
struct NameInputDisplay;

#[derive(Resource)]
struct CharName(String);

#[derive(Resource)]
struct OfflineHost {
    sim: Sim,
    accumulator: f32,
    pending_intent: PlayerIntent,
    recent_toasts: Vec<(String, f32)>,
    look_yaw: f32,
    look_pitch: f32,
    cursor_grabbed: bool,
}

#[derive(Component)]
struct SimVisual {
    id: EntityId,
}

#[derive(Component)]
struct TerrainMarker;

#[derive(Component)]
struct FollowCam;

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

fn setup_title(mut commands: Commands) {
    commands
        .spawn((
            UiRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.72)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("World of ClaudeCraft"),
                TextFont::from_font_size(48.0),
                TextColor(Color::srgb(0.95, 0.86, 0.55)),
            ));
            p.spawn((
                Text::new("Rust rewrite · combat slice"),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.85, 0.9, 0.95)),
            ));
            p.spawn((
                Text::new(footer()),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.7, 0.75, 0.8)),
            ));
            p.spawn((
                Text::new("Press Enter to create a Warrior"),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.9, 0.92, 0.85)),
            ));
        });
}

fn title_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        next.set(AppState::CharCreate);
    }
}

fn setup_char_create(mut commands: Commands, name: Res<CharName>) {
    let label = format!("Name: {}", name.0);
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
            BackgroundColor(Color::srgba(0.04, 0.07, 0.1, 0.8)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Create Warrior"),
                TextFont::from_font_size(36.0),
                TextColor(Color::srgb(0.95, 0.86, 0.55)),
            ));
            p.spawn((
                Text::new("Class: Warrior (only class in 0.1)"),
                TextFont::from_font_size(18.0),
                TextColor(Color::WHITE),
            ));
            p.spawn((
                NameInputDisplay,
                Text::new(label),
                TextFont::from_font_size(22.0),
                TextColor(Color::srgb(0.85, 0.95, 0.85)),
            ));
            p.spawn((
                Text::new("Type a name, Backspace to edit, Enter to enter world"),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
        });
}

fn char_create_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut name: ResMut<CharName>,
    mut next: ResMut<NextState<AppState>>,
    mut q: Query<&mut Text, With<NameInputDisplay>>,
    mut events: EventReader<bevy::input::keyboard::KeyboardInput>,
) {
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
    if keys.just_pressed(KeyCode::Enter) {
        if name.0.trim().is_empty() {
            name.0 = "Aldric".into();
        }
        next.set(AppState::InWorld);
        keys.clear();
    }
    if let Ok(mut text) = q.single_mut() {
        **text = format!("Name: {}", name.0);
    }
}

fn cleanup_ui(mut commands: Commands, q: Query<Entity, With<UiRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    name: Res<CharName>,
) {
    let sim = Sim::new_combat_slice(name.0.trim());
    let host = OfflineHost {
        sim,
        accumulator: 0.0,
        pending_intent: PlayerIntent::default(),
        recent_toasts: Vec::new(),
        look_yaw: 0.0,
        look_pitch: -0.35,
        cursor_grabbed: false,
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

    spawn_all_visuals(&mut commands, &mut meshes, &mut materials, &host.sim);
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
                    Text::new("LMB target/attack · 1 Heroic Strike · RMB look · Esc free cursor"),
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

fn spawn_all_visuals(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    sim: &Sim,
) {
    for e in &sim.entities {
        if !e.alive && e.kind != EntityKind::Mob {
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
    commands.remove_resource::<OfflineHost>();
}

fn grab_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut host: ResMut<OfflineHost>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        host.cursor_grabbed = false;
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
    if mouse.just_pressed(MouseButton::Right) {
        host.cursor_grabbed = true;
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
}

fn camera_look(mut motion: EventReader<MouseMotion>, mut host: ResMut<OfflineHost>) {
    if !host.cursor_grabbed {
        motion.clear();
        return;
    }
    for ev in motion.read() {
        host.look_yaw -= ev.delta.x * 0.0025;
        host.look_pitch = (host.look_pitch - ev.delta.y * 0.0025).clamp(-1.2, 0.2);
    }
}

fn collect_intent(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut host: ResMut<OfflineHost>,
) {
    let mut intent = PlayerIntent {
        facing: host.look_yaw,
        ..default()
    };
    let mut mx = 0.0;
    let mut mz = 0.0;
    if keys.pressed(KeyCode::KeyW) {
        mz += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        mz -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        mx -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        mx += 1.0;
    }
    intent.move_x = mx;
    intent.move_z = mz;
    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        intent.ability = Some(AbilitySlot::Primary);
    }
    if mouse.just_pressed(MouseButton::Left) || keys.pressed(KeyCode::KeyF) {
        intent.attack = true;
        if let Some(p) = host.sim.player() {
            let (px, pz) = (p.x, p.z);
            let mut best: Option<(EntityId, f32)> = None;
            for e in &host.sim.entities {
                if e.kind != EntityKind::Mob || !e.alive {
                    continue;
                }
                let dx = e.x - px;
                let dz = e.z - pz;
                let d = (dx * dx + dz * dz).sqrt();
                if d < 25.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((e.id, d));
                }
            }
            if let Some((id, _)) = best {
                intent.target_id = Some(id);
            }
        }
    }
    if host.sim.player().map(|p| p.auto_attack).unwrap_or(false) {
        intent.attack = true;
        intent.target_id = intent
            .target_id
            .or_else(|| host.sim.player().and_then(|p| p.target));
    }
    host.pending_intent = intent;
}

fn sim_fixed_step(
    time: Res<Time>,
    mut host: ResMut<OfflineHost>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    visuals: Query<&SimVisual>,
) {
    host.accumulator += time.delta_secs();
    let step = DT;
    let mut events_all = Vec::new();
    while host.accumulator >= step {
        host.accumulator -= step;
        let intent = host.pending_intent;
        let (_snap, events) = host.sim.tick(intent);
        events_all.extend(events);
        host.pending_intent.ability = None;
    }

    for ev in events_all {
        match ev {
            SimEvent::LevelUp { level, .. } => {
                host.recent_toasts
                    .push((format!("Level up! You are now {level}."), 3.0));
            }
            SimEvent::Toast { message } => host.recent_toasts.push((message, 3.0)),
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
            _ => {}
        }
    }

    let known: HashSet<EntityId> = visuals.iter().map(|v| v.id).collect();
    for e in &host.sim.entities {
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

fn sync_visuals(
    host: Res<OfflineHost>,
    mut visuals: Query<(
        &SimVisual,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cam: Query<&mut Transform, (With<FollowCam>, Without<SimVisual>)>,
) {
    let player = host.sim.player().map(|p| (p.x, p.y, p.z));
    for (vis, mut tf, mat_h) in &mut visuals {
        if let Some(e) = host.sim.entities.iter().find(|e| e.id == vis.id) {
            let y_off = match e.kind {
                EntityKind::Player => 0.9,
                EntityKind::Mob => 0.3,
                EntityKind::Loot => 0.25,
            };
            if e.alive || e.kind == EntityKind::Mob {
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

fn update_hud(
    host: Res<OfflineHost>,
    mut hp: Query<&mut Text, With<HudHpText>>,
    mut xp: Query<&mut Text, (With<HudXpText>, Without<HudHpText>)>,
    mut target: Query<&mut Text, (With<HudTargetText>, Without<HudHpText>, Without<HudXpText>)>,
    mut toast: Query<
        &mut Text,
        (
            With<HudToastText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
        ),
    >,
) {
    let snap = host.sim.snapshot();
    if let Some(player) = snap.entities.iter().find(|e| e.id == snap.player_id) {
        if let Ok(mut t) = hp.single_mut() {
            **t = format!(
                "HP {:.0}/{:.0}   Rage {:.0}/{:.0}   [1] Heroic Strike {}",
                player.hp,
                player.hp_max,
                player.resource,
                player.resource_max,
                if snap.ability_ready { "READY" } else { "CD" }
            );
        }
    }
    if let Ok(mut t) = xp.single_mut() {
        let bag = snap.progress.bag_item.as_deref().unwrap_or("—");
        **t = format!(
            "Lv {}   XP {}/{}   Copper {}   Bag: {}",
            snap.progress.level,
            snap.progress.xp,
            snap.progress.xp_to_level,
            snap.progress.copper,
            bag
        );
    }
    if let Ok(mut t) = target.single_mut() {
        **t = if let Some(tid) = snap.target_id {
            if let Some(e) = snap.entities.iter().find(|e| e.id == tid) {
                format!(
                    "Target: {}  HP {:.0}/{:.0}{}",
                    e.name,
                    e.hp,
                    e.hp_max,
                    if e.alive { "" } else { " (dead)" }
                )
            } else {
                "Target: none".into()
            }
        } else {
            "Target: none".into()
        };
    }
    if let Ok(mut t) = toast.single_mut() {
        **t = host
            .recent_toasts
            .last()
            .map(|(m, _)| m.clone())
            .unwrap_or_default();
    }
}

fn toast_fade(time: Res<Time>, mut host: ResMut<OfflineHost>) {
    let dt = time.delta_secs();
    for (_, life) in &mut host.recent_toasts {
        *life -= dt;
    }
    host.recent_toasts.retain(|(_, life)| *life > 0.0);
}
