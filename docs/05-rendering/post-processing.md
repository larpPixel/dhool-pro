# Post-Processing

## Bloom

The bloom pass isolates high-luminance pixels and applies a progressive dual
filter.

### Algorithm

1. Extract pixels brighter than the bloom threshold from the HDR beauty image.
2. Downsample the extracted image through 5 mip levels with a 13-tap tent
   filter.
3. Upsample back through the mip chain, adding each level to the next.
4. Add the final bloom texture to the HDR beauty image before tonemapping.

The bloom intensity is energy-clamped. Very bright pixels do not bloom more
than the filter kernel radius allows.

## Tone mapping

The renderer provides three tone mapping curves.

| Curve | Characteristics |
|-------|-----------------|
| GT | Gran Turismo-style, preserves mid-tones well |
| ACES | Academy colour encoding system, film-like roll-off |
| Filmic | Customizable shoulder and toe compression |

The default is GT. Change the curve at startup in `ProjectSettings`.

### Auto-exposure

The auto-exposure pass computes a luminance histogram of the HDR image.

1. Downsample the HDR image to a 64 x 64 luminance buffer.
2. Compute the log-average luminance.
3. Clamp the exposure to the user-configured range (set with `--exposure-min`
   and `--exposure-max`).
4. Smooth the exposure over frames to prevent sudden brightness changes.

The auto-exposure target is 18% grey (middle grey).

## Anti-aliasing

The renderer implements SMAA T2x (Subpixel Morphological Anti-Aliasing with
temporal reprojection).

### Edge detection

The SMAA edge pass evaluates colour and depth differences at each pixel.

- Luma differences above a threshold mark a horizontal or vertical edge.
- Depth differences above a second threshold mark a geometric edge.

The edge stencil is stored in the red (vertical) and green (horizontal)
channels of an `RG8` texture.

### Blend weight calculation

For each detected edge pattern, SMAA computes a blend weight based on the local
edge shape. Long edges get more blending. Sharp corners retain sharpness.

### Temporal reprojection

The temporal pass uses motion vectors to reproject the previous frame and
clamp it to the current frame colour neighbourhood.

1. Fetch the motion vector for each pixel.
2. Sample the previous resolved frame at the reprojected coordinate.
3. Compute the colour min, max, and clamp radius for the current pixel.
4. Clamp the history sample to the neighbourhood range.
5. Blend 70% history, 30% current.

Pixels with invalid motion (disocclusion or first frame) use 100% current
sample.

## Velocity compensation

The velocity compensation pass adjusts the temporal clamp for fast-moving
objects. It increases the neighbourhood clamp radius when the motion vector
magnitude exceeds `--velocity-cutoff` (default 8.0 px).
