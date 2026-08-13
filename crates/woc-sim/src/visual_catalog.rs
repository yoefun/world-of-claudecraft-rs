//! Procedural visual catalog for Bevy (and tests).
//!
//! Maps sim identities (`EntityKind` + `template_id`) onto a presentation key and
//! a small geometry/color recipe. No Bevy types — the client builds meshes from
//! these specs. Inspired by upstream `src/render/characters/manifest.ts` dispatch,
//! but keeps the rewrite's primitive-geometry path (GLB assets stay out of scope).

use woc_protocol::EntityKind;

/// High-level silhouette family used by the client mesh builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualFamily {
    /// Biped capsule + head (+ optional hat / weapon cue).
    Humanoid,
    /// Low quadruped (wolf / stalker).
    Wolf,
    /// Stocky quadruped (boar).
    Boar,
    /// Low crawler (spider / leech).
    Crawler,
    /// Rounded amphibian (toad).
    Toad,
    /// Floating orb (wisp).
    Wisp,
    /// Tall shambler / boss bulk.
    Shambler,
    /// Winged biped (harpy).
    Harpy,
    /// Small demon (imp).
    Imp,
    /// Loot sparkle.
    Loot,
    /// Fallback box.
    Cuboid,
}

/// One child mesh relative to the entity pivot (feet).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualPart {
    pub shape: PartShape,
    /// Local translation of the part center.
    pub offset: [f32; 3],
    /// Full extents (or diameter/height for capsules/spheres — see shape).
    pub size: [f32; 3],
    pub color: [f32; 3],
    /// Animation / gait role (procedural walk cycle).
    pub role: PartRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartShape {
    /// `size = [half_x*2, half_y*2, half_z*2]` → Cuboid extents.
    Cuboid,
    /// `size = [radius, half_height, _]` → Capsule.
    Capsule,
    /// `size = [radius, _, _]` → Sphere.
    Sphere,
    /// `size = [radius, height, _]` → Cylinder.
    Cylinder,
    /// `size = [radius, height, _]` → Cone (apex up).
    Cone,
}

/// Which limb / segment this part is for gait posing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartRole {
    Body,
    Head,
    /// Left leg (humanoid) or front-left (quad).
    LegL,
    /// Right leg (humanoid) or front-right (quad).
    LegR,
    /// Hind-left (quadruped only).
    HindLegL,
    /// Hind-right (quadruped only).
    HindLegR,
    /// Held prop / weapon / ear / wing — not gait-driven.
    Prop,
}

/// Resolved presentation recipe for one entity.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualSpec {
    pub key: &'static str,
    pub family: VisualFamily,
    /// Y offset applied by the renderer so the pivot sits on the ground.
    pub y_offset: f32,
    pub parts: &'static [VisualPart],
    /// Soft emissive boost (0–1) for wisps / loot / portals accents.
    pub emissive: f32,
    /// World-space height above feet for nameplates / quest marks.
    pub label_height: f32,
    /// Gentle vertical bob while idle (wisps / herbs / loot).
    pub bob: bool,
}

/// Zone atmosphere for ClearColor / ambient / terrain tint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneAtmosphere {
    pub zone_tag: &'static str,
    pub clear: [f32; 3],
    pub ambient: [f32; 3],
    pub terrain: [f32; 3],
    pub water: [f32; 4],
}

/// Hub / portal scene marker placement (world XZ).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneMarker {
    pub kind: SceneMarkerKind,
    pub x: f32,
    pub z: f32,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneMarkerKind {
    HubBeacon,
    PortalArch,
    CampFire,
}

/// Resolve a visual key then look up its mesh recipe.
pub fn visual_spec(kind: EntityKind, template_id: Option<&str>) -> VisualSpec {
    let key = visual_key(kind, template_id);
    spec_for_key(key)
}

