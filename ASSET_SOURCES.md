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
