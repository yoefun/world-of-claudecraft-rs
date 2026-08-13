//! Data-driven ability effects and aura tables.
//!
//! Combat dispatches on [`AbilityEffect`]. DoTs/HoTs are keyed by ability id
//! via [`aura_for_ability`] — `woc-sim` must not match on ability id strings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageSchool {
    Physical,
    Fire,
    Nature,
    Shadow,
    Holy,
    Arcane,
    Frost,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbilityEffect {
    WeaponDamage { coefficient: f32 },
    SpellDamage { school: DamageSchool },
    Heal { coefficient: f32 },
    AoeDamage { radius: f32, max_targets: u32 },
    ApplyAura,
    Interrupt,
    Taunt { threat: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuraDef {
    pub id: &'static str,
    pub duration: f32,
    pub tick_interval: f32,
    pub tick_damage: f32,
    pub tick_heal: f32,
}

const REND: AuraDef = AuraDef {
    id: "rend",
    duration: 9.0,
    tick_interval: 3.0,
    tick_damage: 4.0,
    tick_heal: 0.0,
};

const IGNITE: AuraDef = AuraDef {
    id: "ignite",
    duration: 8.0,
    tick_interval: 2.0,
    tick_damage: 6.0,
    tick_heal: 0.0,
};

const STING: AuraDef = AuraDef {
    id: "sting",
    duration: 12.0,
    tick_interval: 3.0,
    tick_damage: 5.0,
    tick_heal: 0.0,
};

/// Aura applied when the named ability lands (in addition to its [`AbilityEffect`]).
pub fn aura_for_ability(ability_id: &str) -> Option<&'static AuraDef> {
    match ability_id {
        "heroic_strike" | "cleave" | "crusader_strike" | "sinister_strike" | "eviscerate" => {
            Some(&REND)
        }
        "fireball" | "incinerate" | "lava_burst" => Some(&IGNITE),
        "serpent_sting" | "corruption" | "moonfire" | "holy_fire" => Some(&STING),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dot_abilities_resolve_auras() {
        assert_eq!(aura_for_ability("heroic_strike").unwrap().id, "rend");
        assert_eq!(aura_for_ability("fireball").unwrap().id, "ignite");
        assert_eq!(aura_for_ability("serpent_sting").unwrap().id, "sting");
        assert!(aura_for_ability("flash_heal").is_none());
        assert!(aura_for_ability("taunt").is_none());
    }
}