/// Upstream-style dispatch: players → `player_<class>`, mobs/NPCs/pets by template
/// with family fallbacks.
pub fn visual_key(kind: EntityKind, template_id: Option<&str>) -> &'static str {
    let tid = template_id.unwrap_or("");
    match kind {
        EntityKind::Player => match tid {
            "warrior" => "player_warrior",
            "paladin" => "player_paladin",
            "hunter" => "player_hunter",
            "rogue" => "player_rogue",
            "priest" => "player_priest",
            "shaman" => "player_shaman",
            "mage" => "player_mage",
            "warlock" => "player_warlock",
            "druid" => "player_druid",
            _ => "player_warrior",
        },
        EntityKind::Npc => match tid {
            "trader_wilkes" | "apothecary_vex" | "ferryman_noll" | "quartermaster_bren" => {
                "npc_vendor"
            }
            "captain_alden" | "warden_selene" | "keeper_orla" | "commander_elara" => {
                "npc_quest_giver"
            }
            "town_crier" | "scout_darian" | "pathfinder_toren" => "npc_townsfolk",
            _ => "npc_townsfolk",
        },
        EntityKind::Mob => match tid {
            "young_wolf" | "scarred_wolf" | "ridge_stalker" | "hunter_wolf" => "mob_wolf",
            "young_boar" | "cragback_boar" => "mob_boar",
            "fen_crawler" | "mire_leech" => "mob_crawler",
            "mire_toad" => "mob_toad",
            "bog_wisp" => "mob_wisp",
            "rotcap_shambler" => "mob_shambler",
            "mire_terror" => "mob_terror",
            "gale_harpy" => "mob_harpy",
            "crypt_warden" => "mob_undead",
            _ => "mob_generic",
        },
        EntityKind::Pet => match tid {
            "hunter_wolf" => "pet_wolf",
            "warlock_imp" => "pet_imp",
            _ => "pet_generic",
        },
        EntityKind::Loot => {
            if matches!(
                tid,
                "eastbrook_meadow_silverleaf"
                    | "eastbrook_brook_peacebloom"
                    | "eastbrook_north_briar"
            ) || tid.contains("silverleaf")
                || tid.contains("peacebloom")
                || tid.contains("briar")
            {
                "gather_herb"
            } else {
                "loot_spark"
            }
        }
    }
}

fn spec_for_key(key: &str) -> VisualSpec {
    match key {
        "player_warrior" => PLAYER_WARRIOR,
        "player_paladin" => PLAYER_PALADIN,
        "player_hunter" => PLAYER_HUNTER,
        "player_rogue" => PLAYER_ROGUE,
        "player_priest" => PLAYER_PRIEST,
        "player_shaman" => PLAYER_SHAMAN,
        "player_mage" => PLAYER_MAGE,
        "player_warlock" => PLAYER_WARLOCK,
        "player_druid" => PLAYER_DRUID,
        "npc_quest_giver" => NPC_QUEST,
        "npc_vendor" => NPC_VENDOR,
        "npc_townsfolk" => NPC_TOWN,
        "mob_wolf" => MOB_WOLF,
        "mob_boar" => MOB_BOAR,
        "mob_crawler" => MOB_CRAWLER,
        "mob_toad" => MOB_TOAD,
        "mob_wisp" => MOB_WISP,
        "mob_shambler" => MOB_SHAMBLER,
        "mob_terror" => MOB_TERROR,
        "mob_harpy" => MOB_HARPY,
        "mob_undead" => MOB_UNDEAD,
        "mob_generic" => MOB_GENERIC,
        "pet_wolf" => PET_WOLF,
        "pet_imp" => PET_IMP,
        "pet_generic" => PET_GENERIC,
        "loot_spark" => LOOT_SPARK,
        "gather_herb" => GATHER_HERB,
        _ => MOB_GENERIC,
    }
}

