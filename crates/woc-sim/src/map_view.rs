//! Pure world-map / minimap projection and terrain paint.
//!
//! Geometry follows the upstream overworld map convention: **+Z north (up on
//! canvas)**, **+X left** (east is −X so facing-right turns decrease facing).

use crate::world::{terrain_height, water_level_at, WORLD_MAX_X, WORLD_MAX_Z, WORLD_MIN_Z};
use woc_content::{zone_at, zone_by_id, BiomeId, ZoneBand, ZONES};

/// World-space axis-aligned region projected onto a map canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapRegion {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl MapRegion {
    pub fn full_strip() -> Self {
        Self {
            min_x: -WORLD_MAX_X,
            max_x: WORLD_MAX_X,
            min_z: WORLD_MIN_Z,
            max_z: WORLD_MAX_Z,
        }
    }

    pub fn from_zone_band(band: &ZoneBand) -> Self {
        Self {
            min_x: -WORLD_MAX_X,
            max_x: WORLD_MAX_X,
            min_z: band.z_min,
            max_z: band.z_max,
        }
    }

    /// Square view centered on a point (used by the minimap).
    pub fn around(x: f32, z: f32, half_span: f32) -> Self {
        let half = half_span.max(8.0);
        Self {
            min_x: x - half,
            max_x: x + half,
            min_z: z - half,
            max_z: z + half,
        }
    }

    pub fn span_x(self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    pub fn span_z(self) -> f32 {
        (self.max_z - self.min_z).max(1.0)
    }

    pub fn contains(self, x: f32, z: f32) -> bool {
        x >= self.min_x && x <= self.max_x && z >= self.min_z && z <= self.max_z
    }
}

/// Resolve the map region for a zone id / strip position.
pub fn region_for_zone(zone_id: &str, player_z: f32) -> MapRegion {
    if let Some(band) = zone_by_id(zone_id) {
        return MapRegion::from_zone_band(band);
    }
    // Layout tags (eastbrook / eastfen / mirefen / thornpeak) and instances.
    let band = match zone_id {
        "eastbrook" | "eastbrook_vale" => Some(&woc_content::ZONE_EASTBROOK),
        "eastfen" | "fenbridge" | "mirefen" | "mirefen_marsh" => Some(&woc_content::ZONE_MIREFEN),
        "thornpeak" | "thornpeak_heights" | "highwatch" => Some(&woc_content::ZONE_THORNPEAK),
        _ if zone_id.starts_with("instance:") || zone_id.starts_with("delve:") => {
            Some(zone_at(player_z))
        }
        _ => None,
    };
    match band {
        Some(b) => MapRegion::from_zone_band(b),
        None => MapRegion::from_zone_band(zone_at(player_z)),
    }
}

/// Project world `(x, z)` into canvas pixel space. Origin is top-left.
///
/// Upstream: `x = maxX - (ix/W)*spanX`, `z = maxZ - (iy/H)*spanZ`.
pub fn world_to_pixel(x: f32, z: f32, region: MapRegion, width: u32, height: u32) -> (f32, f32) {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let px = ((region.max_x - x) / region.span_x()) * w;
    let py = ((region.max_z - z) / region.span_z()) * h;
    (px, py)
}

