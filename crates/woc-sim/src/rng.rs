//! Deterministic mulberry32 PRNG (compatible with common JS ports).

#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Next u32 in [0, 2^32).
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }

    /// Uniform f32 in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Inclusive integer range.
    pub fn gen_range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if lo >= hi {
            return lo;
        }
        lo + (self.next_u32() % (hi - lo + 1))
    }
}

/// Hash two coords into a u32 (for terrain).
pub fn hash2(x: i32, z: i32) -> u32 {
    let mut n = (x as i64 as u32).wrapping_mul(374761393);
    n = n.wrapping_add((z as i64 as u32).wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^ (n >> 16)
}

pub fn noise2(x: f32, z: f32) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let fx = x - x0 as f32;
    let fz = z - z0 as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sz = fz * fz * (3.0 - 2.0 * fz);
    let n00 = (hash2(x0, z0) as f32 / u32::MAX as f32) * 2.0 - 1.0;
    let n10 = (hash2(x0 + 1, z0) as f32 / u32::MAX as f32) * 2.0 - 1.0;
    let n01 = (hash2(x0, z0 + 1) as f32 / u32::MAX as f32) * 2.0 - 1.0;
    let n11 = (hash2(x0 + 1, z0 + 1) as f32 / u32::MAX as f32) * 2.0 - 1.0;
    let ix0 = n00 + (n10 - n00) * sx;
    let ix1 = n01 + (n11 - n01) * sx;
    ix0 + (ix1 - ix0) * sz
}

pub fn fbm2(x: f32, z: f32, octaves: u32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..octaves {
        sum += noise2(x * freq, z * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
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
}
