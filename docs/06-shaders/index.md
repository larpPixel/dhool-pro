# Shaders

All WGSL shaders are in `src/shaders/`.

## Bind group layout

All shaders share a consistent bind group layout.

| Group | Slot | Contents |
|-------|------|----------|
| 0 | Global | Camera view-proj, time, exposure, fog params |
| 1 | Material | Base colour texture, normal texture, ARM texture, sampler |
| 2 | Pass | Pass-specific uniforms and render targets |
| 3 | Dynamic | Per-object transform and material index |

## Shader index

### Geometry pass

| File | Type | Purpose |
|------|------|---------|
| `depth_prepass.wgsl` | Vertex | Write occluder depth, reject early-Z |
| `geometry.wgsl` | Vertex + Fragment | G-buffer: Albedo, Normals, ARM, Velocity, MaterialID |
| `depth_copy.wgsl` | Fragment | Fullscreen R32 depth copy |
| `hiz.wgsl` | Compute | Hierarchical Z mip chain reduction |
| `alpha_shadow.wgsl` | Vertex + Fragment | Alpha-tested shadow map write |
| `velocity_compensation.wgsl` | Compute | Motion vector neighbourhood clamp |

### Lighting pass

| File | Type | Purpose |
|------|------|---------|
| `lighting.wgsl` | Fragment | Deferred PBR Cook-Torrance BRDF resolve |
| `light_volume.wgsl` | Vertex + Fragment | Stencil-guarded light influence |
| `gbuffer_resolve.wgsl` | Fragment | Combine deferred targets |

### Screen-space effects

| File | Type | Purpose |
|------|------|---------|
| `reflections.wgsl` | Fragment | Roughness-filtered environment specular |
| `ssr.wgsl` | Compute | Hi-Z cone-traced screen-space reflections |
| `volumetric_fog.wgsl` | Compute | Froxel scattering accumulation |
| `rain3d.wgsl` | Compute | 4096 rain particle simulation and rendering |

### Post-processing

| File | Type | Purpose |
|------|------|---------|
| `bloom.wgsl` | Compute | Progressive dual-filter bloom |
| `tonemap.wgsl` | Fragment | GT/ACES/Filmic tonemapping |
| `auto_exposure.wgsl` | Compute | Luminance histogram and exposure |

### Anti-aliasing

| File | Type | Purpose |
|------|------|---------|
| `smaa_edges.wgsl` | Fragment | Colour and depth edge detection |
| `smaa_blend.wgsl` | Fragment | Edge pattern blend weight calculation |
| `aa.wgsl` | Fragment | Temporal reprojection and resolve |

### Presentation

| File | Type | Purpose |
|------|------|---------|
| `ui_overlay.wgsl` | Fragment | Debug text and overlay rendering |
| `present.wgsl` | Fragment | Final sRGB gamma correction |