pub fn pixel_to_world(px: f32, py: f32, region: MapRegion, width: u32, height: u32) -> (f32, f32) {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let x = region.max_x - (px / w) * region.span_x();
    let z = region.max_z - (py / h) * region.span_z();
    (x, z)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapMarkerKind {
    Player,
    Ally,
    Party,
    Hub,
    QuestAvailable,
    QuestReady,
    Portal,
    Mob,
    Npc,
}

#[derive(Debug, Clone)]
pub struct MapMarker {
    pub x: f32,
    pub z: f32,
    pub kind: MapMarkerKind,
    pub label: String,
}

/// Paint a rectangular terrain heightfield into an RGBA8 buffer (`width*height*4`).
pub fn paint_terrain_rgba(data: &mut [u8], width: u32, height: u32, region: MapRegion, seed: u32) {
    let w = width as usize;
    let h = height as usize;
    assert_eq!(data.len(), w * h * 4);
    for iy in 0..h {
        let mut prev_h = 0.0f32;
        for ix in 0..w {
            let (x, z) = pixel_to_world(ix as f32 + 0.5, iy as f32 + 0.5, region, width, height);
            let th = terrain_height(x, z, seed);
            let wl = water_level_at(x, z);
            let biome = zone_at(z).biome;
            let (mut r, mut g, mut b) = biome_base(biome);
            if th < wl {
                r = 38;
                g = 84;
                b = 138;
            } else if th > 26.0 {
                r = 168;
                g = 172;
                b = 178;
            } else if th > 11.0 {
                r = 112;
                g = 110;
                b = 102;
            } else if th > 6.0 {
                r = 88;
                g = 102;
                b = 62;
            }
            if near_hub(x, z) {
                r = 125;
                g = 100;
                b = 66;
            }
            let left = if ix == 0 { th } else { prev_h };
            prev_h = th;
            if th >= wl {
                let shade = (1.0 + (th - left) * 0.16).clamp(0.74, 1.28);
                r = ((r as f32) * shade).min(255.0) as u8;
                g = ((g as f32) * shade).min(255.0) as u8;
                b = ((b as f32) * shade).min(255.0) as u8;
            }
            let k = (iy * w + ix) * 4;
            data[k] = r;
            data[k + 1] = g;
            data[k + 2] = b;
            data[k + 3] = 255;
        }
    }
}

fn biome_base(biome: BiomeId) -> (u8, u8, u8) {
    match biome {
        BiomeId::Marsh => (64, 86, 48),
        BiomeId::Peaks => (92, 100, 82),
        BiomeId::Beach => (120, 118, 72),
        BiomeId::Desert => (148, 132, 78),
        BiomeId::Volcano => (110, 70, 54),
        BiomeId::Cave => (70, 68, 66),
        BiomeId::Vale => (58, 105, 48),
    }
}

fn near_hub(x: f32, z: f32) -> bool {
    ZONES
        .iter()
        .any(|zn| (x - zn.hub.x).hypot(z - zn.hub.z) < 14.0)
}

/// Composite terrain + markers into an RGBA8 buffer.
///
/// When `circular_clip` is true, only markers inside a circular disc are drawn
/// (minimap), and out-of-disc terrain pixels are made transparent.
pub fn paint_map_frame(
    data: &mut [u8],
    width: u32,
    height: u32,
    region: MapRegion,
    seed: u32,
    markers: &[MapMarker],
    circular_clip: bool,
) {
    paint_terrain_rgba(data, width, height, region, seed);
    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let radius = width.min(height) as f32 * 0.5 - 2.0;

    if circular_clip {
        apply_circular_mask(data, width, height, cx, cy, radius);
    }

    for marker in markers {
        let (mx, my) = world_to_pixel(marker.x, marker.z, region, width, height);
        if circular_clip {
            let dx = mx - cx;
            let dy = my - cy;
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
        }
        match marker.kind {
            MapMarkerKind::Player => {
                // Facing is painted by the caller via [`paint_player_arrow`].
                fill_disc(data, width, height, mx, my, 3.0, [255, 230, 120, 255]);
            }
            MapMarkerKind::Ally => fill_disc(data, width, height, mx, my, 3.0, [90, 180, 255, 255]),
            MapMarkerKind::Party => {
                fill_disc(data, width, height, mx, my, 3.5, [70, 140, 255, 255])
            }
            MapMarkerKind::Hub => fill_disc(data, width, height, mx, my, 4.0, [210, 170, 90, 255]),
            MapMarkerKind::QuestAvailable => {
                draw_glyph_dot(data, width, height, mx, my, [255, 210, 60, 255])
            }
            MapMarkerKind::QuestReady => {
                draw_glyph_dot(data, width, height, mx, my, [80, 220, 120, 255])
            }
            MapMarkerKind::Portal => {
                fill_disc(data, width, height, mx, my, 3.5, [180, 120, 255, 255])
            }
            MapMarkerKind::Mob => fill_rect(data, width, height, mx, my, 3, [200, 80, 70, 255]),
            MapMarkerKind::Npc => fill_disc(data, width, height, mx, my, 2.5, [160, 200, 120, 255]),
        }
    }
}

/// Draw a facing arrow for the local player. `yaw` is radians; 0 faces +Z (north / up).
pub fn paint_player_arrow(data: &mut [u8], width: u32, height: u32, mx: f32, my: f32, yaw: f32) {
    // Tip points "up" (negative canvas Y) at yaw 0; rotate by −yaw so right turns match world.
    let tip = rotate(0.0, -7.0, -yaw);
    let left = rotate(-4.5, 5.5, -yaw);
    let right = rotate(4.5, 5.5, -yaw);
    fill_triangle(
        data,
        width,
        height,
        mx + tip.0,
        my + tip.1,
        mx + left.0,
        my + left.1,
        mx + right.0,
        my + right.1,
        [255, 236, 140, 255],
    );
}

fn rotate(x: f32, y: f32, angle: f32) -> (f32, f32) {
    let (s, c) = angle.sin_cos();
    (x * c - y * s, x * s + y * c)
}

fn apply_circular_mask(data: &mut [u8], width: u32, height: u32, cx: f32, cy: f32, radius: f32) {
    let w = width as usize;
    let h = height as usize;
    let r2 = radius * radius;
    for iy in 0..h {
        for ix in 0..w {
            let dx = ix as f32 + 0.5 - cx;
            let dy = iy as f32 + 0.5 - cy;
            if dx * dx + dy * dy > r2 {
                let k = (iy * w + ix) * 4;
                data[k + 3] = 0;
            }
        }
    }
}

fn fill_disc(
    data: &mut [u8],
    width: u32,
    height: u32,
    cx: f32,
    cy: f32,
    radius: f32,
    rgba: [u8; 4],
) {
    let r = radius.ceil() as i32;
    let w = width as i32;
    let h = height as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            if (dx * dx + dy * dy) as f32 > radius * radius {
                continue;
            }
            let x = cx as i32 + dx;
            let y = cy as i32 + dy;
            put(data, w, h, x, y, rgba);
        }
    }
}

