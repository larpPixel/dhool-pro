struct Camera {
    view_projection: mat4x4<f32>,
    inverse_view_projection: mat4x4<f32>,
    position_time: vec4<f32>,
    viewport: vec4<f32>,
};

struct Lights {
    directional_direction_count: vec4<f32>,
    directional_color_intensity: vec4<f32>,
    ambient: vec4<f32>,
    sky: vec4<f32>,
    weather: vec4<f32>,
    local_position_radius: array<vec4<f32>, 4>,
    local_color_intensity: array<vec4<f32>, 4>,
    local_direction_outer: array<vec4<f32>, 4>,
    local_inner_shadow: array<vec4<f32>, 4>,
};

struct LocalShadowCameras {
    view_projection: array<mat4x4<f32>, 4>,
};

@group(0) @binding(0) var albedo: texture_2d<f32>;
@group(0) @binding(1) var normals: texture_2d<f32>;
@group(0) @binding(2) var pbr: texture_2d<f32>;
@group(0) @binding(3) var depth: texture_depth_2d;
@group(0) @binding(4) var reflections: texture_2d<f32>;
@group(0) @binding(5) var<uniform> camera: Camera;
@group(0) @binding(6) var shadow_map: texture_depth_2d;
@group(0) @binding(7) var shadow_sampler: sampler_comparison;
@group(0) @binding(8) var<uniform> light_camera: Camera;
@group(0) @binding(9) var<uniform> lights: Lights;
@group(0) @binding(10) var local_shadow_maps: texture_depth_2d_array;
@group(0) @binding(11) var<uniform> local_shadow_cameras: LocalShadowCameras;
const RAIN_GRID_X: u32 = 64u;
const RAIN_GRID_Z: u32 = 64u;
const RAIN_BOUNDS_MIN: vec3<f32> = vec3<f32>(-7.2, 0.14, -21.0);
const RAIN_BOUNDS_SIZE: vec3<f32> = vec3<f32>(14.4, 9.6, 29.5);
const EMISSIVE_SCALE: f32 = 12.0;
const PRACTICAL_SPOT_MIN_FILL: f32 = 0.42;
const PRACTICAL_SPOT_MIN_SHADOW: f32 = 0.35;


struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let position = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    var output: VertexOutput;
    output.position = vec4<f32>(position[vertex_index], 0.0, 1.0);
    output.uv = position[vertex_index] * vec2<f32>(0.5, -0.5) + 0.5;
    return output;
}

fn gt_tonemap(x: vec3<f32>) -> vec3<f32> {
    let peak = 1.0;
    let contrast = 1.0;
    let linear_start = 0.22;
    let linear_length = 0.4;
    let toe_power = 1.33;
    let black = 0.0;
    let l0 = ((peak - linear_start) * linear_length) / contrast;
    let shoulder_start = linear_start + l0;
    let shoulder_value = linear_start + contrast * l0;
    let shoulder_scale = (contrast * peak) / (peak - shoulder_value);
    let shoulder_power = -shoulder_scale / peak;
    let toe_weight = vec3<f32>(1.0) - smoothstep(vec3<f32>(0.0), vec3<f32>(linear_start), x);
    let shoulder_weight = step(vec3<f32>(shoulder_start), x);
    let linear_weight = vec3<f32>(1.0) - toe_weight - shoulder_weight;
    let toe = linear_start * pow(max(x / linear_start, vec3<f32>(0.0)), vec3<f32>(toe_power)) + black;
    let linear = linear_start + contrast * (x - linear_start);
    let shoulder = peak - (peak - shoulder_value) * exp(shoulder_power * (x - shoulder_start));
    return toe * toe_weight + linear * linear_weight + shoulder * shoulder_weight;
}