/// Atmosphere for a zone tag (`eastbrook` / `eastfen` / `mirefen` / `thornpeak`).
pub fn zone_atmosphere(zone_id: &str) -> ZoneAtmosphere {
    match zone_id {
        "eastfen" | "fenbridge" | "mirefen_marsh" | "mirefen" => ZoneAtmosphere {
            zone_tag: "mirefen",
            clear: [0.28, 0.38, 0.36],
            ambient: [0.72, 0.82, 0.78],
            terrain: [0.22, 0.38, 0.28],
            water: [0.12, 0.28, 0.32, 0.72],
        },
        "thornpeak" | "thornpeak_heights" | "highwatch" => ZoneAtmosphere {
            zone_tag: "thornpeak",
            clear: [0.55, 0.62, 0.78],
            ambient: [0.95, 0.95, 1.0],
            terrain: [0.42, 0.44, 0.40],
            water: [0.20, 0.35, 0.55, 0.55],
        },
        _ => ZoneAtmosphere {
            zone_tag: "eastbrook",
            clear: [0.45, 0.62, 0.78],
            ambient: [0.92, 0.94, 0.88],
            terrain: [0.35, 0.52, 0.28],
            water: [0.15, 0.35, 0.55, 0.65],
        },
    }
}

/// Static scene markers: hub beacons, portal arches between zone bands, camp fires.
pub fn scene_markers() -> &'static [SceneMarker] {
    SCENE_MARKERS
}

const SCENE_MARKERS: &[SceneMarker] = &[
    SceneMarker {
        kind: SceneMarkerKind::HubBeacon,
        x: 0.0,
        z: 0.0,
        label: "Eastbrook",
    },
    SceneMarker {
        kind: SceneMarkerKind::HubBeacon,
        x: 0.0,
        z: 300.0,
        label: "Fenbridge",
    },
    SceneMarker {
        kind: SceneMarkerKind::HubBeacon,
        x: 0.0,
        z: 660.0,
        label: "Highwatch",
    },
    // Band seams ≈ portal landmarks on the continuous strip.
    SceneMarker {
        kind: SceneMarkerKind::PortalArch,
        x: 0.0,
        z: 180.0,
        label: "Eastfen Gate",
    },
    SceneMarker {
        kind: SceneMarkerKind::PortalArch,
        x: 0.0,
        z: 540.0,
        label: "Thornpeak Pass",
    },
    SceneMarker {
        kind: SceneMarkerKind::CampFire,
        x: -15.0,
        z: 55.0,
        label: "Wolf Run",
    },
    SceneMarker {
        kind: SceneMarkerKind::CampFire,
        x: 65.0,
        z: 0.0,
        label: "Boar Meadow",
    },
];

// ---- recipes ----------------------------------------------------------------

macro_rules! parts {
    ($($p:expr),* $(,)?) => {{
        const P: &[VisualPart] = &[$($p),*];
        P
    }};
}

const fn rgb(r: f32, g: f32, b: f32) -> [f32; 3] {
    [r, g, b]
}

const fn body(offset_y: f32, radius: f32, half_h: f32, c: [f32; 3]) -> VisualPart {
    VisualPart {
        shape: PartShape::Capsule,
        offset: [0.0, offset_y, 0.0],
        size: [radius, half_h, 0.0],
        color: c,
        role: PartRole::Body,
    }
}

const fn head(offset_y: f32, radius: f32, c: [f32; 3]) -> VisualPart {
    VisualPart {
        shape: PartShape::Sphere,
        offset: [0.0, offset_y, 0.0],
        size: [radius, 0.0, 0.0],
        color: c,
        role: PartRole::Head,
    }
}

const fn leg(role: PartRole, offset: [f32; 3], size: [f32; 3], c: [f32; 3]) -> VisualPart {
    VisualPart {
        shape: PartShape::Cuboid,
        offset,
        size,
        color: c,
        role,
    }
}

