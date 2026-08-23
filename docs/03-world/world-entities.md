# World and Entities

The `World` struct stores all entities and their components.

## EntityId

`EntityId` is a handle to an entity. It is a compact `u64` that the world
internally maps to a component storage row.

```rust
use dhool_pro::world::{EntityId, World};

let mut world = World::new();
let player: EntityId = world.spawn("player");
```

## Transform

Every entity has a `Transform` component with position, rotation, and scale.

```rust
use dhool_pro::math::Vec3;
use dhool_pro::world::Transform;

let transform = world.transform_mut(player).ok_or("missing transform")?;
transform.position = Vec3::new(1.0, 0.0, 0.0);
transform.rotation = glam::Quat::IDENTITY;
transform.scale = Vec3::ONE;
```

## MeshRenderer

A `MeshRenderer` component attaches a mesh and a material to an entity.

```rust
use dhool_pro::world::MeshRenderer;

world.insert(player, MeshRenderer {
    mesh_handle: mesh_id,
    material_handle: material_id,
});
```

## LightComponent

A `LightComponent` makes an entity act as a light source.

```rust
use dhool_pro::world::LightComponent;
use dhool_pro::scene::LightKind;

world.insert(player, LightComponent {
    kind: LightKind::Directional {
        direction: Vec3::new(0.0, -1.0, 0.0),
    },
    color: Vec3::new(1.0, 1.0, 1.0),
    intensity: 2.5,
    range: 100.0,
});
```

## CameraRig

`CameraRig` controls the view and projection for the active camera.

```rust
use dhool_pro::world::CameraRig;

let rig = CameraRig {
    eye: Vec3::new(0.0, 2.0, 5.0),
    target: Vec3::ZERO,
    up: Vec3::Y,
    fov_y_radians: std::f32::consts::FRAC_PI_4,
};
world.set_camera_rig(rig);
```

## Physics and colliders

The world supports `RigidBody` (kinematic velocity) and `Collider` (sphere, box)
for deterministic fixed-step physics.

```rust
use dhool_pro::world::{Collider, RigidBody};

world.insert(player, RigidBody { velocity: Vec3::ZERO, gravity_scale: 1.0 });
world.insert(player, Collider::Sphere { radius: 0.5 });
```
