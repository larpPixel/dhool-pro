# Cloth and environment-lighting implementation

This change treats cloth and lighting as engine contracts, then uses Dust2 and
the monsoon city as validation scenes. It does not special-case the renderer to
make one screenshot look correct.

## What changed and why

The old direct diffuse term was not divided by pi, while sun intensity and
auto-exposure were tuned around that excess energy. Bright exterior plaster
therefore approached white while shaded alleys lacked a stable indirect floor.
CPU and GPU lighting now share the same normalized Burley diffuse convention,
scene-owned environment irradiance, and an albedo-independent exposure proxy.
White balance derives from ambient plus environment light rather than direct
sun.

The old renderer had two sky paths: Dust2 could bind a panorama only for the GPU
background, while every other scene and every lighting calculation used a flat
color. `RenderScene` now requires one `EnvironmentMap`. Its panorama, encoding,
exposure, yaw, and precomputed spherical harmonics drive the CPU reference and
live GPU background, diffuse light, reflection fallback, fog, rain fill, and
white balance. Dust2 owns its exported Source 2 panorama. The AAA scene owns a
vendored cloudy-night HDR panorama. Weather and exposure limits remain in the
separate `EnvironmentProfile`; CLI exposure arguments are explicit overrides.

Cloth is a mesh-level `ClothDefinition` simulated by one deterministic,
fixed-step Verlet/PBD solver. `World`, the offline gallery, and the live GPU path
all use that solver. The GPU uploads only the affected vertex ranges and carries
both current and previous positions into the G-buffer, so temporal effects see
real cloth motion. Moving shadow casters invalidate the cached maps.

## Surface ledger

| Surface | Applies | Result |
|---|---:|---|
| Scene and material contract | Yes | Required environment map, weather/exposure profile, and optional cloth definition are explicit data. |
| Game/world fixed update | Yes | Shared cloth solver runs at the project timestep with deterministic collider ordering. |
| CPU reference renderer | Yes | Panorama background/reflections/fog and SH diffuse use the same map and orientation. |
| Live wgpu renderer | Yes | HDR mip upload, panorama background, SH diffuse, roughness-filtered reflections, fog/rain fill, cloth upload, and scene-owned uniforms. |
| Dust2 | Yes | Dry daylight profile, exported Source 2 panorama, and a green cloth awning fixture. |
| Monsoon city | Yes | Vendored cloudy-night HDR, rain/wetness, and a cloth market awning fixture. |
| glTF import | Yes | Previous positions initialize from imported positions; alpha-mask materials retain alpha-tested behavior. |
| Source 2 import script | Yes | Required skybox extraction and conversion now fail loudly. |
| Asset pipeline | Yes | Empty, malformed, or non-2:1 environment panoramas fail validation. |
| Tests and docs | Yes | Sampling, radian yaw, exposure, SH diffuse, world round-trip, shader validity, frame-graph dependencies, provenance, and cloth contracts are covered. |
| Browser, mobile, network, remote service | No | This repository exposes no applicable client or service path. |

## Engineering lesson

The durable fix was to make radiometry and environment state explicit before
tuning scene values. A map-specific exposure constant can hide an energy error,
but it cannot keep daylight, rain, auto-exposure, and the CPU reference path in
agreement. Likewise, cloth belongs in scene/world state with render integration;
a shader-only wobble would have no collision, shadow, or motion-vector contract.

The full Dust2 run also caught a layout lesson that small scenes missed: adding
12 bytes of previous-position data to every packed vertex pushed the main buffer
past wgpu's 256 MiB limit. Previous positions now use a separate vertex stream.
That keeps each allocation portable and lets cloth update its current and
previous ranges without inflating the primary static-geometry stride.
