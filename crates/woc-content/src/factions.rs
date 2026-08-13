//! Hub factions and standing ranks.
//!
//! Values are i32, Neutral at 0. Missing rows read as Neutral 0.
//! Ladder is compressed so Eastbrook's quest loop can reach Friendly.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Standing {
    Hated,
    Hostile,
    Unfriendly,
    Neutral,
    Friendly,
    Honored,
    Revered,
    Exalted,
}

impl Standing {
    pub const fn as_str(self) -> &'static str {
        match self {
            Standing::Hated => "hated",
            Standing::Hostile => "hostile",
            Standing::Unfriendly => "unfriendly",
            Standing::Neutral => "neutral",
            Standing::Friendly => "friendly",
            Standing::Honored => "honored",
            Standing::Revered => "revered",
            Standing::Exalted => "exalted",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Standing::Hated => "Hated",
            Standing::Hostile => "Hostile",
            Standing::Unfriendly => "Unfriendly",
            Standing::Neutral => "Neutral",
            Standing::Friendly => "Friendly",
            Standing::Honored => "Honored",
            Standing::Revered => "Revered",
            Standing::Exalted => "Exalted",
        }
    }
}

/// Bottom of Hated / absolute floor.
pub const STANDING_FLOOR: i32 = -4200;
pub const HOSTILE_AT: i32 = -3000;
pub const UNFRIENDLY_AT: i32 = -1500;
pub const NEUTRAL_AT: i32 = 0;
pub const FRIENDLY_AT: i32 = 500;
pub const HONORED_AT: i32 = 1500;
pub const REVERED_AT: i32 = 3000;
pub const EXALTED_AT: i32 = 6000;
/// Inclusive cap (Exalted fills 6000..=6299).
pub const STANDING_CAP: i32 = 6299;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepAward {
    pub faction_id: &'static str,
    pub amount: i32,
}

impl RepAward {
    pub const fn new(faction_id: &'static str, amount: i32) -> Self {
        Self { faction_id, amount }
    }
}

#[derive(Debug, Clone)]
pub struct FactionDef {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
}

pub const FACTION_EASTBROOK_WATCH: &str = "eastbrook_watch";
pub const FACTION_EASTFEN_CIRCLE: &str = "eastfen_circle";
pub const FACTION_MIREFEN_FERRY: &str = "mirefen_ferry";
pub const FACTION_HIGHWATCH: &str = "highwatch";

pub static FACTIONS: &[FactionDef] = &[
    FactionDef {
        id: FACTION_EASTBROOK_WATCH,
        name: "Eastbrook Watch",
        blurb: "Captain Alden's garrison on the Vale road.",
    },
    FactionDef {
        id: FACTION_EASTFEN_CIRCLE,
        name: "Eastfen Circle",
        blurb: "Warden Selene's boardwalk wardens.",
    },
    FactionDef {
        id: FACTION_MIREFEN_FERRY,
        name: "Mirefen Ferry",
        blurb: "Orla's lantern camp and Noll's skiff.",
    },
    FactionDef {
        id: FACTION_HIGHWATCH,
        name: "Highwatch",
        blurb: "Commander Elara's hold on the Thornpeak pass.",
    },
];

pub fn faction(id: &str) -> Option<&'static FactionDef> {
    FACTIONS.iter().find(|f| f.id == id)
}

pub fn clamp_reputation(value: i32) -> i32 {
    value.clamp(STANDING_FLOOR, STANDING_CAP)
}

pub fn standing_from_value(value: i32) -> Standing {
    if value >= EXALTED_AT {
        Standing::Exalted
    } else if value >= REVERED_AT {
        Standing::Revered
    } else if value >= HONORED_AT {
        Standing::Honored
    } else if value >= FRIENDLY_AT {
        Standing::Friendly
    } else if value >= NEUTRAL_AT {
        Standing::Neutral
    } else if value >= UNFRIENDLY_AT {
        Standing::Unfriendly
    } else if value >= HOSTILE_AT {
        Standing::Hostile
    } else {
        Standing::Hated
    }
}

