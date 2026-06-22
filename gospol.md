# Vendor-Agnostic Real-Time Rendering Engine
## Full Technical Specification — Final Draft
### Synthesized from: Threat Interactive Pipeline Analyses
#### Sources: Vendor-Agnostic RT Demo · Clair Obscure E33 · Crisis 3 MSAA · Oblivion Remake · Fox Engine (MGSV) · Half-Life Alyx · Days Gone · Misc Graphics Topics


## Part 1 — Foundation and Hardware Philosophy

This engine is a direct counter-movement to the rendering philosophies documented as failures across the reference analyses. The UE5 look — Lambert diffuse, flickering Lumen, noisy Nanite, broken shadows, stochastic virtual texture sampling, and TAA-masked rendering defects — is specifically what this engine exists to disprove. The marketing-driven dependency on vendor-locked hardware ray tracing, DLSS, and FSR is likewise rejected at the architectural level. These technologies are not forbidden because they are technically uninteresting. They are rejected because they make the portable, vendor-agnostic rendering path feel like a second-class citizen rather than the primary product.

The governing tenet is that modern GPUs at Shader Model 5.0 and above possess enormous raw parallel processing power through compute shaders that developers routinely leave untapped when they rely on fixed-function hardware pipelines. The reference vendor-agnostic RT demo proved this in practice: an unoptimized software ray traced pipeline running at 70% native resolution outperformed Nvidia's Star Wars hardware RT demo running at quarter resolution with DLSS enabled. The performance gap was not marginal. The unoptimized vendor-agnostic approach ran around three times faster in a significantly larger environment. The only honest conclusion is that hardware RT, as currently marketed, is expensive, resolution-compromised, and vendor-locked — and software RT through compute shaders is a viable, superior alternative on any GPU that supports it.

The single most important design rule is this: if a feature only looks correct after heavy temporal smoothing, it is not a finished feature. The correct response is to fix the source problem. Masking broken shadows with blurry TAA, hiding noisy GI behind aggressive temporal accumulation, and smearing stochastic texture sampling into acceptability through reconstruction are all forms of the same mistake. This engine does not make that mistake.

Performance is part of image quality. A renderer that only looks good when it misses frame time is not visually successful in production. Stable frame time enables higher sample stability, better motion clarity, more consistent input response, and less aggressive reconstruction. GPU time, memory bandwidth, draw count, dispatch count, texture sample count, and overdraw are therefore treated as image-quality variables — not just engineering metrics.

Vendor independence is enforced at the design level. Hardware ray tracing, proprietary denoisers, vendor upscalers, and frame generation are permitted as optional accelerators in enhanced configurations, but the baseline rendering identity cannot depend on any single hardware company. The portable compute path must be good enough to represent the product honestly on any Shader Model 5 GPU.

Artists need control but not self-destructive freedom. The engine exposes controls for material identity, shadow behavior, reflection proxies, opacity, LODs, and surface classification. Simultaneously, the engine prevents bad defaults from becoming a project's visual identity. A material system that silently allows ten texture samples in the base pass is a tool that has already failed the production, regardless of how expressive it is in other respects.

The fifth principle is motion accountability. A feature does not pass review because it looks correct in a still capture. It must survive camera motion, object motion, disocclusion, particle interaction, low light, high contrast, reflective surfaces, and thin geometry. The renderer makes that standard normal, not exceptional.

### What Hardware Actually Does

Three hardware behaviors underpin every optimization decision in this engine. Understanding them at the mechanism level is what separates correct rules from cargo-culted rules.

The first is Z-testing. When a mesh is rendered behind an already-rasterized pixel, fixed-function hardware compares the incoming depth value against the stored value and rejects the invocation before any pixel shader executes. This rejection is essentially free — it costs almost nothing compared to the shader work it prevents. Every prepass strategy, every front-to-back draw ordering decision, every Hi-Z generation pass, and every occlusion culling dispatch exists to maximize the yield of this one behavior. Nanite's software rasterizer cannot access hardware Z-testing. The Oblivion Remake analysis measured occluded software-rasterized draws at 2,900% less efficiency than standard Z-tested draws. That is not an edge case. It is a structural consequence of how the software rasterizer works, and it is why micropolygon systems are fundamentally anti-performance regardless of how well the content is authored.

The second is quad utilization. The GPU processes pixel shaders in 2×2 pixel quads. If a triangle edge cuts through a quad, the entire quad is invoked but only some pixels produce valid results. Long thin triangles, dense alpha-tested foliage near screen edges, and poorly topologized meshes waste significant shader invocations this way. MSAA amplifies this cost — every additional sample multiplies the invocation count for edge quads. The correct response is not to avoid MSAA but to optimize topology so MSAA amplification is proportional to visible geometric detail rather than wasted geometry.

The third is memory bandwidth. Buffer precision decisions, texture compression choices, GBuffer format choices, and resolve strategy all express themselves as bandwidth constraints before they become shader costs. A 16-bit albedo buffer saves bandwidth at the cost of corrupted MSAA edges. A BC5 normal map preserves precision but samples more expensively than a BC7 normal map carrying the same data plus bent normals. Every format decision must be evaluated against what it costs to read and what quality it preserves at edges, not just what it stores.

---

## Part 2 — Frame Budget Targets

The primary target is native 1080p at 60 FPS in worst-case gameplay scenes on mainstream PC hardware, giving a 16.67ms GPU budget. The reference vendor-agnostic demo ran at approximately 9 to 10ms on equivalent hardware even in its unoptimized state with debugger overhead. That is the standard to beat, not the ceiling.

| Pass | Target Range (ms) | Notes |
|---|---|---|
| Visibility, culling, frame setup | 0.5 – 1.0 | Render graph execution, Hi-Z, proxy scene update |
| Partial prepass | 0.2 – 0.5 | Vertex-only, no alpha tests, no pixel shaders |
| Main geometry and material pass | 1.5 – 3.0 | Base pass, motion vectors, decals |
| Shadow updates | 0.5 – 2.0 | Cached static plus dynamic lights only |
| AO and indirect occlusion | 0.3 – 0.8 | Screen-space plus bent normals composite |
| Screen-space contact shadows | 0.1 – 0.3 | Precomputed material depth |
| Indirect lighting | 0.5 – 1.5 | Probe or baked with dynamic occlusion |
| Direct lighting | 0.5 – 1.5 | Stencil-restricted per-light passes |
| Reflections (SSR + software RT) | 0.5 – 2.0 | SSR first, RT supplements |
| Volumetrics and atmosphere | 0.3 – 1.0 | Blue noise fog |
| Translucency and particles | 0.3 – 1.5 | After volumetrics |
| Tone mapping, bloom, color, lens | 0.3 – 1.0 | Kawase bloom, GT tone mapper |
| Anti-aliasing and reconstruction | 0.2 – 1.0 | SMAA compute plus persistence |
| UI and presentation | 0.1 – 0.3 | |
| **Total Target** | **≤ 16.6ms** | |

These are pressure targets, not laws. A pass may exceed its range only if the scene demonstrably requires it and the player-facing return is worth the cost. The draw cost baseline applies throughout: optimized deferred draws must achieve approximately 1 microsecond per 1,000 pixels rendered. Tiny draws carry a hard floor of 9 to 15 microseconds regardless of pixel count. The Oblivion analysis found a 5,000-pixel draw costing 80 microseconds — sixteen times the optimized ratio — because it fetched two 5,100×5,100 BC3 textures for an asset occupying less than one screen percent. That class of error must be surfaced automatically.

---

## Part 3 — Render Graph and Backend

The render graph is mandatory and is the accountability system for the entire renderer. Every pass, resource, queue, dependency, barrier, aliasing opportunity, load operation, store operation, clear, discard, and history dependency must be represented in the graph. Each pass carries its name, owner, feature category, resolution scale, target formats, inputs, outputs, history dependencies, temporal dependency level, vendor-specific dependency level, and timing budget. A production capture must let an engineer explain the frame from first clear to final present without guessing why an unnamed buffer exists.

Resource clears must be intentional and justified. The reference RT demo wasted performance clearing G-buffers that the base pass overwrites entirely on every frame. The R8 shadow mask must not be cleared when stencil-based light projection is active — doing so destroys the shadow masking logic mid-frame. Only the hardware depth buffer, volumetric fog resources, and the motion vector buffer require explicit frame-start clears. Every other clear is suspect until proven necessary.

