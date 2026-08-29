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
    /// Cap for profession skill rank.
    pub max_skill: u32,
}

/// Ten v1 professions. Forging is the live id `blacksmithing`.
pub static PROFESSIONS: &[ProfessionDef] = &[
    ProfessionDef {
        id: "mining",
        name: "Mining",
        kind: ProfessionKind::Gathering,
        max_skill: 100,
    },
    ProfessionDef {
        id: "herbalism",
        name: "Herbalism",
        kind: ProfessionKind::Gathering,
        max_skill: 100,
    },
    ProfessionDef {
        id: "skinning",
        name: "Skinning",
        kind: ProfessionKind::Gathering,
        max_skill: 100,
    },
    ProfessionDef {
        id: "blacksmithing",
        name: "Blacksmithing",
        kind: ProfessionKind::Crafting,
        max_skill: 125,
    },
    ProfessionDef {
        id: "leatherworking",
        name: "Leatherworking",
        kind: ProfessionKind::Crafting,
        max_skill: 125,
    },
    ProfessionDef {
        id: "tailoring",
        name: "Tailoring",
        kind: ProfessionKind::Crafting,
        max_skill: 125,
    },
    ProfessionDef {
        id: "jewelcrafting",
        name: "Jewelcrafting",
        kind: ProfessionKind::Crafting,
        max_skill: 125,
    },
    ProfessionDef {
        id: "enchanting",
        name: "Enchanting",
        kind: ProfessionKind::Crafting,
        max_skill: 125,
    },
    ProfessionDef {
        id: "engineering",
        name: "Engineering",
        kind: ProfessionKind::Crafting,
        max_skill: 125,
    },
    ProfessionDef {
        id: "alchemy",
        name: "Alchemy",
        kind: ProfessionKind::Crafting,
        max_skill: 125,
    },
];

pub fn profession(id: &str) -> Option<&'static ProfessionDef> {
    PROFESSIONS.iter().find(|p| p.id == id)
}

/// Wolf / boar templates that can be skinned after the kill loot spawns.
pub fn mob_is_skinnable(template_id: &str) -> bool {
    matches!(
        template_id,
        "young_wolf" | "scarred_wolf" | "young_boar" | "cragback_boar"
    )
}

pub fn gathering_tool_item(profession_id: &str) -> Option<&'static str> {
    match profession_id {
        "mining" => Some("copper_pick"),
        "herbalism" => Some("copper_sickle"),
        "skinning" => Some("skinning_knife"),
        _ => None,
    }
}