fn safe_normalize(value: vec3<f32>) -> vec3<f32> {
    let len = length(value);
    return select(vec3<f32>(0.0), value / len, len > 0.0001);
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn pow5(value: f32) -> f32 {
    let value2 = value * value;
    return value2 * value2 * value;
}

fn hash21(value: vec2<f32>) -> f32 {
    return fract(sin(dot(value, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn value_noise(uv: vec2<f32>) -> f32 {
    let cell = floor(uv);
    let local = fract(uv);
    let f = local * local * (vec2<f32>(3.0) - local * 2.0);
    let a = hash21(cell);
    let b = hash21(cell + vec2<f32>(1.0, 0.0));
    let c = hash21(cell + vec2<f32>(0.0, 1.0));
    let d = hash21(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn layered_noise(uv: vec2<f32>) -> f32 {
    return value_noise(uv) * 0.52
        + value_noise(uv * 2.17 + vec2<f32>(13.1, 7.7)) * 0.31
        + value_noise(uv * 5.11 + vec2<f32>(3.5, 19.9)) * 0.17;
}

fn rain_wind_direction() -> vec3<f32> {
    return safe_normalize(vec3<f32>(
        -0.22 - lights.weather.z * 0.12,
        -1.0,
        0.08 + lights.weather.z * 0.05
    ));
}

fn wrap_rain_xz(xz: vec2<f32>) -> vec2<f32> {
    let local = fract((xz - RAIN_BOUNDS_MIN.xz) / RAIN_BOUNDS_SIZE.xz);
    return RAIN_BOUNDS_MIN.xz + local * RAIN_BOUNDS_SIZE.xz;
}

fn wrap_rain_cell(cell: vec2<f32>) -> vec2<f32> {
    let grid = vec2<f32>(f32(RAIN_GRID_X), f32(RAIN_GRID_Z));
    return cell - floor(cell / grid) * grid;
}

fn periodic_rain_delta(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let delta = a - b;
    return delta - round(delta / RAIN_BOUNDS_SIZE.xz) * RAIN_BOUNDS_SIZE.xz;
}

fn rain_cell_seed(cell: vec2<f32>) -> f32 {
    return hash21(cell + vec2<f32>(23.1, 91.7));
}


fn reconstruct_world_position(uv: vec2<f32>, scene_depth: f32) -> vec3<f32> {
    let clip_position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, scene_depth, 1.0);
    let world_h = camera.inverse_view_projection * clip_position;
    return world_h.xyz / max(world_h.w, 0.00001);
}

fn sky_radiance(uv: vec2<f32>) -> vec3<f32> {
    let far_world = reconstruct_world_position(uv, 1.0);
    let ray = safe_normalize(far_world - camera.position_time.xyz);
    let horizon = pow(clamp(ray.y * 0.5 + 0.5, 0.0, 1.0), 1.7);
    let sky = mix(lights.sky.rgb * 0.35, lights.sky.rgb * 2.4 + lights.ambient.rgb * 0.6, horizon);
    let moon_direction = safe_normalize(lights.directional_direction_count.xyz);
    let moon_alignment = max(dot(ray, moon_direction), 0.0);
    let moon = pow(moon_alignment, 720.0)
        * lights.directional_color_intensity.rgb
        * lights.directional_color_intensity.w
        * 0.95;
    return sky + moon;
}

fn procedural_rain_impacts(world_position: vec3<f32>, normal: vec3<f32>, surface: vec4<f32>, time: f32) -> f32 {
    let exposed_floor = smoothstep(0.64, 0.94, normal.y)
        * (1.0 - surface.y)
        * (1.0 - smoothstep(0.18, 0.75, surface.a));
    let grid = vec2<f32>(f32(RAIN_GRID_X), f32(RAIN_GRID_Z));
    let wind = rain_wind_direction();
    let drift_per_meter = wind.xz / max(-wind.y, 0.15);
    let full_fall_drift = drift_per_meter * RAIN_BOUNDS_SIZE.y;
    let source_xz = wrap_rain_xz(world_position.xz - full_fall_drift);
    let source_uv = (source_xz - RAIN_BOUNDS_MIN.xz) / RAIN_BOUNDS_SIZE.xz * grid;
    let base_cell = floor(source_uv);
    var impact = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let cell = wrap_rain_cell(base_cell + vec2<f32>(f32(x), f32(y)));
            let seed = rain_cell_seed(cell);
            let jitter = vec2<f32>(hash21(cell + 11.7), hash21(cell + 73.3));
            let top_xz = RAIN_BOUNDS_MIN.xz + ((cell + jitter) / grid) * RAIN_BOUNDS_SIZE.xz;
            let fall_speed = 8.4 + hash21(cell + 47.77) * 5.2 + f32(u32(cell.x + cell.y) % 3u) * 0.9;
            let fall_phase = fract(seed + time * fall_speed / RAIN_BOUNDS_SIZE.y);
            let impact_age = clamp((fall_phase - 0.84) / 0.16, 0.0, 1.0);
            let impact_gate = step(0.84, fall_phase);
            let hit_xz = wrap_rain_xz(top_xz + full_fall_drift);
            let distance_to_drop = length(periodic_rain_delta(world_position.xz, hit_xz));
            let ring_radius = impact_age * (0.055 + seed * 0.18);
            let distance_to_ring = abs(distance_to_drop - ring_radius);
            let ring = (1.0 - smoothstep(0.006, 0.024, distance_to_ring)) * (1.0 - impact_age);
            let splash = (1.0 - smoothstep(0.0, 0.036, distance_to_drop))
                * smoothstep(0.0, 0.18, impact_age)
                * (1.0 - smoothstep(0.26, 0.62, impact_age));
            impact = max(impact, impact_gate * max(ring, splash * 0.85));
        }
    }
    let speckles = smoothstep(0.91, 0.995, layered_noise(source_uv * 1.7 + vec2<f32>(time * 0.11, -time * 0.09))) * 0.025;
    return clamp(exposed_floor * (impact * 0.72 + speckles) * surface.z * lights.weather.x, 0.0, 0.72);
}

fn rain_wetness(world_position: vec3<f32>, normal: vec3<f32>, surface: vec4<f32>, time: f32) -> f32 {
    let upward = smoothstep(0.10, 0.86, normal.y);
    let side = smoothstep(0.24, 0.82, 1.0 - abs(normal.y));
    let wind = rain_wind_direction();
    let wind_shear = wind.xz / max(-wind.y, 0.15);
    let puddle = smoothstep(0.50, 0.86, layered_noise(world_position.xz * 0.12 - wind_shear * time * 0.012));
    let fine_beads = smoothstep(0.68, 0.98, layered_noise(world_position.xz * 1.35 + wind_shear * time * 0.05));
    let runoff = smoothstep(
        0.66,
        0.96,
        layered_noise(vec2<f32>(
            world_position.x * 0.34 + world_position.z * 0.12,
            world_position.y * 1.2 - time * (0.18 + lights.weather.z * 0.08)
        ))
    );
    let non_emissive = 1.0 - smoothstep(0.12, 0.75, surface.a);
    let material_accepts_water = 1.0 - surface.y * 0.35;
    let horizontal_wet = upward * (0.30 + puddle * 0.48 + fine_beads * 0.10);
    let vertical_runoff = side * runoff * 0.20;
    return clamp((horizontal_wet + vertical_runoff) * non_emissive * material_accepts_water * lights.weather.x, 0.0, 0.88);
}

fn screen_space_occlusion(pixel: vec2<u32>, world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let size = textureDimensions(depth);
    let offsets = array<vec2<i32>, 12>(
        vec2<i32>(3, 1), vec2<i32>(-3, 2), vec2<i32>(2, -3), vec2<i32>(-2, -3),
        vec2<i32>(6, 4), vec2<i32>(-6, 3), vec2<i32>(4, -6), vec2<i32>(-4, -5),
        vec2<i32>(1, 4), vec2<i32>(-1, 5), vec2<i32>(5, -1), vec2<i32>(-5, -2)
    );
    var occlusion = 0.0;
    for (var sample_index = 0u; sample_index < 12u; sample_index += 1u) {
        let sample_pixel_i = clamp(
            vec2<i32>(pixel) + offsets[sample_index],
            vec2<i32>(0),
            vec2<i32>(size) - vec2<i32>(1)
        );
        let sample_pixel = vec2<u32>(sample_pixel_i);
        let sample_depth = textureLoad(depth, sample_pixel_i, 0);
        if sample_depth >= 0.99999 {
            continue;
        }
        let sample_uv = (vec2<f32>(sample_pixel) + 0.5) / vec2<f32>(size);
        let sample_world = reconstruct_world_position(sample_uv, sample_depth);
        let delta = sample_world - world_position;
        let distance_to_sample = length(delta);
        if distance_to_sample > 0.025 && distance_to_sample < 1.85 {
            let horizon = max(dot(normal, delta / distance_to_sample), 0.0);
            let near_contact = 1.0 - smoothstep(0.025, 0.42, distance_to_sample);
            let broad_contact = 1.0 - smoothstep(0.10, 1.85, distance_to_sample);
            occlusion += horizon * (near_contact * 1.15 + broad_contact * 0.55);
        }
    }
    return clamp(1.0 - occlusion * 0.19, 0.24, 1.0);
}

fn shadow_visibility(world_position: vec3<f32>, normal: vec3<f32>, light_direction: vec3<f32>) -> f32 {
    let light_clip = light_camera.view_projection * vec4<f32>(world_position + normal * 0.018, 1.0);
    let light_ndc = light_clip.xyz / max(light_clip.w, 0.00001);
    let uv = light_ndc.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || light_ndc.z >= 1.0 {
        return 1.0;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map));
    let ndotl = max(dot(normal, light_direction), 0.0);
    let bias = max(0.00055, (1.0 - ndotl) * 0.0024);
    var visibility = 0.0;
    for (var y = -2; y <= 2; y += 1) {
        for (var x = -2; x <= 2; x += 1) {
            visibility += textureSampleCompare(
                shadow_map,
                shadow_sampler,
                uv + vec2<f32>(f32(x), f32(y)) * texel * 1.18,
                light_ndc.z - bias
            );
        }
    }
    return visibility / 25.0;
}

fn spot_cone_attenuation(light_index: u32, light_to_surface: vec3<f32>) -> f32 {
    let flags = lights.local_inner_shadow[light_index];
    if flags.w < 0.5 {
        return 1.0;
    }
    let direction_outer = lights.local_direction_outer[light_index];
    let spot_direction = safe_normalize(direction_outer.xyz);
    let inner = flags.x;
    let outer = direction_outer.w;
    let cone = clamp((dot(light_to_surface, spot_direction) - outer) / max(inner - outer, 0.0001), 0.0, 1.0);
    return cone * cone * (3.0 - 2.0 * cone);
}

fn practical_spot_fill(light_index: u32, cone: f32) -> f32 {
    let flags = lights.local_inner_shadow[light_index];
    if flags.w < 0.5 {
        return 1.0;
    }
    return mix(PRACTICAL_SPOT_MIN_FILL, 1.0, clamp(cone, 0.0, 1.0));
}

fn practical_spot_shadow_weight(light_index: u32, cone: f32) -> f32 {
    let flags = lights.local_inner_shadow[light_index];
    if flags.w < 0.5 {
        return 1.0;
    }
    return mix(PRACTICAL_SPOT_MIN_SHADOW, 1.0, clamp(cone, 0.0, 1.0));
}

fn local_shadow_visibility(world_position: vec3<f32>, normal: vec3<f32>, light_index: u32) -> f32 {
    let flags = lights.local_inner_shadow[light_index];
    if flags.y < 0.5 {
        return 1.0;
    }
    let light_clip = local_shadow_cameras.view_projection[light_index]
        * vec4<f32>(world_position + normal * 0.025, 1.0);
    if light_clip.w <= 0.0001 {
        return 1.0;
    }
    let light_ndc = light_clip.xyz / light_clip.w;
    let uv = light_ndc.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || light_ndc.z >= 1.0 || light_ndc.z <= 0.0 {
        return 1.0;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(local_shadow_maps));
    let layer = i32(light_index);
    var visibility = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            visibility += textureSampleCompare(
                local_shadow_maps,
                shadow_sampler,
                uv + vec2<f32>(f32(x), f32(y)) * texel * 1.35,
                layer,
                light_ndc.z - 0.0015
            );
        }
    }
    return visibility / 9.0;
}

fn shade_light(
    base: vec3<f32>,
    normal: vec3<f32>,
    view_direction: vec3<f32>,
    surface: vec4<f32>,
    wetness: f32,
    light_direction: vec3<f32>,
    radiance: vec3<f32>
) -> vec3<f32> {
    let ndotl = max(dot(normal, light_direction), 0.0);
    let ndotv = max(dot(normal, view_direction), 0.0);
    let roughness = max(surface.x, 0.04);
    let half_direction = safe_normalize(light_direction + view_direction);
    let ldoth = max(dot(light_direction, half_direction), 0.0);
    let fd90 = 0.5 + 2.0 * ldoth * ldoth * roughness;
    let light_scatter = 1.0 + (fd90 - 1.0) * pow5(1.0 - ndotl);
    let view_scatter = 1.0 + (fd90 - 1.0) * pow5(1.0 - ndotv);
    let diffuse = base
        * vec3<f32>(1.0 - surface.y)
        * ndotl
        * light_scatter
        * view_scatter
        * surface.z
        * (1.0 - wetness * 0.38);
    let ndoth = max(dot(normal, half_direction), 0.0);
    let specular_power = mix(220.0, 9.0, roughness);
    var f0 = mix(vec3<f32>(0.04), base, vec3<f32>(surface.y));
    f0 = mix(f0, vec3<f32>(0.075), wetness * (1.0 - surface.y));
    let fresnel = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - max(dot(normal, view_direction), 0.0), 5.0);
    let specular_occlusion = clamp(pow(ndotv + surface.z, exp2(-16.0 * roughness - 1.0)) - 1.0 + surface.z, 0.0, 1.0);
    let specular = pow(ndoth, specular_power) * fresnel * ndotl * specular_occlusion * (1.0 + wetness * 1.7);
    return (diffuse + specular) * radiance;
}