The backend targets Direct3D 12 and Vulkan 1.3 or 1.4. It must expose explicit resource states, queue ownership, descriptor indexing or bindless equivalents, pipeline caching, MSAA sample positions, conservative rasterization, hardware ray tracing where available, timestamp queries, pipeline statistics, and crash diagnostics. Conventional rendering paths must exist and be complete before advanced scheduling systems such as Work Graphs become dependencies. Modern API features are welcome when they are production-stable and broadly available on PC hardware; they are not welcome as requirements that break the fallback path.

---

## Part 4 — Geometry and Visibility Pipeline

### The Default Path

The default path for game geometry is optimized hardware rasterization through conventional vertex and pixel shader workflows. Mesh shaders, meshlets, visibility buffers, and software rasterizers are tools to evaluate when they prove measurable benefit, not a philosophy to adopt because they appear modern. They must beat optimized conventional content in direct comparison, not neglected content.

The Oblivion Remake analysis is the standing evidence against premature adoption of visibility-buffer systems. Nanite's software rasterizer, invoked for small triangles to sidestep quad bottlenecks, completely loses hardware Z-testing efficiency. Occluded software-rasterized geometry processed at 2,900% less efficiency than standard Z-tested rendering. Nanite also forces material shading to behave as though unoptimized quad overdraw is present regardless of how well the content is authored. Geometric inefficiencies amplify directly into shadow passes because every vertex rasterized in the base pass may need to be rasterized again from each light's perspective. Nanite's cluster-bound failures guarantee cumulative overdraw equivalent to rasterizing the visibility buffer twice in typical scenes, and shadow passes inherit that cost in full.

Micropolygon geometry also introduces subpixel aliasing that demands blurry TAA to suppress — which blurs the very detail micropolygons were meant to provide. The only exit from that loop is to not enter it.

### Partial Depth Prepass

The prepass is strictly vertex-shader-driven. Its sole job is to populate the hardware depth buffer so Z-testing rejects expensive material pixel shaders behind already-rasterized geometry. Every other function assigned to the prepass is a regression.

The prepass draws large, expensive objects above a configurable screen-footprint threshold that considers both size and surface area. It draws terrain and large world sections with major screen coverage — the Fox Engine analysis identifies terrain as the ideal arbitrary classification for prepass inclusion because its occluder quality is reliable. It draws any object whose materials exceed four texture samples and any object whose expensive surface area risks being overwritten by later base pass draws.

What the prepass never draws: alpha-tested content of any kind, motion vectors, pixel shaders of any kind, small objects, or alpha-heavy meshes. Including alpha-tested foliage in the prepass doubled its cost with 2× MSAA in Crisis 3, because the foliage drew multiple 4K×2K texture atlases that needed at minimum BC4 single-channel compression for the opacity data. The principle is absolute — alpha-tested content does not belong in any depth prepass. Assigning motion vectors to the prepass converts a fast vertex-driven pass into a pixel-shader-heavy pass. The Oblivion Remake measured a 28.5% prepass performance gain from removing velocity calculation, at a cost of only 1.6% in the base pass. Motion vectors belong in the base pass exclusively.

Pre-passed objects must render last in the base pass, not mid-pass. The reference RT demo had pre-passed objects rendering mid-pass, allowing later draws to overwrite 21% of the most expensive material surface area. With correct end-of-pass ordering, Z-test rejection from the prepass depth data limits expensive material shading to the final visible pixel set. The reference demo's prepass provided a 7% whole-frame gain (~1ms) with incorrect draw ordering. Correct ordering should improve that meaningfully.

The Half-Life Alyx prepass is the benchmark standard: a 4× multisampled prepass covering 100,000+ triangles costs under 0.5ms on a desktop 3060. Crisis 3 and Jedi Survivor prepasses climb above 1ms because alpha-tested content and unnecessary pixel shaders are included. The difference is not hardware — it is what goes into the pass.

### Base Pass

Materials write to deferred G-buffers. Draw order is front-to-back to maximize Z-test rejection from prepass depth and from prior draws. Animated objects additionally write motion vectors at the end of the pass as an RG16 output.

The hard texture limit is four samples per material draw. Draws accessing eight to ten textures perform 50% slower than the optimized ratio. No ordinary PBR material requires more than four samples. Complex or hero materials may justify additional samples but must document the reason and accept profiler scrutiny.

Runtime texture blending within the material shader is prohibited. UE5's terrain performs runtime blending across multiple high-resolution atlases and runs five times slower than equivalent eighth-generation terrain rendering as measured in the Oblivion analysis. All blending must be baked per-object.

### Topology

Broad flat surfaces use max-area topology — large triangles that avoid wasted quads on flat regions. Curved regions and silhouettes use appropriately denser topology, balanced by efficient surrounding flat surfaces. The Yuan Hamus analysis confirms that max-area topology does not conflict with Fresnel-based edge normal correction AA because the technique only affects curved surface topology. The asset tools flag long thin triangles, high quad overdraw, dense alpha cards, and meshes that are cheap in vertex count but expensive in pixel cost.

LOD transitions use cross-fade dissolve. No visible pop is acceptable. The Fox Engine uses a BGR88 Bayer matrix across nearly every material for LOD transitions, hair fade, and alpha-tested edges — an 8×8 Bayer dither used in place of alpha-to-coverage, which abuses FXAA's subpixel blending property for edge cleanup. Shadow-specific LODs and reflection-specific LODs are required separately from base LODs. Dense geometry must not serve as a reason to abandon LOD discipline.

### Decals

Decals use mesh-based projection on already-rasterized geometry using depth information to clip to the surface — the Frostbite and CryEngine pattern. A mesh volume renders over existing geometry with no dedicated prepass and no dedicated G-buffer deferred pass. UE5's cube-projected decal system forces a full prepass and a dedicated buffer pass for every project because its lighting techniques require it. That approach is prohibited as a default.

---

## Part 5 — G-Buffer Layout

### Design Intent

The G-buffer achieves three goals simultaneously: supporting correct deferred MSAA without corrupted edges, providing enough per-pixel data for rich material lighting, and being bandwidth-efficient at the lighting stage. Fox Engine's approach — computing diffuse lighting without the albedo buffer and applying albedo only at the resolve stage — is the most significant single GBuffer bandwidth optimization in the reference material and directly informs this layout.

| Buffer | Format | Primary Contents | Alpha Contents |
|---|---|---|---|
| Albedo | RGBA8 | Base color (RGB) — 24-bit minimum | Opacity reserved for skin and plant materials |
| Normals | RGBA16 | World-space normals packed | Material flags and surface class |
| PBR Data | RGBA8 | Roughness (R), Specular intensity / IOR proxy (G), Cavity (B) | Material ID — 8-bit, 256 classes |
| Depth | R32F | Float copy of native 24-bit hardware depth | — |
| Hi-Z | RGBA16 | Min/max 2×2 pixel depth, depth discontinuity | — |
| Motion Vectors | RG16 | Screen-space velocity for animated objects only | — |
| Shadow Mask | R8 array | Per-light shadow data, one index per shadow-casting light | — |

### Format Precision Requirements

Albedo must be at least 24-bit. Crisis 3's 16-bit albedo compression caused color corruption at MSAA sample edges because MSAA resolves texel values at geometric boundaries, and compressed 16-bit base color produced tinted fringing — green and purple artifacts on geometry edges. This was directly traced to the albedo compression scheme and is resolved by RGBA8 minimum.

Normals require at least 24 to 30 bits. Crisis 3's 17-bit normal packing produced inaccurate subsample shading results and contributed to specular instability at MSAA edges. RGBA16 with material flags in the alpha is the correct format. Material flags belong in the normal map alpha because glass and window surfaces share one flag value, concrete and curb materials use a secondary value, and the alpha channel otherwise goes unused.

No metallic buffer is required. Fox Engine demonstrated that metallic behavior can be fully expressed through the material ID lookup table and the specular intensity channel. A traditional 8-bit metallic channel is redundant when the ID table carries colored specular, F0, and IOR per material class.

The R8 shadow mask must not be cleared when stencil-based light projection is active. Clearing it destroys the masking data the shadow projection invocations depend on.

### Fox Engine Innovation: Albedo-Free Lighting Stage

Fox Engine's most significant GBuffer optimization is computing diffuse lighting without albedo as a shader input. The lighting stage operates on normals, roughness, specular intensity, AO, shadow data, and material IDs — but not base color. Albedo is blended in only during the lighting resolve stage.

