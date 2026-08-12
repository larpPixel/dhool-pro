# `opt.md` implementation ledger

This file is the coverage ledger for the pipeline described by `opt.md`. The
transcript mixes production recommendations, workflow requirements, examples,
opinions, marketing advice, platform-specific observations, and proposals that
the speaker explicitly labels as unfinished research. Treating all of those as
equally implementable would hide gaps rather than close them.

Status meanings:

- **Live**: exercised by the wgpu renderer in `src/gpu.rs` and its shaders.
- **Reference**: exercised by the deterministic CPU renderer or engine tooling.
- **Contract**: represented and validated as an engine/asset/render-graph rule.
- **Research**: the transcript explicitly leaves the technique unresolved; it
  is not presented as production-ready behavior here.
- **Not applicable**: commentary or a platform/product path outside this demo.

## Frame setup, resources, and visibility

| Transcript requirement | Status | Implementation |
|---|---|---|
| Keep start-of-frame GPU work small | Contract | The baseline graph contains only explicit visibility setup; animation and scene management remain CPU-side. |
| Clear depth and motion/coverage, but do not clear fully overwritten G-buffers or post targets | Live + Reference | Reverse-Z depth and motion are cleared; G-buffer and full-screen overwrite targets use load/overwrite behavior. The CPU reference resets depth, coverage, and velocity without wiping every material field. Graph validation rejects suspicious clears. |
| Never rasterize a triangle merely to clear a target | Contract | No clear-only draw exists; required clears are attachment load operations. |
| Partial depth prepass contains only large/expensive opaque occluders | Live + Contract | `prepass_eligible` batches use the depth-only pipeline. Asset validation rejects alpha-tested prepass participation. Scene tests constrain the selected occluders. |
| No opacity sampling, motion vectors, or pixel shader work in the ordinary prepass | Live + Contract | The depth pipeline is vertex-only. Motion is emitted by the base pass. |
| Render alpha-tested/non-prepassed geometry before prepassed geometry in the base pass | Live | Base-pass batches execute `false` then `true` for `prepass_eligible`, leaving prepassed expensive surfaces last. |
| Front-to-back/coverage-aware ordering and quad-overdraw diagnostics | Contract + Reference | Scene authoring exposes prepass eligibility; asset/profiler rules cover texture count and prepass misuse; CPU debug views expose overdraw and quad utilization. |
| Reverse-Z with 32-bit hardware depth | Live + Reference | Main camera and shadow cameras use reverse-Z, D32 clears to zero, compare operations are `GreaterEqual`, and all depth consumers use the same convention. |
| Hi-Z min/max/discontinuity resource | Live + Reference | R32 depth copy feeds the compute Hi-Z pass; the reference kernel and architecture tests validate reduction behavior. |
| Occlusion culling derived from prepass Hi-Z | Research | The transcript calls this under-researched. Hi-Z is available, but the renderer does not claim an unvalidated draw-culling policy. |

## Materials, geometry, and asset workflow

| Transcript requirement | Status | Implementation |
|---|---|---|
| Four ordinary texture samples maximum | Contract | Import budgets, material descriptors, profiler warnings, and tests enforce/report the limit. |
| No runtime texture blending; bake object-specific blends | Contract | `MaterialDesc` rejects runtime blending and the import workflow produces packed material textures. |
| Separate BC4 opacity for alpha-tested depth/shadow work | Contract | Asset validation requires a dedicated BC4 opacity role. |
| BC7 normal/bent-normal plus compact base/AO and specular/roughness packing | Contract | Asset descriptions and validation encode the packing policy. |
| Geometry cutouts near the camera, stable LODs farther away, dithered transitions and shadows | Contract | Asset validation/debug contracts cover topology and LOD state; the demo has no runtime streaming LOD system and therefore does not falsely claim live transitions. |
| Material-class/ID-driven shading instead of an extra metallic G-buffer | Live + Reference | The packed PBR/material fields and material flags drive glass, emissive, metal, wetness, and contact behavior. |
| Artist-visible texture, topology, shadow, reflection-proxy, and prepass budgets | Contract | Asset pipeline validation and profiler warnings surface these decisions rather than silently accepting them. |
| Dust2 source import and packed PBR conversion | Live workflow | `scripts/import_cs2_dust2.sh`, `src/gltf_import.rs`, and the asset documentation cover import, material assignment, texture limits, and provenance. |

## Shadows, lighting, and reflections

| Transcript requirement | Status | Implementation |
|---|---|---|
| Perspective/local shadow maps sized to relevant light bounds | Live | Spot-light shadow cameras use reverse-Z perspective projection and per-light range/FOV. |
| Directional shadow bounds focus on relevant scene/camera geometry | Live | The directional shadow camera derives focused bounds from nearby scene vertices. |
| Cache static shadow maps and avoid rendering ground/floor casters | Live + Contract | Directional/local cache flags prevent redundant updates; caster classification and validation exclude inappropriate floor/ground geometry. |
| Restrict local lighting/shadow evaluation to useful pixels | Live + Contract | Depth/Hi-Z, range/cone rejection, local shadow arrays, contact shadows, and graph stencil-light contracts bound work. The current wgpu path emulates the restriction in shader logic because it has no D24S8 stencil volume pass. |
| Albedo-independent lighting signal and material-aware resolve | Live + Reference | The CPU path separates irradiance before albedo. Live lighting stores material-independent diffuse illuminance in HDR alpha for exposure reduction while RGB retains the resolved material response. |
| AO/bent-normal and material-depth contact shadowing without mandatory TAA | Live + Reference | Depth/normal/PBR inputs drive deterministic SSAO/contact logic; debug contracts expose both independently. |
| SSR first at half resolution, software tracing only as a supplement, cubemap/sky fallback last | Live | Half-resolution SSR dispatch precedes 70%-resolution software reflection supplementation and fallback sampling. |
| Two-frame reflection history with motion/roughness rejection | Live | Ping-pong reflection history uses current motion and roughness rejection. |
| Flag SSR when it costs more than software RT | Reference + Contract | CPU pass reports and profiler policy emit the warning; live timestamp-query reporting remains future backend instrumentation. |

