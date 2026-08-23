# Scene Data Model

`RenderScene` is the immutable description of a frame. The engine converts the
`World` into a `RenderScene` before each render submission.

## Mesh

```rust
use dhool_pro::scene::{Mesh, Vertex};

let mesh = Mesh {
    name: "player",
    vertices: vec![...],
    indices: vec![0, 1, 2, ...],
    material_id: Some(material_id),
};
```

## Vertex format

| Channel | Type | Purpose |
|---------|------|---------|
| `position` | `[f32; 3]` | Object-space position |
| `normal` | `[f32; 3]` | Object-space normal |
| `tangent` | `[f32; 4]` | Tangent vector (w is handedness) |
| `texcoord` | `[f32; 2]` | UV texture coordinates |
| `color` | `[f32; 4]` | Per-vertex colour |

```rust
let vertex = Vertex {
    position: [0.0, 0.0, 0.0],
    normal: [0.0, 1.0, 0.0],
    tangent: [1.0, 0.0, 0.0, 1.0],
    texcoord: [0.0, 0.0],
    color: [1.0, 1.0, 1.0, 1.0],
};
```

## Material

```rust
use dhool_pro::scene::{Material, MaterialClass, MaterialFlags};

let material = Material {
    name: "stone",
    class: MaterialClass::StandardOpaque,
    flags: MaterialFlags::CASTS_SHADOW,
    base_color: Vec4::new(0.8, 0.7, 0.6, 1.0),
    roughness: 0.9,
    metallic: 0.1,
    ambient_occlusion: 1.0,
    emissive: Vec4::ZERO,
};
```

### Material classes

| Class | Behaviour |
|-------|-----------|
| `StandardOpaque` | Full deferred PBR, depth tested |
| `AlphaTest` | Discards fragments below `alpha_cutoff` |
| `Emissive` | Adds self-illumination to the beauty pass |
| `Cloth` | Uses cloth-specific BRDF parameters |

## Light

```rust
use dhool_pro::scene::{Light, LightKind};

let light = Light {
    name: "lamp",
    kind: LightKind::Point {
        position: Vec3::new(0.0, 3.0, 0.0),
    },
    color: Vec3::new(1.0, 0.7, 0.3),
    intensity: 50.0,
    range: 12.0,
    casts_shadow: true,
};
```

## Environment map

`EnvironmentMap` holds a high-dynamic-range panorama that drives sky, diffuse
irradiance (spherical harmonics), and filtered specular reflections.