This has three practical benefits. It eliminates albedo bandwidth from the lighting pass entirely. It enables auto-exposure analysis from a pure diffuse luminance signal without albedo contaminating the exposure measurement. And it allows the lighting resolve to simultaneously blend fog lookup tables and AO alongside albedo compositing, producing the final tone-mapped image in a single combined pass with pixel luminance stored in the alpha channel for bloom extraction, eliminating the need for a separate luminance pass.

This engine adopts this pattern. The lighting stage reads normals, roughness, cavity, material IDs, shadow masks, and depth. Albedo is applied at the resolve stage alongside AO and fog compositing.

### Material ID Lookup Tables

Rather than storing specular color, F0, IOR, anisotropy, subsurface profile, wetness, cavity scaling, and reflection response per-pixel in the GBuffer, a compact 8-bit material ID indexes into a small per-frame lookup table carrying all enriching parameters for that class. This is the Fox Engine approach: 256 possible material classes, each with its own entry in the lookup table, indexed for free from the existing PBR data alpha channel.

A material ID entry carries colored specular for metals like gold where specular color differs from albedo, the correct F0 or IOR value for Fresnel behavior, anisotropy parameters with hair IDs using anisotropic shading, the subsurface scattering profile index, a wetness modifier, a cavity scaling factor, the reflection response class, and the BRDF variant selector. The specular intensity channel in the PBR Data buffer then acts as a per-pixel modulator on the ID-provided specular color, handling cavity maps and specular occlusion masks without requiring additional GBuffer channels. Fox Engine's implementation uses this system across every material in the scene, not just hero assets, and the result is specular occlusion richness that "puts visuals far ahead of most modern titles" according to the analysis.

### Depth Copy and Hi-Z Generation

After the base pass, the native 24-bit hardware depth is copied to the R32F buffer via a full-screen triangle pixel shader — not a compute shader. This R32F buffer feeds Hi-Z generation and the software ray tracing pass. The R32F is then downscaled to half-resolution RGBA16 containing minimum, maximum, and 2×2 pixel discontinuity data, and further downscaled to quarter-resolution for tiled light culling inputs.

---

## Part 6 — Lighting Architecture

### Diffuse Model

The standard material path uses Burley (Disney) diffuse. Lambert diffuse is available only for low-spec or stylized materials. Lambert lacks the directional retroreflection behavior of rough surfaces under indirect light and produces flat, plasticky results that characterize hundreds of Unreal Engine shipped titles. Brian Karis pushed Lambert as the Unreal default and it is directly responsible for the pervasive synthetic look in modern games. Frostbite papers and Fox Engine content both demonstrate measurably richer indirect lighting from more accurate diffuse models. Burley is the minimum acceptable baseline for anything claiming physical plausibility.

### Specular Model

The specular path uses GGX microfacet BRDF with roughness-driven lobe width, Schlick Fresnel approximation adjusted per material class via the ID lookup table, colored specular for metals, anisotropy for hair and brushed metal IDs, clearcoat where required by the material class, and specular occlusion from bent normals and cavity. The Fox Engine analysis confirms that specular occlusion through cavity maps is not a luxury — it is the difference between plausible and implausibly glowing materials. This must be a default behavior, not an optional one.

### Stencil-Based Light Volume Culling

Each shadow-casting local light follows a specific stencil workflow. First, a geometry volume representing the light's casting range is rendered into the hardware depth stencil channel, optionally refined with a second volume draw. The stencil then restricts shadow map projection pixel shader invocations to pixels within the light volume, and light shading invocations are restricted to pixels tagged by the stencil.

Fox Engine's stencil-based local light shading is significantly more efficient than UE5's approach. UE5's DX12 lighting draws without stencil are 50% more expensive. Even with stencil, UE5 pays near-full-screen costs because the lighting bound is evaluated against camera view rather than visible lit surface area. Fox Engine scales cost with actual lit surface area, not light volume size, which is why local shadowed lights in Fox Engine shade extremely fast despite writing to two large RGBA16 outputs — the stencil prevents wasted invocations on every unlit pixel.

### Tiled Light Culling

Tile-based light list generation via compute shader, fed by the quarter-resolution Hi-Z depth discontinuity buffers. Tile culling determines which lights affect which screen-space tiles before the main lighting draw executes. The reference RT demo showed that this geometry volume-based culling method is faster than pure compute-based alternatives.

### Main Lighting Draw

A single multi-purpose shader handles direct lighting, indirect lighting composite, SSDO composite, and RT reflection buffer composite in one pass. Output is native RGBA16. A secondary RG11B10 is produced for skin subsurface scattering — if no skin materials are present its cost is negligible.

The compositing order for SSDO is critical and non-negotiable. SSDO composites after the albedo/specular resolve, never before. Crisis 3 composited AO before the lighting resolve and it destroyed subsample lighting data at MSAA edges, causing uncanny fringing even when full subsample shading was forced. The correct sequence is: compute direct lighting with subsampling, resolve albedo and specular with albedo blended into lighting per the Fox Engine pattern, then composite SSDO and fog onto the resolved buffer simultaneously.

### Emissive Materials

Emissive materials render as a separate pass after the main lighting draw. Emissive output must not affect the alpha channel of the lighting buffer, which stores pixel luminance for bloom extraction.

### Tone Mapping

Tone mapping is not an artistic filter. It is the technical foundation that determines whether physically based lighting, material color, reflections, exposure, bloom, and color grading remain believable. A badly configured tone mapper destroys correct PBR work after the renderer has done expensive accurate shading — the Gran Turismo developers' conclusion and the Fox Engine reverse engineer's conclusion are identical on this point: "Tone mapping is not an artistic choice. It is the technical foundation for actual artistic changes like color grading."

The Gran Turismo 7 GT tone mapper is explicitly the recommendation. The things.txt analysis compares ACES 1, ACES 2, AgX, PBR Neutral, and GT across multiple exposure and scene conditions. GT preserves color richness under overexposure, retains material detail in highlights, maintains correct color gradients in sky and bright materials, and avoids the muted color appearance that consistently appears in ACES 1/2 and AgX under various exposure conditions. UE5 uses ACES 1. GT is measurably closer to photographic reference and to the tone mapping evidence found in Fox Engine's pipeline, which reverse engineers believe is extremely similar to the GT mapper — a likely contributor to MGSV's competitive realism relative to contemporary titles.

---

## Part 7 — Reflection Architecture

### The Layered Stack

Reflections operate as a layered hierarchy in a fixed priority order. SSR runs first at half resolution, covering all screen-space information across high and low roughness surfaces. Software RT supplements SSR at approximately 70% native resolution for low-roughness surfaces where SSR fails due to off-screen data limitations. Cubemaps serve as fallback data only, never as the default answer for important reflective materials. The two active passes must not run redundantly on the same pixels — SSR covers its half-resolution region and software RT supplements where SSR cannot.

### Screen-Space Reflections

SSR must be high quality and cost-controlled. The reference RT demo's SSR was completely unoptimized and exceeded the cost of the software RT pass — a direct pipeline error with no valid justification. The Fox Engine (MGSV) SSR operates at half resolution on high-roughness surfaces with no noise, on a smaller evaluated surface area than the reference demo, and the visual quality "almost borders on screen-space GI territory." Optimized SSR implementations in Phantom Pain, Frostbite titles, and Crisis 3 are the benchmarks. SSR handles high-roughness reflections that UE5 SSR cannot support even on surfaces with several times higher specular value and much lower roughness, making the cost comparison even more damning.

SSR must support depth-aware resolve, roughness-aware filtering, zero stochastic noise in the output, clean off-screen rejection, and specular occlusion integration via the stencil mask saved before tracing. If SSR cost exceeds software RT cost at any point during production, the profiler flags it as a pipeline error. There is no valid scenario where SSR should cost more than compute ray tracing.

### Software Ray Tracing via Compute Shaders

The software RT pass is implemented entirely in compute shaders with no hardware BVH, no DXR, and no vendor RT APIs, compatible with any Shader Model 5 GPU. The reference demo proved this at scale.

Tracing happens at 70% of native resolution. Checkerboard tracing at 50% is prohibited. At 50%, the buffer is three times smaller, spatial upscaling produces excessive pixelation, and four frames are required to temporally resolve — which never achieves stable results in normal gameplay motion. At 70% native only two frames are required, and spatial coverage pairs cleanly with reconstruction upscaling without visible pixelation during camera motion. This was the specific resolution chosen by the demo's engineers and it is the correct balance.

