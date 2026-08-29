//! Bevy mesh builders for procedural character / creature / scene visuals.

use bevy::animation::graph::{
    AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationNodeType,
};
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::{AnimationClip, AnimationPlayer, RepeatAnimation};
use bevy::asset::AssetId;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use std::collections::HashMap;
use std::time::Duration;
use woc_content::npc;
use woc_protocol::{EntityId, EntityKind, EntitySnapshot};
use woc_sim::{
    eastbrook_buildings, mount_visual_spec, scene_markers, terrain_height, visual_spec,
    zone_atmosphere, Aabb, PartRole, PartShape, SceneMarkerKind, VisualFamily, VisualPart,
    VisualSpec, WORLD_SEED,
};

/// Y offset applied to the rider mesh while mounted.
const MOUNT_RIDER_LIFT: f32 = 0.55;

use crate::anim::{GaitLimb, VisualMotion};
use crate::asset_map;
use crate::part_tex::{self, PartTextures};

/// Marks the root of a sim-driven entity visual (children hold mesh parts or a GLB scene).
#[derive(Component)]
pub(crate) struct SimVisual {
    pub(crate) id: EntityId,
    pub(crate) key: &'static str,
    pub(crate) mounted: Option<String>,
    pub(crate) bob: bool,
    /// Uniform root scale (GLB kits need non-1.0; procedural stays 1.0).
    pub(crate) scale: f32,
    /// True when presentation comes from an upstream GLB scene.
    pub(crate) uses_glb: bool,
}

/// Child mesh that belongs to the procedural body (not overhead markers).
#[derive(Component)]
pub(crate) struct VisualPartMesh;

/// Floating quest / vendor cue above an NPC.
#[derive(Component)]
pub(crate) struct OverheadMarker;

/// Ground selection ring under the current combat target.
#[derive(Component)]
pub(crate) struct TargetRing;

/// Static world dressing (buildings, portals, beacons) — despawned with the zone scene.
#[derive(Component)]
pub(crate) struct SceneProp;

/// Tracks which zone atmosphere is currently applied.
#[derive(Resource, Default)]
pub(crate) struct ActiveAtmosphere {
    pub(crate) zone_tag: String,
}

#[derive(Clone)]
struct GltfAnimationSet {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    idle_variants: Vec<AnimationNodeIndex>,
    walk: AnimationNodeIndex,
    walk_variants: Vec<AnimationNodeIndex>,
    walk_back: Option<AnimationNodeIndex>,
    run: AnimationNodeIndex,
    run_variants: Vec<AnimationNodeIndex>,
    run_is_distinct: bool,
    death: Option<AnimationNodeIndex>,
}

#[derive(Resource, Default)]
pub(crate) struct GltfAnimationLibrary {
    entries: HashMap<AssetId<Gltf>, Option<GltfAnimationSet>>,
}

#[derive(Component)]
pub(crate) struct GltfAnimationState {
    gltf: Handle<Gltf>,
    set: Option<GltfAnimationSet>,
    owner: Option<EntityId>,
    motion_source: Option<Entity>,
    variant: usize,
    player: Option<Entity>,
    current: Option<GltfClip>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GltfClip {
    Idle,
    Walk,
    WalkBack,
    Run,
    Death,
}

impl GltfAnimationState {
    fn new(
        gltf: Handle<Gltf>,
        owner: Option<EntityId>,
        motion_source: Option<Entity>,
        variant: usize,
    ) -> Self {
        Self {
            gltf,
            set: None,
            owner,
            motion_source,
            variant,
            player: None,
            current: None,
        }
    }
}

#[derive(Component)]
pub(crate) struct GlbVisualMesh;

#[derive(Component, Clone)]
pub(crate) struct GlbMaterialBase {
    base_color: Color,
    emissive: LinearRgba,
}

fn named_animation_nodes(
    gltf: &Gltf,
    nodes: &[AnimationNodeIndex],
    matches: impl Fn(&str) -> bool,
) -> Vec<AnimationNodeIndex> {
    // Iterate in source clip order, not HashMap order, so every client picks
    // the same variant for the same entity.
    gltf.animations
        .iter()
        .enumerate()
        .filter_map(|(index, clip)| {
            let name = gltf
                .named_animations
                .iter()
                .find(|(_, handle)| handle.id() == clip.id())
                .map(|(name, _)| name.as_ref())?;
            matches(name).then(|| nodes.get(index).copied()).flatten()
        })
        .collect()
}

fn is_idle_animation(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.contains("idle") || lower.contains("breath") || lower == "stand")
        && !lower.contains("hit")
        && !lower.contains("jump")
        && !lower.contains("sit")
        && !lower.contains("lie")
}

