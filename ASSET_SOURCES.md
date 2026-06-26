# Demo Asset Sources

## Suzanne

- Source: Khronos Group glTF Sample Assets
- Upstream: https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/Suzanne
- License: CC0 1.0 Universal
- Author credit from upstream: UX3D / Norbert Nopper
- Files: `assets/demo/suzanne/*`

The files are vendored so tests and demos are deterministic and do not require network access.
Run `scripts/fetch_demo_assets.sh` to refresh them from the pinned upstream paths.

## AAA Industrial Scene

The live renderer's industrial alley uses these Poly Haven assets at 1K:

- Models: Barrel 01, Street Lamp 01, Concrete Road Barrier, Steel Frame Shelves 01,
  Utility Box 01, Old Tyre, and Metal Trash Can.
- Surfaces: Asphalt 02, Concrete Floor, Brick Wall 08, and Metal Plate 02.
- Source: https://polyhaven.com/
- API/download manifest: `scripts/fetch_aaa_assets.sh`
- License: CC0 1.0 Universal

The downloaded glTF models retain their base-color, tangent-space normal, and packed
ambient-occlusion/roughness/metallic textures. The files are vendored under
`assets/aaa` so the scene is deterministic and works offline.

## Generated Alley Decals

The expanded alley uses these built-in image-generation outputs as project-local
texture/decal assets:

- `assets/aaa/generated/cyan_transit_panel.png`
- `assets/aaa/generated/amber_hazard_placard.png`
- `assets/aaa/generated/wet_tarp_cable_net.png`

The prompts asked for front-facing rainy industrial game textures with no logos,
watermarks, or readable brand text: a cyan transit schematic panel, an amber
hazard placard, and a wet tarp/cable-net material. They are loaded by
`build_aaa_scene` as regular `TextureImage` base-color textures, then applied to
new wall, hanging-sheet, and far-depth set dressing meshes.

The scene also uses two generated material texture sets under
`assets/aaa/generated/textures`:

- `wet_corrugated_metal_{base,normal,arm}.png`
- `oily_patched_concrete_{base,normal,arm}.png`

The base maps were created with the built-in image generation tool from tileable
PBR game-texture prompts for rainy corrugated service metal and oily patched
concrete. The normal maps and packed ARM maps are derived locally from those
base maps for deterministic offline rendering.

## Rain and Wetness

Rain streaks are rendered as world-space depth-tested geometry in
`src/shaders/rain3d.wgsl`. The generated
`assets/aaa/rain/generated_streak_detail.png` atlas is sampled per drop to add
bead breakup and glints without using a flat screen-space overlay. It was
created with the built-in image generation tool from a grayscale VFX texture
prompt asking for narrow vertical rain streak variants, bright thin cores, soft
halos, broken segments, droplets, and specular beads on a pure black background.

Puddle breakup uses the generated `assets/aaa/puddles/generated_puddle_detail.png`
source map plus derived tangent-space normal and packed ARM maps. The source was
created with the built-in image generation tool from a grayscale seamless VFX
texture prompt asking for rain ripple rings, tiny specular beads, broken watery
streaks, and shallow glossy urban puddle detail. The derived maps are generated
locally from that source height field for deterministic rendering. The puddle
material carries low albedo opacity and uses the glass/water material flag in
the normal-buffer alpha so the lighting and reflection passes composite it as a
transparent reflective surface instead of an opaque ground decal.

Floor impacts, broader puddle wetness, object wetness, and wall runoff are
generated procedurally from world position, normals, time, and scene lighting in
`src/shaders/lighting.wgsl`.

Older checkouts may contain OpenGameArt rain overlay/ripple images under
`assets/aaa/rain`, but the current live path does not bind or sample those flat
overlay assets.
