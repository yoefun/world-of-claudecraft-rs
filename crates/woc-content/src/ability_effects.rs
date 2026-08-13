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
    /// Apply an absorb shield to the heal-target (self / friendly).
    Absorb {
        amount: f32,
    },
    /// Close to melee if `dist` is in `(MELEE, gap]`, then weapon hit.
    Charge {
        gap: f32,
    },
    /// Offset the caster along facing, then clamp to walkable ground.
    Blink {
        distance: f32,
    },
    /// Spend HP for resource (Life Tap). Leaves at least 1 HP if alive.
    Convert {
        hp_cost: f32,
        resource_gain: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityFlags {
    pub requires_stealth: bool,
    /// Cleared after the ability starts. Default true.
    pub breaks_stealth: bool,
    pub combo_add: u8,
    pub combo_spend: bool,
    pub combo_per_point: f32,
    pub self_aoe: bool,
    pub interrupt_lockout: f32,
    pub rage_dump: bool,
}

impl AbilityFlags {
    pub const DEFAULT: Self = Self {
        requires_stealth: false,
        breaks_stealth: true,
        combo_add: 0,
        combo_spend: false,
        combo_per_point: 0.0,
        self_aoe: false,
        interrupt_lockout: 0.0,
        rage_dump: false,
    };

    pub const fn combo_builder(self, points: u8) -> Self {
        let mut s = self;
        s.combo_add = points;
        s
    }

    pub const fn combo_finisher(self, per_point: f32) -> Self {
        let mut s = self;
        s.combo_spend = true;
        s.combo_per_point = per_point;
        s
    }

    pub const fn stealth_opener(self) -> Self {
        let mut s = self;
        s.requires_stealth = true;
        s
    }

    pub const fn lockout(self, seconds: f32) -> Self {
        let mut s = self;
        s.interrupt_lockout = seconds;
        s
    }

    pub const fn dump_rage(self) -> Self {
        let mut s = self;
        s.rage_dump = true;
        s
    }

    pub const fn around_self(self) -> Self {
        let mut s = self;
        s.self_aoe = true;
        s
    }
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
    pub absorb: f32,
    pub breaks_on_damage: bool,
    /// Outgoing damage multiplier while the aura remains (`1.0` = none).
    pub damage_mult: f32,
    /// Damage dealt back to a melee attacker of the bearer.
    pub thorns: f32,
    /// Extra armor while the aura remains (added in `deal_damage`).
    pub armor_flat: f32,
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
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
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
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
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
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
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
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
    }
}

const fn absorb(id: &'static str, duration: f32, amount: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult: 1.0,
        absorb: amount,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
    }
}

const fn buff(id: &'static str, duration: f32, damage_mult: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult: 1.0,
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult,
        thorns: 0.0,
        armor_flat: 0.0,
    }
}

const fn form(id: &'static str, duration: f32, move_mult: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult,
        absorb: 0.0,
        breaks_on_damage: true,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
    }
}

const fn fear(id: &'static str, duration: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: true,
        move_mult: 0.0,
        absorb: 0.0,
        breaks_on_damage: true,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat: 0.0,
    }
}

const fn thorns(id: &'static str, duration: f32, thorns: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult: 1.0,
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns,
        armor_flat: 0.0,
    }
}

const fn armor_aura(id: &'static str, duration: f32, armor_flat: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult: 1.0,
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult: 1.0,
        thorns: 0.0,
        armor_flat,
    }
}

const fn stance(id: &'static str, duration: f32, damage_mult: f32, armor_flat: f32) -> AuraDef {
    AuraDef {
        id,
        duration,
        tick_interval: 0.0,
        tick_damage: 0.0,
        tick_heal: 0.0,
        stun: false,
        move_mult: 1.0,
        absorb: 0.0,
        breaks_on_damage: false,
        damage_mult,
        thorns: 0.0,
        armor_flat,
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
    absorb("power_word_shield", 15.0, 45.0),
    buff("battle_shout", 120.0, 1.1),
    buff("aspect_of_the_hawk", 120.0, 1.1),
    dot("seal_righteousness", 9.0, 3.0, 3.0),
    thorns("lightning_shield", 120.0, 8.0),
    fear("fear", 4.0),
    form("ghost_wolf", 120.0, 1.4),
    form("travel_form", 120.0, 1.4),
    armor_aura("devotion_aura", 3600.0, 20.0),
    stance("defensive_stance", 3600.0, 0.9, 20.0),
];

pub fn aura(id: &str) -> Option<&'static AuraDef> {
    AURAS.iter().find(|a| a.id == id)
}

impl AuraDef {
    pub fn is_hot(self) -> bool {
        self.tick_heal > 0.0 && self.tick_damage <= 0.0
    }

    pub fn is_self_buff(self) -> bool {
        self.tick_damage <= 0.0 && !self.stun && self.move_mult >= 1.0 && self.absorb <= 0.0
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
        assert!(aura("power_word_shield").unwrap().absorb > 0.0);
        assert!((aura("aspect_of_the_hawk").unwrap().damage_mult - 1.1).abs() < f32::EPSILON);
        assert!(aura("missing").is_none());
        let shield = aura("lightning_shield").expect("lightning_shield");
        assert!(shield.thorns > 0.0);
        assert!(aura("fear").expect("fear").breaks_on_damage);
        let travel = aura("travel_form").expect("travel_form");
        assert!((travel.move_mult - 1.4).abs() < f32::EPSILON);
        assert!(travel.breaks_on_damage);
        let wolf = aura("ghost_wolf").expect("ghost_wolf");
        assert!((wolf.move_mult - 1.4).abs() < f32::EPSILON);
        assert!(wolf.breaks_on_damage);
        assert!(aura("devotion_aura").expect("devotion_aura").armor_flat > 0.0);
        let defensive = aura("defensive_stance").expect("defensive_stance");
        assert!((defensive.damage_mult - 0.9).abs() < f32::EPSILON);
        assert!(defensive.armor_flat > 0.0);
        assert!(aura("seal_righteousness").is_some());
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
            assert!(a.move_mult >= 0.0, "{}", a.id);
        }
    }
}
