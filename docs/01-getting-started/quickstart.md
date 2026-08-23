# Build Your First Game

This tutorial builds a game that moves a red cuboid across the screen.

## Prerequisites

- Rust 1.81 or later.
- A GPU with Vulkan 1.2, DX12, or Metal support.

## Step 1: Add the dependency

```toml
[dependencies]
dhool-pro = { git = "https://git.jr4.in/jR4dh3y/dhool-pro" }
```

## Step 2: Create a scene

A scene holds meshes, materials, and lights.

```rust
use dhool_pro::math::{Color, Vec3};
use dhool_pro::{Light, Material, Mesh, SceneBuilder};

let mut scene = SceneBuilder::new();

// Add a red PBR material.
let material = scene.add_material(
    Material::standard("player_mat", Color::rgb(0.8, 0.2, 0.1))
);

// Add a cuboid mesh.
scene.add_mesh(Mesh::cuboid("player", Vec3::ONE), material);

// Add a directional light.
scene.add_light(
    Light::directional("sun", Vec3::new(0.2, -1.0, -0.3), Color::WHITE, 2.5)
);

let render_scene = scene.build()?;
```

## Step 3: Build the engine

```rust
use dhool_pro::{FrameInput, GameEngine, ProjectSettings};

let settings = ProjectSettings::default();
let mut game = GameEngine::builder(settings)
    .scene(render_scene)
    .build()?;
```

## Step 4: Register a system

A fixed system runs at the project fixed timestep.

```rust
use dhool_pro::SystemContext;

builder = builder.fixed_system("move-player", |ctx: &mut SystemContext| {
    let player = ctx.world.find_by_name("player")
        .ok_or("missing player")?;
    let transform = ctx.world.transform_mut(player)
        .ok_or("missing transform")?;
    transform.position.x += ctx.time.delta_seconds;
    Ok(())
});
```

## Step 5: Advance frames

```rust
let frame = game.frame(FrameInput::default())?;
println!("Entities: {}", game.world.entity_count());
```

## Step 6: Run the live renderer

Pass the built engine to `run_live_game`.

```rust
use dhool_pro::gpu::{run_live_game, LiveOptions};

run_live_game(game, LiveOptions::default())?;
```

## Complete example

See `examples/custom_game.rs` in the repository.