fn fill_rect(data: &mut [u8], width: u32, height: u32, cx: f32, cy: f32, half: i32, rgba: [u8; 4]) {
    let w = width as i32;
    let h = height as i32;
    for dy in -half..=half {
        for dx in -half..=half {
            put(data, w, h, cx as i32 + dx, cy as i32 + dy, rgba);
        }
    }
}

fn draw_glyph_dot(data: &mut [u8], width: u32, height: u32, cx: f32, cy: f32, rgba: [u8; 4]) {
    fill_disc(data, width, height, cx, cy, 4.0, rgba);
    // Dark centre so it reads as a pin rather than a terrain speck.
    fill_disc(data, width, height, cx, cy, 1.5, [20, 18, 12, 255]);
}

#[allow(clippy::too_many_arguments)]
fn fill_triangle(
    data: &mut [u8],
    width: u32,
    height: u32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    rgba: [u8; 4],
) {
    let min_x = x0.min(x1).min(x2).floor() as i32;
    let max_x = x0.max(x1).max(x2).ceil() as i32;
    let min_y = y0.min(y1).min(y2).floor() as i32;
    let max_y = y0.max(y1).max(y2).ceil() as i32;
    let w = width as i32;
    let h = height as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if point_in_triangle(x as f32 + 0.5, y as f32 + 0.5, x0, y0, x1, y1, x2, y2) {
                put(data, w, h, x, y, rgba);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn point_in_triangle(
    px: f32,
    py: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
) -> bool {
    let d1 = sign(px, py, x0, y0, x1, y1);
    let d2 = sign(px, py, x1, y1, x2, y2);
    let d3 = sign(px, py, x2, y2, x0, y0);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn sign(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    (px - x1) * (y0 - y1) - (x0 - x1) * (py - y1)
}

fn put(data: &mut [u8], w: i32, h: i32, x: i32, y: i32, rgba: [u8; 4]) {
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let k = ((y * w + x) * 4) as usize;
    // Simple over: opaque markers replace terrain.
    data[k] = rgba[0];
    data[k + 1] = rgba[1];
    data[k + 2] = rgba[2];
    data[k + 3] = rgba[3];
}

/// Collect static hub / portal markers visible in a region.
pub fn static_markers_for_region(region: MapRegion) -> Vec<MapMarker> {
    let mut out = Vec::new();
    for band in ZONES {
        if region.contains(band.hub.x, band.hub.z)
            || (band.hub.z >= region.min_z && band.hub.z <= region.max_z)
        {
            out.push(MapMarker {
                x: band.hub.x,
                z: band.hub.z,
                kind: MapMarkerKind::Hub,
                label: band.hub.name.to_string(),
            });
        }
    }
    for dungeon in woc_content::DUNGEONS {
        if region.contains(dungeon.entrance_x, dungeon.entrance_z) {
            out.push(MapMarker {
                x: dungeon.entrance_x,
                z: dungeon.entrance_z,
                kind: MapMarkerKind::Portal,
                label: dungeon.name.to_string(),
            });
        }
    }
    for delve in woc_content::DELVES {
        if region.contains(delve.entrance_x, delve.entrance_z) {
            out.push(MapMarker {
                x: delve.entrance_x,
                z: delve.entrance_z,
                kind: MapMarkerKind::Portal,
                label: delve.name.to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::WORLD_SEED;

    #[test]
    fn world_to_pixel_puts_north_at_top_and_plus_x_on_the_left() {
        let region = MapRegion {
            min_x: -10.0,
            max_x: 10.0,
            min_z: -10.0,
            max_z: 10.0,
        };
        let (px_north, py_north) = world_to_pixel(0.0, 10.0, region, 100, 100);
        // Upstream: canvas-left is +X (west), canvas-right is −X (east).
        let (px_plus_x, py_mid) = world_to_pixel(10.0, 0.0, region, 100, 100);
        let (px_minus_x, _) = world_to_pixel(-10.0, 0.0, region, 100, 100);
        assert!(py_north < 5.0, "north should be near top, got {py_north}");
        assert!(px_plus_x < 5.0, "+X should be near left, got {px_plus_x}");
        assert!(
            px_minus_x > 95.0,
            "−X should be near right, got {px_minus_x}"
        );
        assert!((py_mid - 50.0).abs() < 1.0);
        assert!((px_north - 50.0).abs() < 1.0);
    }

    #[test]
    fn pixel_roundtrip_is_stable() {
        let region = MapRegion::full_strip();
        let (x, z) = (12.0, 40.0);
        let (px, py) = world_to_pixel(x, z, region, 180, 540);
        let (rx, rz) = pixel_to_world(px, py, region, 180, 540);
        assert!((rx - x).abs() < 0.05, "x {rx} vs {x}");
        assert!((rz - z).abs() < 0.05, "z {rz} vs {z}");
    }

    #[test]
    fn paint_terrain_fills_opaque_pixels() {
        let mut data = vec![0u8; 32 * 32 * 4];
        paint_terrain_rgba(
            &mut data,
            32,
            32,
            MapRegion::from_zone_band(&woc_content::ZONE_EASTBROOK),
            WORLD_SEED,
        );
        for i in 0..(32 * 32) {
            assert_eq!(data[i * 4 + 3], 255, "pixel {i} alpha");
        }
    }

    #[test]
    fn region_for_eastbrook_matches_vale_band() {
        let r = region_for_zone("eastbrook", 0.0);
        assert_eq!(r.min_z, woc_content::ZONE_EASTBROOK.z_min);
        assert_eq!(r.max_z, woc_content::ZONE_EASTBROOK.z_max);
    }

    #[test]
    fn static_markers_include_eastbrook_hub_and_crypt() {
        let markers =
            static_markers_for_region(MapRegion::from_zone_band(&woc_content::ZONE_EASTBROOK));
        assert!(markers.iter().any(|m| m.kind == MapMarkerKind::Hub));
        assert!(markers
            .iter()
            .any(|m| m.kind == MapMarkerKind::Portal && m.label.contains("Crypt")));
    }

    #[test]
    fn paint_map_frame_draws_player_near_centre_for_around_region() {
        let mut data = vec![0u8; 64 * 64 * 4];
        let region = MapRegion::around(0.0, 0.0, 40.0);
        let markers = [MapMarker {
            x: 0.0,
            z: 0.0,
            kind: MapMarkerKind::Hub,
            label: "test".into(),
        }];
        paint_map_frame(&mut data, 64, 64, region, WORLD_SEED, &markers, false);
        let (mx, my) = world_to_pixel(0.0, 0.0, region, 64, 64);
        assert!((mx - 32.0).abs() < 1.0 && (my - 32.0).abs() < 1.0);
    }
}
