# Runtime and Scheduler

The scheduler manages when systems execute.

## Schedule

`Schedule` is a list of `GameSystem` registrations. The engine has one schedule
with two phases.

- **Fixed systems** run at the project fixed timestep. Use these for physics and
  deterministic gameplay.
- **Per-frame systems** run once per presented frame. Use these for camera
  movement and UI.

```rust
use dhool_pro::runtime::{GameSystem, SystemContext, SystemResult};

fn my_system(ctx: &mut SystemContext) -> SystemResult {
    // Read or mutate the world.
    let count = ctx.world.entity_count();
    println!("{count} entities");
    Ok(())
}
```

## SystemContext

`SystemContext` gives a system access to the world, resources, events, and time.

| Field | Type | Purpose |
|-------|------|---------|
| `world` | `&mut World` | Entity-component storage |
| `resources` | `&mut Resources` | Typed resource map |
| `events` | `&mut Events` | Typed event queues |
| `time` | `Time` | Delta time and fixed timestep info |

## Resources

`Resources` stores game-specific state by Rust type.

```rust
struct Score(i32);

// Write a resource.
resources.insert(Score(42));

// Read the resource.
if let Some(score) = resources.get::<Score>() {
    println!("Score: {}", score.0);
}
```

## Events

`Events` provides typed event queues. An event stays in the queue until a system
drains its type.

```rust
struct DamageEvent { amount: f32, target: EntityId }

// Write a damage event.
events.write(DamageEvent { amount: 10.0, target: player });

// Read damage events.
for event in events.read::<DamageEvent>() {
    apply_damage(event.target, event.amount);
}
```

## Time

`Time` holds timing information for the current frame.

| Field | Type | Purpose |
|-------|------|---------|
| `delta_seconds` | `f32` | Wall-clock seconds since the last frame |
| `fixed_delta_seconds` | `f32` | Fixed timestep duration |
| `frame_count` | `u64` | Total frames since engine start |
| `elapsed` | `Duration` | Total elapsed wall-clock time |
