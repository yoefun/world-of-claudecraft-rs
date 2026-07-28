//! Golden harness: upstream pin a3e5e959 seed 20061 height/noise samples.
//!
//! Vectors exported from TypeScript `src/sim/world.ts` + `rng.ts`.
//! ε ≈ 1e-3 covers f32 vs JS number drift without hiding structural bugs.

use serde::Deserialize;
use woc_sim::{
    fbm2, ground_height, hash2, noise2, terrain_height, terrain_steepness,
};

#[derive(Debug, Deserialize)]
struct Golden {
    seed: u32,
    epsilon: f64,
    noise: Vec<NoiseSample>,
    heights: Vec<HeightSample>,
}

#[derive(Debug, Deserialize)]
struct NoiseSample {
    x: i32,
    y: i32,
    hash2: f64,
    noise2: f64,
    fbm2: f64,
}

#[derive(Debug, Deserialize)]
struct HeightSample {
    x: f64,
    z: f64,
    terrain_height: f64,
    ground_height: f64,
    steepness: f64,
}

fn load_golden() -> Golden {
    serde_json::from_str(include_str!("data/terrain_golden.json")).expect("parse golden")
}

fn assert_close(label: &str, got: f32, want: f64, eps: f64) {
    let g = got as f64;
    assert!(
        (g - want).abs() <= eps,
        "{label}: got {g} want {want} (eps={eps})"
    );
}

#[test]
fn golden_noise_matches_upstream() {
    let g = load_golden();
    assert_eq!(g.seed, 20061);
    for s in &g.noise {
        assert_close(
            &format!("hash2({},{})", s.x, s.y),
            hash2(s.x, s.y, g.seed),
            s.hash2,
            g.epsilon,
        );
        assert_close(
            &format!("noise2({},{})", s.x, s.y),
            noise2(s.x as f32 + 0.25, s.y as f32 - 0.5, g.seed),
            s.noise2,
            g.epsilon,
        );
        assert_close(
            &format!("fbm2({},{})", s.x, s.y),
            fbm2(s.x as f32 * 0.013, s.y as f32 * 0.013, g.seed, 4),
            s.fbm2,
            g.epsilon,
        );
    }
}

#[test]
fn golden_heights_match_upstream() {
    let g = load_golden();
    for s in &g.heights {
        let x = s.x as f32;
        let z = s.z as f32;
        assert_close(
            &format!("terrain_height({x},{z})"),
            terrain_height(x, z, g.seed),
            s.terrain_height,
            g.epsilon,
        );
        assert_close(
            &format!("ground_height({x},{z})"),
            ground_height(x, z, g.seed),
            s.ground_height,
            g.epsilon,
        );
        assert_close(
            &format!("steepness({x},{z})"),
            terrain_steepness(x, z, g.seed),
            s.steepness,
            g.epsilon,
        );
    }
}