## Atmosphere and post processing

| Transcript requirement | Status | Implementation |
|---|---|---|
| Half-resolution blue-noise volumetric fog | Live + Reference | Compute fog runs at half resolution and is bilinearly composited during tone mapping. |
| Depth-tested volumetric rain and lighting-reactive wet surfaces | Live | Instanced rain prisms depth-test against reverse-Z and share the weather/light/shadow field with impacts, puddles, and reflections. |
| Half/quarter-resolution composite effects rather than native full-screen effect buffers | Live | SSR/fog are half resolution; bloom is quarter resolution and merged with fog in the tone-map resolve. |
| Kawase-style bloom with no needless output clear | Live + Reference | Extract and separable blur overwrite reusable reduced-resolution targets and resolve with tone mapping. |
| GT-style toe/linear/shoulder tone curve | Live + Reference | WGSL and native kernel implementations share the same curve family. |
| Diffuse-only auto-exposure and automated white balance with artist bounds/bias | Live | A one-workgroup reduction computes geometric-mean exposure from material-independent diffuse illuminance, excludes emissive/specular color, derives bounded white-balance gains from scene illuminants, and feeds the GT resolve. Exposure and balance ranges are deliberately bounded; per-scene artist bias is not yet exposed. |
| 64-pixel 3D LUT and BT.2020 experiments | Research | The transcript asks for later format/color-space testing. No arbitrary LUT is shipped as if validated. |
| Depth-of-field architecture | Research | The transcript describes near/far half-resolution buffers but leaves quality policy and tuning unresolved; the demo does not implement cinematic DoF. |

## Anti-aliasing and motion

| Transcript requirement | Status | Implementation |
|---|---|---|
| SMAA edge detection and blend stages with independently inspectable edge data | Live + Reference | Dedicated edge and blend targets/passes exist; tests and debug coverage ensure detectable output. |
| T2x uses only the immediately previous spatial SMAA frame, never an accumulated temporal result | Live + Reference | The AA shader writes `spatial_resolve` to history and only uses the previous history texture for presentation. CPU persistence similarly stores the unblended current frame. |
| Temporal work only enhances detected edges | Live + Reference | Both paths gate history blending by the SMAA/custom edge mask; non-edge texture detail is not temporally accumulated. |
| Motion vectors are produced in the base pass and used for reprojection/rejection | Live | RG16 motion is cleared each frame, written with G-buffer material outputs, and consumed by reflection and AA history. |
| Responsive masks for unstable particles/effects | Live + Reference | `MaterialFlags::responsive_aa` is packed into the normal/material flag channel; live and CPU T2x paths reject history on flagged pixels. Emissive demo materials opt in. |
| Programmable sample-location SMAA/MSAA hybrid and saved coverage history | Research | The speaker explicitly presents this as an early proposal requiring further R&D. wgpu also does not expose portable programmable sample locations. |
| Single-direction, depth-aware velocity compensation with separable smoothing | Research | The transcript explicitly says sample count, trail length, tiling, and plausibility require dedicated R&D. No conventional symmetric blur is substituted and mislabeled as complete. |

## Tooling, validation, and non-applicable material

| Transcript requirement | Status | Implementation |
|---|---|---|
| Named graph resources/passes, formats, scales, dependencies, history and budgets | Contract | `RenderGraph` models and validates these fields; the baseline architecture test guards it. |
| Debug views for depth, normals, material data, overdraw, topology, shadows, reflections, AA and history | Reference + Contract | `DebugView`, profiler required-view coverage, and tests enumerate and render the complete declared set. |
| Asset and pass warnings for known anti-patterns | Contract | The profiler and asset pipeline flag excess textures, alpha prepass, bad clears, floor shadows, missing reflection proxies, and incomplete debug coverage. |
| D3D11/D3D12/Vulkan comparisons and vendor marketing claims | Not applicable | This renderer uses wgpu's available backend and does not encode unverified cross-API performance claims. |
| Marketing templates, Patreon/chart instructions, consumer advocacy, and studio policy commentary | Not applicable | These are not engine behavior. |

## Open production gaps

The following are the only transcript-derived items that should be described as
unfinished production work rather than shipped behavior:

1. Per-scene artist temperature bias and temporally smoothed eye-adaptation
   controls on top of the live bounded auto-exposure/white-balance result.
2. Native GPU timestamp/pipeline-statistics capture that can compare live SSR
   and software tracing cost instead of relying on the reference profiler.
3. A complete runtime LOD system with synchronized base/shadow dithered fades.

The motion-blur, programmable-sample-location, 3D-LUT/BT.2020, DoF, occlusion
culling, and several MSAA proposals stay in the research tier because the
source itself does not define a validated production result. They require
measured research, not placeholder effects.
