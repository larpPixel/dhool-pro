# Environment lighting surface ledger

## User-visible behavior

The monsoon city now has a real cloudy night sky instead of an empty navy
background. The cloud panorama is also the global light source: surfaces,
reflections, fog, rain, and exposure respond to the same sky that the camera
sees. Dust2 keeps its own Source 2 sky and uses it through the same contract.

## Applies

| Surface | Contract |
|---|---|
| Scene builders | Every `RenderScene` owns a required `EnvironmentMap`; AAA/demo use the bundled HDR and Dust2 uses its exported panorama. |
| World state | Environment data survives `RenderScene` to `World` to `RenderScene` round-trips. |
| CPU reference | Camera rays sample the panorama for background, reflected rays sample it on misses, SH9 supplies diffuse irradiance, and fog uses view-direction radiance. |
| Live GPU | The HDR/sRGB panorama is decoded to linear RGBA16F with a full mip chain; lighting, reflections, fog, and rain receive the same texture and uniform. |
| Diffuse lighting | Nine cosine-convolved spherical-harmonic coefficients supply directional indirect light. |
| Reflections | Rough surfaces select blurrier panorama mips; the old flat sky gradient and analytic moon fallback are removed. |
| Atmosphere and weather | Fog reconstructs view rays, rain samples directional fill, and white balance uses irradiance; precipitation and wetness remain separate weather state. |
| Asset import | The environment is registered as radiance data. Empty, pixel-count-mismatched, or non-2:1 panoramas are errors. |
| Render graph | Environment radiance is an explicit dependency of reflection supplementation, fog, HDR lighting, and world-space rain. |
| Asset workflow | The official Poly Haven HDR is vendored for offline runs, refreshable through `scripts/fetch_aaa_assets.sh`, licensed, and checksummed in `ASSET_SOURCES.md`. |
| Tests | Synthetic sampling proves yaw/exposure/SH behavior; tests cover scene fixtures, world round-trip, asset rejection, frame-graph edges, GPU uniforms, and every WGSL module. |
| Documentation | README, asset provenance, implementation ledger, Dust2 import notes, and this ledger describe one shared environment contract. |

## Does not apply

| Surface | Reason |
|---|---|
| Browser, mobile, and network clients | This repository has no browser, mobile, API, or service client. |
| glTF material textures | Environment radiance is scene-level data, not a fourth per-material sample. |
| Source 2 redistribution | Dust2-derived assets remain local and ignored; only the user's licensed export is loaded. |
| Screen-space lens overlays | Rain remains depth-tested world geometry; no decorative overlay duplicates the sky or final frame. |

## Removed hacks

- Deleted the Dust2-only GPU panorama selector and its 1x1 flat-color fallback.
- Deleted the duplicate `sky_color` field from scene and world state.
- Corrected yaw from radians-added-as-texture-turns to radians divided by `2*pi`.
- Removed the analytic moon disc over the photographic panorama.
- Removed the second hard-coded fog blend from the lighting shader.
- Removed the CPU hemisphere-gradient sky and flat reflection fallback.

## Verification record

The exact commands and runtime limitations for this change belong in the final
task report. Static build, focused behavior tests, full tests, lints, CPU gallery
output, and the nearest available live GPU capture must be reported separately;
one must not be used as evidence for another.
