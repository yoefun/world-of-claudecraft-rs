pub trait Rng {
    fn next_u32(&mut self) -> u32;

    fn chance(&mut self, percent: u8) -> bool {
        debug_assert!(percent <= 100);
        (self.next_u32() % 100) < u32::from(percent)
    }
}

pub struct ScriptedRng {
    seq: std::vec::IntoIter<u32>,
}

impl ScriptedRng {
    #[allow(clippy::unnecessary_to_owned)]
    pub fn from_seq(seq: &[u32]) -> Self {
        Self {
            seq: seq.to_vec().into_iter(),
        }
    }
}

impl Rng for ScriptedRng {
    fn next_u32(&mut self) -> u32 {
        self.seq
            .next()
            .expect("ScriptedRng exhausted; harvest/craft drew more than the test scripted")
    }
}

pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }
}

impl Rng for XorShift64 {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_chance_consumes_one_draw() {
        let mut rng = ScriptedRng::from_seq(&[3, 50]);
        assert!(rng.chance(5));
        assert!(!rng.chance(50));
    }

    #[test]
    fn xorshift_is_deterministic() {
        let mut a = XorShift64::new(42);
        let mut b = XorShift64::new(42);
        let seq_a: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let seq_b: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        assert_eq!(seq_a, seq_b);
    }
}
