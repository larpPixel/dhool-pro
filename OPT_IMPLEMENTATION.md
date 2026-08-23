# `opt.md` implementation ledger

This ledger maps the transcript to code that actually executes. `opt.md` also
contains opinions, examples from other engines, marketing suggestions, and
several proposals the speaker explicitly calls unfinished research. Those are
not relabeled as shipped features.

Status definitions:

- **Live**: executed by the wgpu renderer and validated on the Vulkan path.
- **Reference**: implemented by the deterministic CPU renderer/tooling.
- **Contract**: enforced by the render graph or asset validation.
- **Research**: the source does not define a production-ready answer.
- **Out of scope**: commentary rather than renderer behavior.

## Frame setup and geometry

| Requirement | Status | Evidence |
|---|---|---|
| Keep GPU-driven frame setup small | Live | Scene, animation, draw ordering, and batch creation stay CPU-side. GPU compute starts with explicit visibility/post work. |
| Clear only depth, coverage/motion, and resources that cannot be overwritten | Live + Contract | Attachment clears are limited to required depth/stencil, motion/coverage, and history-like targets. Full-screen outputs use overwrite/load semantics. Graph validation rejects suspicious clears. |
| Never draw geometry just to clear a target | Live | All required clears use attachment load operations. |
| Non-alpha selective depth prepass | Live + Contract | Only `prepass_eligible` opaque batches enter the vertex-only prepass. Asset validation rejects alpha-tested or motion-vector prepasses. |
| Alpha-tested geometry first, then non-prepassed opaque, then prepassed geometry | Live | The base pass has three explicit categories in that order. Batches are front-to-back within each category. |
| No opacity samples or pixel shader in the ordinary prepass | Live | `depth_prepass.wgsl` is vertex-only. Alpha opacity is sampled only by base and alpha-shadow pipelines. |
| Four ordinary texture samples, packed material textures, no runtime texture blending | Contract | Material and import validation enforce the sample budget, packed roles, bent normals, and baked blends. |
| Dedicated alpha opacity texture | Live + Contract | Alpha materials upload an R8 single-channel opacity mip chain (the uncompressed runtime equivalent of the required BC4 role); asset metadata requires BC4 source data. |
| Alpha-aware shadow maps without penalizing opaque casters | Live | Opaque casters use the vertex-only shadow pipeline; alpha casters use `alpha_shadow.wgsl` and the dedicated opacity texture. |
| Dual material profiles instead of a metallic-only interpretation | Live | A 256-entry dielectric/conductor GPU table is indexed by packed material ID. It also carries skin, hair, and foliage response metadata. |
| Skin transmission/wrapped diffuse and anisotropic hair response | Live + Reference | The live material-profile lighting path handles skin/foliage backscatter and a strand-oriented hair highlight; the CPU renderer retains class-specific Fresnel behavior. Eyes remain ordinary compact forward/deferred material geometry because the transcript calls advanced eye/hair work unfinished. |
| Fixed-step dynamic cloth geometry | Live + Reference | A shared deterministic Verlet/PBD solver handles pins, structural and bending constraints, wind, floor/sphere/box contacts, tangent-basis rebuilding, previous positions, GPU range uploads, motion vectors, and shadow-cache invalidation. Scene cloth is explicit mesh metadata rather than a renderer-specific effect. |
| LOD topology, two-position Bayer transitions, and synchronized shadow fades | Contract / missing runtime system | Import validation catches dense alpha cards and long thin triangles. This demo has no streaming runtime LOD set, so it does not claim live fades. |

## Depth, visibility, shadows, and lighting

| Requirement | Status | Evidence |
|---|---|---|
| Reverse-Z 32-bit depth | Live + Reference | Main depth is D32S8, shadow depth is D32, clear is zero, and comparisons are reverse-Z throughout. |
| MSAA depth/G-buffer and retained coverage | Live | The base pass uses a true 2x multisampled D32S8/albedo/normal/PBR/motion set. A custom per-sample resolve chooses the closest surface, averages matching samples, resolves D32, and writes R8 coverage. |
| Programmable sample locations | Research / API limitation | The transcript calls its pattern an early proposal. Portable wgpu does not expose programmable locations; the renderer fails clearly if the adapter cannot provide its required 2x D32 format support. |
| Hi-Z min/max/discontinuity data | Live + Reference | Resolved depth is copied to R32 and reduced into the half-resolution Hi-Z compute target; native reference tests cover reduction. |
| Prepass Hi-Z occlusion draw culling | Research | The source explicitly says this policy is under-researched. Hi-Z exists, but no speculative culling policy is claimed. |
| Focused/cached directional and local shadow maps | Live | Shadow cameras derive relevant bounds/range/FOV; static results cache after the first render; ground/floor casters are excluded. |
| Distance-dependent shadow resolution and full cascade/atlas system | Partial | Directional and local maps have distinct budgets and focused projections. A streaming cascade/atlas allocator is not present. |
| Hardware stencil-volume local lighting | Live | Main depth is D32S8. Instanced local-light bounds run a z-fail front/back increment/decrement volume pass, and the additive local-light resolve is rejected outside nonzero stencil. Directional/ambient lighting remains a single full-screen pass. |
| Noiseless shadow projection plus contact detail | Live + Reference | Cached raster shadow maps are combined with material/depth-aware screen-space contact visibility without a noisy temporal dependency. |
| AO/bent-normal/specular occlusion | Live + Reference | Live lighting derives stable screen-space/contact visibility from resolved D32, normals and material data; material cavity and bent-normal contracts feed specular occlusion. CPU debug views expose the individual terms. |
| Energy-normalized diffuse and scene-owned environment profiles | Live + Reference | CPU and GPU Burley diffuse use the same `1/pi` convention and restrained sky-hemisphere fill. Sky orientation/exposure, precipitation, wetness, wind, and exposure defaults live on `RenderScene`, so dry Dust2 and rainy monsoon rendering do not depend on hard-coded global shader weather. |
| Probe/RSM world-space GI | Reference / architecture only | The CPU renderer has probe-like indirect response and the graph no longer pretends a separate live GPU probe pass exists. A live RSM/probe streaming system is not implemented. |