The ray budget at 70% native on target hardware is 1 to 2 rays per pixel before denoising. The world-space proxy geometry used for tracing must be simple enough for the BVH to traverse within that budget. Large occluders — buildings, vehicles, dumpsters, mailboxes, doors — must be represented in world space. Emissive objects must be represented even if physically small, because directly lit and emissive portions dominate viewer attention in reflected surfaces and siphon the eye. Proxy geometry uses vertex-color-only data with no textures. Static structures are baked into larger combined proxy objects to reduce instance overhead. Dynamic objects such as doors, vehicles, and characters retain individual proxy structures. Color evaluation is skipped on proxy portions not in direct lighting, which provides a meaningful performance saving with minimal visual cost. The target complexity is closer to Crisis Remastered eighth-generation console reflection geometry than to any full-PBR scene representation.

All ray sampling uses blue noise. White noise and IGN are prohibited. IGN produces visible repeating diagonal patterns that TAA cannot suppress without blurring to the point of destroying detail. The Dead Space Remake implementation is the reference for blue noise RT sampling.

### Glass and Transparent Reflections

Glass and window surfaces are identified by their material flag in the normal map alpha channel. When a pixel is flagged as glass, the compute shader applies interlaced vertical line tracing: even vertical lines trace exterior reflections — sky, street, neon signs, outside geometry — while odd vertical lines trace interior reflections such as blinds or interior geometry that does not appear in the main G-buffers. Both passes feed the temporal reconstruction step, which resolves the alternating data into a single stable output. Interior geometry absent from the G-buffers must exist in the simplified world-space proxy scene.

### Temporal Reconstruction and Denoiser

The denoiser is semi-independent and must not rely on TAA to clean up noise. History rejection triggers on fast camera motion or surface disocclusion detected via depth and motion vector mismatch. Two-frame accumulation produces two RGBA16 outputs — one resolved result and one saved for the next frame's reconstruction input — followed by a spatial upscale from 70% to native resolution with a roughness-related parameter stored in the alpha for compositing.

The temporal component must survive fast camera pans, object motion, disocclusion, glass surface movement, and emissive changes without ghosting or lag. Disocclusion detection via motion vector and depth mismatch is the correct mechanism. A denoiser that relies on TAA to hide its failures is not a finished denoiser.

---

## Part 8 — Shadow Pipeline

### Why Shadows Are Expensive in Modern Engines

Shadows are expensive because every triangle in the main render may need to be rasterized again from each light's perspective. Unoptimized asset geometry, missing shadow atlas management, and absent caster classification multiply this base cost. In E33, shadows cost 2ms — more than three times what optimized PC games achieve. Days Gone and Half-Life Alyx handle shadows in approximately 0.6ms with better visual quality. The primary causes measured: drawing floors into shadow maps, Nanite's geometric inefficiency propagating into shadow passes, low shadow resolution compensating for geometry issues, and missing caching.

### Atlas Management

A 4K×4K D32 shadow map atlas with two reserved 256×256 spaces for specific light types. Targeted quad draws clear only changed atlas regions rather than clearing the full atlas every frame. Sky and cloud shadows write into index 0 of the R8 shadow mask array.

### Caster Classification

Artists classify shadow behavior per object through a set of explicit categories: main shadow casters that render into full cascades, contact-only casters that render only into the near cascade, distant-only casters, first-person casters with a dedicated near pass, non-shadow casters excluded entirely, and shadow proxies using low-poly stand-ins for complex mesh shadow casting. Ground and floor meshes are excluded from all shadow passes regardless of category classification. Drawing floor geometry into shadow maps was a primary measured contributor to E33's 2ms shadow cost, and every optimized reference title in the analysis excludes horizontal large-surface-area geometry from shadow passes entirely. This exclusion is mandatory at all quality levels.

### Stencil Restriction Per Light

Each light's shadow projection invocations are restricted by stencil. A geometry volume representing the light's casting range is rendered into the hardware depth stencil channel, optionally refined with a second volume draw. The stencil restricts shadow map projection pixel shader invocations to only lit pixels. The result writes to the corresponding R8 shadow mask index.

### Caching

Static lights and static casters cache aggressively and only re-render on geometry or material change. Dynamic sun requires per-frame cascade updates. Interior static lights rarely require per-frame updates. Cache invalidation must be traceable in the render graph so accidental every-frame updates are visible.

| Quality Level | Atlas | Update Policy | Floor Exclusion |
|---|---|---|---|
| Low | 2K×2K | Cached static only | Mandatory |
| Medium | 4K×4K | Mixed cache | Mandatory |
| High | 4K×4K | Per-frame dynamic | Mandatory |

---

## Part 9 — Occlusion, Bent Normals, and Screen-Space Contact Shadows

### Multi-Scale Occlusion

Occlusion operates at multiple scales simultaneously. Object AO is baked per-asset and grounds assets in their environment. Bent normals pre-calculate the least-occluded direction within a hemisphere boundary for specular occlusion and indirect light direction. Small-radius screen-space AO handles creases and contact occlusion. Larger-radius SSDO provides broad environmental grounding and color bleeding. A single AO technique applied uniformly is always the wrong answer — GTAO-style methods produce wide haloing on thin occluders when implemented naively, and HBAO-style methods are stronger for small-radius work. The engine combines both by scale.

### Bent Normals

Bent normals pre-calculate the least-occluded direction within a hemisphere boundary for each surface texel. They require two extra texture channels and are typically limited to hero assets in most productions. This is a production mistake with measurable consequences. The things.txt analysis is explicit: the specular leaking that causes modern visuals to implausibly glow comes directly from missing bent normals. Object AO alone is insufficient for specular occlusion — bent normals are required for the specular occlusion term to be meaningful. The Fox Engine deploys them broadly across all materials, not just heroes, and the specular quality across ordinary environment assets reflects that decision. Bent normals are stored in the BC7 normal texture alongside tangent-space normals as described in the texture packing section, so the two extra channels cost nothing additional in the packing scheme.

### Screen-Space Contact Shadows

Days Gone introduced screen-space shadows using precomputed depth information baked into material textures. Traditional shadow maps exclude many small objects and fine geometric details to save performance — floors, small props, and high-poly meshes are common exclusions. Screen-space contact shadows recover this micro-detail without re-rendering complex geometry. Artists bake depth information into a material texture channel. The shadow shader uses the G-buffer depth and the baked material depth together to compute plausible contact shadow detail. Texture manipulation is faster than rendering high-poly geometry, so micro-detail like seams, surface irregularities, and contact edges gain dynamic light interaction at very low cost.

Bend Studio released their Unreal implementation and Epic integrated it because the performance benefit was too strong to ignore. The current UE integration relies on temporal jitter to hide artifacts. This engine's implementation targets full TAA-independence — other implementations have demonstrated this is achievable.

### SSDO Compositing Order

SSDO composites after the albedo/specular resolve, never before. Compositing AO before the resolve destroys subsample lighting data. This is the exact failure in Crisis 3 that caused uncanny MSAA edges even when full subsample shading was forced. The color bleeding pass produces its output from a 50-microsecond albedo downsample, though lit scene downsampling produces higher quality color bleeding than albedo buffer downsampling and is the preferred approach where affordable.

---

## Part 10 — Indirect Lighting

The baseline indirect lighting path combines baked or probe-based lighting, dynamic occlusion, bent normal influence on probe sampling direction, spherical harmonic occlusion, and screen-space enhancement. This path is preferred over noisy full-scene GI that only becomes acceptable after aggressive temporal accumulation. Dynamic GI may exist in the enhanced tier but must be judged harshly: it must converge under a still camera, remain stable during camera motion, survive disocclusion, handle emissives and particles without smearing, and not flicker or produce unstable artifacts at any point during normal gameplay. Lumen fails all of these criteria. GI that fails these standards belongs in the experimental tier until it does not.

Probe and atlas systems must be fully inspectable through the render graph. Probe placement, update frequency, occlusion influence, sky contribution, dynamic object influence, and upsampling behavior must all be visible. A half-resolution buffer upscaled naively producing visible low-resolution blotches on indirectly lit objects is a renderer defect, not an acceptable cost of probe-based lighting.

---

## Part 11 — Materials and Texture Packing

### Material Identity

A renderer cannot build a realistic image from generic base color, roughness, and a normal map. The full required material property set is: base color, roughness, specular intensity or IOR proxy, colored specular via material ID, cavity, object AO, bent normals, optional material depth for screen-space contact shadows, material ID, anisotropy via ID, skin behavior via ID, hair behavior via ID, wetness via ID, clearcoat via ID, glass behavior via ID, and reflection response class via ID. The material ID system carries most enriching parameters without requiring additional GBuffer channels — the per-pixel cost is a single 8-bit index.

