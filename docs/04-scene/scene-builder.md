# Scene Builder

`SceneBuilder` constructs a `RenderScene` with meshes, materials, and lights.

## Create a builder

```rust
use dhool_pro::SceneBuilder;

let mut scene = SceneBuilder::new();
```

## Add a material

```rust
use dhool_pro::math::Color;
use dhool_pro::Material;

let mat = scene.add_material(
    Material::standard("metal", Color::rgb(0.5, 0.5, 0.5))
);
```

The `add_material` method returns a `MaterialId`. Use this ID to attach the
material to a mesh.

## Add a mesh

```rust
use dhool_pro::math::Vec3;
use dhool_pro::Mesh;

let mesh_id = scene.add_mesh(
    Mesh::cuboid("crate", Vec3::new(1.0, 1.0, 1.0)),
    mat,
);
```

Preset mesh constructors:

| Constructor | Shape |
|-------------|-------|
| `Mesh::cuboid(name, size)` | Box with six faces |
| `Mesh::sphere(name, radius, segments)` | UV sphere |
| `Mesh::plane(name, width, height, seg_x, seg_y)` | Flat grid |
| `Mesh::from_gltf(path)` | Load from a glTF 2.0 file |

## Add a light

```rust
use dhool_pro::scene::LightKind;
use dhool_pro::Light;

scene.add_light(Light::directional(
    "sun",
    Vec3::new(0.2, -1.0, -0.3),
    Color::WHITE,
    2.5,
));
```

| Constructor | Light Type | Notes |
|-------------|------------|-------|
| `Light::directional(name, dir, color, intensity)` | Sun light | Always casts shadows |
| `Light::point(name, pos, color, intensity, range)` | Point light | Local falloff |
| `Light::spot(name, pos, dir, color, intensity, range, angle)` | Spot light | Cone attenuation |

## Set the environment

```rust
use dhool_pro::scene::{EnvironmentEncoding, EnvironmentMap, EnvironmentProfile};

scene.set_environment(EnvironmentMap {
    hdr_panorama: image,
    encoding: EnvironmentEncoding::LatLong,
    profile: EnvironmentProfile {
        sky_tint: Color::WHITE,
        fog_color: Color::rgb(0.4, 0.45, 0.5),
        fog_density: 0.015,
        ambient_intensity: 0.3,
    },
});
```

## Build the scene

```rust
let render_scene = scene.build()?;
```

`build()` returns a `RenderScene`. If the scene has no camera, no meshes, or
invalid materials, `build()` returns a `SceneBuildError`.