## Reflections, atmosphere, and post

| Requirement | Status | Evidence |
|---|---|---|
| SSR first, software world-space tracing second, sky/proxy fallback last | Live | Half-resolution SSR precedes 70% software reflection supplementation and proxy/sky fallback. |
| Two-frame reflection history with rejection | Live | Reflection history ping-pongs and rejects by motion, depth, and roughness. |
| Warn when SSR is slower than software tracing | Reference + live measurement | The production gate evaluates CPU-reference reports. `--profile-gpu` now emits real GPU stage timestamps; individual SSR/software queries remain a finer-grained future split. |
| Half-resolution blue-noise fog | Live + Reference | Compute fog runs at half resolution and is bilinearly merged during tone mapping. |
| Depth-tested world-space rain and reactive wetness | Live | Instanced rain prisms use resolved reverse-Z depth and shared light/shadow/weather data. Wet materials, impacts, and reflections use the same field. |
| Diffuse-only exposure, bounded white balance, artist compensation, temporal adaptation | Live | Ping-pong 1x1 exposure history uses diffuse illuminance from HDR alpha, artist min/max/EV/balance controls, separate brighten/darken rates, and measured frame rate. |
| GT toe/linear/shoulder mapping and post-lighting bloom after velocity compensation | Live + Reference | Bloom extracts only energy above a soft HDR threshold, caps extreme source luminance, and uses a compact bilinear Gaussian kernel. This keeps thin practical lights smooth without turning them into broad colored blobs; the deterministic reference retains its small C++ Kawase kernel. The GT resolve merges bloom at restrained intensity with fog, exposure, white balance, and camera effects. |
| Single-direction depth-aware velocity compensation with separable smoothing | Live | Native-resolution motion/depth reconstruction uses a near-field camera cutoff, extreme-motion fallback, one-sided uniform samples, foreground rejection, and a separate perpendicular smoothing pass. Artist controls are CLI-exposed. |
| Rain presentation | Live | Depth-tested world-space rain keeps the original drop density across the expanded Dust II volume. Screen-space lens drops are deliberately excluded because they obscure the scene and duplicate the HDR image during composition. |
| Separate UI target and gamma-correct presentation | Live | SMAA writes an intermediate scene target; UI writes a separate linear RGBA8 target; the final pass composites premultiplied UI and applies user `--display-gamma`/`--display-brightness`. UI never contaminates temporal history. |
| Depth of field | Research | The transcript says DoF still needs work and supplies no validated tuning/quality policy. No placeholder blur is mislabeled as completion. |
| 64-pixel 3D LUT and BT.2020 experiments | Research | The source asks for later format/color-space experiments; the SDR path does not claim them. |

## Anti-aliasing

| Requirement | Status | Evidence |
|---|---|---|
| SMAA edge and blend stages | Live + Reference | Dedicated RG8 edge and RGBA16 blend targets are inspectable independently. |
| SMAA S2x/MSAA coverage integration | Live | Custom 2x coverage feeds edge detection, so partially covered geometry becomes a spatial edge even when luma contrast is weak. |
| T2x uses exactly the previous spatial result | Live + Reference | History stores current spatial SMAA, never the temporally blended presentation. Only detected edges accept history. |
| Motion/reprojection and responsive rejection | Live + Reference | Base-pass RG16 motion drives history lookup/rejection. Responsive materials reject temporal history. |
| Infinite TAA accumulation or temporal texture supersampling | Deliberately absent | The implementation preserves non-edge texture clarity and does not accumulate an indefinite history. |

## Workflow, profiling, and debugability

| Requirement | Status | Evidence |
|---|---|---|
| Named resources/passes with formats, scales, samples, dependencies, histories, and budgets | Contract | `RenderGraph` models these fields, including multisample count, and architecture tests validate the real live order. |
| GPU timing | Live | `--profile-gpu` enables adapter timestamp features, brackets six renderer stages, reads results back, and prints milliseconds. Unsupported adapters return an explicit error. |
| Debug views for depth, normals, material IDs, motion, shadows, AO, reflections, coverage, overdraw, topology, light volumes, and history | Reference + Contract | `DebugView::ALL`, required-view gates, gallery output, and renderer tests cover the declared set. Live capture verifies final presentation; a live interactive G-buffer selector is not claimed. |
| Artist controls | Live + Contract | Exposure bounds/EV, white-balance strength, velocity strength/cutoff, display gamma, and brightness are CLI-exposed. Import reports expose texture/topology/shadow/prepass budgets. |
| Cross-API/vendor performance claims | Out of scope | The renderer reports the selected wgpu backend and adapter but does not turn observations from other engines into universal claims. |
| Marketing scripts, Patreon instructions, and consumer advocacy | Out of scope | These are not engine behavior. |

## Remaining production work

These are real gaps, not hidden behind pass names:

1. Runtime streaming LOD selection with synchronized base/shadow Bayer fades.
2. A production probe/RSM GI streaming system and measured live comparison.
3. A full scalable directional cascade/atlas allocator with distance-driven map sizes.
4. Per-pass timestamp granularity for direct SSR-versus-software-RT comparison.
5. Interactive live debug-target selection rather than reference/gallery-only views.

The transcript's programmable sample-location layout, DoF policy, wide-gamut LUT
format, advanced eye model, and final subsurface approach remain research because
the source itself does not settle them.
