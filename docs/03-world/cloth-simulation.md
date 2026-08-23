# Cloth Simulation

Dhool Pro uses a Verlet integration solver for cloth simulation. The solver is
deterministic. It produces the same result on every platform.

## ClothDefinition

Define a cloth with resolution, size, and constraints.

```rust
use dhool_pro::cloth::{ClothDefinition, ClothSettings, ClothPin};

let cloth = ClothDefinition {
    name: "awning",
    width_segments: 16,
    height_segments: 12,
    width: 4.0,
    height: 3.0,
    settings: ClothSettings {
        structural_stiffness: 0.8,
        bending_stiffness: 0.1,
        damping: 0.05,
        gravity: 9.81,
        wind_force: Vec3::new(2.0, 0.0, 0.5),
        relaxation_iterations: 4,
    },
    pins: vec![
        ClothPin { segment_x: 0, segment_y: 0, anchored: true },
        ClothPin { segment_x: 16, segment_y: 0, anchored: true },
    ],
};
```

## Solver steps

The solver runs inside the fixed-update loop.

1. Apply gravity and wind to each vertex.
2. Enforce distance constraints between connected vertices.
3. Apply damping to reduce oscillation.
4. Resolve collisions with floor, sphere, and box colliders.
5. Recalculate tangent bases for normal mapping.
6. Write the new vertex positions to the GPU staging buffer.

## Colliders

`ClothColliderSnapshot` captures the transform of a collider at solver time.

```rust
use dhool_pro::cloth::ClothColliderSnapshot;

let cloth_collider = ClothColliderSnapshot::Sphere {
    center: Vec3::new(0.0, 1.0, 0.0),
    radius: 0.5,
};
```

## ClothError

The solver returns `ClothError` for invalid configurations.

| Error | Cause |
|-------|-------|
| `TooManyVertices` | Cloth exceeds the 4096 vertex limit |
| `DegenerateMesh` | Cloth has zero area or overlapping vertices |
| `ConstraintOverflow` | Distance constraint list exceeds the buffer |

## GPU synchronisation

After the solver finishes, the engine uploads the new vertex positions to a GPU
buffer. Shadow maps that cover the cloth invalidate on position change.
