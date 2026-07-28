//! Upstream-aligned procedural heightfield (pin a3e5e959 / seed 20061).
//!
//! Sim motion and Bevy rendering both sample these pure functions.

use crate::rng::fbm2;
use woc_content::{
    BiomeId, CampDef, HeightStamp, ZoneBand, CAMPS, JAIL_TERRAIN_EDITS, LAKE_BLEND_RADIUS_MULT,
    SOWFIELD_FLAT_FALLOFF, SOWFIELD_FLAT_HEIGHT, SOWFIELD_FLAT_X_MAX, SOWFIELD_FLAT_X_MIN,
    SOWFIELD_FLAT_Z_MAX, SOWFIELD_FLAT_Z_MIN, WATER_LEVEL, WORLD_SEED as CONTENT_WORLD_SEED, ZONES,
};

/// Re-export production seed for hosts.
pub const WORLD_SEED: u32 = CONTENT_WORLD_SEED;

/// Strip half-width / max |x| (re-exported as owned consts for downstream crates).
pub const WORLD_MAX_X: f32 = woc_content::WORLD_MAX_X;
pub const WORLD_MIN_Z: f32 = woc_content::WORLD_MIN_Z;
pub const WORLD_MAX_Z: f32 = woc_content::WORLD_MAX_Z;

/// Deprecated name kept as alias of strip half-width (`WORLD_MAX_X`).
pub const WORLD_HALF: f32 = WORLD_MAX_X;

const HILL_SCALE: f32 = 0.013;
const DETAIL_SCALE: f32 = 0.05;
const RIDGE_HEIGHT: f32 = 40.0;
const RIDGE_SIGMA: f32 = 10.0;
const PASS_HALF_WIDTH: f32 = 10.0;
const PASS_SHOULDER: f32 = 34.0;
pub const TERRACE_STEP: f32 = 6.0;
pub const TERRACE_TREAD: f32 = 0.6;
pub const TERRACE_APRON: f32 = 0.5;
const OUTSIDE_FADE_START: f32 = 2.0;
const OUTSIDE_FADE_END: f32 = 10.0;
const STEEPNESS_SAMPLE: f32 = 0.35;
/// Player climb limit (rise/run) — upstream PLAYER_MAX_CLIMB_SLOPE.
pub const PLAYER_MAX_CLIMB_SLOPE: f32 = 1.5;
pub const WALL_STEEPNESS_MARGIN: f32 = 1.7;

const MIREFEN_CRATER_X: f32 = 149.5;
const MIREFEN_CRATER_Z: f32 = 295.0;
const MIREFEN_CRATER_BOWL_R: f32 = 20.0;
const MIREFEN_CRATER_R: f32 = 30.0;
const MIREFEN_CRATER_DEPTH: f32 = 2.6;
const MIREFEN_CRATER_RIM_H: f32 = 0.95;

struct BiomeShape {
    hill: f32,
    base: f32,
    hub_height: f32,
}