/// Inclusive bottom of `standing`.
pub fn standing_at(standing: Standing) -> i32 {
    match standing {
        Standing::Hated => STANDING_FLOOR,
        Standing::Hostile => HOSTILE_AT,
        Standing::Unfriendly => UNFRIENDLY_AT,
        Standing::Neutral => NEUTRAL_AT,
        Standing::Friendly => FRIENDLY_AT,
        Standing::Honored => HONORED_AT,
        Standing::Revered => REVERED_AT,
        Standing::Exalted => EXALTED_AT,
    }
}

/// Exclusive top of `standing`, except Exalted which is inclusive cap + 1.
pub fn standing_next(standing: Standing) -> i32 {
    match standing {
        Standing::Hated => HOSTILE_AT,
        Standing::Hostile => UNFRIENDLY_AT,
        Standing::Unfriendly => NEUTRAL_AT,
        Standing::Neutral => FRIENDLY_AT,
        Standing::Friendly => HONORED_AT,
        Standing::Honored => REVERED_AT,
        Standing::Revered => EXALTED_AT,
        Standing::Exalted => STANDING_CAP.saturating_add(1),
    }
}

pub fn vendor_discount_pct(standing: Standing) -> u32 {
    match standing {
        Standing::Exalted => 20,
        Standing::Revered => 15,
        Standing::Honored => 10,
        Standing::Friendly => 5,
        _ => 0,
    }
}

/// Ceiling of `base * (100 - pct) / 100`. Never free unless `base` is 0.
pub fn discounted_price(base: u32, standing: Standing) -> u32 {
    let pct = vendor_discount_pct(standing);
    if pct == 0 {
        return base;
    }
    let n = (base as u64) * (100 - pct as u64);
    ((n + 99) / 100) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_ladder_is_ordered() {
        assert_eq!(standing_from_value(0), Standing::Neutral);
        assert_eq!(standing_from_value(499), Standing::Neutral);
        assert_eq!(standing_from_value(500), Standing::Friendly);
        assert_eq!(standing_from_value(1500), Standing::Honored);
        assert_eq!(standing_from_value(3000), Standing::Revered);
        assert_eq!(standing_from_value(6000), Standing::Exalted);
        assert_eq!(standing_from_value(-1), Standing::Unfriendly);
        assert_eq!(standing_from_value(-1500), Standing::Unfriendly);
        assert_eq!(standing_from_value(-1501), Standing::Hostile);
        assert_eq!(standing_from_value(-3000), Standing::Hostile);
        assert_eq!(standing_from_value(-3001), Standing::Hated);
        assert!(Standing::Neutral < Standing::Friendly);
        assert!(Standing::Unfriendly < Standing::Neutral);
    }

    #[test]
    fn clamp_hits_floor_and_cap() {
        assert_eq!(clamp_reputation(i32::MIN), STANDING_FLOOR);
        assert_eq!(clamp_reputation(i32::MAX), STANDING_CAP);
        assert_eq!(clamp_reputation(0), 0);
    }

    #[test]
    fn discount_never_zeros_a_positive_price() {
        assert_eq!(discounted_price(20, Standing::Neutral), 20);
        assert_eq!(discounted_price(20, Standing::Friendly), 19);
        assert_eq!(discounted_price(20, Standing::Exalted), 16);
        assert_eq!(discounted_price(1, Standing::Exalted), 1);
        assert_eq!(discounted_price(0, Standing::Exalted), 0);
    }

    #[test]
    fn four_hub_factions_exist() {
        assert_eq!(FACTIONS.len(), 4);
        assert!(faction(FACTION_EASTBROOK_WATCH).is_some());
        assert!(faction(FACTION_EASTFEN_CIRCLE).is_some());
        assert!(faction(FACTION_MIREFEN_FERRY).is_some());
        assert!(faction(FACTION_HIGHWATCH).is_some());
        assert!(faction("missing").is_none());
    }
}
