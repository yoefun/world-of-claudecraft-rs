//! Data-driven ability effects and aura tables.
//!
//! Combat dispatches on [`AbilityEffect`]. DoTs, HoTs, and crowd-control auras
//! are declared on [`crate::abilities::AbilityDef::aura`] and resolved through
//! [`crate::abilities::aura_for_ability`] — `woc-sim` must not match on ability
//! id strings.

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
    WeaponDamage {
        coefficient: f32,
    },
    SpellDamage {
        school: DamageSchool,
    },
    Heal {
        coefficient: f32,
    },
    /// Holy Shock-style: heal a friendly (or self), otherwise damage a foe.
    HealOrHarm {
        coefficient: f32,
    },
    AoeDamage {
        radius: f32,
        max_targets: u32,
    },
    ApplyAura,
    Interrupt,
    Taunt {
        threat: f32,
    },
    /// Weapon hit that only lands when the target is at or below `hp_pct`.
    Execute {
        hp_pct: f32,
        coefficient: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuraDef {
    pub id: &'static str,
    pub duration: f32,
    pub tick_interval: f32,
    pub tick_damage: f32,
    pub tick_heal: f32,
    /// When true, the bearer cannot act or move.
    pub stun: bool,
    /// Horizontal speed multiplier while the aura remains (`1.0` = none).
    pub move_mult: f32,
}

const fn dot(id: &'static str, duration: f32, tick_interval: f32, tick_damage: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval,
        tick_damage,
        tick_heal: 0.0,
        stun: false,
        move_mult: 1.0,
    }
}

const fn hot(id: &'static str, duration: f32, tick_interval: f32, tick_heal: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval,
        tick_damage: 0.0,
        tick_heal,
        stun: false,
        move_mult: 1.0,
    }
}

const fn slow(id: &'static str, duration: f32, move_mult: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult,
    }
}

const fn stun(id: &'static str, duration: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: true,
        move_mult: 0.0,
    }
}

pub static AURAS: &[AuraDef] = &[
    dot("rend", 9.0, 3.0, 4.0),
    dot("ignite", 8.0, 2.0, 6.0),
    dot("serpent_sting", 12.0, 3.0, 5.0),
    dot("corruption", 12.0, 3.0, 6.0),
    dot("moonfire", 12.0, 3.0, 5.0),
    dot("holy_fire", 9.0, 3.0, 5.0),
    dot("immolate", 12.0, 3.0, 6.0),
    dot("flame_shock", 12.0, 3.0, 5.0),
    dot("shadow_word_pain", 12.0, 3.0, 5.0),
    hot("rejuvenation", 12.0, 3.0, 6.0),
    slow("chill", 6.0, 0.5),
    stun("cheap_shot", 2.0),
    stun("hammer_of_justice", 3.0),
];

pub fn aura(id: &str) -> Option<&'static AuraDef> {
    AURAS.iter().find(|a| a.id == id)
}

impl AuraDef {
    pub fn is_hot(self) -> bool {
        self.tick_heal > 0.0 && self.tick_damage <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aura_table_resolves_named_defs() {
        assert_eq!(aura("rend").unwrap().tick_damage, 4.0);
        assert!(aura("chill").unwrap().move_mult < 1.0);
        assert!(aura("cheap_shot").unwrap().stun);
        assert!(aura("rejuvenation").unwrap().is_hot());
        assert!(aura("missing").is_none());
    }

    #[test]
    fn aura_ids_are_unique() {
        for a in AURAS {
            assert!(
                AURAS.iter().filter(|b| b.id == a.id).count() == 1,
                "duplicate aura id {}",
                a.id
            );
            assert!(a.duration > 0.0, "{}", a.id);
            assert!(a.move_mult >= 0.0 && a.move_mult <= 1.0, "{}", a.id);
        }
    }
}