fn biome_shape(b: BiomeId) -> BiomeShape {
    match b {
        BiomeId::Vale => BiomeShape {
            hill: 26.0,
            base: 0.0,
            hub_height: 1.5,
        },
        BiomeId::Marsh => BiomeShape {
            hill: 11.0,
            base: -1.0,
            hub_height: 1.2,
        },
        BiomeId::Peaks => BiomeShape {
            hill: 34.0,
            base: 7.0,
            hub_height: 9.0,
        },
        BiomeId::Beach => BiomeShape {
            hill: 5.0,
            base: -2.4,
            hub_height: 0.8,
        },
        BiomeId::Desert => BiomeShape {
            hill: 15.0,
            base: 2.5,
            hub_height: 2.0,
        },
        BiomeId::Volcano => BiomeShape {
            hill: 42.0,
            base: 9.0,
            hub_height: 6.0,
        },
        BiomeId::Cave => BiomeShape {
            hill: 9.0,
            base: 1.0,
            hub_height: 1.0,
        },
    }
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn terrace_step(v: f32, step: f32, tread: f32, apron: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let band = (v / step).floor();
    let frac = v / step - band;
    let riser = if frac < tread {
        0.0
    } else {
        smoothstep(tread, 1.0, frac)
    };
    ((band + riser) * step).max((v.min(step)) * apron)
}

pub fn water_level() -> f32 {
    WATER_LEVEL
}

pub fn is_in_water_body(x: f32, z: f32) -> bool {
    for zone in ZONES {
        for lake in zone.lakes {
            let dx = x - lake.x;
            let dz = z - lake.z;
            let r_max = lake.radius * LAKE_BLEND_RADIUS_MULT;
            if dx * dx + dz * dz < r_max * r_max {
                return true;
            }
        }
    }
    false
}

pub fn water_level_at(x: f32, z: f32) -> f32 {
    if is_in_water_body(x, z) {
        water_level()
    } else {
        f32::NEG_INFINITY
    }
}

pub fn water_bodies() -> Vec<(f32, f32, f32)> {
    let mut out = Vec::new();
    for zone in ZONES {
        for lake in zone.lakes {
            out.push((lake.x, lake.z, lake.radius * LAKE_BLEND_RADIUS_MULT));
        }
    }
    out
}

fn shape_at(_x: f32, z: f32) -> (f32, f32) {
    let zones = ZONES;
    let mut hill = biome_shape(zones[0].biome).hill;
    let mut base = biome_shape(zones[0].biome).base;
    for i in 0..zones.len().saturating_sub(1) {
        let boundary = zones[i].z_max;
        let t = smoothstep(boundary - 30.0, boundary + 35.0, z);
        let next = biome_shape(zones[i + 1].biome);
        hill = lerp(hill, next.hill, t);
        base = lerp(base, next.base, t);
    }
    (hill, base)
}

fn base_height(x: f32, z: f32, seed: u32) -> f32 {
    let (hill, base) = shape_at(x, z);
    let mut h = (fbm2(x * HILL_SCALE + 100.0, z * HILL_SCALE + 100.0, seed, 4) - 0.5) * hill + base;
    h += (fbm2(x * DETAIL_SCALE, z * DETAIL_SCALE, seed.wrapping_add(7), 2) - 0.5) * 2.2;

    for zone in ZONES {
        let dx = x - zone.hub.x;
        let dz = z - zone.hub.z;
        let d_hub = (dx * dx + dz * dz).sqrt();
        if d_hub < zone.hub.radius * 1.6 {
            let blend = smoothstep(zone.hub.radius * 0.7, zone.hub.radius * 1.6, d_hub);
            let hub_h = biome_shape(zone.biome).hub_height;
            h = h * blend + hub_h * (1.0 - blend);
        }
    }

    let min_land = water_level() + 1.4;
    if h < min_land {
        h = min_land - (min_land - h) * 0.12;
    }

    for zone in ZONES {
        for lake in zone.lakes {
            let d_lake = ((x - lake.x).powi(2) + (z - lake.z).powi(2)).sqrt();
            if d_lake < lake.radius * LAKE_BLEND_RADIUS_MULT {
                let lake_blend = smoothstep(
                    lake.radius * 0.55,
                    lake.radius * LAKE_BLEND_RADIUS_MULT,
                    d_lake,
                );
                h = h * lake_blend + (water_level() - 4.0) * (1.0 - lake_blend);
            }
        }
    }
    h
}

pub fn sowfield_flatten_weight(x: f32, z: f32) -> f32 {
    let dx = (SOWFIELD_FLAT_X_MIN - x)
        .max(x - SOWFIELD_FLAT_X_MAX)
        .max(0.0);
    let dz = (SOWFIELD_FLAT_Z_MIN - z)
        .max(z - SOWFIELD_FLAT_Z_MAX)
        .max(0.0);
    if dx == 0.0 && dz == 0.0 {
        return 1.0;
    }
    let d = (dx * dx + dz * dz).sqrt();
    if d >= SOWFIELD_FLAT_FALLOFF {
        return 0.0;
    }
    1.0 - smoothstep(0.0, 1.0, d / SOWFIELD_FLAT_FALLOFF)
}

fn mirefen_impact_crater_offset(x: f32, z: f32) -> f32 {
    let dx = x - MIREFEN_CRATER_X;
    let dz = z - MIREFEN_CRATER_Z;
    let d = (dx * dx + dz * dz).sqrt();
    if d >= MIREFEN_CRATER_R {
        return 0.0;
    }
    let bowl_t = d / MIREFEN_CRATER_BOWL_R;
    let bowl = if d < MIREFEN_CRATER_BOWL_R {
        -MIREFEN_CRATER_DEPTH * (1.0 - smoothstep(0.0, 1.0, bowl_t))
    } else {
        0.0
    };
    let rim_start = MIREFEN_CRATER_BOWL_R * 0.82;
    if d <= rim_start {
        return bowl;
    }
    let rim_t = (d - rim_start) / (MIREFEN_CRATER_R - rim_start);
    let rim =
        MIREFEN_CRATER_RIM_H * smoothstep(0.0, 0.35, rim_t) * (1.0 - smoothstep(0.72, 1.0, rim_t));
    bowl + rim
}

fn apply_stamp(e: &HeightStamp, x: f32, z: f32, h: f32) -> f32 {
    if e.radius <= 0.0 {
        return h;
    }
    let dx = x - e.x;
    let dz = z - e.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d >= e.radius {
        return h;
    }
    let t = d / e.radius;
    let w = if e.flat_falloff {
        1.0
    } else {
        1.0 - smoothstep(0.0, 1.0, t)
    };
    if e.level_mode {
        lerp(h, e.delta, w)
    } else {
        h + e.delta * w
    }
}

fn apply_edit_layer(x: f32, z: f32, h0: f32) -> f32 {
    let mut h = h0;
    for e in JAIL_TERRAIN_EDITS {
        h = apply_stamp(e, x, z, h);
    }
    h
}

fn ridges() -> [(f32, f32); 2] {
    [(ZONES[0].z_max, 0.0), (ZONES[1].z_max, 0.0)]
}

/// Baseline terrain height (renderer mesh).
pub fn terrain_height(x: f32, z: f32, seed: u32) -> f32 {
    let mut h = base_height(x, z, seed);

    for camp in CAMPS {
        flatten_camp(&mut h, x, z, seed, camp);
    }

    let beyond = (WORLD_MIN_Z - z)
        .max(z - WORLD_MAX_Z)
        .max(x.abs() - WORLD_MAX_X)
        .max(0.0);
    let mountain_detail = 1.0 - smoothstep(OUTSIDE_FADE_START, OUTSIDE_FADE_END, beyond);

    let sow = sowfield_flatten_weight(x, z);
    if sow > 0.0 {
        h = lerp(h, SOWFIELD_FLAT_HEIGHT, sow);
    }

    let mut mountain_add = 0.0;
    for (ridge_z, pass_x) in ridges() {
        let dz = (z - ridge_z).abs();
        if dz < RIDGE_SIGMA * 3.0 {
            let pass = smoothstep(PASS_HALF_WIDTH, PASS_SHOULDER, (x - pass_x).abs());
            if pass > 0.0 {
                let profile = (-(dz * dz) / (2.0 * RIDGE_SIGMA * RIDGE_SIGMA)).exp();
                let crest = 1.0
                    + (fbm2(x * 0.03, ridge_z * 0.03, seed.wrapping_add(19), 2) - 0.5)
                        * 0.4
                        * mountain_detail
                    + (fbm2(x * 0.11, ridge_z * 0.11, seed.wrapping_add(23), 2) - 0.5)
                        * 0.14
                        * mountain_detail;
                mountain_add += RIDGE_HEIGHT * crest * profile * pass;
            }
        }
    }

    let rim_x = smoothstep(WORLD_MAX_X - 30.0, WORLD_MAX_X - 6.0, x.abs());
    let rim_s = smoothstep(WORLD_MIN_Z + 30.0, WORLD_MIN_Z + 6.0, z);
    let rim_n = smoothstep(WORLD_MAX_Z - 30.0, WORLD_MAX_Z - 6.0, z);
    let rim = rim_x.max(rim_s).max(rim_n);
    if rim > 0.0 {
        let rim_crest = 1.0
            + (fbm2(x * 0.025, z * 0.025, seed.wrapping_add(29), 3) - 0.5) * 0.35 * mountain_detail
            + (fbm2(x * 0.09, z * 0.09, seed.wrapping_add(37), 2) - 0.5) * 0.15 * mountain_detail;
        mountain_add += rim * 55.0 * rim_crest;
    }

    if mountain_add != 0.0 {
        let terraced = terrace_step(mountain_add, TERRACE_STEP, TERRACE_TREAD, TERRACE_APRON);
        h += terraced * mountain_detail + mountain_add * (1.0 - mountain_detail);
    }
    h += mirefen_impact_crater_offset(x, z);
    apply_edit_layer(x, z, h)
}

fn flatten_camp(h: &mut f32, x: f32, z: f32, seed: u32, camp: &CampDef) {
    let dx = x - camp.center_x;
    let dz = z - camp.center_z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < camp.radius * 1.8 {
        let ch = base_height(camp.center_x, camp.center_z, seed);
        let blend = smoothstep(camp.radius * 0.8, camp.radius * 1.8, d);
        *h = *h * blend + ch * (1.0 - blend);
    }
}

/// Walkable ground height (dungeons/docks stubs included).
pub fn ground_height(x: f32, z: f32, seed: u32) -> f32 {
    // Instance floors: far positive X reserved (matches prior dungeon shell placement).
    if x > 5_000.0 {
        return 0.0;
    }
    terrain_height(x, z, seed)
}

pub fn terrain_steepness(x: f32, z: f32, seed: u32) -> f32 {
    let e = STEEPNESS_SAMPLE;
    let hx = (ground_height(x + e, z, seed) - ground_height(x - e, z, seed)) / (2.0 * e);
    let hz = (ground_height(x, z + e, seed) - ground_height(x, z - e, seed)) / (2.0 * e);
    (hx * hx + hz * hz).sqrt()
}

pub fn clamp_to_world(x: f32, z: f32) -> (f32, f32) {
    (
        x.clamp(-WORLD_MAX_X, WORLD_MAX_X),
        z.clamp(WORLD_MIN_Z, WORLD_MAX_Z),
    )
}

pub fn zone_band_at(z: f32) -> &'static ZoneBand {
    woc_content::zone_at(z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_max_steepness(seed: u32, from: (f32, f32), to: (f32, f32)) -> f32 {
        let dist = ((to.0 - from.0).hypot(to.1 - from.1)).max(0.001);
        let steps = (dist / 0.5).ceil() as i32;
        let mut max = 0.0f32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = from.0 + (to.0 - from.0) * t;
            let z = from.1 + (to.1 - from.1) * t;
            max = max.max(terrain_steepness(x, z, seed));
        }
        max
    }

    #[test]
    fn climb_limit_constant() {
        assert!((PLAYER_MAX_CLIMB_SLOPE - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn ridge_crossings_outside_pass_are_impassable() {
        let seed = WORLD_SEED;
        for &rz in &[180.0f32, 540.0] {
            let mut x: f32 = -172.0;
            while x <= 172.0 {
                if x.abs() < PASS_HALF_WIDTH + 26.0 {
                    x += 4.0;
                    continue;
                }
                let max = path_max_steepness(seed, (x, rz - 50.0), (x, rz + 50.0));
                assert!(
                    max > WALL_STEEPNESS_MARGIN,
                    "ridge z={rz} at x={x}: steepness={max}"
                );
                x += 4.0;
            }
        }
    }

    #[test]
    fn pass_shoulder_is_wall() {
        let seed = WORLD_SEED;
        for &rz in &[180.0f32, 540.0] {
            let mut x = 16.0;
            while x <= 34.0 {
                for side in [-1.0f32, 1.0] {
                    let xx = side * x;
                    let max = path_max_steepness(seed, (xx, rz - 50.0), (xx, rz + 50.0));
                    assert!(
                        max > WALL_STEEPNESS_MARGIN,
                        "shoulder ridge z={rz} x={xx}: {max}"
                    );
                }
                x += 2.0;
            }
        }
    }

    #[test]
    fn road_pass_corridor_is_crossable() {
        let seed = WORLD_SEED;
        for &rz in &[180.0f32, 540.0] {
            let max = path_max_steepness(seed, (0.0, rz - 50.0), (0.0, rz + 50.0));
            assert!(
                max < PLAYER_MAX_CLIMB_SLOPE,
                "pass at z={rz} should be climbable, got {max}"
            );
        }
    }

    #[test]
    fn hub_plateau_near_eastbrook() {
        let h = terrain_height(0.0, 0.0, WORLD_SEED);
        assert!(
            (h - 1.5).abs() < 0.75,
            "eastbrook hub should be near hubHeight 1.5, got {h}"
        );
    }

    #[test]
    fn golden_grid_samples_are_finite() {
        let seed = WORLD_SEED;
        for x in [-100, -50, 0, 50, 100] {
            for z in [-100, 0, 100, 300, 660] {
                let h = terrain_height(x as f32, z as f32, seed);
                assert!(h.is_finite(), "height at ({x},{z})");
                let g = ground_height(x as f32, z as f32, seed);
                assert!(g.is_finite());
            }
        }
    }
}
