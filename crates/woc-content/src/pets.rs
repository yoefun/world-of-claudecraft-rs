//! Class pet definitions (hunter companion / warlock demon).

use crate::PlayerClass;

#[derive(Debug, Clone)]
pub struct PetDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Only this class may summon this pet.
    pub owner_class: PlayerClass,
    pub hp: f32,
    pub attack_damage: f32,
    pub level: u32,
    /// Optional extra hit (`wolf_bite` / `imp_firebolt`); `None` = white swings only.
    pub ability_id: Option<&'static str>,
}

pub static PETS: &[PetDef] = &[
    PetDef {
        id: "hunter_wolf",
        name: "Wolf Companion",
        owner_class: PlayerClass::Hunter,
        hp: 60.0,
        attack_damage: 8.0,
        level: 1,
        ability_id: Some("wolf_bite"),
    },
    PetDef {
        id: "warlock_imp",
        name: "Imp",
        owner_class: PlayerClass::Warlock,
        hp: 40.0,
        attack_damage: 10.0,
        level: 1,
        ability_id: Some("imp_firebolt"),
    },
];

pub fn pet(id: &str) -> Option<&'static PetDef> {
    PETS.iter().find(|p| p.id == id)
}

/// Default pet for a summoning class, if any.
pub fn pet_for_class(class: PlayerClass) -> Option<&'static PetDef> {
    PETS.iter().find(|p| p.owner_class == class)
}