fn is_forward_walk_animation(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.contains("walk") || lower.contains("locomotion"))
        && !lower.contains("back")
        && !lower.contains("strafe")
        && !lower.contains("jump")
}

fn is_backward_walk_animation(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.contains("back") || lower.contains("reverse")) && lower.contains("walk")
}

fn is_run_animation(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.contains("run") || lower.contains("sprint") || lower.contains("gallop"))
        && !lower.contains("strafe")
        && !lower.contains("jump")
}

fn is_death_animation(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("death") || lower == "die" || lower.contains("dead")
}

fn build_animation_set(
    gltf: &Gltf,
    graphs: &mut Assets<AnimationGraph>,
) -> Option<GltfAnimationSet> {
    if gltf.animations.is_empty() {
        return None;
    }
    let (graph, nodes) = AnimationGraph::from_clips(gltf.animations.iter().cloned());
    let mut idle_variants = named_animation_nodes(gltf, &nodes, is_idle_animation);
    let idle = idle_variants
        .first()
        .copied()
        .or_else(|| nodes.first().copied())?;
    if idle_variants.is_empty() {
        idle_variants.push(idle);
    }

    let mut walk_variants = named_animation_nodes(gltf, &nodes, is_forward_walk_animation);
    let walk = walk_variants.first().copied().unwrap_or(idle);
    if walk_variants.is_empty() {
        walk_variants.push(walk);
    }
    let walk_back = named_animation_nodes(gltf, &nodes, is_backward_walk_animation)
        .first()
        .copied();

    let run_variants = named_animation_nodes(gltf, &nodes, is_run_animation);
    let run_is_distinct = !run_variants.is_empty();
    let run = run_variants.first().copied().unwrap_or(walk);
    let death = named_animation_nodes(gltf, &nodes, is_death_animation)
        .first()
        .copied();
    Some(GltfAnimationSet {
        graph: graphs.add(graph),
        idle,
        idle_variants,
        walk,
        walk_variants,
        walk_back,
        run,
        run_variants: if run_is_distinct {
            run_variants
        } else {
            vec![run]
        },
        run_is_distinct,
        death,
    })
}

/// Connect loaded GLB scenes to Bevy's animation graph and tag their meshes.
pub(crate) fn prepare_gltf_animations(
    mut commands: Commands,
    mut library: ResMut<GltfAnimationLibrary>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut roots: Query<(Entity, &mut GltfAnimationState)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    meshes: Query<
        Entity,
        (
            With<MeshMaterial3d<StandardMaterial>>,
            Without<GlbVisualMesh>,
        ),
    >,
) {
    for (root, mut state) in &mut roots {
        let asset_id = state.gltf.id();
        if state.set.is_none() {
            if let Some(gltf) = gltfs.get(&state.gltf) {
                if !library.entries.contains_key(&asset_id) {
                    let set = build_animation_set(gltf, &mut graphs);
                    library.entries.insert(asset_id, set);
                }
                state.set = library.entries.get(&asset_id).cloned().flatten();
            }
        }

        let Some(set) = state.set.clone() else {
            continue;
        };

        if state.player.is_none() {
            for child in children.iter_descendants(root) {
                if let Ok(mut player) = players.get_mut(child) {
                    let mut transitions = AnimationTransitions::new();
                    let idle = animation_node(&set, GltfClip::Idle, state.variant);
                    transitions.play(&mut player, idle, Duration::ZERO).repeat();
                    commands
                        .entity(child)
                        .insert((AnimationGraphHandle(set.graph.clone()), transitions));
                    state.player = Some(child);
                    break;
                }
            }
        }

        for child in children.iter_descendants(root) {
            if meshes.get(child).is_ok() {
                commands.entity(child).insert(GlbVisualMesh);
            }
        }
    }
}

/// Capture GLB material colors after the asynchronously spawned scene is ready.
pub(crate) fn capture_glb_materials(
    mut commands: Commands,
    materials: Res<Assets<StandardMaterial>>,
    meshes: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        (With<GlbVisualMesh>, Without<GlbMaterialBase>),
    >,
) {
    for (entity, handle) in &meshes {
        let Some(material) = materials.get(&handle.0) else {
            continue;
        };
        commands.entity(entity).insert(GlbMaterialBase {
            base_color: material.base_color.clone(),
            emissive: material.emissive,
        });
    }
}