### Texture Packing Scheme

| Slot | Format | Contents |
|---|---|---|
| 1 | BC7 | Tangent-space normals (RG) and bent normals (BA) |
| 2 | BC3 | Base color (RGB) and AO or material depth (A) |
| 3 | BC3 | Specular masks and roughness or height data |
| 4 (optional) | BC1 | Binary masks only, where the range is strictly zero or one |
| Opacity | BC4 | Single channel only — separate from all other data |

BC7 is used for normals rather than BC5. BC5 compresses only two channels and is expensive to sample because it performs relatively little compression for its cost. BC7 covers the same two tangent-space normal channels plus the two bent normal channels in a single sample, providing better overall value. Days Gone and Half-Life Alyx both use BC7 for tangent normals. BC5 as a default format is prohibited.

Opacity is always a separate BC4 texture. Shadow passes and prepasses must test opacity for alpha-tested geometry. If opacity is packed with RGB data in a BC3 or BC7 texture, the depth-only or shadow pass shader must fetch the full texture to access the alpha channel. A separate BC4 contains only the opacity channel — no RGB — so depth-only passes fetch only what they need at significantly lower bandwidth cost. Prepasses must not shade anything requiring an opacity map unless Forward+ rendering is active.

Two BC3 slots provide color, AO, specular masks, roughness, and depth data at compressed bandwidth. BC3 is BC1 compression for RGB with a BC4-compressed alpha, making it efficient for the mixed data that occupies those channels. The fourth BC1 slot is optional and only applies where masks are strictly binary. For the lowest-spec fallback, all color data goes into BC1 — harsh compression but maximum geometry timing performance on low-end hardware.

BC3 packing for shadow rendering requires attention: if opacity is mixed with RGB in a BC3 slot, shadow passes must not use that texture. Any mesh that renders into a depth or shadow pass must have its opacity available as a standalone BC4 channel.

### Foliage

Close-up foliage LODs must use geometry cutouts rather than alpha-tested planes to minimize pixel shader invocations. The Phantom Pain analysis identifies this explicitly: close-up foliage needs accurate geometry cutouts to minimize how pixel invocation-bound the rendering becomes. Mid-distance foliage uses opacity-masked alpha geometry. Distant foliage uses stable LODs or impostors. Wind animation should prefer texture-based displacement in the vertex shader over geometry recomputation.

---

## Part 12 — Anti-Aliasing Architecture

### Approach: Modernized SMAA4X with Persistence

The engine targets SMAA compute shader plus persistence-based geometric accumulation. This is not TSR, not pure TAA, not DLSS. TSR costs 3.5ms in E33 and produces a flickery, blurry result. CryEngine's SMAA TX2 costs approximately 0.5ms and produces sharper, more stable motion. That is a 7× cost difference with inferior output from the more expensive option. The choice is straightforward.

### SMAA Compute Shader

SMAA updated by its creator to use a compute shader rather than a pixel shader — due to stencil quad inefficiencies in the pixel shader path — provides a 40% performance uplift in slow scenarios. Neither the mainstream industry nor modern CryEngine currently uses the compute version despite the creator making a presentation on it. This engine implements the compute shader version as the baseline. The standalone SMAA pass must be validated through a debug view showing its edge detection output independently. SMAA that is not smoothing basic edges is not functioning and must be fixed before any temporal component is added on top of it.

### Resolution-Tiered Configuration

At 1080p the configuration is 2× MSAA plus SMAA compute morphological plus persistence via motion vectors. At 1440p the configuration is 2× MSAA plus SMAA compute, because 1440p is 50% denser than 1080p which halves the MSAA requirement. At 4K through integer scaling from 1080p, persistence sampling alone may suffice with 2× MSAA as an option. 8× MSAA is never shipped — diminishing returns begin after 4× and 8× is overkill at any resolution.

2× MSAA alone is explicitly insufficient. The Half-Life Alyx analysis states this directly: 2× MSAA is not enough unless paired with persistence jittering at 60 FPS to accumulate four effective samples across frames. 4× MSAA is the minimum for meaningful geometric anti-aliasing at 1080p, and it is demonstrated as viable at 37% GPU utilization on a desktop 3060 in a realistic architectural environment.

### MSAA Cost and Topology

MSAA cost scales with quad utilization, topology quality, and alpha-to-coverage content. If topology is optimized — flat surfaces using max-area topology, curvature and silhouettes using appropriate density — MSAA does not produce a significant performance penalty. The Half-Life Alyx analysis makes this explicit: "If someone says MSAA doubles their geometry cost, their non-MSAA geometry timing is unoptimized." The MSAA amplification should be proportional to visible detail added, not a tax on poorly topologized geometry.

### Deferred MSAA Implementation

MSAA in a deferred renderer has specific known failure modes that must all be addressed. The GBuffer format precision requirements in Part 5 already handle the first two — 24-bit minimum albedo and 24 to 30-bit minimum normals are required precisely because 16-bit albedo and 17-bit normals corrupted MSAA edges in Crisis 3.

Beyond buffer precision, the implementation requires a custom edge stencil built from G-buffer geometric data. The stencil tags every 4×4 pixel quad containing MSAA subsamples into the depth stencil at pixel frequency. Crisis 3's heuristic stencil missed significant numbers of valid geometric edges, causing jagged geometry even at 8× MSAA on the correct edges and wasting subsample computation on incorrect ones. A correct stencil from G-buffer geometry produces dramatically better results even at 4× MSAA.

The pass structure separates pixel frequency and sample frequency work explicitly. Pixel frequency draws use non-multisampled render targets for non-edge quads. Sample frequency draws use dedicated per-sample pixel shader invocations for edge quads, with the original multisampled render targets as input. HDR lighting output goes into two separate non-MSAA RGBA16 buffers at pixel frequency — buffer 0 carries sample 0 data and buffer 1 carries sample 1 data. This avoids the quad utilization penalty from sample-frequency deferred shading while preserving subsample data for correct edge resolution.

SSDO compositing order applies here too: after the albedo/specular resolve, never before. Compositing AO before the resolve is what caused Crisis 3's uncanny MSAA edges even when full subsample shading was forced, because the AO composite destroyed the subsample lighting data before the resolve could use it.

### Persistence Sampling

Persistence accumulates geometric samples across frames using motion vectors and is vendor-agnostic, unlike MFAA which is Nvidia-exclusive. Modern motion vectors with history rejection eliminate the ghosting that plagued older implementations in low-luminance scenes. Persistence flicker is only a display problem at low resolution — 1080p and above on PC suppresses it. At stable 60 FPS, persistence provides two free geometric samples per frame through temporal accumulation, effectively doubling geometric MSAA coverage at no additional rasterization cost. This is a secondary justification for targeting 60 FPS as a platform goal beyond playability.

### Yuan Hamus Fresnel-Based Edge Normal Correction

Yuan Hamus, formerly of Slightly Mad Studios, developed a shader-based specular anti-aliasing technique that addresses the root cause of edge aliasing rather than its symptoms. Flat triangle surfaces produce incorrect normals at edges because triangle geometry cannot represent an underlying smooth surface. These incorrect normals affect what portions of reflection maps are sampled and skew SSR results. The aliasing and the incorrect normals occur at the same geometric edges for the same fundamental reason.

The technique applies a targeted change to how edge normals are calculated in the pixel shader, generating obtuse reflection vectors at edges. The Fresnel approximation is adjusted to allow these obtuse vectors to mirror the background behind the surface edge. At the sub-pixel level, SSR then blends the physical surface edge with its background through correct reflection behavior rather than spatial blending — geometric anti-aliasing as a side effect of physically correct edge normals. A half-resolution copy of the previous frame serves as SSR input for this purpose.

The technique works best with high-definition meshes and correct normal baking. Lower-poly meshes with significant normal errors cannot be fully corrected. Marmoset Toolbag and Instant LOD are confirmed to bake normals to the quality level this technique requires. It is compatible with max-area topology because it only affects curved surface topology, not flat regions. It does not replace spatial or temporal AA — it supplements geometric edge quality independently of both.

### FXAA Correct Configuration

