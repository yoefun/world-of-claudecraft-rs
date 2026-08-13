//! Bevy mesh builders for procedural character / creature / scene visuals.

use bevy::prelude::*;
use woc_content::npc;
use woc_protocol::{EntityId, EntityKind, EntitySnapshot};
use woc_sim::{
    eastbrook_buildings, scene_markers, terrain_height, visual_spec, zone_atmosphere, Aabb,
    PartRole, PartShape, SceneMarkerKind, VisualPart, VisualSpec, WORLD_SEED,
};

use crate::anim::{GaitLimb, VisualMotion};

/// Marks the root of a sim-driven entity visual (children hold mesh parts).
#[derive(Component)]
pub(crate) struct SimVisual {
    pub(crate) id: EntityId,
    pub(crate) key: &'static str,
    pub(crate) bob: bool,
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

pub(crate) fn spawn_entity_visual(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    snap: &EntitySnapshot,
) {
    let spec = visual_spec(snap.kind, snap.template_id.as_deref());
    let alive = snap.alive;
    let root = commands
        .spawn((
            SimVisual {
                id: snap.id,
                key: spec.key,
                bob: spec.bob,
            },
            VisualMotion::default(),
            Transform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    spawn_parts(commands, meshes, materials, root, &spec, alive);
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

fn spawn_parts(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    parent: Entity,
    spec: &VisualSpec,
    alive: bool,
) {
    for part in spec.parts {
        let mesh = mesh_for_part(meshes, part);
        let color = if alive {
            Color::srgb(part.color[0], part.color[1], part.color[2])
        } else {
            Color::srgb(0.18, 0.18, 0.18)
        };
        let mut mat = StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.85,
            ..default()
        };
        if spec.emissive > 0.0 && alive {
            mat.emissive = LinearRgba::from(Color::srgb(
                part.color[0] * spec.emissive * 4.0,
                part.color[1] * spec.emissive * 4.0,
                part.color[2] * spec.emissive * 4.0,
            ));
        }
        let translation = Vec3::new(part.offset[0], part.offset[1], part.offset[2]);
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
    if def.is_quest_giver {
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
                Transform::from_xyz(0.0, y + 0.15, 0.0),
            ))
            .id();
        let dot = commands
            .spawn((
                OverheadMarker,
                Mesh3d(meshes.add(Sphere::new(0.08))),
                MeshMaterial3d(mat),
                Transform::from_xyz(0.0, y - 0.22, 0.0),
            ))
            .id();
        commands.entity(parent).add_child(stem);
        commands.entity(parent).add_child(dot);
    } else if def.is_vendor {
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.85, 0.45),
            emissive: LinearRgba::from(Color::srgb(0.1, 0.4, 0.15)),
            ..default()
        });
        let bag = commands
            .spawn((
                OverheadMarker,
                Mesh3d(meshes.add(Cuboid::new(0.35, 0.28, 0.22))),
                MeshMaterial3d(mat),
                Transform::from_xyz(0.0, y, 0.0),
            ))
            .id();
        commands.entity(parent).add_child(bag);
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

/// Rebuild mesh parts when an entity's visual key changes (class swap / template).
pub(crate) fn respawn_parts_if_needed(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    entity: Entity,
    vis: &mut SimVisual,
    snap: &EntitySnapshot,
    children: Option<&Children>,
) {
    let spec = visual_spec(snap.kind, snap.template_id.as_deref());
    if vis.key == spec.key {
        return;
    }
    if let Some(children) = children {
        let kids: Vec<Entity> = children.iter().collect();
        for child in kids {
            commands.entity(child).despawn();
        }
    }
    vis.key = spec.key;
    vis.bob = spec.bob;
    spawn_parts(commands, meshes, materials, entity, &spec, snap.alive);
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
    class_id: &str,
    origin: Vec3,
) -> Entity {
    let spec = visual_spec(EntityKind::Player, Some(class_id));
    let root = commands
        .spawn((
            Transform::from_translation(origin),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();
    spawn_parts(commands, meshes, materials, root, &spec, true);
    root
}