/// Apply the dead/alive presentation to every material inside a loaded GLB scene.
pub(crate) fn sync_glb_materials(
    host: Res<crate::GameHost>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    visuals: Query<(Entity, &SimVisual)>,
    children: Query<&Children>,
    glb_mats: Query<(&MeshMaterial3d<StandardMaterial>, &GlbMaterialBase), With<GlbVisualMesh>>,
) {
    for (entity, visual) in &visuals {
        let Some(snapshot) = host
            .snapshot
            .entities
            .iter()
            .find(|snapshot| snapshot.id == visual.id)
        else {
            continue;
        };
        if !matches!(
            snapshot.kind,
            EntityKind::Mob | EntityKind::Npc | EntityKind::Loot
        ) {
            continue;
        }
        apply_glb_alive_tint(
            &mut materials,
            children
                .iter_descendants(entity)
                .filter_map(|child| glb_mats.get(child).ok()),
            snapshot.alive,
        );
    }
}

fn variant_node(
    variants: &[AnimationNodeIndex],
    fallback: AnimationNodeIndex,
    variant: usize,
) -> AnimationNodeIndex {
    variants
        .get(variant % variants.len().max(1))
        .copied()
        .unwrap_or(fallback)
}

fn animation_node(set: &GltfAnimationSet, clip: GltfClip, variant: usize) -> AnimationNodeIndex {
    match clip {
        GltfClip::Idle => variant_node(&set.idle_variants, set.idle, variant),
        GltfClip::Walk => variant_node(&set.walk_variants, set.walk, variant),
        GltfClip::WalkBack => set.walk_back.unwrap_or(set.walk),
        GltfClip::Run => variant_node(&set.run_variants, set.run, variant),
        GltfClip::Death => set.death.unwrap_or(set.idle),
    }
}

fn animation_speed(set: &GltfAnimationSet, clip: GltfClip, speed: f32) -> f32 {
    match clip {
        GltfClip::Walk => (speed / 2.2).clamp(0.6, 1.8),
        GltfClip::WalkBack => {
            let speed = (speed / 2.2).clamp(0.6, 1.8);
            if set.walk_back.is_some() {
                speed
            } else {
                -speed
            }
        }
        GltfClip::Run => {
            if set.run_is_distinct {
                (speed / 7.0).clamp(0.6, 1.6)
            } else {
                (speed / 2.2).clamp(0.6, 1.8)
            }
        }
        GltfClip::Idle | GltfClip::Death => 1.0,
    }
}

fn seed_animation_phase(
    active: &mut bevy::animation::ActiveAnimation,
    set: &GltfAnimationSet,
    node: AnimationNodeIndex,
    variant: usize,
    clip: GltfClip,
    graphs: &Assets<AnimationGraph>,
    clips: &Assets<AnimationClip>,
) {
    if matches!(clip, GltfClip::Death) {
        return;
    }
    let Some(graph) = graphs.get(&set.graph) else {
        return;
    };
    let Some(AnimationNodeType::Clip(handle)) = graph.get(node).map(|node| &node.node_type) else {
        return;
    };
    let Some(animation) = clips.get(handle) else {
        return;
    };
    let duration = animation.duration();
    if duration > 0.0 {
        let phase = (variant % 11) as f32 / 11.0 * duration;
        active.set_seek_time(phase);
    }
}

/// Drive GLB clips from the same authoritative snapshot and locomotion pose as procedural visuals.
pub(crate) fn drive_gltf_animations(
    host: Res<crate::GameHost>,
    mut roots: Query<&mut GltfAnimationState>,
    motions: Query<&VisualMotion>,
    graphs: Res<Assets<AnimationGraph>>,
    clips: Res<Assets<AnimationClip>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for mut state in &mut roots {
        let Some(player_entity) = state.player else {
            continue;
        };
        let Some(set) = state.set.clone() else {
            continue;
        };

        let (alive, pose, speed) = if let Some(owner) = state.owner {
            let Some(snapshot) = host
                .snapshot
                .entities
                .iter()
                .find(|entity| entity.id == owner)
            else {
                continue;
            };
            let motion = state
                .motion_source
                .and_then(|source| motions.get(source).ok());
            (
                snapshot.alive,
                motion
                    .map(|motion| motion.last_pose)
                    .unwrap_or(woc_sim::WalkPose::Idle),
                motion.map(|motion| motion.last_speed).unwrap_or(0.0),
            )
        } else {
            (true, woc_sim::WalkPose::Idle, 0.0)
        };

        let requested = if !alive {
            GltfClip::Death
        } else {
            match pose {
                woc_sim::WalkPose::Idle => GltfClip::Idle,
                woc_sim::WalkPose::Walk => GltfClip::Walk,
                woc_sim::WalkPose::WalkBack => GltfClip::WalkBack,
                woc_sim::WalkPose::Run => GltfClip::Run,
            }
        };
        let actual = if requested == GltfClip::Death && set.death.is_none() {
            GltfClip::Idle
        } else {
            requested
        };
        let node = animation_node(&set, actual, state.variant);
        let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else {
            // Scene insertion and animation graph setup use deferred commands;
            // keep the discovered player across this frame so the next update
            // can use the newly-added components.
            state.current = None;
            continue;
        };

        if state.current != Some(actual) {
            let active = transitions.play(&mut player, node, Duration::from_millis(120));
            if actual == GltfClip::Death && set.death.is_some() {
                active.set_repeat(RepeatAnimation::Count(1));
            } else {
                active.repeat();
            }
            seed_animation_phase(active, &set, node, state.variant, actual, &graphs, &clips);
            state.current = Some(actual);
        }
        if let Some(active) = player.animation_mut(node) {
            active.set_speed(animation_speed(&set, actual, speed.abs()));
        }
    }
}

