use crate::content::stations::STATIONS;
use crate::professions::types::{StationType, Vec2, STATION_RADIUS};

pub fn in_station_range(pos: Vec2, kind: StationType) -> bool {
    STATIONS
        .iter()
        .find(|s| s.kind == kind)
        .map(|s| pos.distance(s.pos) <= STATION_RADIUS)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::professions::types::StationType;

    #[test]
    fn forge_at_origin_is_in_range() {
        assert!(in_station_range(
            Vec2 { x: 0.0, z: 0.0 },
            StationType::Forge
        ));
    }

    #[test]
    fn loom_position_from_brief_is_in_range() {
        assert!(in_station_range(
            Vec2 { x: 20.0, z: -10.0 },
            StationType::Loom
        ));
    }

    #[test]
    fn jewelers_bench_position_from_brief_is_in_range() {
        let pos = Vec2 { x: 120.0, z: -50.0 };
        assert!(in_station_range(pos, StationType::JewelersBench));
        assert!(!in_station_range(pos, StationType::Forge));
        assert!(!in_station_range(pos, StationType::Loom));
        assert!(!in_station_range(pos, StationType::Toolworks));
    }

    #[test]
    fn far_from_forge_is_out_of_range() {
        assert!(!in_station_range(
            Vec2 { x: 100.0, z: 100.0 },
            StationType::Forge
        ));
    }
}
