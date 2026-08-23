# Engine Overview

The Dhool Pro engine has two rendering paths and one deterministic game loop.

## Two renderers

The **CPU renderer** (`CpuRenderer`) is a software rasterizer. It produces the
same output on every platform. Use it for regression tests and frame comparisons.

The **GPU renderer** runs on WGPU (Vulkan, DX12, Metal, GLES, WebGPU). It uses
a deferred PBR pipeline with compute shaders for reflections, fog, and bloom.

## Engine phases

The engine runs in four phases each frame.

1. **Input poll.** The window system collects keyboard, mouse, and resize events.
2. **Fixed-update loop.** The scheduler runs `fixed_system` registrations at the
   configured fixed timestep. This runs zero, one, or several times per frame
   depending on elapsed time. Cloth simulation and physics run here.
3. **Per-frame update.** `per_frame_system` registrations run once per frame.
   The fly camera and animated lights update here.
4. **Render submission.** The engine synchronises the world state to the render
   scene and submits the WGPU command queue or calls the CPU rasteriser.

## Project settings

`ProjectSettings` holds configuration that cannot change after startup.

- `target_platform`: The target OS and graphics API family.
- `fixed_timestep`: Duration of one fixed-update tick (default 16.66 ms).
- `quality_tier`: Visual fidelity preset (Low to Cinematic).
- `render_resolution`: Internal rendering resolution.

```rust
use dhool_pro::{ProjectSettings, TargetPlatform, QualityTier};

let settings = ProjectSettings {
    target_platform: TargetPlatform::Current,
    fixed_timestep: std::time::Duration::from_secs_f32(1.0 / 60.0),
    quality_tier: QualityTier::High,
    render_resolution: (1280, 720),
};
```

## Engine builder pattern

`GameEngine::builder` returns a `GameEngineBuilder`. Call `.scene()` with a
`RenderScene`, then `.fixed_system()` or `.per_frame_system()` for each system,
then `.build()`.

```rust
let mut engine = GameEngine::builder(settings)
    .scene(scene)
    .fixed_system("move", move_system)
    .build()?;
```

The builder rejects missing scenes, invalid timing, and invalid resolution.