pub(crate) fn spawn_entity_visual(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    textures: &PartTextures,
    asset_server: &AssetServer,
    snap: &EntitySnapshot,
) {
    let spec = visual_spec(snap.kind, snap.template_id.as_deref());
    let alive = snap.alive;
    let mounted = snap.mounted.clone();
    let glb = asset_map::glb_for_visual_key(spec.key);
    let scale = if glb.is_some() {
        asset_map::glb_scale_for_visual_key(spec.key)
    } else {
        1.0
    };
    let root = commands
        .spawn((
            SimVisual {
                id: snap.id,
                key: spec.key,
                mounted: mounted.clone(),
                bob: spec.bob,
                scale,
                uses_glb: glb.is_some(),
            },
            VisualMotion::default(),
            Transform::from_scale(Vec3::splat(scale)),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    if let Some(path) = glb {
        if let Some(mount_id) = mounted.as_deref() {
            let mount_spec = mount_visual_spec(mount_id);
            if let Some(mount_path) = asset_map::glb_for_visual_key(mount_spec.key) {
                // Keep the rider and mount as separate scene roots so both remain visible and
                // can use their own skeletons while sharing the sim-driven root transform.
                spawn_gltf_scene(
                    commands,
                    asset_server,
                    root,
                    path,
                    Vec3::new(0.0, MOUNT_RIDER_LIFT, 0.0),
                    1.0,
                    Some(snap.id),
                    Some(root),
                );
                let mount_scale = asset_map::glb_scale_for_visual_key(mount_spec.key);
                spawn_gltf_scene(
                    commands,
                    asset_server,
                    root,
                    mount_path,
                    Vec3::ZERO,
                    mount_scale / scale,
                    Some(snap.id),
                    Some(root),
                );
            } else {
                spawn_gltf_scene(
                    commands,
                    asset_server,
                    root,
                    path,
                    Vec3::new(0.0, MOUNT_RIDER_LIFT, 0.0),
                    1.0,
                    Some(snap.id),
                    Some(root),
                );
                spawn_parts(
                    commands,
                    meshes,
                    materials,
                    textures,
                    root,
                    &mount_spec,
                    alive,
                    0.0,
                );
            }
        } else {
            spawn_gltf_scene(
                commands,
                asset_server,
                root,
                path,
                Vec3::ZERO,
                1.0,
                Some(snap.id),
                Some(root),
            );
        }
    } else {
        let rider_lift = if mounted.is_some() {
            MOUNT_RIDER_LIFT
        } else {
            0.0
        };
        spawn_parts(
            commands, meshes, materials, textures, root, &spec, alive, rider_lift,
        );
        if let Some(mount_id) = mounted.as_deref() {
            let mount_spec = mount_visual_spec(mount_id);
            spawn_parts(
                commands,
                meshes,
                materials,
                textures,
                root,
                &mount_spec,
                alive,
                0.0,
            );
        }
    }
    spawn_overhead_markers(
        commands,
        meshes,
        materials,
        root,
        snap.kind,
        snap.template_id.as_deref(),
        &spec,
    );
}

fn spawn_gltf_scene(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    scene_path: &'static str,
    translation: Vec3,
    scale: f32,
    owner: Option<EntityId>,
    motion_source: Option<Entity>,
) -> Entity {
    let source_path = scene_path
        .split_once('#')
        .map_or(scene_path, |(source, _)| source);
    let variant = animation_variant(owner, source_path);
    let scene = commands
        .spawn((
            SceneRoot(asset_server.load(scene_path)),
            GltfAnimationState::new(
                asset_server.load::<Gltf>(source_path),
                owner,
                motion_source,
                variant,
            ),
            Transform::from_translation(translation).with_scale(Vec3::splat(scale)),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    commands.entity(parent).add_child(scene);
    scene
}

/// Stable per-entity seed so identical GLB instances do not all start the same
/// idle variant or locomotion clip. It must not use process-randomized hashing:
/// online clients should make the same visual choice for a given entity id.
fn animation_variant(owner: Option<EntityId>, source_path: &str) -> usize {
    let mut hash = owner.unwrap_or_default() as u64 ^ 0x9e37_79b9_7f4a_7c15;
    for byte in source_path.bytes() {
        hash = hash.rotate_left(7) ^ u64::from(byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    (hash ^ (hash >> 32)) as usize
}

fn spawn_parts(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    textures: &PartTextures,
    parent: Entity,
    spec: &VisualSpec,
    alive: bool,
    y_lift: f32,
) {
    for part in spec.parts {
        let mesh = mesh_for_part(meshes, part);
        let color = if alive {
            Color::srgb(part.color[0], part.color[1], part.color[2])
        } else {
            Color::srgb(0.18, 0.18, 0.18)
        };
        let albedo = part_tex::texture_for(textures, spec.family, part.role);
        let metallic = if matches!(part.role, PartRole::Prop)
            && matches!(spec.family, VisualFamily::Humanoid | VisualFamily::Imp)
        {
            0.55
        } else {
            0.02
        };
        let mut mat = StandardMaterial {
            base_color: color,
            base_color_texture: Some(albedo),
            perceptual_roughness: if metallic > 0.3 { 0.35 } else { 0.72 },
            metallic,
            reflectance: 0.45,
            ..default()
        };
        if spec.emissive > 0.0 && alive {
            mat.emissive = LinearRgba::from(Color::srgb(
                part.color[0] * spec.emissive * 4.0,
                part.color[1] * spec.emissive * 4.0,
                part.color[2] * spec.emissive * 4.0,
            ));
        }
        let translation = Vec3::new(part.offset[0], part.offset[1] + y_lift, part.offset[2]);
        let mut entity = commands.spawn((
            VisualPartMesh,
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(mat)),
            Transform::from_translation(translation),
        ));
        if matches!(
            part.role,
            PartRole::LegL | PartRole::LegR | PartRole::HindLegL | PartRole::HindLegR
        ) {
            entity.insert(GaitLimb {
                role: part.role,
                rest_translation: translation,
            });
        }
        let child = entity.id();
        commands.entity(parent).add_child(child);
    }
}

fn spawn_overhead_markers(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    parent: Entity,
    kind: EntityKind,
    template_id: Option<&str>,
    spec: &VisualSpec,
) {
    if kind != EntityKind::Npc {
        return;
    }
    let Some(def) = template_id.and_then(npc) else {
        return;
    };
    let y = spec.label_height + 0.25;
    #[derive(Clone, Copy)]
    enum Cue {
        Quest,
        Vendor,
        Repair,
        Trainer,
        Hearth,
        Auction,
        Bank,
        Mail,
    }
    let mut cues = Vec::new();
    if def.is_quest_giver() {
        cues.push(Cue::Quest);
    }
    if def.is_vendor() {
        cues.push(Cue::Vendor);
    }
    if def.can_repair() {
        cues.push(Cue::Repair);
    }
    if def.is_profession_trainer() || def.is_class_trainer() {
        cues.push(Cue::Trainer);
    }
    if def.is_innkeeper() {
        cues.push(Cue::Hearth);
    }
    if def.is_auctioneer() {
        cues.push(Cue::Auction);
    }
    if def.is_banker() {
        cues.push(Cue::Bank);
    }
    if def.is_mailbox() {
        cues.push(Cue::Mail);
    }
    let total = cues.len() as f32;
    for (idx, cue) in cues.into_iter().enumerate() {
        let x = (idx as f32 - (total - 1.0) * 0.5) * 0.32;
        match cue {
            Cue::Quest => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.95, 0.82, 0.25),
                    emissive: LinearRgba::from(Color::srgb(0.9, 0.6, 0.1)),
                    ..default()
                });
                let stem = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Cuboid::new(0.12, 0.45, 0.12))),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_xyz(x, y + 0.15, 0.0),
                    ))
                    .id();
                let dot = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Sphere::new(0.08))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y - 0.22, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(stem);
                commands.entity(parent).add_child(dot);
            }
            Cue::Vendor => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.35, 0.85, 0.45),
                    emissive: LinearRgba::from(Color::srgb(0.1, 0.4, 0.15)),
                    ..default()
                });
                let bag = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Cuboid::new(0.32, 0.26, 0.22))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(bag);
            }
            Cue::Repair => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.75, 0.78, 0.82),
                    emissive: LinearRgba::from(Color::srgb(0.25, 0.28, 0.32)),
                    ..default()
                });
                let hammer = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Cuboid::new(0.34, 0.12, 0.12))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(hammer);
            }
            Cue::Trainer => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.35, 0.62, 0.95),
                    emissive: LinearRgba::from(Color::srgb(0.08, 0.18, 0.45)),
                    ..default()
                });
                let book = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Cuboid::new(0.28, 0.2, 0.18))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(book);
            }
            Cue::Hearth => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.95, 0.45, 0.25),
                    emissive: LinearRgba::from(Color::srgb(0.5, 0.12, 0.04)),
                    ..default()
                });
                let hearth = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Sphere::new(0.15))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(hearth);
            }
            Cue::Auction => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.92, 0.78, 0.28),
                    emissive: LinearRgba::from(Color::srgb(0.45, 0.32, 0.05)),
                    ..default()
                });
                let gavel = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Cuboid::new(0.32, 0.26, 0.22))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(gavel);
            }
            Cue::Bank => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.35, 0.55, 0.85),
                    emissive: LinearRgba::from(Color::srgb(0.08, 0.16, 0.32)),
                    ..default()
                });
                let vault = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Cuboid::new(0.28, 0.22, 0.22))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(vault);
            }
            Cue::Mail => {
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.82, 0.62, 0.32),
                    emissive: LinearRgba::from(Color::srgb(0.28, 0.16, 0.04)),
                    ..default()
                });
                let post = commands
                    .spawn((
                        OverheadMarker,
                        Mesh3d(meshes.add(Cuboid::new(0.30, 0.16, 0.20))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(x, y, 0.0),
                    ))
                    .id();
                commands.entity(parent).add_child(post);
            }
        }
    }
}

