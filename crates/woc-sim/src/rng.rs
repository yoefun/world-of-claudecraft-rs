//! Deterministic mulberry32 PRNG + upstream-aligned terrain noise.

#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        let mut s = seed;
        if s == 0 {
            s = 0x9e3779b9;
        }
        Self { state: s }
    }

    /// Next u32 in [0, 2^32).
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    /// Uniform f32 in [0, 1) — matches upstream `Rng.next()`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f64 / 4294967296.0) as f32
    }

    /// Inclusive integer range.
    pub fn gen_range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if lo >= hi {
            return lo;
        }
        lo + (self.next_u32() % (hi - lo + 1))
    }
}

/// Stateless hash noise for terrain — upstream `hash2(x, y, seed)` → \[0, 1\].
#[inline]
pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    // Math.imul / >>> 0 semantics.
    let mut h = seed as i32;
    h = (h ^ x.wrapping_mul(374761393)).wrapping_mul(668265263);
    h = (h ^ y.wrapping_mul(1274126177)).wrapping_mul(461845907);
    let mut hu = h as u32;
    hu ^= hu >> 13;
    hu = hu.wrapping_mul(1274126177);
    hu ^= hu >> 16;
    (hu as f64 / 4294967296.0) as f32
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise in \[0, 1\] — upstream `noise2`.
pub fn noise2(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let a = hash2(xi, yi, seed);
    let b = hash2(xi + 1, yi, seed);
    let c = hash2(xi, yi + 1, seed);
    let d = hash2(xi + 1, yi + 1, seed);
    let u = smooth(xf);
    let v = smooth(yf);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/// Fractal noise in \[0, 1\] — upstream `fbm2(x, y, seed, octaves)`.
pub fn fbm2(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut total = 0.0;
    for i in 0..octaves {
        sum += noise2(x * freq, y * freq, seed.wrapping_add(i.wrapping_mul(1013))) * amp;
        total += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mulberry32_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn hash2_in_unit_interval() {
        for x in -5..5 {
            for y in -5..5 {
                let h = hash2(x, y, 20061);
                assert!((0.0..1.0).contains(&h), "hash2={h}");
            }
        }
    }

    #[test]
    fn fbm2_deterministic_and_bounded() {
        let a = fbm2(1.25, -3.5, 20061, 4);
        let b = fbm2(1.25, -3.5, 20061, 4);
        assert_eq!(a, b);
        assert!((0.0..=1.0).contains(&a), "fbm={a}");
    }

    #[test]
    fn known_hash2_sample_stable() {
        // Upstream seed 20061 / pin a3e5e959 — hash2(0,0).
        let h = hash2(0, 0, 20061);
        assert!(
            (h - 0.797_993_66).abs() < 1e-5,
            "hash2(0,0,20061) drifted: {h}"
        );
    }
}
