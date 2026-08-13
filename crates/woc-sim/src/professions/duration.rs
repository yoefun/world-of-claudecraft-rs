use woc_protocol::TICK_RATE;

pub fn ticks_from_seconds(seconds: f32) -> u32 {
    (seconds * TICK_RATE as f32).ceil() as u32
}

pub fn clamp_cast_seconds(seconds: f32) -> f32 {
    seconds.clamp(1.5, 5.0)
}

pub fn craft_cast_seconds(skill_req: u32) -> f32 {
    let raw = match skill_req {
        0 => 1.75,
        1..=25 => 2.5,
        26..=50 => 3.0,
        51..=75 => 3.5,
        _ => 4.0,
    };
    clamp_cast_seconds(raw)
}

pub fn gather_cast_seconds(tool_tiers_above: u8, proficiency_bands_above: u8) -> f32 {
    let raw = 2.5 - 0.4 * f32::from(tool_tiers_above) - 0.15 * f32::from(proficiency_bands_above);
    clamp_cast_seconds(raw)
}

pub fn enchant_family_seconds() -> f32 {
    1.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_stay_inside_ux_band() {
        assert_eq!(craft_cast_seconds(0), 1.75);
        assert_eq!(craft_cast_seconds(100), 4.0);
        assert_eq!(gather_cast_seconds(0, 0), 2.5);
        assert_eq!(gather_cast_seconds(4, 4), 1.5);
        assert_eq!(enchant_family_seconds(), 1.5);
        assert_eq!(ticks_from_seconds(1.5), 30);
    }
}
