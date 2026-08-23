# Dhool Pro

Dhool Pro is a reusable Rust game engine crate. A game owns its scene, systems, and
typed resources. Dhool runs startup, fixed-update, and per-frame schedules, manages
the world and asset checks, and renders through either the deterministic CPU path or
the live vendor-agnostic `wgpu` path.

The rainy city and Dust2 are optional repository demos. A downstream game does not
compile or package those assets.

## Use Dhool in another project

Add the engine as a Git or path dependency. The default feature set contains the
engine and renderers but no bundled demo content.

```toml
[dependencies]
dhool-pro = { git = "https://git.jr4.in/jR4dh3y/dhool-pro" }
```

Build a scene, register systems, then advance the game from your platform loop:

```rust
use dhool_pro::math::{Color, Vec3};
use dhool_pro::{FrameInput, GameEngine, Light, Material, Mesh, ProjectSettings, SceneBuilder};

let mut scene = SceneBuilder::new();
let material = scene.add_material(Material::standard("player", Color::rgb(0.8, 0.2, 0.1)));
scene.add_mesh(Mesh::cuboid("player", Vec3::ONE), material);
scene.add_light(Light::directional("sun", Vec3::new(0.2, -1.0, -0.3), Color::WHITE, 2.0));

let mut game = GameEngine::builder(ProjectSettings::default())
    .scene(scene.build()?)
    .fixed_system("move-player", |context: &mut dhool_pro::SystemContext<'_>| {
        let player = context.world.find_by_name("player").ok_or("missing player")?;
        context.world.transform_mut(player).ok_or("missing transform")?.position.x +=
            context.time.delta_seconds;
        Ok(())
    })
    .build()?;
let frame = game.frame(FrameInput::default())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Systems run in registration order. Fixed systems use the project's fixed timestep.
Game-specific state belongs in `Resources`, keyed by its Rust type. The engine
provides typed `Events` queues for communication between systems; an event remains
queued until a system drains its type. The engine builder rejects missing scenes,
invalid timing, invalid resolutions, invalid asset contracts, and named system
failures instead of starting a partially configured game.

The complete downstream-style program is [`examples/custom_game.rs`](examples/custom_game.rs):

```sh
cargo run --example custom_game
```

Pass a built `GameEngine` to `run_live_game(game, options)` to run its schedules and
stream world changes into an interactive GPU window. Use
`run_live_renderer(scene, options)` when a static scene and free camera are enough.
Live rendering currently accepts one directional light and six local or spot lights;
it returns an explicit scene error above those limits. A live game may spawn and
remove meshes that use its startup material table, but it cannot add materials after
the GPU resources have been created. Light color and intensity can change at runtime.
Light types, order, and shadow flags are fixed after startup; local shadow-casting
lights also keep their startup transforms. The renderer reports an error if a game
breaks one of these GPU resource contracts.
[`examples/live_custom_scene.rs`](examples/live_custom_scene.rs) is the smallest
complete windowed client:

```sh
cargo run --example live_custom_scene
```

## Run the bundled renderer showcase

The live path uses `wgpu` with standard raster and compute shaders, not hardware
ray-tracing APIs or a vendor SDK. Enable `demo-content` when running repository
showcases:

## Run the live GPU renderer

```sh
cargo run --release --features demo-content -- --live
```

The default scene is a rain-soaked monsoon city scene assembled from vendored Poly
Haven CC0 models, PBR surfaces, a cloudy night HDR panorama, and project-local
generated facade/market textures.
The foreground alley opens into a wider cross-street, market lane, and distant
apartment skyline. Press `Space` to pause or resume the cinematic camera; hold
right mouse to mouselook; use `W`/`A`/`S`/`D` to move, `Q`/`E` to descend/ascend,
`Shift` to boost, arrow keys as keyboard look fallback, and `Escape` to exit.
Select a present-capable adapter by name when testing multiple GPUs:

```sh
DHOOL_GPU=AMD cargo run --release --features demo-content -- --live
DHOOL_GPU=NVIDIA cargo run --release --features demo-content -- --live
```

The GPU frame executes cached directional/local shadow maps, a vertex-only
partial depth prepass, hardware depth-tested PBR+motion G-buffer rasterization,
a fullscreen R32 depth copy, half-resolution Hi-Z/fog/SSR compute passes,
temporal reflections over simplified proxy geometry, light-volume guarded HDR
deferred lighting with screen-space occlusion, world-space 3D rain with a
generated streak-detail atlas, textured puddle normals and reflection breakup,
energy-capped Gaussian bloom, GT tone mapping, and a final SMAA T2x-style
edge/persistence resolve. Bloom stays close to clipped highlights while the
world-space lights, shadows, and reflections carry the scene illumination.
Materials use mipmapped base-color, tangent-space normal, and packed
AO/roughness/metallic maps.

Lighting and weather come from scene-owned environment data. One 2:1 panorama
drives the visible sky, spherical-harmonic diffuse light, roughness-filtered
reflections, fog color, rain fill, and white balance in both renderers. The
monsoon city uses the bundled Kloppenheim cloudy-night HDR with rain and wet
surfaces; Dust2 preserves its exported Source 2 panorama and dry daylight
settings. Exposure and yaw are part of the map, so the visible clouds and their
light cannot drift apart. The shared Burley diffuse path remains
energy-normalized, keeping bright surfaces detailed without crushing shaded
alleys.

Scene meshes can opt into the shared fixed-step cloth component. The same
deterministic solver drives the world/reference path and live GPU vertex uploads,
including pin constraints, wind, floor/sphere/box collision, rebuilt tangent
bases, motion vectors, and shadow-cache invalidation. The monsoon and Dust2
scenes each include a tessellated cloth awning as a runtime validation fixture.

![Rainy monsoon city live GPU scene](demo-output/aaa-live.png)

The asset bundle is checked in for deterministic offline runs. Refresh the
vendored upstream assets from the recorded manifest with:

```sh
scripts/fetch_aaa_assets.sh
```

## Run the reference frame

```sh
cargo run --release --features demo-content -- --dump-ppm frame.ppm
```

## Build the complete demo gallery

```sh
cargo run --release --features demo-content -- --gallery demo-output
python3 -m http.server 8080 -d demo-output
```

Open `http://localhost:8080`. By default the gallery exports diagnostics for
the same `assets/aaa` monsoon city scene used by `--live`: 17 production views
(beauty plus 16 diagnostics) and `timings.txt` with per-pass CPU measurements
and warnings.

Asset origins, licenses, and refresh commands are recorded in
[`ASSET_SOURCES.md`](ASSET_SOURCES.md). For the deterministic CPU glTF showcase,
pass another core glTF 2.0 asset:

```sh
cargo run --release --features demo-content -- --gallery demo-output --asset path/to/model.gltf
```

## Verify

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features --all-targets
DHOOL_GPU=AMD cargo run --release --features demo-content -- --live --frames 4
DHOOL_GPU=NVIDIA cargo run --release --features demo-content -- --live --frames 4
```

Use a fixed camera and an overlay-free capture when comparing lighting changes:

```sh
DHOOL_GPU=AMD cargo run --release --features demo-content -- --live --map dust2 --static-camera \
  --frames 40 --capture /tmp/dhool-dust2.png
```

The raw analyses in `files/` directly inform the implementation: `vendor-agnostic.txt`
defines the compute-reflection direction; `crisis3.txt` and `hla.txt` inform the
depth-only prepass; `fox.txt` informs the deferred layout; `1.txt` informs contact
shadows; and `things.txt` informs the AA and GT tone mapping. CPU gallery timings remain CPU
measurements and are not directly comparable to the GPU targets in `gospol.md`.
