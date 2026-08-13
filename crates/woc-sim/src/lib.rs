pub const TICK_HZ: u32 = 20;

pub mod content;
pub mod gold;
pub mod inventory;
pub mod item;
pub mod rng;

pub fn ticks_from_seconds(seconds: f32) -> u32 {
    (seconds * TICK_HZ as f32).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_rate_is_twenty_hertz() {
        assert_eq!(TICK_HZ, 20);
        assert_eq!(ticks_from_seconds(1.5), 30);
        assert_eq!(ticks_from_seconds(1.51), 31);
    }
}
