//! Locomotion-state hysteresis for character / creature walk animation.
//!
//! Port of upstream `src/render/locomotion.ts` (pin `a3e5e959`). Render-space
//! speed is noisy (frame jitter, snapshot stalls, terrain bob); without leeway
//! a single-frame dip flips the anim to idle and resets the walk cycle. This
//! module latches "moving", smooths cadence speed, and holds gait / direction.

/// u/s above which an entity is "moving".
pub const MOVE_ENTER_SPEED: f32 = 0.4;
/// Seconds to keep "moving" latched after speed dips.
pub const MOVE_HOLD_TIME: f32 = 0.22;
/// EMA rate for the cadence-driving speed.
pub const SPEED_SMOOTH_RATE: f32 = 12.0;
/// u/s smoothed speed to switch the gait to run.
pub const GAIT_RUN_ENTER: f32 = 5.2;
/// u/s smoothed speed to drop the gait to walk.
pub const GAIT_RUN_EXIT: f32 = 3.6;
/// Minimum dwell between gait/direction switches.
pub const GAIT_HOLD_TIME: f32 = 0.25;
/// Displacement/sec above this is a snap, not locomotion.
const TELEPORT_SPEED: f32 = 25.0;

/// Per-entity hysteresis state; the client keeps one per visual.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocoTrack {
    pub move_hold: f32,
    pub smooth_speed: f32,
    pub moving_backwards: bool,
    pub run_gait: bool,
    pub gait_hold: f32,
    pub dir_pending_frames: u32,
}

/// Frame result used by the walk / run pose picker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocoState {
    /// Smoothed speed (footstep / gait cadence).
    pub speed: f32,
    pub moving: bool,
    pub backwards: bool,
    pub running: bool,
}

impl LocoTrack {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Advance locomotion hysteresis by one frame.
///
/// `vx`/`vz` are render-space horizontal displacement since last frame;
/// `facing` is entity yaw (radians, 0 = +Z) for backpedal detection;
/// `dt` is frame delta in seconds.
pub fn update_locomotion(
    t: &mut LocoTrack,
    vx: f32,
    vz: f32,
    facing: f32,
    dt: f32,
) -> LocoState {
    let dist = (vx * vx + vz * vz).sqrt();
    let mut speed = dist / dt.max(1e-4);
    if speed > TELEPORT_SPEED {
        // Teleport snap, not locomotion.
        speed = 0.0;
    }

    if speed > MOVE_ENTER_SPEED {
        t.move_hold = MOVE_HOLD_TIME;
    } else {
        t.move_hold = (t.move_hold - dt).max(0.0);
    }
    let moving = t.move_hold > 0.0;

    // Smooth cadence speed; while latched-but-stalled keep the last value so
    // footsteps don't lurch toward zero on a stalled frame. Blend capped at 0.5
    // so one long hitch frame cannot fully overwrite the average.
    if speed > MOVE_ENTER_SPEED || !moving {
        let blend = (dt * SPEED_SMOOTH_RATE).min(0.5);
        t.smooth_speed += (speed - t.smooth_speed) * blend;
    }

    if t.gait_hold > 0.0 {
        t.gait_hold -= dt;
    }

    // Only re-judge direction on frames with real displacement; a stalled frame
    // keeps the last direction. A direction CHANGE needs 3 consecutive confirming
    // frames so a one-frame correction nudge does not flash walkBack.
    if speed > MOVE_ENTER_SPEED && dist > 1e-6 {
        let forwards = (vx * facing.sin() + vz * facing.cos()) / dist;
        let backwards = forwards < -0.3;
        if backwards != t.moving_backwards {
            t.dir_pending_frames += 1;
            if t.dir_pending_frames >= 3 {
                t.moving_backwards = backwards;
                t.dir_pending_frames = 0;
            }
        } else {
            t.dir_pending_frames = 0;
        }
    } else if !moving {
        t.moving_backwards = false;
        t.dir_pending_frames = 0;
    }

    if !moving {
        t.run_gait = false;
        t.gait_hold = 0.0;
    } else {
        let want = if t.run_gait {
            t.smooth_speed > GAIT_RUN_EXIT
        } else {
            t.smooth_speed >= GAIT_RUN_ENTER
        };
        if want != t.run_gait && t.gait_hold <= 0.0 {
            t.run_gait = want;
            t.gait_hold = GAIT_HOLD_TIME;
        }
    }

    LocoState {
        speed: t.smooth_speed,
        moving,
        backwards: moving && t.moving_backwards,
        running: moving && t.run_gait,
    }
}

/// Desired high-level pose for procedural (or clip-driven) animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkPose {
    Idle,
    Walk,
    WalkBack,
    Run,
}

/// Pick idle / walk / walkBack / run from a locomotion frame.
pub fn desired_walk_pose(s: &LocoState) -> WalkPose {
    if !s.moving {
        return WalkPose::Idle;
    }
    if s.backwards {
        return WalkPose::WalkBack;
    }
    if s.running {
        WalkPose::Run
    } else {
        WalkPose::Walk
    }
}

