pub const TIER_SKILL_STEP: u32 = 25;

pub fn tier_for_skill(skill: u32) -> u8 {
    (skill / TIER_SKILL_STEP).min(5) as u8
}

pub fn skill_gain_amount(current: u32, req: u32, cap: u32) -> u32 {
    if current >= cap {
        return 0;
    }
    let delta = current.saturating_sub(req);
    match delta {
        0..=24 => 2,
        25..=74 => 1,
        _ => 0,
    }
}

pub fn gain_skill(current: u32, req: u32, cap: u32) -> u32 {
    if current >= cap {
        return 0;
    }
    let mut amount = skill_gain_amount(current, req, cap);
    if amount == 0 && current == cap.saturating_sub(1) {
        amount = 1;
    }
    let new_value = (current + amount).min(cap);
    new_value.saturating_sub(current)
}

pub fn masterwork_proc_chance(player_skill: u32, recipe_req: u32) -> u8 {
    let player_tier = tier_for_skill(player_skill);
    let recipe_tier = tier_for_skill(recipe_req);
    let mut chance = 3u8;
    if player_tier > recipe_tier {
        chance = chance.saturating_add(player_tier - recipe_tier);
    }
    chance.min(15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_actions_grant_zero() {
        assert_eq!(skill_gain_amount(80, 0, 100), 0);
        assert_eq!(skill_gain_amount(30, 0, 100), 1);
        assert_eq!(skill_gain_amount(10, 0, 100), 2);
    }

    #[test]
    fn masterwork_base_chance() {
        assert_eq!(masterwork_proc_chance(0, 0), 3);
        assert!(masterwork_proc_chance(125, 0) <= 15);
    }
}
