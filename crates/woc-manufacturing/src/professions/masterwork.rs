use crate::item::Quality;

pub fn masterwork_proc_chance(player_skill: u16, recipe_req: u16) -> u8 {
    let player_tier = super::skill::tier_for_skill(player_skill);
    let recipe_tier = super::skill::tier_for_skill(recipe_req);
    let mut chance = 3u8;
    if player_tier > recipe_tier {
        chance = chance.saturating_add(player_tier - recipe_tier);
    }
    chance.min(15)
}

pub fn bump_quality(q: Quality) -> Quality {
    match q {
        Quality::Common => Quality::Uncommon,
        Quality::Uncommon => Quality::Rare,
        Quality::Rare => Quality::Epic,
        Quality::Epic => Quality::Epic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_chance_at_equal_tier() {
        assert_eq!(masterwork_proc_chance(0, 0), 3);
    }

    #[test]
    fn higher_skill_adds_one_per_tier_above_recipe() {
        assert_eq!(masterwork_proc_chance(125, 0), 8);
    }

    #[test]
    fn chance_never_exceeds_fifteen() {
        for skill in [0u16, 25, 50, 75, 100, 125, 200] {
            assert!(masterwork_proc_chance(skill, 0) <= 15);
        }
        assert_eq!(masterwork_proc_chance(125, 0), 8);
    }

    #[test]
    fn bump_quality_steps_up_one_tier() {
        assert_eq!(bump_quality(Quality::Common), Quality::Uncommon);
        assert_eq!(bump_quality(Quality::Epic), Quality::Epic);
    }
}
