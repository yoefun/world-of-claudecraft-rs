//! Maps sim `visual_key` / class ids onto Bevy asset paths under `assets/`.
//!
//! Paths mirror upstream `public/` (KayKit / Quaternius / project GLBs).
//! Missing keys fall back to procedural mesh recipes in `visuals`.

/// GLB scene path relative to the Bevy `assets/` root, including `#Scene0`.
pub(crate) fn glb_for_visual_key(key: &str) -> Option<&'static str> {
    Some(match key {
        // Players (KayKit adventures pack, class-mapped).
        "player_warrior" => "models/chars/players/knight.glb#Scene0",
        "player_paladin" => "models/chars/players/paladin.glb#Scene0",
        "player_hunter" => "models/chars/players/ranger.glb#Scene0",
        "player_rogue" => "models/chars/players/rogue.glb#Scene0",
        "player_priest" => "models/chars/players/mage_classic.glb#Scene0",
        "player_shaman" => "models/chars/players/druid.glb#Scene0",
        "player_mage" => "models/chars/players/mage.glb#Scene0",
        "player_warlock" => "models/chars/players/mage.glb#Scene0",
        "player_druid" => "models/chars/players/druid.glb#Scene0",
        // NPCs — reuse humanoid player kits until dedicated town GLBs are wired.
        "npc_quest_giver" => "models/chars/players/knight.glb#Scene0",
        "npc_vendor" => "models/chars/players/rogue_hooded.glb#Scene0",
        "npc_townsfolk" => "models/chars/players/barbarian.glb#Scene0",
        // Mobs / pets (Quaternius + upstream creature set).
        "mob_wolf" | "pet_wolf" => "models/creatures/wolf_basic.glb#Scene0",
        "mob_boar" => "models/creatures/wild_boar.glb#Scene0",
        "mob_crawler" => "models/creatures/spider.glb#Scene0",
        "mob_toad" => "models/creatures/frog.glb#Scene0",
        "mob_wisp" => "models/creatures/ghost.glb#Scene0",
        "mob_shambler" => "models/creatures/golelingevolved.glb#Scene0",
        "mob_terror" => "models/creatures/demon.glb#Scene0",
        "mob_harpy" => "models/creatures/tribal.glb#Scene0",
        "mob_undead" => "models/chars/enemies/skeleton_warrior.glb#Scene0",
        "mob_generic" => "models/creatures/orc.glb#Scene0",
        "pet_imp" | "pet_generic" => "models/creatures/goblin.glb#Scene0",
        // Mounts
        "mount_pony" | "brown_pony" => "models/creatures/stag.glb#Scene0",
        "mount_steed" | "swift_bay_steed" => "models/creatures/bull.glb#Scene0",
        "mount_gryphon" | "tawny_gryphon" => "models/creatures/dragonevolved.glb#Scene0",
        _ => return None,
    })
}

/// Return the source glTF path without the `#Scene0` subasset label.
/// Uniform scale so KayKit / Quaternius roots sit near a 1.8 yd humanoid.
pub(crate) fn glb_scale_for_visual_key(key: &str) -> f32 {
    match key {
        k if k.starts_with("player_") || k.starts_with("npc_") => 1.0,
        "mob_wolf" | "pet_wolf" | "mob_boar" | "mob_crawler" | "mob_toad" => 1.0,
        "mob_terror" | "mob_shambler" | "mount_gryphon" | "tawny_gryphon" => 0.85,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{glb_for_visual_key, glb_scale_for_visual_key};
    use std::path::Path;

    const MAPPED_KEYS: &[&str] = &[
        "player_warrior",
        "player_paladin",
        "player_hunter",
        "player_rogue",
        "player_priest",
        "player_shaman",
        "player_mage",
        "player_warlock",
        "player_druid",
        "npc_quest_giver",
        "npc_vendor",
        "npc_townsfolk",
        "mob_wolf",
        "mob_boar",
        "mob_crawler",
        "mob_toad",
        "mob_wisp",
        "mob_shambler",
        "mob_terror",
        "mob_harpy",
        "mob_undead",
        "mob_generic",
        "pet_wolf",
        "pet_imp",
        "pet_generic",
        "mount_pony",
        "mount_steed",
        "mount_gryphon",
    ];

    #[test]
    fn every_mapped_glb_exists_in_workspace_assets() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for key in MAPPED_KEYS {
            let path = glb_for_visual_key(key).expect("mapped key must resolve");
            let asset = path.split_once('#').map_or(path, |(asset, _)| asset);
            assert!(
                assets.join(asset).is_file(),
                "missing GLB for {key}: {}",
                assets.join(asset).display()
            );
            assert!(glb_scale_for_visual_key(key).is_finite());
            assert!(glb_scale_for_visual_key(key) > 0.0);
        }
    }

    #[test]
    fn every_mapped_glb_avoids_unsupported_bevy_extensions() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let unsupported = [
            "EXT_meshopt_compression",
            "EXT_texture_webp",
            "KHR_mesh_quantization",
        ];

        for key in MAPPED_KEYS {
            let path = glb_for_visual_key(key).expect("mapped key must resolve");
            let asset = path.split_once('#').map_or(path, |(asset, _)| asset);
            let bytes = std::fs::read(assets.join(asset)).expect("mapped GLB must be readable");
            assert!(bytes.len() >= 20, "truncated GLB for {key}");
            assert_eq!(&bytes[0..4], b"glTF", "invalid GLB magic for {key}");

            let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
            let json_end = 20 + json_len;
            assert!(json_end <= bytes.len(), "truncated JSON chunk for {key}");
            let json = std::str::from_utf8(&bytes[20..json_end]).expect("GLB JSON must be UTF-8");
            for extension in unsupported {
                assert!(
                    !json.contains(extension),
                    "{key} still declares unsupported extension {extension}"
                );
            }
        }
    }
}