FXAA's reputation for extreme blur is a result of industry-wide misconfiguration, not a fundamental property of the algorithm. The correct configuration for quality work uses the PC version at preset 39 with the subpixel value disabled. Preset 39 removes FXAA's dithering behavior. Disabling the subpixel value removes the main blurring behavior. UE5 uses the PC version at 75% subpix and never exposes preset 39 to players because cinematic-level scalability is not intended for games. Best-quality FXAA in correct configuration is a valid cheap fallback and is measurably competitive with SMAA at lower cost. SMAA with compute shader handles diagonal edges more accurately and is the preferred default, but FXAA at preset 39 with subpix disabled is not the blurry failure mode the industry has come to expect.

### TAA as an Optional Pass

If TAA is included it is a tunable dial between noise suppression and detail preservation, documented explicitly as a tradeoff. Conservative TAA preserves geometry detail and motion clarity at the cost of surviving fog noise and low-frequency pattern noise. Aggressive TAA cleans those patterns at the cost of soft geometry and motion trailing. TAA must not be used to mask noisy shadows, noisy GI, stochastic virtual texture sampling, bad fog, missing specular filtering, bad LOD transitions, alpha instability, low-resolution reflections, or broken motion vectors. These are broken features. Fix the feature.

---

## Part 13 — Post-Processing Pipeline

### Volumetric Fog

Blue noise sampling is mandatory. IGN is prohibited. CryEngine and KCD2 use IGN and the visible repeating diagonal patterns in their fog are called out specifically in the reference analyses as among the worst-looking volumetric implementations available. The temporal component cannot suppress IGN patterns without blurring aggressively enough to destroy detail. Half-Life Alyx and Days Gone are the reference implementations for blue noise fog. Fog composites onto the lit frame after the main lighting draw.

### Bloom

Bloom uses the Kawase blur family rather than a Gaussian blur chain. Intel measured Kawase as outperforming Gaussian in most scenarios. MGSV's Kawase-based bloom is identified in the analysis as the best-looking bloom across all reference titles. The post-DoF result is blurred down to 270p and the 270p texture is recycled and blurred progressively until it overwrites the previous frame's bloom RG11B10. Bloom composites during the tone mapping stage. The Fox Engine approach stores pixel luminance in the alpha channel of the lit buffer at resolve time, eliminating a separate luminance extraction pass — this engine adopts that approach. Bloom must not produce muddy haze, temporal pumping, color desaturation, or loss of material detail.

### Depth of Field

Two half-resolution RGBA16 buffers handle near and far field. A depth-related texture is mipmapped before near and far blur passes execute. Near and far blurred versions composite on the post-fog frame. DoF is conservative in gameplay and may use higher quality settings in cinematics. Dithered haze, screen-static noise, silhouette leakage, and TAA-dependent smoothing are unacceptable defaults.

### Rain Effects

A rain occlusion map combined with a noise texture produces a result blurred in two passes. Rain drops are restricted to bright and illuminated areas via the previous frame's bloom RG11B10 — rain on dark surfaces is naturally suppressed. The rain projection pass tiles its inputs with an aerial interior exclusion texture to prevent interior rain rendering.

### HDR Luminance Averaging

The post-DoF result downsamples from 270p to a 64×64 HDR luminance RG16, then averages to a 1×1 RG16. The 1×1 buffer drives auto-exposure adaptation for tone mapping. Separating the diffuse lighting buffer from albedo as described in the Fox Engine lighting stage provides a purer luminance signal for this step.

### Anti-Aliasing Pass Order

The tone-mapped RGB10A2 feeds SMAA compute edge detection producing an RG8 edge mask. The stencil restricts SMAA blending to edge pixels only. SMAA blending writes an RGBA16 blend mask producing an edge-smoothed RGB10A2. The SMAA TX2 temporal component runs with RGBA history and temporal information in the alpha. The final transfer goes to RGBA8 with screen composite effects in RG11B10 including film grain. Film grain is applied at final output, never earlier in the pipeline.

### Sharpening

Sharpening is mild and optional. It is not a repair tool for blurry TAA, bad upscaling, or low internal resolution. If the renderer requires aggressive sharpening to look acceptable, the base image is insufficient and the sharpening is hiding that fact.

---

## Part 14 — Variable Rate Super Sampling Research Path

Standard image-based VRS as a shading rate reduction tool is low priority. The general consensus from developers is negative — aggressive rate reduction produces visible artifacts in motion — and Tier 2 VRS is not universally available on PC hardware. The genuinely interesting application is Variable Rate Super Sampling.

The VRSS concept decouples pixel shader invocations from geometric sampling rate. 4× SSAA at 1080p provides the geometric sampling density of 4× MSAA. With 2×2 VRS applied, pixel shader invocations run at approximately native 1080p rate while geometric sampling remains at 4× density. This is geometrically equivalent to 4× MSAA with pixel shader cost approaching native resolution rendering.

