use super::types::{ProfessionId, TIER_SKILL_STEP};

#[derive(Clone, Debug, Default)]
pub struct ProfessionSkills {
    values: [u16; 10],
}

impl ProfessionSkills {
    fn index(id: ProfessionId) -> usize {
        match id {
            ProfessionId::Mining => 0,
            ProfessionId::Herbalism => 1,
            ProfessionId::Skinning => 2,
            ProfessionId::Forging => 3,
            ProfessionId::Leatherworking => 4,
            ProfessionId::Tailoring => 5,
            ProfessionId::Jewelcrafting => 6,
            ProfessionId::Enchanting => 7,
            ProfessionId::Engineering => 8,
            ProfessionId::Alchemy => 9,
        }
    }

    pub fn get(&self, id: ProfessionId) -> u16 {
        self.values[Self::index(id)]
    }

    pub fn gain(&mut self, id: ProfessionId, skill_req: u16) -> u16 {
        let current = self.get(id);
        let cap = id.max_skill();
        if current >= cap {
            return 0;
        }
        let mut amount = skill_gain_amount(current, skill_req, cap);
        if amount == 0 && current == cap.saturating_sub(1) {
            amount = 1;
        }
        let new_value = (current + amount).min(cap);
        let actual_gain = new_value.saturating_sub(current);
        self.values[Self::index(id)] = new_value;
        actual_gain
    }
}

pub fn tier_for_skill(skill: u16) -> u8 {
    (skill / TIER_SKILL_STEP).min(5) as u8
}

pub fn skill_gain_amount(current: u16, req: u16, cap: u16) -> u16 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gathering_and_crafting_caps_differ() {
        assert_eq!(ProfessionId::Mining.max_skill(), 100);
        assert_eq!(ProfessionId::Forging.max_skill(), 125);
    }

    #[test]
    fn skills_are_independent_and_stop_at_cap() {
        let mut skills = ProfessionSkills::default();
        assert_eq!(skills.gain(ProfessionId::Mining, 0), 2);
        assert_eq!(skills.get(ProfessionId::Herbalism), 0);
        skills.values[0] = 99;
        assert_eq!(skills.gain(ProfessionId::Mining, 0), 1);
        assert_eq!(skills.gain(ProfessionId::Mining, 0), 0);
        assert_eq!(skills.get(ProfessionId::Mining), 100);
    }

    #[test]
    fn gray_actions_grant_zero() {
        assert_eq!(skill_gain_amount(80, 0, 100), 0);
        assert_eq!(skill_gain_amount(30, 0, 100), 1);
        assert_eq!(skill_gain_amount(10, 0, 100), 2);
    }
}
