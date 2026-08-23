# Profiler

`EngineProfiler` measures per-pass CPU and GPU timing. Use it to find
performance bottlenecks before merging changes.

## FrameBudget

A `FrameBudget` asserts that passes complete within their time budget.

```rust
use dhool_pro::profiler::{EngineProfiler, FrameBudget, FrameReport};

let profiler = EngineProfiler::new(
    FrameBudget {
        total_ms: 16.66,         // 60 FPS budget
        pass_ms: 8.0,            // Max per-pass
        gpu_ms: 14.0,            // Max GPU frame time
    },
);
```

## FrameReport

After each frame, the profiler returns a `FrameReport`.

```rust
let report: FrameReport = engine.frame(FrameInput::default())?.report;

println!("Frame {:.1} ms", report.frame_ms);
for pass in report.passes {
    println!("  {}: {:.2} ms", pass.name, pass.cpu_ms);
}
```

## GateFailure

When a pass exceeds its budget, the profiler records a `GateFailure`.

```rust
use dhool_pro::profiler::GateFailure;

match report.gate_result {
    Some(GateFailure::OverBudget { pass, cpu_ms, budget_ms }) => {
        eprintln!("{pass} over budget: {cpu_ms:.1} > {budget_ms:.1} ms");
    }
    _ => {}
}
```

## GPU profiler

Pass `--profile-gpu` to print GPU timestamp queries per pass.

```sh
cargo run --release --features demo-content -- --live --profile-gpu
```

## Debug views

Use `--gallery demo-output` to export all 19 debug view images as PNG files.

```sh
cargo run --release --features demo-content -- --gallery demo-output
```

The gallery directory contains:

| File | Purpose |
|------|---------|
| `beauty.png` | Final tonemapped frame |
| `depth.png` | Linearised depth buffer |
| `normals.png` | World-space normals |
| `roughness.png` | Per-pixel roughness |
| `material-id.png` | Material class IDs |
| `shadow-mask.png` | Shadow attenuation |
| `ambient-occlusion.png` | SSAO factor |
| `motion-vectors.png` | Screen velocity |
| `reflections.png` | SSR + environment reflection |
| `edge-stencil.png` | SMAA edge detection mask |
| `overdraw.png` | Fragment invocation count |
| `texture-count.png` | Texture sample density |
| `shadow-casters.png` | Shadow culling volumes |
| `light-volumes.png` | Local light influence bounds |
| `history-validity.png` | Temporal reprojection confidence |

## Quality tiers

| Tier | Shadow Map | SSR Steps | SSAO Samples | Fog Grid | VRAM |
|------|-----------|-----------|-------------|----------|------|
| Low | 1024 | 16 | 8 | 80x45x32 | ~250 MB |
| Medium | 2048 | 32 | 16 | 120x67x48 | ~450 MB |
| High | 2048 | 64 | 24 | 160x90x64 | ~700 MB |
| Ultra | 4096 | 128 | 32 | 200x112x64 | ~1.2 GB |
| Cinematic | 4096 | 256 | 64 | 240x135x96 | ~2.0 GB |
