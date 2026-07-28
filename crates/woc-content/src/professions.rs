//! Profession definitions (gathering + crafting).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfessionKind {
    Gathering,
    Crafting,
}

#[derive(Debug, Clone, Copy)]
pub struct ProfessionDef {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: ProfessionKind,
    /// Cap for profession skill rank (framework slice uses a flat 1–max ladder).
    pub max_skill: u32,
}

/// Eastbrook framework professions: herbalism gathers reagents; alchemy crafts them.
pub static PROFESSIONS: &[ProfessionDef] = &[
    ProfessionDef {
        id: "herbalism",
        name: "Herbalism",
        kind: ProfessionKind::Gathering,
        max_skill: 75,
    },
    ProfessionDef {
        id: "alchemy",
        name: "Alchemy",
        kind: ProfessionKind::Crafting,
        max_skill: 75,
    },
];

pub fn profession(id: &str) -> Option<&'static ProfessionDef> {
    PROFESSIONS.iter().find(|p| p.id == id)
}
