# Shadows, Reflections, and Fog

## Shadow maps

The renderer uses cascaded shadow maps for the directional light and individual
shadow maps for local lights. Each map is a `D32` texture with percentage-closer
filtering (PCF).

### Cascade distribution

The directional shadow splits into 4 cascades. Each cascade covers a slice of
the view frustum. The cascade boundaries adapt to the scene depth range.

### Alpha-tested shadows

Materials with `MaterialClass::AlphaTest` write to a separate alpha shadow pass.
The pass samples the mesh base colour texture and discards fragments below the
alpha cutoff.

### Shadow caching

Shadow maps are cached as long as the light and the occluding geometry do not
move. This reduces shadow pass execution by about 60% in typical scenes.

## Screen-space reflections (SSR)

The SSR pass traces reflection rays against the Hi-Z depth pyramid.

### Ray march algorithm

1. Start the ray at the pixel world-space position.
2. Step along the reflected direction.
3. At each mip level, test the depth against the Hi-Z buffer.
4. Stop when the ray depth matches the scene depth within a threshold.

### Temporal reprojection

The SSR pass accumulates reflection samples across frames. Each frame:

1. Reproject the previous frame reflection into the current frame using motion
   vectors.
2. Clamp the history sample to the current frame neighbourhood range.
3. Blend the history with the current frame sample.

The blend factor increases with roughness. Rough surfaces discard history faster
to prevent ghosting.

### Probe fallback

Pixels that fail the SSR ray march (no screen-space hit) use the roughness-
filtered environment map. The environment is pre-filtered with GGX importance
sampling into a cubemap mip chain.

## Volumetric fog

The fog pass uses a froxel grid in view space.

### Grid dimensions

| Quality Tier | Froxel Resolution |
|-------------|-------------------|
| Low | 80 x 45 x 32 |
| Medium | 120 x 67 x 48 |
| High | 160 x 90 x 64 |
| Ultra | 200 x 112 x 64 |
| Cinematic | 240 x 135 x 96 |

### Scattering model

The fog pass evaluates the Henyey-Greenstein phase function.

$$g = \\frac{\\sum (\\cos \\theta)}{N}$$

$$p(\\theta) = \\frac{1 - g^2}{4\\pi (1 + g^2 - 2g \\cos \\theta)^{3/2}}$$

The in-scattering accumulates along each froxel ray and blends with the
deferred PBR colour.

## Rain system

The rain shader (`rain3d.wgsl`) simulates 4096 world-space particles.

### Particle lifecycle

1. Spawn particles in a 64 x 64 grid above the camera.
2. Apply wind and gravity each frame.
3. Move particles toward the ground.
4. Wrap particles to the top of the simulation volume when they pass below the
   camera.

### Rendering

Each particle is a tube with generated streak detail. The tube widens with
distance to simulate perspective foreshortening.

### Puddle interaction

Rain particles below a height threshold trigger puddle normal ripples. The
puddle normal map scrolls with wind direction to simulate flowing water.
