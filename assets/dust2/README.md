# assets/dust2 - local CS2 Dust2 import

The `--map dust2` scene now loads a local export from the user's licensed
Counter-Strike 2 install instead of the old hand-authored placeholder layout or
physics-only skeleton.
The generated Valve-derived files live under `generated/cs2/`, which is ignored
by git and must not be redistributed unless you have explicit rights to do so.

## Importing from a local CS2 install

Install or download ValveResourceFormat's `Source2Viewer-CLI`, then run:

```bash
SOURCE2VIEWER_CLI=/path/to/Source2Viewer-CLI ./scripts/import_cs2_dust2.sh
```

By default the importer reads:

```text
/mnt/windows/Program Files (x86)/Steam/steamapps/common/Counter-Strike Global Offensive/game
```

Override that with `CS2_GAME=/path/to/Counter-Strike Global Offensive/game` if
your Steam library is elsewhere.

The script exports:

- `generated/cs2/maps/de_dust2.glb`
- `generated/cs2/maps/*.png` material textures exported beside the visual GLB
- `generated/cs2/materials/skybox/sky_de_dust2.{vmat,exr,png}`
- `generated/cs2/maps/de_dust2/world_physics_physics.glb`
- `generated/cs2/manifest.json`

`build_dust2_scene()` prefers the visual `de_dust2.glb` export, caps decoded
runtime texture size for memory safety, filters Source 2 tool/clip meshes, adds
the converted Dust2 panorama directly to live sky lighting, and retains the
physics GLB only as a compatibility fallback. The importer fails if sky
extraction or conversion does not produce a non-empty file. It no longer
silently falls back to CC0 placeholder boxes.

## Roaming the map

Run `cargo run --release -- --live --map dust2`. The Dust2 camera starts inside
the imported map at player height. Hold right mouse to look, use `W`/`A`/`S`/`D`
to move, `Q`/`E` to descend/ascend, and `Shift` to boost.

Dust2 uses a dry-daylight environment profile: rain and wet-surface effects are
disabled, while sky exposure/yaw and auto-exposure limits come from the scene.
For a deterministic comparison capture, add `--static-camera --frames 40
--capture /tmp/dhool-dust2.png`; capture mode omits the FPS overlay.

## Legacy CC0 placeholders

The `surfaces/` and `models/` folders still contain the previous Poly Haven CC0
placeholder assets used before the CS2 import path existed. They are retained for
asset provenance only and are not used by the current Dust2 scene.
