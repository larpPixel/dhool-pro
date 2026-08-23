# Cloth and environment-lighting implementation

This change treats cloth and lighting as engine contracts, then uses Dust2 and
the monsoon city as validation scenes. It does not special-case the renderer to
make one screenshot look correct.

## What changed and why

The old direct diffuse term was not divided by pi, while sun intensity and
auto-exposure were tuned around that excess energy. Bright exterior plaster
therefore approached white while shaded alleys lacked a stable indirect floor.
CPU and GPU lighting now share the same normalized Burley diffuse convention,
a restrained sky-hemisphere fill, and an albedo-independent exposure proxy.
White balance derives from ambient plus sky rather than direct sun.

Weather, sky orientation, sky exposure, and exposure defaults previously lived
in global live-renderer constants. They now belong to `RenderScene` through an
`EnvironmentProfile`. Dust2 is dry daylight. The AAA scene remains rainy. CLI
exposure arguments are explicit overrides rather than hidden defaults.

Cloth is a mesh-level `ClothDefinition` simulated by one deterministic,
fixed-step Verlet/PBD solver. `World`, the offline gallery, and the live GPU path
all use that solver. The GPU uploads only the affected vertex ranges and carries
both current and previous positions into the G-buffer, so temporal effects see
real cloth motion. Moving shadow casters invalidate the cached maps.

## Surface ledger

| Surface | Applies | Result |
|---|---:|---|
| Scene and material contract | Yes | Environment profile and optional cloth definition are explicit data. |
| Game/world fixed update | Yes | Shared cloth solver runs at the project timestep with deterministic collider ordering. |
| CPU reference renderer | Yes | Normalized diffuse, sky fill, and cloth previous-position motion are represented. |
| Live wgpu renderer | Yes | Fixed-step cloth upload, motion vectors, shadow invalidation, and scene-owned lighting/weather uniforms. |
| Dust2 | Yes | Dry daylight profile, Source 2 panorama settings, and a green cloth awning fixture. |
| Monsoon city | Yes | Rain/wetness retained and a cloth market awning fixture added. |
| glTF import | Yes | Previous positions initialize from imported positions; alpha-mask materials retain alpha-tested behavior. |
| Source 2 import script | Yes | Required skybox extraction and conversion now fail loudly. |
| Tests and docs | Yes | Solver behavior, pins, collisions, transform round-trip, scene profiles, fixtures, and shader contracts are covered. |
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