fn mesh_for_part(meshes: &mut ResMut<Assets<Mesh>>, part: &VisualPart) -> Handle<Mesh> {
    match part.shape {
        PartShape::Cuboid => meshes.add(Cuboid::new(part.size[0], part.size[1], part.size[2])),
        PartShape::Capsule => meshes.add(Capsule3d::new(part.size[0], part.size[1] * 2.0)),
        PartShape::Sphere => meshes.add(Sphere::new(part.size[0])),
        PartShape::Cylinder => meshes.add(Cylinder::new(part.size[0], part.size[1])),
        PartShape::Cone => meshes.add(Cone::new(part.size[0], part.size[1])),
    }
}

/// Rebuild mesh parts when an entity's visual key or mount state changes.
pub(crate) fn respawn_parts_if_needed(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    textures: &PartTextures,
    asset_server: &AssetServer,
    entity: Entity,
    vis: &mut SimVisual,
    snap: &EntitySnapshot,
    children: Option<&Children>,
) {
    let spec = visual_spec(snap.kind, snap.template_id.as_deref());
    let mounted = snap.mounted.clone();
    if vis.key == spec.key && vis.mounted == mounted {
        return;
    }
    commands.entity(entity).remove::<SceneRoot>();
    commands.entity(entity).remove::<GltfAnimationState>();
    if let Some(children) = children {
        let kids: Vec<Entity> = children.iter().collect();
        for child in kids {
            commands.entity(child).despawn();
        }
    }
    let glb = asset_map::glb_for_visual_key(spec.key);
    let scale = if glb.is_some() {
        asset_map::glb_scale_for_visual_key(spec.key)
    } else {
        1.0
    };
    vis.key = spec.key;
    vis.mounted = mounted.clone();
    vis.bob = spec.bob;
    vis.scale = scale;
    vis.uses_glb = glb.is_some();
    if let Some(path) = glb {
        if let Some(mount_id) = mounted.as_deref() {
            let mount_spec = mount_visual_spec(mount_id);
            if let Some(mount_path) = asset_map::glb_for_visual_key(mount_spec.key) {
                spawn_gltf_scene(
                    commands,
                    asset_server,
                    entity,
                    path,
                    Vec3::new(0.0, MOUNT_RIDER_LIFT, 0.0),
                    1.0,
                    Some(snap.id),
                    Some(entity),
                );
                let mount_scale = asset_map::glb_scale_for_visual_key(mount_spec.key);
                spawn_gltf_scene(
                    commands,
                    asset_server,
                    entity,
                    mount_path,
                    Vec3::ZERO,
                    mount_scale / scale,
                    Some(snap.id),
                    Some(entity),
                );
            } else {
                spawn_gltf_scene(
                    commands,
                    asset_server,
                    entity,
                    path,
                    Vec3::new(0.0, MOUNT_RIDER_LIFT, 0.0),
                    1.0,
                    Some(snap.id),
                    Some(entity),
                );
                spawn_parts(
                    commands,
                    meshes,
                    materials,
                    textures,
                    entity,
                    &mount_spec,
                    snap.alive,
                    0.0,
                );
            }
        } else {
            spawn_gltf_scene(
                commands,
                asset_server,
                entity,
                path,
                Vec3::ZERO,
                1.0,
                Some(snap.id),
                Some(entity),
            );
        }
    } else {
        let rider_lift = if mounted.is_some() {
            MOUNT_RIDER_LIFT
        } else {
            0.0
        };
        spawn_parts(
            commands, meshes, materials, textures, entity, &spec, snap.alive, rider_lift,
        );
        if let Some(mount_id) = mounted.as_deref() {
            let mount_spec = mount_visual_spec(mount_id);
            spawn_parts(
                commands,
                meshes,
                materials,
                textures,
                entity,
                &mount_spec,
                snap.alive,
                0.0,
            );
        }
    }
    spawn_overhead_markers(
        commands,
        meshes,
        materials,
        entity,
        snap.kind,
        snap.template_id.as_deref(),
        &spec,
    );
}

