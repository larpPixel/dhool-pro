# Pipeline Overview

The WGPU renderer uses a deferred shading pipeline with compute-based screen
space effects.

## Pass order

The render graph executes passes in this order.

```
Depth Prepass -------> G-Buffer ------> Hi-Z ------> SSR ------> Volumetric Fog
                          |
                          v
                    Light Volumes
                          |
                          v
                    Deferred PBR Resolve
                          |
                          v
                    3D Rain System
                          |
                          v
                    Dual Bloom
                          |
                          v
                    Tone Mapping
                          |
                          v
                    SMAA T2x
                          |
                          v
                    UI Overlay -> Present
```

## Pass descriptions

| Pass | Resolves | Purpose |
|------|----------|---------|
| Depth Prepass | `D32` | Write occluder depths, reject early-Z |
| G-Buffer | `Albedo`, `Normals`, `ARM`, `Velocity`, `MaterialID` | Store surface properties |
| Hi-Z | `R32` depth mips | Hierarchical depth for culling and SSR |
| SSR | Reflection image | Screen-space ray-traced reflections |
| Volumetric Fog | Fog accumulation | Froxel-based in-scattering |
| Light Volumes | `D32S8` | Stencil-filtered light influence |
| Deferred PBR | Beauty HDR | Cook-Torrance BRDF resolve |
| Rain 3D | Beauty HDR | 4096 world-space rain particles |
| Bloom | Bloom texture | Progressive dual-filter glow |
| Tone Map | LDR sRGB | GT tonemapping + auto-exposure |
| SMAA T2x | Final presentation | Morphological + temporal AA |

## CPU renderer

The `CpuRenderer` implements the same PBR model in software. It runs
deterministically and produces bit-exact output across platforms. Use it for
regression tests and offline frame dumps.

```sh
cargo run --release --features demo-content -- --dump-ppm /tmp/frame.ppm
```