const PLAYER_WARRIOR: VisualSpec = VisualSpec {
    key: "player_warrior",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.95, 0.34, 0.55, rgb(0.55, 0.22, 0.18)),
        head(1.85, 0.22, rgb(0.85, 0.70, 0.55)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.42, 1.15, 0.0],
            size: [0.12, 0.85, 0.12],
            role: PartRole::Prop,
            color: rgb(0.65, 0.65, 0.70),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_PALADIN: VisualSpec = VisualSpec {
    key: "player_paladin",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.05,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.95, 0.36, 0.55, rgb(0.75, 0.70, 0.35)),
        head(1.85, 0.22, rgb(0.85, 0.72, 0.55)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [-0.40, 1.05, 0.05],
            size: [0.35, 0.55, 0.08],
            role: PartRole::Prop,
            color: rgb(0.80, 0.78, 0.55),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_HUNTER: VisualSpec = VisualSpec {
    key: "player_hunter",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.92, 0.32, 0.52, rgb(0.30, 0.48, 0.28)),
        head(1.78, 0.21, rgb(0.80, 0.65, 0.48)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.0, 1.15, -0.28],
            size: [0.08, 0.75, 0.08],
            role: PartRole::Prop,
            color: rgb(0.45, 0.32, 0.18),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_ROGUE: VisualSpec = VisualSpec {
    key: "player_rogue",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.90, 0.30, 0.50, rgb(0.22, 0.25, 0.28)),
        head(1.72, 0.20, rgb(0.75, 0.60, 0.48)),
        VisualPart {
            shape: PartShape::Cone,
            offset: [0.0, 1.95, 0.0],
            size: [0.22, 0.28, 0.0],
            role: PartRole::Prop,
            color: rgb(0.15, 0.15, 0.18),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_PRIEST: VisualSpec = VisualSpec {
    key: "player_priest",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.12,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.95, 0.33, 0.55, rgb(0.92, 0.90, 0.85)),
        head(1.85, 0.21, rgb(0.88, 0.75, 0.60)),
        VisualPart {
            shape: PartShape::Cylinder,
            offset: [0.0, 2.15, 0.0],
            size: [0.28, 0.06, 0.0],
            role: PartRole::Prop,
            color: rgb(1.0, 0.85, 0.40),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_SHAMAN: VisualSpec = VisualSpec {
    key: "player_shaman",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.04,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.94, 0.34, 0.54, rgb(0.35, 0.45, 0.70)),
        head(1.82, 0.21, rgb(0.78, 0.62, 0.48)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.38, 1.20, 0.0],
            size: [0.10, 0.90, 0.10],
            role: PartRole::Prop,
            color: rgb(0.55, 0.40, 0.25),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_MAGE: VisualSpec = VisualSpec {
    key: "player_mage",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.08,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.95, 0.32, 0.55, rgb(0.25, 0.40, 0.80)),
        head(1.85, 0.21, rgb(0.85, 0.72, 0.58)),
        VisualPart {
            shape: PartShape::Cone,
            offset: [0.0, 2.05, 0.0],
            size: [0.28, 0.35, 0.0],
            role: PartRole::Prop,
            color: rgb(0.20, 0.30, 0.65),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_WARLOCK: VisualSpec = VisualSpec {
    key: "player_warlock",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.06,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.95, 0.33, 0.55, rgb(0.40, 0.22, 0.55)),
        head(1.85, 0.21, rgb(0.80, 0.65, 0.55)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.35, 1.10, 0.05],
            size: [0.22, 0.28, 0.08],
            role: PartRole::Prop,
            color: rgb(0.55, 0.35, 0.20),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const PLAYER_DRUID: VisualSpec = VisualSpec {
    key: "player_druid",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.03,
    label_height: 2.35,
    bob: false,
    parts: parts![
        body(0.94, 0.34, 0.54, rgb(0.35, 0.55, 0.30)),
        head(1.82, 0.22, rgb(0.78, 0.60, 0.45)),
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 2.05, 0.0],
            size: [0.16, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.45, 0.70, 0.35),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const NPC_QUEST: VisualSpec = VisualSpec {
    key: "npc_quest_giver",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.1,
    label_height: 2.3,
    bob: false,
    parts: parts![
        body(0.92, 0.32, 0.52, rgb(0.45, 0.55, 0.35)),
        head(1.78, 0.21, rgb(0.82, 0.68, 0.52)),
        VisualPart {
            shape: PartShape::Cylinder,
            offset: [0.0, 2.10, 0.0],
            size: [0.22, 0.08, 0.0],
            role: PartRole::Prop,
            color: rgb(0.95, 0.80, 0.25),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const NPC_VENDOR: VisualSpec = VisualSpec {
    key: "npc_vendor",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 2.3,
    bob: false,
    parts: parts![
        body(0.90, 0.36, 0.50, rgb(0.55, 0.40, 0.28)),
        head(1.72, 0.22, rgb(0.80, 0.65, 0.50)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.0, 0.55, 0.35],
            size: [0.55, 0.35, 0.25],
            role: PartRole::Prop,
            color: rgb(0.45, 0.30, 0.18),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const NPC_TOWN: VisualSpec = VisualSpec {
    key: "npc_townsfolk",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 2.3,
    bob: false,
    parts: parts![
        body(0.90, 0.31, 0.50, rgb(0.50, 0.55, 0.45)),
        head(1.72, 0.20, rgb(0.82, 0.68, 0.52)),
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const MOB_WOLF: VisualSpec = VisualSpec {
    key: "mob_wolf",
    family: VisualFamily::Wolf,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 1.15,
    bob: false,
    parts: parts![
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.0, 0.35, 0.0],
            size: [0.55, 0.40, 1.05],
            role: PartRole::Prop,
            color: rgb(0.42, 0.36, 0.30),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.45, 0.55],
            size: [0.22, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.38, 0.32, 0.28),
        },
        VisualPart {
            shape: PartShape::Cone,
            offset: [0.0, 0.55, -0.55],
            size: [0.10, 0.35, 0.0],
            role: PartRole::Prop,
            color: rgb(0.35, 0.30, 0.26),
        },
        leg(
            PartRole::LegL,
            [-0.18, 0.22, 0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
        leg(
            PartRole::LegR,
            [0.18, 0.22, 0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
        leg(
            PartRole::HindLegL,
            [-0.18, 0.22, -0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
        leg(
            PartRole::HindLegR,
            [0.18, 0.22, -0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
    ],
};

const MOB_BOAR: VisualSpec = VisualSpec {
    key: "mob_boar",
    family: VisualFamily::Boar,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 1.15,
    bob: false,
    parts: parts![
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.0, 0.40, 0.0],
            size: [0.70, 0.50, 1.05],
            role: PartRole::Prop,
            color: rgb(0.48, 0.32, 0.22),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.48, 0.55],
            size: [0.26, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.42, 0.28, 0.20),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.18, 0.40, 0.72],
            size: [0.06, 0.06, 0.28],
            role: PartRole::Prop,
            color: rgb(0.90, 0.88, 0.80),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [-0.18, 0.40, 0.72],
            size: [0.06, 0.06, 0.28],
            role: PartRole::Prop,
            color: rgb(0.90, 0.88, 0.80),
        },
        leg(
            PartRole::LegL,
            [-0.22, 0.20, 0.28],
            [0.12, 0.38, 0.12],
            rgb(0.40, 0.28, 0.18)
        ),
        leg(
            PartRole::LegR,
            [0.22, 0.20, 0.28],
            [0.12, 0.38, 0.12],
            rgb(0.40, 0.28, 0.18)
        ),
        leg(
            PartRole::HindLegL,
            [-0.22, 0.20, -0.28],
            [0.12, 0.38, 0.12],
            rgb(0.40, 0.28, 0.18)
        ),
        leg(
            PartRole::HindLegR,
            [0.22, 0.20, -0.28],
            [0.12, 0.38, 0.12],
            rgb(0.40, 0.28, 0.18)
        ),
    ],
};

const MOB_CRAWLER: VisualSpec = VisualSpec {
    key: "mob_crawler",
    family: VisualFamily::Crawler,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 0.7,
    bob: false,
    parts: parts![
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.28, 0.0],
            size: [0.38, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.25, 0.35, 0.22),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.35, 0.12, 0.25],
            size: [0.45, 0.06, 0.08],
            role: PartRole::Prop,
            color: rgb(0.20, 0.28, 0.18),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [-0.35, 0.12, 0.25],
            size: [0.45, 0.06, 0.08],
            role: PartRole::Prop,
            color: rgb(0.20, 0.28, 0.18),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.35, 0.12, -0.25],
            size: [0.45, 0.06, 0.08],
            role: PartRole::Prop,
            color: rgb(0.20, 0.28, 0.18),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [-0.35, 0.12, -0.25],
            size: [0.45, 0.06, 0.08],
            role: PartRole::Prop,
            color: rgb(0.20, 0.28, 0.18),
        },
    ],
};

const MOB_TOAD: VisualSpec = VisualSpec {
    key: "mob_toad",
    family: VisualFamily::Toad,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 0.85,
    bob: false,
    parts: parts![
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.35, 0.0],
            size: [0.42, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.30, 0.48, 0.28),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.48, 0.32],
            size: [0.24, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.35, 0.50, 0.30),
        },
    ],
};

const MOB_WISP: VisualSpec = VisualSpec {
    key: "mob_wisp",
    family: VisualFamily::Wisp,
    y_offset: 0.0,
    emissive: 0.55,
    label_height: 1.5,
    bob: true,
    parts: parts![
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.85, 0.0],
            size: [0.28, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.45, 0.85, 0.95),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.55, 0.0],
            size: [0.16, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.70, 0.95, 1.0),
        },
    ],
};

const MOB_SHAMBLER: VisualSpec = VisualSpec {
    key: "mob_shambler",
    family: VisualFamily::Shambler,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 2.4,
    bob: false,
    parts: parts![
        body(1.05, 0.42, 0.65, rgb(0.40, 0.45, 0.28)),
        head(1.95, 0.30, rgb(0.55, 0.50, 0.30)),
    ],
};

const MOB_TERROR: VisualSpec = VisualSpec {
    key: "mob_terror",
    family: VisualFamily::Shambler,
    y_offset: 0.0,
    emissive: 0.15,
    label_height: 3.2,
    bob: false,
    parts: parts![
        body(1.35, 0.55, 0.85, rgb(0.35, 0.18, 0.28)),
        head(2.55, 0.38, rgb(0.45, 0.20, 0.30)),
        VisualPart {
            shape: PartShape::Cone,
            offset: [0.35, 2.95, 0.0],
            size: [0.12, 0.40, 0.0],
            role: PartRole::Prop,
            color: rgb(0.55, 0.25, 0.35),
        },
        VisualPart {
            shape: PartShape::Cone,
            offset: [-0.35, 2.95, 0.0],
            size: [0.12, 0.40, 0.0],
            role: PartRole::Prop,
            color: rgb(0.55, 0.25, 0.35),
        },
    ],
};

const MOB_HARPY: VisualSpec = VisualSpec {
    key: "mob_harpy",
    family: VisualFamily::Harpy,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 2.2,
    bob: true,
    parts: parts![
        body(1.10, 0.28, 0.45, rgb(0.55, 0.45, 0.55)),
        head(1.85, 0.20, rgb(0.80, 0.70, 0.55)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.55, 1.25, 0.0],
            size: [0.85, 0.08, 0.35],
            role: PartRole::Prop,
            color: rgb(0.50, 0.40, 0.50),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [-0.55, 1.25, 0.0],
            size: [0.85, 0.08, 0.35],
            role: PartRole::Prop,
            color: rgb(0.50, 0.40, 0.50),
        },
    ],
};

const MOB_UNDEAD: VisualSpec = VisualSpec {
    key: "mob_undead",
    family: VisualFamily::Humanoid,
    y_offset: 0.0,
    emissive: 0.08,
    label_height: 2.4,
    bob: false,
    parts: parts![
        body(1.05, 0.36, 0.60, rgb(0.55, 0.58, 0.50)),
        head(1.95, 0.24, rgb(0.75, 0.78, 0.70)),
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.40, 1.20, 0.0],
            size: [0.12, 0.95, 0.12],
            role: PartRole::Prop,
            color: rgb(0.40, 0.42, 0.38),
        },
        leg(
            PartRole::LegL,
            [-0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
        leg(
            PartRole::LegR,
            [0.14, 0.35, 0.0],
            [0.14, 0.70, 0.16],
            rgb(0.25, 0.22, 0.20)
        ),
    ],
};

const MOB_GENERIC: VisualSpec = VisualSpec {
    key: "mob_generic",
    family: VisualFamily::Cuboid,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 1.0,
    bob: false,
    parts: parts![VisualPart {
        shape: PartShape::Cuboid,
        offset: [0.0, 0.35, 0.0],
        size: [0.90, 0.55, 1.30],
        role: PartRole::Prop,
        color: rgb(0.45, 0.35, 0.28),
    }],
};

const PET_WOLF: VisualSpec = VisualSpec {
    key: "pet_wolf",
    family: VisualFamily::Wolf,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 1.0,
    bob: false,
    parts: parts![
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.0, 0.30, 0.0],
            size: [0.45, 0.35, 0.90],
            role: PartRole::Prop,
            color: rgb(0.40, 0.48, 0.55),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.38, 0.48],
            size: [0.18, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.38, 0.45, 0.52),
        },
        leg(
            PartRole::LegL,
            [-0.18, 0.22, 0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
        leg(
            PartRole::LegR,
            [0.18, 0.22, 0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
        leg(
            PartRole::HindLegL,
            [-0.18, 0.22, -0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
        leg(
            PartRole::HindLegR,
            [0.18, 0.22, -0.32],
            [0.10, 0.40, 0.10],
            rgb(0.35, 0.30, 0.26)
        ),
    ],
};

const PET_IMP: VisualSpec = VisualSpec {
    key: "pet_imp",
    family: VisualFamily::Imp,
    y_offset: 0.0,
    emissive: 0.2,
    label_height: 1.4,
    bob: true,
    parts: parts![
        body(0.55, 0.20, 0.28, rgb(0.70, 0.25, 0.20)),
        head(1.00, 0.18, rgb(0.75, 0.30, 0.22)),
        VisualPart {
            shape: PartShape::Cone,
            offset: [0.12, 1.22, 0.0],
            size: [0.06, 0.18, 0.0],
            role: PartRole::Prop,
            color: rgb(0.55, 0.20, 0.15),
        },
        VisualPart {
            shape: PartShape::Cone,
            offset: [-0.12, 1.22, 0.0],
            size: [0.06, 0.18, 0.0],
            role: PartRole::Prop,
            color: rgb(0.55, 0.20, 0.15),
        },
    ],
};

const PET_GENERIC: VisualSpec = VisualSpec {
    key: "pet_generic",
    family: VisualFamily::Cuboid,
    y_offset: 0.0,
    emissive: 0.0,
    label_height: 0.9,
    bob: false,
    parts: parts![VisualPart {
        shape: PartShape::Cuboid,
        offset: [0.0, 0.30, 0.0],
        size: [0.55, 0.40, 0.80],
        role: PartRole::Prop,
        color: rgb(0.35, 0.55, 0.65),
    }],
};

const GATHER_HERB: VisualSpec = VisualSpec {
    key: "gather_herb",
    family: VisualFamily::Loot,
    y_offset: 0.0,
    emissive: 0.18,
    label_height: 1.05,
    bob: true,
    parts: parts![
        VisualPart {
            shape: PartShape::Cylinder,
            offset: [0.0, 0.25, 0.0],
            size: [0.08, 0.45, 0.0],
            role: PartRole::Prop,
            color: rgb(0.25, 0.55, 0.22),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.12, 0.55, 0.0],
            size: [0.16, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.45, 0.85, 0.40),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [-0.10, 0.48, 0.08],
            size: [0.14, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.55, 0.90, 0.45),
        },
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.62, -0.10],
            size: [0.12, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.70, 0.95, 0.55),
        },
    ],
};

const LOOT_SPARK: VisualSpec = VisualSpec {
    key: "loot_spark",
    family: VisualFamily::Loot,
    y_offset: 0.0,
    emissive: 0.35,
    label_height: 0.85,
    bob: true,
    parts: parts![
        VisualPart {
            shape: PartShape::Sphere,
            offset: [0.0, 0.30, 0.0],
            size: [0.22, 0.0, 0.0],
            role: PartRole::Prop,
            color: rgb(0.95, 0.80, 0.25),
        },
        VisualPart {
            shape: PartShape::Cuboid,
            offset: [0.0, 0.12, 0.0],
            size: [0.35, 0.18, 0.28],
            role: PartRole::Prop,
            color: rgb(0.55, 0.38, 0.15),
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_class_keys() {
        assert_eq!(visual_key(EntityKind::Player, Some("mage")), "player_mage");
        assert_eq!(
            visual_key(EntityKind::Player, Some("warrior")),
            "player_warrior"
        );
        assert_eq!(
            visual_spec(EntityKind::Player, Some("priest")).key,
            "player_priest"
        );
        assert!(visual_spec(EntityKind::Player, Some("priest")).emissive > 0.0);
    }

    #[test]
    fn mob_family_dispatch() {
        assert_eq!(visual_key(EntityKind::Mob, Some("young_wolf")), "mob_wolf");
        assert_eq!(
            visual_key(EntityKind::Mob, Some("fen_crawler")),
            "mob_crawler"
        );
        assert_eq!(
            visual_key(EntityKind::Mob, Some("mire_terror")),
            "mob_terror"
        );
        assert_eq!(
            visual_key(EntityKind::Mob, Some("unknown_critter")),
            "mob_generic"
        );
    }

    #[test]
    fn npc_roles() {
        assert_eq!(
            visual_key(EntityKind::Npc, Some("trader_wilkes")),
            "npc_vendor"
        );
        assert_eq!(
            visual_key(EntityKind::Npc, Some("captain_alden")),
            "npc_quest_giver"
        );
    }

    #[test]
    fn every_spec_has_parts() {
        for tid in [
            Some("mage"),
            Some("young_boar"),
            Some("bog_wisp"),
            Some("warlock_imp"),
            None,
        ] {
            let kind = if tid == Some("mage") {
                EntityKind::Player
            } else if tid == Some("warlock_imp") {
                EntityKind::Pet
            } else if tid.is_none() {
                EntityKind::Loot
            } else {
                EntityKind::Mob
            };
            let spec = visual_spec(kind, tid);
            assert!(!spec.parts.is_empty(), "empty parts for {tid:?}");
        }
    }

    #[test]
    fn humanoid_and_quad_specs_have_gait_legs() {
        let warrior = visual_spec(EntityKind::Player, Some("warrior"));
        assert!(
            warrior.parts.iter().any(|p| p.role == PartRole::LegL)
                && warrior.parts.iter().any(|p| p.role == PartRole::LegR),
            "humanoids need biped legs for walk cycle"
        );
        let wolf = visual_spec(EntityKind::Mob, Some("young_wolf"));
        assert!(
            wolf.parts.iter().any(|p| p.role == PartRole::HindLegL)
                && wolf.parts.iter().any(|p| p.role == PartRole::HindLegR),
            "wolves need hind legs for walk cycle"
        );
        let npc = visual_spec(EntityKind::Npc, Some("town_crier"));
        assert!(npc.parts.iter().any(|p| p.role == PartRole::LegL));
    }

    #[test]
    fn gather_nodes_use_herb_visual() {
        assert_eq!(
            visual_key(EntityKind::Loot, Some("eastbrook_meadow_silverleaf")),
            "gather_herb"
        );
        let spec = visual_spec(EntityKind::Loot, Some("eastbrook_brook_peacebloom"));
        assert_eq!(spec.key, "gather_herb");
        assert!(spec.bob);
    }

    #[test]
    fn zone_atmosphere_tags() {
        assert_eq!(zone_atmosphere("eastbrook").zone_tag, "eastbrook");
        assert_eq!(zone_atmosphere("eastfen").zone_tag, "mirefen");
        assert_eq!(zone_atmosphere("thornpeak").zone_tag, "thornpeak");
    }

    #[test]
    fn scene_markers_cover_hubs_and_gates() {
        let markers = scene_markers();
        assert!(markers
            .iter()
            .any(|m| m.kind == SceneMarkerKind::HubBeacon && m.z == 0.0));
        assert!(markers
            .iter()
            .any(|m| m.kind == SceneMarkerKind::PortalArch && (m.z - 180.0).abs() < 0.1));
        assert!(markers
            .iter()
            .any(|m| m.kind == SceneMarkerKind::PortalArch && (m.z - 540.0).abs() < 0.1));
    }
}
