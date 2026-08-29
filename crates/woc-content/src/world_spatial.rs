//! Upstream-aligned world spatial tables (pin a3e5e959).
//! Continuous north-running strip: x in [-WORLD_MAX_X, WORLD_MAX_X], z bands per zone.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiomeId {
    Vale,
    Marsh,
    Peaks,
    Beach,
    Desert,
    Volcano,
    Cave,
}

#[derive(Debug, Clone, Copy)]
pub struct HubDef {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct LakeDef {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ZoneBand {
    pub id: &'static str,
    pub name: &'static str,
    pub z_min: f32,
    pub z_max: f32,
    pub biome: BiomeId,
    pub hub: HubDef,
    pub lakes: &'static [LakeDef],
}

#[derive(Debug, Clone, Copy)]
pub struct CampDef {
    pub center_x: f32,
    pub center_z: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct HeightStamp {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub delta: f32,
    pub flat_falloff: bool,
    pub level_mode: bool,
}

/// Production world seed (upstream `main.ts` / `terrain_walls.test.ts`).
pub const WORLD_SEED: u32 = 20061;

pub const WORLD_SIZE: f32 = 360.0;
pub const WORLD_MAX_X: f32 = WORLD_SIZE / 2.0;
pub const WATER_LEVEL: f32 = -4.5;
pub const LAKE_BLEND_RADIUS_MULT: f32 = 1.6;

pub const ZONE_EASTBROOK: ZoneBand = ZoneBand {
    id: "eastbrook_vale",
    name: "Eastbrook Vale",
    z_min: -180.0,
    z_max: 180.0,
    biome: BiomeId::Vale,
    hub: HubDef {
        x: 0.0,
        z: 0.0,
        radius: 26.0,
        name: "Eastbrook",
    },
    lakes: &[LakeDef {
        x: -92.0,
        z: 88.0,
        radius: 30.0,
    }],
};

pub const ZONE_MIREFEN: ZoneBand = ZoneBand {
    id: "mirefen_marsh",
    name: "Mirefen Marsh",
    z_min: 180.0,
    z_max: 540.0,
    biome: BiomeId::Marsh,
    hub: HubDef {
        x: 0.0,
        z: 300.0,
        radius: 20.0,
        name: "Fenbridge",
    },
    lakes: &[
        LakeDef {
            x: -110.0,
            z: 310.0,
            radius: 35.0,
        },
        LakeDef {
            x: 60.0,
            z: 380.0,
            radius: 25.0,
        },
        LakeDef {
            x: -40.0,
            z: 450.0,
            radius: 20.0,
        },
    ],
};

pub const ZONE_THORNPEAK: ZoneBand = ZoneBand {
    id: "thornpeak_heights",
    name: "Thornpeak Heights",
    z_min: 540.0,
    z_max: 900.0,
    biome: BiomeId::Peaks,
    hub: HubDef {
        x: 0.0,
        z: 660.0,
        radius: 20.0,
        name: "Highwatch",
    },
    lakes: &[LakeDef {
        x: -70.0,
        z: 760.0,
        radius: 18.0,
    }],
};

pub const ZONES: &[ZoneBand] = &[ZONE_EASTBROOK, ZONE_MIREFEN, ZONE_THORNPEAK];

pub const WORLD_MIN_Z: f32 = ZONE_EASTBROOK.z_min;
pub const WORLD_MAX_Z: f32 = ZONE_THORNPEAK.z_max;

/// Camp flatten disks (upstream ZONE1+2+3 camps with radius > 0).
pub static CAMPS: &[CampDef] = &[
    // Zone 1
    CampDef {
        center_x: -15.0,
        center_z: 55.0,
        radius: 22.0,
    },
    CampDef {
        center_x: 20.0,
        center_z: 70.0,
        radius: 20.0,
    },
    CampDef {
        center_x: 0.0,
        center_z: 95.0,
        radius: 8.0,
    },
    CampDef {
        center_x: 55.0,
        center_z: 12.0,
        radius: 22.0,
    },
    CampDef {
        center_x: 80.0,
        center_z: -15.0,
        radius: 18.0,
    },
    CampDef {
        center_x: 118.0,
        center_z: -26.0,
        radius: 5.0,
    },
    CampDef {
        center_x: -60.0,
        center_z: 5.0,
        radius: 22.0,
    },
    CampDef {
        center_x: -75.0,
        center_z: 57.0,
        radius: 14.0,
    },
    CampDef {
        center_x: -82.0,
        center_z: -62.0,
        radius: 20.0,
    },
    CampDef {
        center_x: 65.0,
        center_z: -65.0,
        radius: 24.0,
    },
    CampDef {
        center_x: 90.0,
        center_z: -90.0,
        radius: 16.0,
    },
    CampDef {
        center_x: 92.0,
        center_z: -92.0,
        radius: 2.0,
    },
    CampDef {
        center_x: 80.0,
        center_z: 78.0,
        radius: 18.0,
    },
    CampDef {
        center_x: 92.0,
        center_z: 90.0,
        radius: 4.0,
    },
    CampDef {
        center_x: 88.0,
        center_z: 90.0,
        radius: 6.0,
    },
    CampDef {
        center_x: 88.0,
        center_z: 92.0,
        radius: 3.0,
    },
    // Zone 2
    CampDef {
        center_x: -40.0,
        center_z: 230.0,
        radius: 22.0,
    },
    CampDef {
        center_x: 35.0,
        center_z: 225.0,
        radius: 20.0,
    },
    CampDef {
        center_x: -82.0,
        center_z: 273.0,
        radius: 15.0,
    },
    CampDef {
        center_x: -120.0,
        center_z: 350.0,
        radius: 13.0,
    },
    CampDef {
        center_x: -132.0,
        center_z: 333.0,
        radius: 5.0,
    },
    CampDef {
        center_x: 70.0,
        center_z: 300.0,
        radius: 20.0,
    },
    CampDef {
        center_x: 95.0,
        center_z: 340.0,
        radius: 16.0,
    },
    CampDef {
        center_x: 98.0,
        center_z: 348.0,
        radius: 3.0,
    },
    CampDef {
        center_x: 90.0,
        center_z: 420.0,
        radius: 20.0,
    },
    CampDef {
        center_x: 115.0,
        center_z: 450.0,
        radius: 16.0,
    },
    CampDef {
        center_x: 118.0,
        center_z: 455.0,
        radius: 5.0,
    },
    CampDef {
        center_x: -80.0,
        center_z: 420.0,
        radius: 22.0,
    },
    CampDef {
        center_x: -105.0,
        center_z: 455.0,
        radius: 18.0,
    },
    CampDef {
        center_x: -120.0,
        center_z: 480.0,
        radius: 8.0,
    },
    CampDef {
        center_x: 15.0,
        center_z: 470.0,
        radius: 20.0,
    },
    CampDef {
        center_x: -25.0,
        center_z: 490.0,
        radius: 16.0,
    },
    CampDef {
        center_x: -5.0,
        center_z: 500.0,
        radius: 12.0,
    },
    CampDef {
        center_x: 18.0,
        center_z: 472.0,
        radius: 8.0,
    },
    CampDef {
        center_x: 24.0,
        center_z: 492.0,
        radius: 5.0,
    },
    CampDef {
        center_x: 0.0,
        center_z: 510.0,
        radius: 2.0,
    },
    CampDef {
        center_x: 72.0,
        center_z: 428.0,
        radius: 11.0,
    },
    CampDef {
        center_x: 110.0,
        center_z: 440.0,
        radius: 11.0,
    },
    // Zone 3 (skip radius 0 training dummy)
    CampDef {
        center_x: -50.0,
        center_z: 590.0,
        radius: 22.0,
    },
    CampDef {
        center_x: 45.0,
        center_z: 600.0,
        radius: 20.0,
    },
    CampDef {
        center_x: -82.0,
        center_z: 575.0,
        radius: 5.0,
    },
    CampDef {
        center_x: 75.0,
        center_z: 625.0,
        radius: 18.0,
    },
    CampDef {
        center_x: 105.0,
        center_z: 600.0,
        radius: 14.0,
    },
    CampDef {
        center_x: 100.0,
        center_z: 617.0,
        radius: 5.0,
    },
    CampDef {
        center_x: -90.0,
        center_z: 700.0,
        radius: 22.0,
    },
    CampDef {
        center_x: -60.0,
        center_z: 730.0,
        radius: 18.0,
    },
    CampDef {
        center_x: -125.0,
        center_z: 740.0,
        radius: 18.0,
    },
    CampDef {
        center_x: -132.0,
        center_z: 748.0,
        radius: 2.0,
    },
    CampDef {
        center_x: -45.0,
        center_z: 768.0,
        radius: 4.0,
    },
    CampDef {
        center_x: 110.0,
        center_z: 760.0,
        radius: 20.0,
    },
    CampDef {
        center_x: 135.0,
        center_z: 795.0,
        radius: 16.0,
    },
    CampDef {
        center_x: 145.0,
        center_z: 815.0,
        radius: 8.0,
    },
    CampDef {
        center_x: 55.0,
        center_z: 820.0,
        radius: 20.0,
    },
    CampDef {
        center_x: 34.0,
        center_z: 845.0,
        radius: 16.0,
    },
    CampDef {
        center_x: 40.0,
        center_z: 855.0,
        radius: 14.0,
    },
    CampDef {
        center_x: -40.0,
        center_z: 830.0,
        radius: 20.0,
    },
    CampDef {
        center_x: -40.0,
        center_z: 838.0,
        radius: 16.0,
    },
    CampDef {
        center_x: -34.0,
        center_z: 842.0,
        radius: 5.0,
    },
    CampDef {
        center_x: 80.0,
        center_z: 845.0,
        radius: 4.0,
    },
    CampDef {
        center_x: 80.0,
        center_z: 845.0,
        radius: 7.0,
    },
];

/// Jail plateau far off-world (upstream jail.ts).
pub const JAIL_TERRAIN_EDITS: &[HeightStamp] = &[HeightStamp {
    x: -12_000.0,
    z: -12_000.0,
    radius: 48.0,
    delta: 0.0,
    flat_falloff: true,
    level_mode: true,
}];

/// Sowfield flatten rectangle (upstream vale_cup_layout.ts).
pub const SOWFIELD_FLAT_X_MIN: f32 = -56.0;
pub const SOWFIELD_FLAT_X_MAX: f32 = 34.0;
pub const SOWFIELD_FLAT_Z_MIN: f32 = -141.0;
pub const SOWFIELD_FLAT_Z_MAX: f32 = -83.0;
pub const SOWFIELD_FLAT_HEIGHT: f32 = -2.6;
pub const SOWFIELD_FLAT_FALLOFF: f32 = 8.0;

pub fn zone_at(z: f32) -> &'static ZoneBand {
    for zone in ZONES {
        if z < zone.z_max {
            return zone;
        }
    }
    ZONES.last().expect("zones non-empty")
}

pub fn zone_by_id(id: &str) -> Option<&'static ZoneBand> {
    ZONES.iter().find(|z| z.id == id || alias_matches(id, z.id))
}

fn alias_matches(alias: &str, id: &str) -> bool {
    matches!(
        (alias, id),
        ("eastbrook", "eastbrook_vale")
            | ("eastfen", "mirefen_marsh")
            | ("mirefen", "mirefen_marsh")
            | ("thornpeak", "thornpeak_heights")
            | ("highwatch", "thornpeak_heights")
            | ("fenbridge", "mirefen_marsh")
    )
}

/// Compatibility aliases used by existing sim/client strings.
pub fn canonical_zone_id(id: &str) -> Option<&'static str> {
    zone_by_id(id).map(|z| z.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_zone_bands_cover_strip() {
        assert_eq!(ZONES.len(), 3);
        assert_eq!(WORLD_MIN_Z, -180.0);
        assert_eq!(WORLD_MAX_Z, 900.0);
        assert_eq!(zone_at(0.0).id, "eastbrook_vale");
        assert_eq!(zone_at(300.0).id, "mirefen_marsh");
        assert_eq!(zone_at(660.0).id, "thornpeak_heights");
    }
}
