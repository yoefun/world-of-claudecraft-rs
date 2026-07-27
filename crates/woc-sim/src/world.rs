//! Procedural heightfield (Eastbrook-like open vale; not byte-identical to upstream).

use crate::rng::fbm2;

/// World horizontal extent in yards (centered at origin).
pub const WORLD_HALF: f32 = 120.0;

/// Fixed world seed for the combat slice (matches offline play stability).
pub const WORLD_SEED: u32 = 0xC1A0_DEC0;

pub fn terrain_height(x: f32, z: f32, seed: u32) -> f32 {
    let sx = x * 0.02 + (seed as f32 * 0.0001);
    let sz = z * 0.02 + (seed as f32 * 0.00013);
    let base = fbm2(sx, sz, 4) * 4.5;
    let gentle = fbm2(sx * 0.35, sz * 0.35, 2) * 2.0;
    // Slight bowl so the town flat sits near origin.
    let bowl = ((x * x + z * z).sqrt() / WORLD_HALF).clamp(0.0, 1.0);
    base + gentle + bowl * 1.5
}

pub fn clamp_to_world(x: f32, z: f32) -> (f32, f32) {
    (
        x.clamp(-WORLD_HALF, WORLD_HALF),
        z.clamp(-WORLD_HALF, WORLD_HALF),
    )
}
