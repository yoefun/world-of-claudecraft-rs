# Bevy assets (upstream `public/` mirror)

Full mirror of World of ClaudeCraft upstream `public/` at the pinned rewrite release, laid out for Bevy’s `AssetServer` (paths are relative to this folder).

| Path | Contents |
|------|----------|
| `models/` | GLB characters, creatures, props, biome kits |
| `textures/` | Shared / material textures |
| `audio/` | Music and SFX |
| `ui/` | Icons and chrome |
| `env/` | Environment maps / skies |
| `fonts/` | UI fonts |
| `vfx/` | Effect textures / sprites |

Licensing notes live in `CREDITS.upstream.md`. Restricted packs are still present (plan B: no cull yet).

`woc-client` points `AssetPlugin.file_path` at the packaged executable's sibling
`assets/` directory first, then falls back to the workspace root `assets/` during
development. The release workflow copies only the legal runtime closure listed in
[`runtime-manifest.txt`](runtime-manifest.txt); the rest of this mirror is source
material and is not part of the client archive.