The unsolved problems that keep this in the research tier are: VRAM cost from the 4× supersample buffer; deferred rendering needing stencil logic to handle repeated pixels and copy relevant lit pixels intelligently; triangle derivative distortion from VRS sampling (Gears 5 addressed this by accounting for the distortion inside the material shader); sampling grid quality (4× SSAA uses an ordered grid which looks worse than MSAA's rotated grid, requiring programmable sample locations for a rotated or optimized pattern); and alpha-tested content incompatibility with hair and foliage. 60 FPS persistence could improve grid quality further by alternating sample positions across frames, but this requires custom downscalers, derivative correction throughout the pipeline, and significant R&D before it is production-stable.

This path remains experimental until sampling quality, VRAM, derivative correction, and alpha content handling are all resolved.

---

## Part 15 — Forward+ Path

Forward+ exists because some image quality goals are more naturally achieved when lighting happens directly in the material pass. It is the correct path for MSAA-heavy modes, high-frame-rate modes, alpha-to-coverage content, and complex foliage and hair rendering where deferred's reconstruction complexity creates unnecessary problems.

Half-Life Alyx is the reference standard: 1080p 60 FPS at 37% GPU utilization on a desktop 3060 with 4× MSAA in a realistic architectural environment. The complete frame budget runs under 11ms with optimization headroom. The path demonstrates that forward rendering with MSAA is not just viable — it is highly efficient when geometry and materials are properly authored.

Forward+ advantages over deferred in the appropriate scenarios include cleaner MSAA because lighting happens in the same pass as material evaluation so MSAA samples carry full lighting information without deferred resolve complexity, single buffer write avoiding GBuffer bandwidth cost, no separate millisecond-scale lighting stage, and natural alpha-to-coverage behavior with the MSAA buffer.

The constraints of Forward+ are real. It generally requires a full prepass, and the base pass becomes expensive when materials sample too many textures or evaluate too many lights. Unreal Engine's Forward+ supports only four shadow-casting overlapping lights — insufficient for gameplay or cinematic scenarios. Unity and Wicked Engine implementations support significantly more. This engine must not reproduce Unreal's limitation. The prepass in Forward+ mode is a full prepass but remains strictly vertex-shader-driven with no pixel shaders, no velocity, and no opacity sampling unless alpha-to-coverage is the active transparency mode.

---

## Part 16 — Debugging, Profiling, and Tooling

### Render Graph as Accountability System

Every pass carries its name, owner, feature category, resolution scale, target formats, inputs, outputs, history dependencies, temporal dependency level, vendor-specific dependency level, and timing budget. A production capture explains the frame from first clear to final present without unnamed buffers or unexplained resource writes. If a pass exists only because an older pipeline habit carried forward, the render graph makes that visible.

### Required Debug Views

The engine must visualize all of the following: depth (linear and hardware), normals, roughness, metalness or material class, specular data, material IDs color-coded, bent normals and their occlusion influence, object AO, cavity, velocity, overdraw heatmap, quad utilization, texture count per draw, shadow caster participation, shadow density, light volumes, stencil masks, SSR hits and misses, SSR versus RT cost comparison (flagged when SSR exceeds RT), reflection fallback source, AO radius and halo risk, SSDO standalone output, screen-space contact shadow output, temporal history validity heatmap, TAA rejection map, MSAA sample coverage, MSAA edge stencil correctness, MSAA per-sample lighting, hardware versus software raster cost (when visibility-buffer paths are active), LOD transition state, tone mapper pre and post comparison, and Kawase bloom chain output.

These are not optional debug aids. They are the minimum required for production review. Any renderer that cannot expose these views cannot be reviewed correctly, and a renderer that cannot be reviewed correctly will ship with preventable errors.

### Automated Profiler Flags

The profiler automatically flags draws accessing more than four textures, shadow passes that include floor or ground plane geometry, unnecessary G-buffer clears where the base pass overwrites anyway, bad draw ordering with expensive materials rendering before prepass-eligible geometry, motion vectors computed in the prepass rather than the base pass, SSR cost exceeding software RT cost, overwritten expensive material surface area above 15% (the reference demo was at 21%), any draw deviating significantly from the 1 microsecond per 1,000 pixels ratio, alpha-tested content in the prepass, BC5 as a default compression format, SSDO composited before the lighting resolve, and standalone SMAA not producing detectable edge detection output.

### Asset Pipeline Validation

At import, the asset pipeline flags excessive texture counts per material, bad compression choices, opacity packed with RGB for any mesh that renders in depth-only passes, missing bent normals on materials that need specular occlusion, missing normal or roughness MIP filtering, bad normal bakes, long thin triangles, high quad overdraw, dense alpha cards in depth passes, missing shadow caster classification, missing reflection proxy assignments for large dynamic objects, and bad LOD transition distances. These failures are cheaper to catch at import than after they appear in a captured frame.

### Performance Regression Testing

Each major renderer change reports pass timing deltas, memory deltas, target format changes, draw and dispatch count changes, shader permutation changes, image differences, and temporal artifact review results. Regressions in any pass without a documented justification are blocked.

---

## Part 17 — Scalability System

All quality tiers maintain vendor-agnostic operation. No tier enables hardware RT, DLSS, FSR, or vendor-specific technology. Floor exclusion from shadows and the four-texture material limit are mandatory at all tiers.

| Feature | Low | Medium | High |
|---|---|---|---|
| RT resolution | 50% native | 60% native | 70% native |
| Temporal accumulation | 4-frame | 3-frame | 2-frame |
| Rays per pixel | 1 | 1–2 | 2 |
| Reflection proxy geometry | Minimal | Standard | Full set |
| Shadow atlas | 2K×2K | 4K×4K | 4K×4K |
| Shadow update policy | Cached static only | Mixed | Per-frame dynamic |
| Volumetric fog quality | Low sample count | Medium | Full blue noise |
| SSDO | Off | Half-resolution | Full resolution |
| Contact shadows | Off | On | On |
| Color bleeding | Off | Off | On |
| Bent normal occlusion | Object AO only | Bent normals | Full specular occlusion |
| Post-processing | Bloom and tone map | Plus DoF and rain | Full stack |
| Anti-aliasing | SMAA only | SMAA plus 2× persistence | Full tiered SMAA4X |
| Material texture limit | 2 | 3 | 4 |

Upscaling is not a scalability tier. It must not compensate for a missing performance budget. All tiers target their output resolution natively. If performance is insufficient at a given tier, reduce a feature — do not reduce the output resolution.

---

## Part 18 — Anti-Patterns Reference Table

Every entry in this table comes from a measured pipeline capture. Source abbreviations: [RT] Vendor-Agnostic RT Demo · [E33] Clair Obscure E33 · [C3] Crisis 3 MSAA · [OB] Oblivion Remake · [FE] Fox Engine · [HLA] Half-Life Alyx · [DG] Days Gone · [TH] Misc Topics

| Anti-Pattern | Source | Measured Consequence |
|---|---|---|
| Hardware RT APIs, DLSS, FSR | [RT] | 8ms at quarter-res vs faster software RT at 70% native; vendor lock-in |
| TSR / Epic TAA | [E33] | 3.5ms for blurry output vs 0.5ms SMAA TX2 with sharper results |
| Lumen GI | [E33] | Smear artifacts, TAA dependency, unstable under any motion |
| Nanite / micropolygon geometry | [E33][OB] | Software rasterizer 2,900% less efficient under occlusion; shadow cost multiplication |
| Virtual textures in UE5 style | [E33] | Stochastic noise, forced full prepass, TAA dependency |
| IGN volumetric fog | [RT][E33] | Visible repeating diagonal patterns; TAA cannot suppress without blurring |
| Lambert diffuse | [E33][FE] | Flat plasticky indirect lighting; missing retroreflection behavior |
| Alpha-tested content in prepass | [C3][HLA] | Doubled prepass cost with 2× MSAA in Crisis 3 |
| Motion vectors in prepass | [OB] | +28.5% prepass cost for only +1.6% base pass saving |
| Floors drawn into shadow passes | [E33][OB] | 2ms shadow cost vs 0.6ms in optimized titles |
| More than four textures per draw | [RT][OB] | 50%+ draw slowdown; no visual justification |
| BC5 as default normal format | [RT][TH] | Expensive to sample; two channels only; BC7 covers same data plus bent normals |
| SSDO composited before lighting resolve | [C3] | Destroys subsample lighting data; uncanny MSAA edges |
| SSR running simultaneously with RT | [RT] | SSR exceeded RT cost in reference demo; redundant work |
| TAA masking broken rendering | All | Treats symptoms; does not fix underlying issues |
| Unnecessary G-buffer clears | [RT] | Performance waste; base pass overwrites them anyway |
| Pre-passed objects rendering mid-pass | [RT][FE] | 21% expensive material surface area overwritten; ~7% whole-frame waste |
| SMAA pixel shader version | [TH] | 40% slower than compute version in slow scenarios |
| 8× MSAA in production | [C3][HLA] | Diminishing returns after 4×; overkill at any resolution |
| 2× MSAA without persistence | [HLA] | Insufficient geometric coverage without 4 effective accumulated samples |
| Runtime material texture blending | [OB] | Terrain 5× slower than optimized eighth-generation equivalent |
| Aggressive sharpening as default | [E33] | Not a repair tool; signals base image is insufficient |
| UE5 Forward+ 4-light limit | [TH] | Insufficient for gameplay and cinematics |
| Default FXAA configuration | [TH] | 75% subpix and absent preset 39 earns blurry reputation incorrectly |
| Albedo below 24-bit | [C3] | Color corruption at MSAA sample edges; green/purple fringing |
| Normals below 24 bits | [C3] | Insufficient for subsample lighting accuracy |
| No bent normals outside hero assets | [FE][TH] | Specular leaking; implausibly glowing surfaces across ordinary environment geometry |
| No separate BC4 opacity channel | [TH] | Shadow and prepass shaders fetch full textures for a single opacity bit |
| ACES 1/2 or AgX tone mapping | [TH][FE] | Muted colors, incorrect highlight gradients, damage to PBR material identity |

---

## Part 19 — Reference Performance Data

All figures from Threat Interactive captures on a desktop RTX 3060 with hardware RT disabled, used as a Shader Model 5 representative.

| Benchmark | Measured Cost |
|---|---|
| CryEngine SMAA TX2 | ~0.5ms |
| UE5 TSR | ~3.5ms |
| Reference demo software RT at 70% native, unoptimized | ~8ms with debugger overhead inflation |
| Reference demo actual median via in-engine counter | ~30% faster than debugger-reported figure |
| Reference demo SSR (broken) | Exceeded RT cost — pipeline error |
| Reference demo total frame time (debugger) | ~13.8ms |
| Crisis 3 prepass with alpha-tested foliage at 2× MSAA | ~2× cost vs no MSAA |
| Half-Life Alyx prepass, 4× MSAA, over 100K triangles | Under 0.5ms |
| Half-Life Alyx full frame, 4× MSAA, 1080p | 37% GPU utilization on desktop 3060 |
| Fox Engine full frame budget | Under 11ms with optimization headroom |
| UE5 Nanite visbuffer pass alone | ~4ms |
| E33 shadow cost | ~2ms vs 0.6ms in Days Gone and Half-Life Alyx |
| UE5 E33 skin caching animation cost | ~1ms |
| Prepass motion vector removal gain | +28.5% prepass speed at +1.6% base pass cost |
| Partial prepass gain in reference demo with incorrect ordering | ~1ms / 7% whole-frame |
| Overwritten expensive material surface area in reference demo | 21% — target is under 15% |
| Fox Engine stencil light vs UE5 no-stencil DX12 | UE5 DX12 is 50% more expensive |
| SMAA compute vs pixel shader | 40% faster in slow scenarios |
| UE5 5.6 hardware Lumen improvement | ~22% — software Lumen (the game developer path) shows near-zero improvement |
| Prepassing large geometry, Fox Engine analysis | Reduces most expensive draw cost by 50% |

---

## Part 20 — Production Gates

A feature is production-ready only when it looks acceptable natively without temporal smoothing required to hide defects, behaves correctly through camera motion, object motion, and disocclusion, survives low light, high contrast, and particle interaction, fits its performance envelope in worst-case benchmark scenes, fits its memory budget, has a working fallback path, has all required debug views, exposes artist controls, documents its failure modes, and can be disabled without breaking unrelated passes.

No feature passes review on screenshots alone.

The deeper definition is cultural: the engine is done enough for production when it can reject bad defaults before those defaults become the visual identity of the game.

---

## Part 21 — Risk Register

The first major risk is temporal dependency, where features become acceptable only under TAA. The mitigation is a mandatory native diagnostic mode and explicit temporal dependency labels per pass in the render graph. Any pass marked as temporally dependent that cannot hold quality in native mode is not production-ready.

The second risk is content budget drift, where artists ship too many textures, bad topology, and expensive shadows because the tools do not surface cost clearly. The mitigation is validation at asset import, continuous GPU capture review per milestone, and content reports tying specific assets to frame time contribution.

The third risk is vendor path divergence, where hardware RT or proprietary features become the only visually competitive path. The mitigation is making the portable compute path feature-complete first and requiring hardware RT paths to prove acceleration rather than correctness. The compute path is the standard.

The fourth risk is meshlet ideology, where visibility-buffer systems are adopted because they look modern rather than because they are measurably better. The mitigation is direct comparison against optimized conventional rendering before adoption, with required debug views for software rasterization cost, cluster leakage, and shadow amplification. The Oblivion and E33 analyses are standing evidence.

The fifth risk is post-process compensation, where sharpening, bloom, fog, and temporal accumulation mask weak base rendering. The mitigation is base-image review before any post processing in the frame and strict artifact gates for every post pass.

The sixth risk is tone mapping error, where a poorly configured mapper destroys correct PBR work after expensive accurate shading. The mitigation is treating tone mapping as a core lighting component, validating against photographic material and exposure references, and specifically validating the GT tone mapper against the GT7 research comparisons.

The seventh risk is MSAA edge stencil regression, where the custom stencil silently degrades quality to Crisis 3 levels without obvious failure. The mitigation is the MSAA edge stencil correctness debug view and explicit regression testing on benchmark scenes at every major renderer change.

---

## Part 22 — Roadmap

The project opens with a research harness covering prepass policies, MSAA resolve design including the custom stencil, morphological AA configurations, specular filtering, shadow map strategies, SSR quality and cost, GT tone mapping validation, texture packing schemes including the BC7 plus BC3 plus BC4 opacity structure, topology and quad overdraw measurement, Kawase bloom tuning, SMAA compute shader implementation, and the Yuan Hamus Fresnel edge correction prototype.

The second phase builds the core renderer: RHI, render graph with full pass metadata, shader system, resource allocator, capture labels, forward and deferred scene rendering paths, GPU timestamp queries per pass, and core debug views.

The third phase delivers the production raster path: partial prepass with correct end-of-pass draw ordering for pre-passed objects, compact deferred renderer with the Fox Engine albedo-free lighting stage, material ID lookup tables, direct lighting with stencil restriction, shadow maps with caster classification and mandatory floor exclusion, SSDO composited after resolve, screen-space contact shadows targeting TAA-independence, probe indirect lighting, GT tone mapper, SSR, and Kawase bloom.

The fourth phase handles anti-aliasing and sampling: SMAA compute shader, correct deferred MSAA with custom edge stencil and frequency separation and dual-output HDR buffers, FXAA in correct preset 39 configuration, Fresnel-based edge normal correction, persistence sampling, temporal MSAA research, and resolve validation against Crisis 3's known failure cases.

The fifth phase adds reflections and software RT: optimized SSR with high-roughness support, simplified proxy reflection scene, compute shader software RT at 70% native, blue noise sampling, semi-independent denoiser, glass interlaced reflection handling, and hardware RT as an optional accelerator for the enhanced tier.

The sixth phase delivers production tooling: asset validators at import with automatic flagging, GPU capture regression testing per milestone, material linting, shadow participation analysis, LOD inspection, texture packing reports, temporal artifact test suite, scalability profiles, memory budgets, crash diagnostics, shader cache robustness, documentation, and sample content covering all benchmark scene types.

---

## Part 23 — Test Matrix

The renderer ships with standing benchmark scenes covering dense urban environments with glass, emissives, and wet surfaces; forest foliage at close range and mid-distance; baked interior lighting with local lights; wet night scenes with many emissives; bright HDR exterior overexposure stress; low-light interior near-black environments; fast horizontal camera pans; slow forward dollies through complex geometry; character close-ups covering skin, hair, and first-person assets; glass surfaces with simultaneous interior and exterior reflections; many overlapping local lights; static and dynamic sun cases; and Forward+ high-frame-rate mode scenes.

Each scene is tested in native no-temporal diagnostic mode as the primary quality gate, morphological AA only, MSAA mode with edge stencil visualization active, conservative persistence mode, reflection fallback modes, shadow scalability modes, motion vector visualization mode, and full GPU timing capture.

---

## Part 24 — Definition of Done

The render engine is production-ready when a native-resolution frame looks stable without mandatory temporal blur, the baseline path runs at target frame rate in worst-case benchmark scenes, every major pass is timed and debuggable in a GPU capture, artists can see exactly why specific content is expensive or unstable, shadows and reflections have clear quality standards and debug visibility, AO and bent normals and tone mapping preserve material plausibility, anti-aliasing preserves motion clarity without smearing, vendor-specific features remain optional accelerators, fallback modes preserve the image philosophy, and GPU captures explain the frame from first clear to final present.

The deeper definition is cultural: the engine passes when it can reject bad defaults before those defaults define the look of the game.

---

*Sources: Threat Interactive — Vendor-Agnostic RT and MASSIVE Project Update; Clair Obscure Expedition 33 Analysis; Crisis 3 MSAA Analysis; Oblivion Remake GPU Optimization; Metal Gear Solid V Fox Engine Analysis; Half-Life Alyx Forward Rendering Analysis; Days Gone Screen-Space Shadows; Miscellaneous Graphics Topics covering FXAA, Bent Normals, Texture Packing, Forward+, VRS, GT Tone Mapping, and Kawase Blur. External technical anchors: Vulkan 1.4 specification, Direct3D 12 Agility SDK, Gran Turismo 7 tone mapping research.*

---

## Part 25 — Executable Reference Demo

The repository now includes two executable paths. The live GPU renderer uses `wgpu`
to target Vulkan on Linux and Direct3D 12 on Windows without vendor APIs. Its frame
contains a 2048-pixel directional shadow map with 3x3 PCF, a vertex-only partial
depth prepass, hardware depth-tested RGBA8 albedo, RGBA16F tangent-space normal/flag
and RGBA8 PBR G-buffers, a 70%-resolution WGSL compute reflection supplement with
blue-noise hashing, and a fullscreen deferred resolve with metallic-aware PBR,
screen-space reflections, practical lights, emissive bloom, rain, fog, a moon disk,
vignette and GT tone mapping. The live camera uses a restrained cinematic sway,
accepts manual arrow-key control, and handles resizing in real time.

The deterministic CPU renderer remains as the regression and diagnostic harness. It
implements the accountable render graph, asset-policy validation, floor shadow
exclusion, AO, layered SSR and software reflection fallbacks, Kawase bloom, SMAA edge
detection, persistence, and the complete required diagnostic set tracked by the
profiler.

The live demo is a layered wet industrial alley assembled from seven Poly Haven CC0
glTF model sets and four CC0 PBR surfaces. Its asphalt, concrete, brick and metal
materials use mipmapped base-color, tangent-space normal and packed
ambient-occlusion/roughness/metallic maps. Barrels, barriers, shelving, street lamps,
a utility box, a tyre and trash cans provide foreground and midground composition;
the procedural shell adds wet ground, walls, shutters and cyan/amber emissive
practicals. The 1K source files are vendored under `assets/aaa`, provenance is in
`ASSET_SOURCES.md`, and `scripts/fetch_aaa_assets.sh` makes acquisition repeatable.
Run `cargo run --release -- --live` for the GPU renderer. Run
`cargo run --release -- --gallery demo-output` to produce a self-contained CPU
reference gallery, PNG outputs for beauty and all diagnostic views, and a per-pass
timing report.

The live pipeline has been validated through Vulkan presentation on both AMD RADV
and NVIDIA hardware. That establishes cross-vendor execution, not the final frame
budget. Production completion still requires GPU timestamp queries, shadow atlases
for multiple dynamic lights, a world-space proxy BVH tracer and denoiser, SMAA/MSAA
on the GPU, Direct3D 12 testing on Windows, temporal motion stress testing, and the
complete benchmark-scene matrix in Part 23.