/// Tint body-part materials when alive state flips (mobs/corpses).
pub(crate) fn apply_alive_tint(
    materials: &mut ResMut<Assets<StandardMaterial>>,
    mat_handles: impl Iterator<Item = Handle<StandardMaterial>>,
    kind: EntityKind,
    alive: bool,
    template_id: Option<&str>,
) {
    let spec = visual_spec(kind, template_id);
    let mut part_i = 0;
    for handle in mat_handles {
        let Some(part) = spec.parts.get(part_i) else {
            break;
        };
        part_i += 1;
        let Some(mat) = materials.get_mut(&handle) else {
            continue;
        };
        if alive {
            mat.base_color = Color::srgb(part.color[0], part.color[1], part.color[2]);
            if spec.emissive > 0.0 {
                mat.emissive = LinearRgba::from(Color::srgb(
                    part.color[0] * spec.emissive * 4.0,
                    part.color[1] * spec.emissive * 4.0,
                    part.color[2] * spec.emissive * 4.0,
                ));
            }
        } else {
            mat.base_color = Color::srgb(0.15, 0.15, 0.15);
            mat.emissive = LinearRgba::BLACK;
        }
    }
}

/// Apply the dead/alive presentation to materials authored inside a GLB scene.
pub(crate) fn apply_glb_alive_tint<'a>(
    materials: &mut ResMut<Assets<StandardMaterial>>,
    entries: impl Iterator<Item = (&'a MeshMaterial3d<StandardMaterial>, &'a GlbMaterialBase)>,
    alive: bool,
) {
    for (handle, base) in entries {
        let Some(material) = materials.get_mut(&handle.0) else {
            continue;
        };
        if alive {
            material.base_color = base.base_color.clone();
            material.emissive = base.emissive;
        } else {
            material.base_color = Color::srgb(0.15, 0.15, 0.15);
            material.emissive = LinearRgba::BLACK;
        }
    }
}

