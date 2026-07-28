//! Character collision primitives (AABB sweep) for player motion.

pub mod aabb;
pub mod buildings;

pub use aabb::{sweep_character_xz, Aabb};
pub use buildings::{eastbrook_buildings, EASTBROOK_INN};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PLAYER_RADIUS;

    #[test]
    fn sweep_stops_before_solid_aabb() {
        // Thin wall at x ∈ [1, 2], z ∈ [-2, 2].
        let wall = Aabb::new(1.0, -5.0, -2.0, 2.0, 5.0, 2.0);
        let (nx, nz) = sweep_character_xz(0.0, 0.0, 0.0, 5.0, 0.0, PLAYER_RADIUS, 1.8, &[wall]);
        // Character center must stay west of the wall face minus radius.
        assert!(
            nx <= 1.0 - PLAYER_RADIUS + 1e-3,
            "tunneled through wall: nx={nx}"
        );
        assert!((nz - 0.0).abs() < 1e-5);
    }

    #[test]
    fn sweep_allows_motion_parallel_to_wall() {
        let wall = Aabb::new(1.0, -5.0, -2.0, 2.0, 5.0, 2.0);
        // Start clear of the wall (x=0), move purely in +z — no hit.
        let (nx, nz) = sweep_character_xz(0.0, 0.0, 0.0, 0.0, 3.0, PLAYER_RADIUS, 1.8, &[wall]);
        assert!((nx - 0.0).abs() < 1e-5);
        assert!((nz - 3.0).abs() < 1e-5);
    }

    #[test]
    fn eastbrook_inn_is_solid_volume() {
        let inn = EASTBROOK_INN;
        assert!(inn.max_x > inn.min_x);
        assert!(inn.max_z > inn.min_z);
        assert!(inn.max_y > inn.min_y);
        assert!(
            eastbrook_buildings().iter().any(|b| b == &inn),
            "inn must be registered in eastbrook_buildings()"
        );
    }
}
