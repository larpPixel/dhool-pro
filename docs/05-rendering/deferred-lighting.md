# Deferred Lighting

The deferred renderer separates geometry and lighting into two stages.

## G-buffer layout

The geometry pass writes four textures.

| Target | Format | Content |
|--------|--------|---------|
| Albedo | `RGBA8Unorm` | Base colour (sRGB) |
| Normals | `RGBA16Snorm` | World-space normal (XY) + roughness (Z) |
| ARM | `RGBA8Unorm` | Ambient occlusion, roughness, metallic |
| Velocity | `RG16Float` | Screen-space motion vector |
| MaterialID | `R8Uint` | Material class identifier |

## Light volume culling

To reduce fragment shader invocations, the renderer bounds each local light
with a stencil volume. Fragments outside the volume skip lighting evaluation.

The algorithm:

1. Render the light bounding sphere or cone to the stencil buffer.
2. The stencil test passes only for fragments inside the volume.
3. Run the PBR resolve only on those fragments.

## PBR resolve

The lighting shader computes the Cook-Torrance microfacet BRDF.

$$f_r = f_d + f_s$$

### Diffuse term (Burley)

$$f_d = \\frac{c}{\\pi} \\cdot (1 - F_{90}) \\cdot (1 - \\mu)^5$$

where $c$ is the base colour, $\\mu$ is the NdotL cosine, and $F_{90}$ is the
Fresnel reflectance at grazing angle.

### Specular term

$$f_s = \\frac{D \\cdot G \\cdot F}{4 \\cdot (N \\cdot L) \\cdot (N \\cdot V)}$$

Components:

| Term | Function | Parameter |
|------|----------|-----------|
| $D$ | GGX normal distribution | $\\alpha = \\text{roughness}^2$ |
| $G$ | Smith geometry shadowing | Schlick-GGX |
| $F$ | Schlick Fresnel | $F_0$ from metallic base colour |

### Energy conservation

The diffuse lobe is normalised so that bright surfaces do not wash out under
direct light. The emissive channel adds unshadowed radiance on top of the BRDF.

## Light types

The deferred resolve accepts up to 1 directional light and 6 local lights.

- The directional light uses cascaded shadow maps.
- Point and spot lights use cube or perspective shadow maps.
- Lights beyond the limit produce a scene error at build time.