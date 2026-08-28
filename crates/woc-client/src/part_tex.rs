//! Procedural albedo maps for character / creature mesh parts.
//!
//! Flat `base_color` alone reads as plastic geometry; these small grayscale
//! maps are multiplied by part tint so cloth / skin / fur / metal read clearly.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use woc_sim::{PartRole, VisualFamily};

const TEX_SIZE: u32 = 64;

#[derive(Resource, Clone)]
pub(crate) struct PartTextures {
    pub(crate) skin: Handle<Image>,
    pub(crate) cloth: Handle<Image>,
    pub(crate) leather: Handle<Image>,
    pub(crate) fur: Handle<Image>,
    pub(crate) metal: Handle<Image>,
    pub(crate) scale: Handle<Image>,
    pub(crate) crystal: Handle<Image>,
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, init_part_textures);
}

fn init_part_textures(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(PartTextures {
        skin: images.add(make_tex(skin_pixel)),
        cloth: images.add(make_tex(cloth_pixel)),
        leather: images.add(make_tex(leather_pixel)),
        fur: images.add(make_tex(fur_pixel)),
        metal: images.add(make_tex(metal_pixel)),
        scale: images.add(make_tex(scale_pixel)),
        crystal: images.add(make_tex(crystal_pixel)),
    });
}

/// Pick an albedo map for a body part given its family / role.
pub(crate) fn texture_for(
    textures: &PartTextures,
    family: VisualFamily,
    role: PartRole,
) -> Handle<Image> {
    match role {
        PartRole::Head => match family {
            VisualFamily::Humanoid | VisualFamily::Imp => textures.skin.clone(),
            VisualFamily::Wolf
            | VisualFamily::Boar
            | VisualFamily::Harpy
            | VisualFamily::Shambler => textures.fur.clone(),
            VisualFamily::Crawler | VisualFamily::Toad => textures.scale.clone(),
            VisualFamily::Wisp | VisualFamily::Loot => textures.crystal.clone(),
            VisualFamily::Cuboid => textures.leather.clone(),
        },
        PartRole::Body => match family {
            VisualFamily::Humanoid | VisualFamily::Imp => textures.cloth.clone(),
            VisualFamily::Wolf | VisualFamily::Boar | VisualFamily::Harpy => textures.fur.clone(),
            VisualFamily::Crawler | VisualFamily::Toad => textures.scale.clone(),
            VisualFamily::Shambler => textures.leather.clone(),
            VisualFamily::Wisp | VisualFamily::Loot => textures.crystal.clone(),
            VisualFamily::Cuboid => textures.cloth.clone(),
        },
        PartRole::LegL | PartRole::LegR | PartRole::HindLegL | PartRole::HindLegR => match family {
            VisualFamily::Humanoid | VisualFamily::Imp => textures.leather.clone(),
            VisualFamily::Crawler | VisualFamily::Toad => textures.scale.clone(),
            VisualFamily::Wisp | VisualFamily::Loot => textures.crystal.clone(),
            _ => textures.fur.clone(),
        },
        PartRole::Prop => match family {
            VisualFamily::Humanoid | VisualFamily::Imp => textures.metal.clone(),
            VisualFamily::Wisp | VisualFamily::Loot => textures.crystal.clone(),
            VisualFamily::Crawler | VisualFamily::Toad => textures.scale.clone(),
            _ => textures.fur.clone(),
        },
    }
}

fn make_tex(pixel: fn(u32, u32) -> u8) -> Image {
    let n = TEX_SIZE as usize;
    let mut data = vec![0u8; n * n * 4];
    for y in 0..TEX_SIZE {
        for x in 0..TEX_SIZE {
            let v = pixel(x, y);
            let i = (y as usize * n + x as usize) * 4;
            data[i] = v;
            data[i + 1] = v;
            data[i + 2] = v;
            data[i + 3] = 255;
        }
    }
    Image::new(
        Extent3d {
            width: TEX_SIZE,
            height: TEX_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn hash2(x: u32, y: u32) -> u32 {
    let mut n = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^ (n >> 16)
}

fn skin_pixel(x: u32, y: u32) -> u8 {
    let n = (hash2(x, y) & 31) as u8;
    200u8.saturating_add(n / 2).saturating_sub(8)
}

fn cloth_pixel(x: u32, y: u32) -> u8 {
    let weave = if ((x / 4) + (y / 4)) % 2 == 0 { 18 } else { 0 };
    let thread = if x % 8 == 0 || y % 8 == 0 { 12 } else { 0 };
    let n = (hash2(x.wrapping_mul(3), y) & 15) as i16;
    (165 + weave + thread + n).clamp(90, 240) as u8
}

fn leather_pixel(x: u32, y: u32) -> u8 {
    let grain = ((hash2(x / 2, y / 2) & 63) as i16) - 20;
    let pore = if hash2(x, y) % 17 == 0 { -25 } else { 0 };
    (150 + grain + pore).clamp(70, 220) as u8
}

fn fur_pixel(x: u32, y: u32) -> u8 {
    let strand = ((x.wrapping_add(y.wrapping_mul(3))) % 5) as i16 * 8;
    let n = ((hash2(x, y.wrapping_mul(2)) & 31) as i16) - 10;
    (140 + strand + n).clamp(60, 230) as u8
}

fn metal_pixel(x: u32, y: u32) -> u8 {
    let brush = ((x.wrapping_add(y / 3)) % 6) as i16 * 6;
    let glint = if (x + y * 2) % 23 == 0 { 50 } else { 0 };
    (170 + brush + glint).clamp(100, 255) as u8
}

fn scale_pixel(x: u32, y: u32) -> u8 {
    let sx = x % 8;
    let sy = y % 6;
    let row = y / 6;
    let ox = if row % 2 == 0 { sx } else { (sx + 4) % 8 };
    let edge = if ox == 0 || sy == 0 { 35 } else { 0 };
    let n = (hash2(x / 8, y / 6) & 20) as i16;
    (145 + edge + n).clamp(80, 230) as u8
}

fn crystal_pixel(x: u32, y: u32) -> u8 {
    let facet = ((x / 8) ^ (y / 8)) as i16 * 12;
    let spark = if hash2(x, y) % 29 == 0 { 70 } else { 0 };
    (160 + facet + spark).clamp(90, 255) as u8
}
