//! Axis-aligned bounding boxes and character XZ sweeps.

/// Axis-aligned box in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
}

impl Aabb {
    pub const fn new(
        min_x: f32,
        min_y: f32,
        min_z: f32,
        max_x: f32,
        max_y: f32,
        max_z: f32,
    ) -> Self {
        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// Character footprint AABB standing on `(x, y, z)` with horizontal radius and height.
    pub fn character(x: f32, y: f32, z: f32, radius: f32, height: f32) -> Self {
        Self::new(
            x - radius,
            y,
            z - radius,
            x + radius,
            y + height,
            z + radius,
        )
    }

    #[inline]
    pub fn overlaps_y(&self, other: &Aabb) -> bool {
        self.min_y < other.max_y && self.max_y > other.min_y
    }

    /// Expand this AABB horizontally by `radius` (Minkowski sum for a point-radius character).
    pub fn expand_xz(&self, radius: f32) -> Self {
        Self::new(
            self.min_x - radius,
            self.min_y,
            self.min_z - radius,
            self.max_x + radius,
            self.max_y,
            self.max_z + radius,
        )
    }
}

/// Sweep a point from `(ox, oz)` by `(dx, dz)` against an expanded AABB in XZ.
/// Returns earliest entry time `t ∈ [0, 1]` if the segment hits the box, else `None`.
/// Already-overlapping starts return `Some(0.0)`.
fn sweep_point_xz(ox: f32, oz: f32, dx: f32, dz: f32, box_xz: &Aabb) -> Option<f32> {
    // Slab method in 2D (X/Z).
    let mut t_enter = 0.0_f32;
    let mut t_exit = 1.0_f32;

    // X slabs
    if dx.abs() < 1e-8 {
        if ox < box_xz.min_x || ox > box_xz.max_x {
            return None;
        }
    } else {
        let inv = 1.0 / dx;
        let mut t1 = (box_xz.min_x - ox) * inv;
        let mut t2 = (box_xz.max_x - ox) * inv;
        if t1 > t2 {
            core::mem::swap(&mut t1, &mut t2);
        }
        t_enter = t_enter.max(t1);
        t_exit = t_exit.min(t2);
        if t_enter > t_exit {
            return None;
        }
    }

    // Z slabs
    if dz.abs() < 1e-8 {
        if oz < box_xz.min_z || oz > box_xz.max_z {
            return None;
        }
    } else {
        let inv = 1.0 / dz;
        let mut t1 = (box_xz.min_z - oz) * inv;
        let mut t2 = (box_xz.max_z - oz) * inv;
        if t1 > t2 {
            core::mem::swap(&mut t1, &mut t2);
        }
        t_enter = t_enter.max(t1);
        t_exit = t_exit.min(t2);
        if t_enter > t_exit {
            return None;
        }
    }

    if t_exit < 0.0 || t_enter > 1.0 {
        return None;
    }
    // Inside at start (t_enter < 0) → blocked immediately.
    Some(t_enter.max(0.0))
}

/// Sweep a character along a horizontal delta, stopping before the first solid AABB.
///
/// Vertical extent still matters: colliders that do not overlap the character in Y
/// are ignored (walk under high overhangs / ignore buried volumes).
#[allow(clippy::too_many_arguments)]
pub fn sweep_character_xz(
    x: f32,
    y: f32,
    z: f32,
    dx: f32,
    dz: f32,
    radius: f32,
    height: f32,
    colliders: &[Aabb],
) -> (f32, f32) {
    if dx.abs() < 1e-8 && dz.abs() < 1e-8 {
        return (x, z);
    }

    let moving = Aabb::character(x, y, z, radius, height);
    let mut best_t = 1.0_f32;
    let skin = 1e-3_f32;

    for solid in colliders {
        if !moving.overlaps_y(solid) {
            continue;
        }
        let expanded = solid.expand_xz(radius);
        // Point-sweep the character center against the Minkowski-expanded solid.
        if let Some(t) = sweep_point_xz(x, z, dx, dz, &expanded) {
            if t < best_t {
                best_t = t;
            }
        }
    }

    if best_t >= 1.0 {
        return (x + dx, z + dz);
    }
    if best_t <= 0.0 {
        // Already overlapping or flush — do not advance into the solid.
        return (x, z);
    }

    let t = (best_t - skin).max(0.0);
    (x + dx * t, z + dz * t)
}
