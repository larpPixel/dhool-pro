# How to Create a Custom Scene

This guide shows how to build a scene with the `SceneBuilder` API.

## Step 1: Create a new project

```sh
cargo new my_game
cd my_game
```

Add the dependency to `Cargo.toml`.

```toml
[dependencies]
dhool-pro = { git = "https://git.jr4.in/jR4dh3y/dhool-pro" }
```

## Step 2: Build a scene

```rust
use dhool_pro::math::{Color, Vec3};
use dhool_pro::{Light, Material, Mesh, SceneBuilder};

let mut scene = SceneBuilder::new();

// Ground plane.
let ground_mat = scene.add_material(
    Material::standard("ground", Color::rgb(0.3, 0.3, 0.35))
);
scene.add_mesh(Mesh::plane("ground", 20.0, 20.0, 1, 1), ground_mat);

// Central crate.
let crate_mat = scene.add_material(
    Material::standard("crate", Color::rgb(0.8, 0.6, 0.3))
);
scene.add_mesh(Mesh::cuboid("crate", Vec3::splat(1.0)), crate_mat);

// Directional light.
scene.add_light(Light::directional(
    "sun",
    Vec3::new(0.5, -0.8, -0.3),
    Color::WHITE,
    3.0,
));

// Environment.
let env_image = dhool_pro::scene::TextureImage::from_hdr("assets/env.hdr")?;
scene.set_environment(dhool_pro::scene::EnvironmentMap {
    hdr_panorama: env_image,
    encoding: dhool_pro::scene::EnvironmentEncoding::LatLong,
    profile: dhool_pro::scene::EnvironmentProfile {
        sky_tint: Color::WHITE,
        fog_color: Color::rgb(0.5, 0.5, 0.6),
        fog_density: 0.01,
        ambient_intensity: 0.3,
    },
});

let render_scene = scene.build()?;
```

## Step 3: Run the live renderer

```rust
use dhool_pro::gpu::{run_live_renderer, LiveOptions};

run_live_renderer(render_scene, LiveOptions::default())?;
```

## How to customise

- Change `Material::standard(name, colour)` arguments.
- Add more lights with `scene.add_light()`.
- Load glTF models with `Mesh::from_gltf("path/to/model.gltf")`.
- Replace the environment map with your own HDR panorama.