/// Ensure a single ground ring exists for the current combat target.
pub(crate) fn ensure_target_ring(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    q: &Query<Entity, With<TargetRing>>,
) -> Entity {
    if let Some(e) = q.iter().next() {
        return e;
    }
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.75, 0.25, 0.65),
        emissive: LinearRgba::from(Color::srgb(0.5, 0.35, 0.05)),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            TargetRing,
            Mesh3d(meshes.add(Cylinder::new(0.85, 0.06))),
            MeshMaterial3d(mat),
            Transform::from_xyz(0.0, -50.0, 0.0),
            Visibility::Hidden,
        ))
        .id()
}

/// Spawn buildings, hub beacons, portal arches, and camp props.
pub(crate) fn spawn_scene_props(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.30, 0.18),
        perceptual_roughness: 0.9,
        ..default()
    });
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.55, 0.52),
        perceptual_roughness: 0.95,
        ..default()
    });
    let roof = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.22, 0.18),
        perceptual_roughness: 0.85,
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.80, 0.30),
        emissive: LinearRgba::from(Color::srgb(0.6, 0.4, 0.05)),
        perceptual_roughness: 0.4,
        ..default()
    });
    let portal = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.55, 0.95, 0.75),
        emissive: LinearRgba::from(Color::srgb(0.15, 0.25, 0.55)),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.3,
        ..default()
    });
    let ember = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.45, 0.12),
        emissive: LinearRgba::from(Color::srgb(1.2, 0.4, 0.05)),
        ..default()
    });

    for aabb in eastbrook_buildings() {
        spawn_building(commands, meshes, &wood, &stone, &roof, aabb);
    }

    for marker in scene_markers() {
        let y = terrain_height(marker.x, marker.z, WORLD_SEED);
        match marker.kind {
            SceneMarkerKind::HubBeacon => {
                let pillar = commands
                    .spawn((
                        SceneProp,
                        Mesh3d(meshes.add(Cylinder::new(0.35, 4.5))),
                        MeshMaterial3d(stone.clone()),
                        Transform::from_xyz(marker.x, y + 2.25, marker.z),
                    ))
                    .id();
                let _ = pillar;
                commands.spawn((
                    SceneProp,
                    Mesh3d(meshes.add(Sphere::new(0.45))),
                    MeshMaterial3d(gold.clone()),
                    Transform::from_xyz(marker.x, y + 4.8, marker.z),
                ));
            }
            SceneMarkerKind::PortalArch => {
                // Two pillars + lintel + translucent gate plane.
                for dx in [-1.6_f32, 1.6] {
                    commands.spawn((
                        SceneProp,
                        Mesh3d(meshes.add(Cuboid::new(0.45, 4.2, 0.45))),
                        MeshMaterial3d(stone.clone()),
                        Transform::from_xyz(marker.x + dx, y + 2.1, marker.z),
                    ));
                }
                commands.spawn((
                    SceneProp,
                    Mesh3d(meshes.add(Cuboid::new(3.6, 0.4, 0.45))),
                    MeshMaterial3d(stone.clone()),
                    Transform::from_xyz(marker.x, y + 4.3, marker.z),
                ));
                commands.spawn((
                    SceneProp,
                    Mesh3d(meshes.add(Cuboid::new(2.8, 3.4, 0.12))),
                    MeshMaterial3d(portal.clone()),
                    Transform::from_xyz(marker.x, y + 2.0, marker.z),
                ));
            }
            SceneMarkerKind::CampFire => {
                commands.spawn((
                    SceneProp,
                    Mesh3d(meshes.add(Cylinder::new(0.55, 0.25))),
                    MeshMaterial3d(wood.clone()),
                    Transform::from_xyz(marker.x, y + 0.12, marker.z),
                ));
                commands.spawn((
                    SceneProp,
                    Mesh3d(meshes.add(Sphere::new(0.35))),
                    MeshMaterial3d(ember.clone()),
                    Transform::from_xyz(marker.x, y + 0.45, marker.z),
                ));
            }
        }
    }
}