/// Cadence scale for a walk/run cycle (matches upstream locomotionTimeScale).
pub fn locomotion_time_scale(pose: WalkPose, speed: f32) -> Option<f32> {
    const WALK_REF: f32 = 2.2;
    const RUN_REF: f32 = 7.0;
    let scale = match pose {
        WalkPose::Walk | WalkPose::WalkBack => (speed / WALK_REF).clamp(0.6, 1.8),
        WalkPose::Run => (speed / RUN_REF).clamp(0.6, 1.6),
        WalkPose::Idle => return None,
    };
    Some(if pose == WalkPose::WalkBack {
        -scale
    } else {
        scale
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FPS: f32 = 1.0 / 60.0;

    fn walk_step(t: &mut LocoTrack, dt: f32, speed: f32) -> LocoState {
        update_locomotion(t, 0.0, speed * dt, 0.0, dt)
    }

    #[test]
    fn steady_walk_reports_moving() {
        let mut t = LocoTrack::new();
        let mut s = walk_step(&mut t, FPS, 2.2);
        for _ in 0..30 {
            s = walk_step(&mut t, FPS, 2.2);
        }
        assert!(s.moving);
        assert!(!s.backwards);
        assert!(s.speed > 1.5);
    }

    #[test]
    fn single_stalled_frame_keeps_moving() {
        let mut t = LocoTrack::new();
        for _ in 0..20 {
            walk_step(&mut t, FPS, 2.2);
        }
        let stalled = update_locomotion(&mut t, 0.0, 0.0, 0.0, FPS);
        assert!(stalled.moving);
    }

    #[test]
    fn short_stall_within_grace_stays_moving() {
        let mut t = LocoTrack::new();
        for _ in 0..20 {
            walk_step(&mut t, FPS, 2.2);
        }
        let mut s = LocoState {
            speed: 0.0,
            moving: true,
            backwards: false,
            running: false,
        };
        for _ in 0..9 {
            s = update_locomotion(&mut t, 0.0, 0.0, 0.0, FPS);
        }
        assert!(9.0 * FPS < MOVE_HOLD_TIME);
        assert!(s.moving);
    }

    #[test]
    fn genuine_stop_idles_after_grace() {
        let mut t = LocoTrack::new();
        for _ in 0..20 {
            walk_step(&mut t, FPS, 2.2);
        }
        let mut s = update_locomotion(&mut t, 0.0, 0.0, 0.0, FPS);
        for _ in 0..30 {
            s = update_locomotion(&mut t, 0.0, 0.0, 0.0, FPS);
        }
        assert!(!s.moving);
    }

    #[test]
    fn backpedal_holds_through_stall() {
        let mut t = LocoTrack::new();
        for _ in 0..10 {
            update_locomotion(&mut t, 0.0, -2.2 * FPS, 0.0, FPS);
        }
        let moving = update_locomotion(&mut t, 0.0, -2.2 * FPS, 0.0, FPS);
        assert!(moving.backwards);
        let stalled = update_locomotion(&mut t, 0.0, 0.0, 0.0, FPS);
        assert!(stalled.moving);
        assert!(stalled.backwards);
    }

    #[test]
    fn teleport_snap_is_not_moving() {
        let mut t = LocoTrack::new();
        let s = update_locomotion(&mut t, 50.0, 0.0, 0.0, FPS);
        assert!(!s.moving);
    }

    #[test]
    fn alternating_walk_stall_never_drops_moving() {
        let mut t = LocoTrack::new();
        walk_step(&mut t, FPS, 2.2);
        let mut ever_stopped = false;
        for i in 0..60 {
            let s = if i % 2 == 0 {
                update_locomotion(&mut t, 0.0, 0.0, 0.0, FPS)
            } else {
                walk_step(&mut t, FPS, 2.2)
            };
            if !s.moving {
                ever_stopped = true;
            }
        }
        assert!(!ever_stopped);
    }

    #[test]
    fn steady_run_settles_into_run_gait() {
        let mut t = LocoTrack::new();
        let mut s = walk_step(&mut t, FPS, 7.0);
        for _ in 0..20 {
            s = walk_step(&mut t, FPS, 7.0);
        }
        assert!(s.running);
        assert_eq!(desired_walk_pose(&s), WalkPose::Run);
    }

    #[test]
    fn noisy_mid_speed_does_not_flip_gait() {
        let mut t = LocoTrack::new();
        for _ in 0..30 {
            walk_step(&mut t, FPS, 7.0);
        }
        let mut flips = 0;
        let mut was_running = true;
        for i in 0..60 {
            // Swing around the old single-threshold (~4.5) zone.
            let speed = if i % 2 == 0 { 4.0 } else { 5.0 };
            let s = walk_step(&mut t, FPS, speed);
            if s.running != was_running {
                flips += 1;
                was_running = s.running;
            }
        }
        assert!(
            flips <= 2,
            "gait hysteresis should suppress mid-threshold flip-flops, got {flips}"
        );
    }

    #[test]
    fn walk_pose_and_time_scale() {
        let walk = LocoState {
            speed: 2.2,
            moving: true,
            backwards: false,
            running: false,
        };
        assert_eq!(desired_walk_pose(&walk), WalkPose::Walk);
        let scale = locomotion_time_scale(WalkPose::Walk, 2.2).unwrap();
        assert!((scale - 1.0).abs() < 1e-3);

        let back = LocoState {
            speed: 2.2,
            moving: true,
            backwards: true,
            running: false,
        };
        assert_eq!(desired_walk_pose(&back), WalkPose::WalkBack);
        assert!(locomotion_time_scale(WalkPose::WalkBack, 2.2).unwrap() < 0.0);

        let idle = LocoState {
            speed: 0.0,
            moving: false,
            backwards: false,
            running: false,
        };
        assert_eq!(desired_walk_pose(&idle), WalkPose::Idle);
        assert!(locomotion_time_scale(WalkPose::Idle, 0.0).is_none());
    }
}
