//! Hardcoded Eastbrook building colliders (framework scaffold).

use super::Aabb;

/// Eastbrook Inn — north of the town square / chapel path.
///
/// Footprint sits past the graveyard (`z ≈ 8`) so northbound walkers from spawn
/// `(2, 4)` collide with a solid volume.
pub const EASTBROOK_INN: Aabb = Aabb::new(
    -1.0,  // min_x
    -2.0,  // min_y (hub plateau ~1.5)
    10.0,  // min_z
    3.0,   // max_x
    12.0,  // max_y
    14.0,  // max_z
);

/// All solid building AABBs in Eastbrook Vale (extensible).
pub fn eastbrook_buildings() -> &'static [Aabb] {
    &[EASTBROOK_INN]
}