fn spawn_building(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    wood: &Handle<StandardMaterial>,
    stone: &Handle<StandardMaterial>,
    roof: &Handle<StandardMaterial>,
    aabb: &Aabb,
) {
    let cx = (aabb.min_x + aabb.max_x) * 0.5;
    let cz = (aabb.min_z + aabb.max_z) * 0.5;
    let cy = (aabb.min_y + aabb.max_y) * 0.5;
    let sx = (aabb.max_x - aabb.min_x).max(0.5);
    let sy = (aabb.max_y - aabb.min_y).max(0.5);
    let sz = (aabb.max_z - aabb.min_z).max(0.5);
    // Walls
    commands.spawn((
        SceneProp,
        Mesh3d(meshes.add(Cuboid::new(sx, sy, sz))),
        MeshMaterial3d(stone.clone()),
        Transform::from_xyz(cx, cy, cz),
    ));
    // Roof slab slightly larger / above
    commands.spawn((
        SceneProp,
        Mesh3d(meshes.add(Cuboid::new(sx + 0.6, 0.35, sz + 0.6))),
        MeshMaterial3d(roof.clone()),
        Transform::from_xyz(cx, aabb.max_y + 0.2, cz),
    ));
    // Door hint on +Z face
    commands.spawn((
        SceneProp,
        Mesh3d(meshes.add(Cuboid::new(1.1, 2.0, 0.15))),
        MeshMaterial3d(wood.clone()),
        Transform::from_xyz(cx, aabb.min_y + 1.0, aabb.max_z + 0.05),
    ));
}

/// Apply zone sky / ambient when the local player's zone changes.
pub(crate) fn sync_zone_atmosphere(
    zone_id: &str,
    active: &mut ActiveAtmosphere,
    clear: &mut ClearColor,
    ambient: &mut AmbientLight,
) {
    let atmo = zone_atmosphere(zone_id);
    if active.zone_tag == atmo.zone_tag {
        return;
    }
    active.zone_tag = atmo.zone_tag.to_string();
    clear.0 = Color::srgb(atmo.clear[0], atmo.clear[1], atmo.clear[2]);
    ambient.color = Color::srgb(atmo.ambient[0], atmo.ambient[1], atmo.ambient[2]);
}

/// Spawn a class preview root (for character create) and return its entity.
pub(crate) fn spawn_class_preview(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    textures: &PartTextures,
    asset_server: &AssetServer,
    class_id: &str,
    origin: Vec3,
) -> Entity {
    let spec = visual_spec(EntityKind::Player, Some(class_id));
    let glb = asset_map::glb_for_visual_key(spec.key);
    let scale = if glb.is_some() {
        asset_map::glb_scale_for_visual_key(spec.key)
    } else {
        1.0
    };
    let root = commands
        .spawn((
            Transform::from_translation(origin).with_scale(Vec3::splat(scale)),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    if let Some(path) = glb {
        commands
            .entity(root)
            .insert(SceneRoot(asset_server.load(path)));
    } else {
        spawn_parts(
            commands, meshes, materials, textures, root, &spec, true, 0.0,
        );
    }
    root
}
