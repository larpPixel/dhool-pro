# Diffuse glow surface ledger

User-visible behavior: practical lights illuminate nearby surfaces naturally, with a restrained optical halo that neither bands nor dominates the scene.

| Surface | Applies | Source | Required change | Check | State |
|---|---|---|---|---|---|
| Entry points | yes | `--live` GPU renderer | Keep all live scenes on the shared bloom chain | 40-frame fixed-camera AMD Vulkan capture | verified |
| Clients | yes | WGPU presentation path | Preserve HDR bloom through tone mapping and SMAA | Live shader contract test and AMD Vulkan capture | verified |
| Providers | yes | `src/shaders/bloom.wgsl`, `src/shaders/tonemap.wgsl`, and `src/gpu.rs` | Use a compact Gaussian blur, soft HDR threshold, source-energy cap, and restrained blend | Naga shader validation and live GPU pipeline creation | verified |
| Contracts | yes | `src/graph.rs` | Keep the diffuse Gaussian chain and quarter-resolution RGB11F output | Graph validation test | verified |
| Reverse state | no | Bloom has no user-controlled state transition | No reverse action exists | Source inspection | not applicable |
| Connection modes | yes | AMD and NVIDIA through vendor-agnostic WGPU | Use only standard filtered texture sampling | AMD RADV Vulkan runtime; NVIDIA has no present-capable adapter in this session | verified on AMD |
| Tests | yes | `tests/renderer.rs`, `tests/architecture.rs` | Lock in the threshold, cap, intensity, Gaussian chain, and graph pass | 45 tests passed; 1 heavy Dust2 import ignored | verified |
| Documents | yes | `README.md`, `OPT_IMPLEMENTATION.md` | Describe the restrained energy-capped implementation | Diff inspection | verified |

## Reference comparison

- [Unreal Engine](https://dev.epicgames.com/documentation/en-us/unreal-engine/bloom-in-unreal-engine) separates bloom intensity and threshold from emissive power, and warns that excessive emissive bloom can overwhelm the image.
- [Unity URP](https://docs.unity3d.com/Packages/com.unity.render-pipelines.universal@14.0/manual/post-processing-bloom.html) exposes threshold, intensity, scatter, clamp, and high-quality filtering. [Unity's light-fixture guide](https://docs.unity3d.com/2018.3/Documentation/uploads/ExpertGuides/Create_High-Quality_Light_Fixtures_in_Unity.pdf) identifies glowing blobs, mismatched emissive and light intensity, missing fixture occlusion, and excess indirect contribution as realism failures.
- [Godot](https://docs.godotengine.org/en/stable/classes/class_environment.html) exposes an HDR threshold, bleed scale, luminance cap, and a default glow intensity of `0.3`, confirming that source selection and energy limits belong beside blur width.

## Runtime result

The AMD RADV Vulkan path presented 40 fixed-camera frames at 35.9 FPS and captured `/tmp/dhool-natural-lighting.png`. The cyan and amber tubes retain clipped bright cores with a compact soft edge; the earlier billboard-sized colored halos are absent. The host's inactive NVIDIA adapter was not present-capable, so the same runtime check did not run there.
