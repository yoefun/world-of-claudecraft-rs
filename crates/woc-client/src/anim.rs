//! Procedural gait posing for entity visuals (walk / run / death / remove fade).

use bevy::prelude::*;
use woc_sim::{
    desired_walk_pose, locomotion_time_scale, update_locomotion, LocoTrack, PartRole, VisualFamily,
    WalkPose,
};

/// Seconds of fade before a visual is despawned after leaving the snapshot.
pub(crate) const REMOVE_FADE_SEC: f32 = 0.35;

/// Rest pose + gait role for a limb mesh child.
#[derive(Component, Clone, Copy)]
pub(crate) struct GaitLimb {
    pub(crate) role: PartRole,
    pub(crate) rest_translation: Vec3,
}

/// Per-visual locomotion / removal presentation state.
#[derive(Component)]
pub(crate) struct VisualMotion {
    pub(crate) loco: LocoTrack,
    pub(crate) last_x: f32,
    pub(crate) last_z: f32,
    pub(crate) seeded: bool,
    pub(crate) cycle: f32,
    /// Countdown while fading out after leaving the snapshot (`None` = live).
    pub(crate) remove_timer: Option<f32>,
}

impl Default for VisualMotion {
    fn default() -> Self {
        Self {
            loco: LocoTrack::new(),
            last_x: 0.0,
            last_z: 0.0,
            seeded: false,
            cycle: 0.0,
            remove_timer: None,
        }
    }
}

/// Advance locomotion from a position delta and return the walk pose + cadence.
pub(crate) fn sample_gait(
    motion: &mut VisualMotion,
    x: f32,
    z: f32,
    yaw: f32,
    dt: f32,
) -> (WalkPose, f32) {
    if !motion.seeded {
        motion.last_x = x;
        motion.last_z = z;
        motion.seeded = true;
        return (WalkPose::Idle, 0.0);
    }
    let vx = x - motion.last_x;
    let vz = z - motion.last_z;
    motion.last_x = x;
    motion.last_z = z;
    let state = update_locomotion(&mut motion.loco, vx, vz, yaw, dt.max(1e-4));
    let pose = desired_walk_pose(&state);
    if let Some(scale) = locomotion_time_scale(pose, state.speed) {
        let rate = if pose == WalkPose::Run { 9.0 } else { 7.0 };
        motion.cycle += dt * scale * rate;
    }
    (pose, state.speed)
}

fn swing_angle(cycle: f32, phase: f32, amp: f32) -> f32 {
    (cycle + phase).sin() * amp
}

pub(crate) fn apply_limb_gait(
    role: PartRole,
    rest: Vec3,
    pose: WalkPose,
    cycle: f32,
    transform: &mut Transform,
) {
    let amp = match pose {
        WalkPose::Idle => {
            transform.translation = rest;
            transform.rotation = Quat::IDENTITY;
            return;
        }
        WalkPose::Walk | WalkPose::WalkBack => 0.48,
        WalkPose::Run => 0.72,
    };
    let phase = match role {
        PartRole::LegL | PartRole::HindLegR => 0.0,
        PartRole::LegR | PartRole::HindLegL => std::f32::consts::PI,
        _ => {
            transform.translation = rest;
            transform.rotation = Quat::IDENTITY;
            return;
        }
    };
    let angle = swing_angle(cycle, phase, amp);
    // Pivot near the hip: rest is part center; rotate around local X (pitch).
    let hip_lift = rest.y.max(0.15) * 0.35;
    let hip = Vec3::new(rest.x, rest.y + hip_lift, rest.z);
    let rot = Quat::from_rotation_x(angle);
    let offset = rest - hip;
    transform.translation = hip + rot * offset;
    transform.rotation = rot;
}

/// Root pose for a dead actor: tip onto the side.
pub(crate) fn death_root_rotation(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2 * 0.92)
}

/// Families that use gait limb swings.
pub(crate) fn family_uses_gait(family: VisualFamily) -> bool {
    matches!(
        family,
        VisualFamily::Humanoid | VisualFamily::Wolf | VisualFamily::Boar | VisualFamily::Imp
    )
}