fn sample_reflection_history(uv: vec2<f32>) -> vec4<f32> {
    let reflection_size = textureDimensions(reflections);
    let coord = uv * vec2<f32>(reflection_size) - vec2<f32>(0.5);
    let base = floor(coord);
    let f = fract(coord);
    let p0 = clamp(vec2<i32>(base), vec2<i32>(0), vec2<i32>(reflection_size) - vec2<i32>(1));
    let p1 = clamp(p0 + vec2<i32>(1, 0), vec2<i32>(0), vec2<i32>(reflection_size) - vec2<i32>(1));
    let p2 = clamp(p0 + vec2<i32>(0, 1), vec2<i32>(0), vec2<i32>(reflection_size) - vec2<i32>(1));
    let p3 = clamp(p0 + vec2<i32>(1, 1), vec2<i32>(0), vec2<i32>(reflection_size) - vec2<i32>(1));
    let a = textureLoad(reflections, p0, 0);
    let b = textureLoad(reflections, p1, 0);
    let c = textureLoad(reflections, p2, 0);
    let d = textureLoad(reflections, p3, 0);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let size = textureDimensions(albedo);
    let pixel = min(vec2<u32>(input.position.xy), size - vec2<u32>(1u));
    let scene_depth = textureLoad(depth, vec2<i32>(pixel), 0);
    if scene_depth >= 0.99999 {
        return vec4<f32>(sky_radiance(input.uv), 1.0);
    }

    let surface_raw = textureLoad(pbr, vec2<i32>(pixel), 0);
    let normal = normalize(textureLoad(normals, vec2<i32>(pixel), 0).xyz);
    let world_position = reconstruct_world_position(input.uv, scene_depth);
    let wetness = rain_wetness(world_position, normal, surface_raw, camera.position_time.w);
    let impact = procedural_rain_impacts(world_position, normal, surface_raw, camera.position_time.w);
    let water_coverage = clamp(wetness + impact * 0.45, 0.0, 1.0);
    let effective_surface = vec4<f32>(
        mix(surface_raw.x, 0.055, water_coverage * 0.72),
        surface_raw.y,
        surface_raw.z,
        surface_raw.a
    );
    let base_raw = textureLoad(albedo, vec2<i32>(pixel), 0).rgb;
    let base = mix(base_raw, base_raw * 0.58, wetness * (1.0 - surface_raw.y) * 0.46);
    let reflected_sample = sample_reflection_history(input.uv);
    let reflected = reflected_sample.rgb;
    let reflected_roughness = reflected_sample.a;
    let emissive_strength = surface_raw.a * EMISSIVE_SCALE;
    let emissive_mask = smoothstep(0.015, 0.09, surface_raw.a);

    let light_direction = safe_normalize(lights.directional_direction_count.xyz);
    let view_direction = safe_normalize(camera.position_time.xyz - world_position);
    let ssao = screen_space_occlusion(pixel, world_position, normal);
    let ambient = lights.ambient.rgb * (0.48 + max(normal.y, 0.0) * 0.52);
    let sun_shadow = shadow_visibility(world_position, normal, light_direction);
    var hdr = base * vec3<f32>(1.0 - surface_raw.y) * ambient * ssao * effective_surface.z * (1.0 - emissive_mask * 0.85);
    if lights.directional_color_intensity.w > 0.0 && length(light_direction) > 0.0 {
        let directional_radiance =
            lights.directional_color_intensity.rgb * lights.directional_color_intensity.w * sun_shadow;
        hdr += shade_light(base, normal, view_direction, effective_surface, wetness, light_direction, directional_radiance);
    }
    let local_count = u32(clamp(lights.directional_direction_count.w, 0.0, 4.0));
    for (var light_index = 0u; light_index < local_count; light_index += 1u) {
        let position_radius = lights.local_position_radius[light_index];
        let to_light = position_radius.xyz - world_position;
        let distance_to_light = length(to_light);
        let local_direction = to_light / max(distance_to_light, 0.0001);
        let light_to_surface = -local_direction;
        let attenuation = pow(clamp(1.0 - distance_to_light / position_radius.w, 0.0, 1.0), 2.0);
        let cone = spot_cone_attenuation(light_index, light_to_surface);
        let spot = practical_spot_fill(light_index, cone);
        let color_intensity = lights.local_color_intensity[light_index];
        let local_shadow = mix(
            1.0,
            local_shadow_visibility(world_position, normal, light_index),
            practical_spot_shadow_weight(light_index, cone)
        );
        let local_radiance = color_intensity.rgb * color_intensity.w * attenuation * spot * local_shadow;
        hdr += shade_light(base, normal, view_direction, effective_surface, wetness, local_direction, local_radiance);
    }
    hdr += base_raw * emissive_strength;
    let surface_fresnel = 0.025 + pow(1.0 - max(dot(normal, view_direction), 0.0), 5.0) * 0.62;
    let puddle_visibility = smoothstep(0.12, 0.78, water_coverage) * (1.0 - smoothstep(0.48, 0.92, effective_surface.x));
    let base_reflected_weight = clamp(
        (surface_fresnel + surface_raw.y * 0.38) * puddle_visibility * (0.55 + impact * 0.18) * (1.0 - reflected_roughness * 0.35),
        0.0,
        0.58
    );
    let lit_luminance = max(luminance(hdr), 0.02);
    let reflection_luminance = max(luminance(reflected), 0.0001);
    let reflection_energy_cap = lit_luminance * (0.24 + water_coverage * 0.34 + surface_fresnel * 0.28);
    let reflected_weight = min(base_reflected_weight, reflection_energy_cap / reflection_luminance);
    hdr += reflected * reflected_weight;
    let rain_light_mask = clamp(luminance(hdr) * 0.24 + emissive_strength * 0.05, 0.04, 1.0);
    let impact_light = mix(lights.sky.rgb, lights.directional_color_intensity.rgb, vec3<f32>(0.35))
        * impact
        * rain_light_mask
        * (0.10 + wetness * 0.18);
    hdr += impact_light;
    let fog = smoothstep(0.78, 1.0, scene_depth);
    hdr = mix(hdr, vec3<f32>(0.045, 0.09, 0.16), fog * 0.45);
    let vignette_distance = distance(input.uv, vec2<f32>(0.5));
    let vignette = 1.0 - smoothstep(0.42, 0.78, vignette_distance) * 0.42;
    return vec4<f32>(max(hdr * vignette, vec3<f32>(0.0)), 1.0);
}
