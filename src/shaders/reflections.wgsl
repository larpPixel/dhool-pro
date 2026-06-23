struct Frame {
    dimensions: vec4<f32>,
    counters: vec4<u32>,
};

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


@group(0) @binding(0) var normals: texture_2d<f32>;
@group(0) @binding(1) var pbr: texture_2d<f32>;
@group(0) @binding(2) var depth: texture_depth_2d;
@group(0) @binding(3) var output_reflection: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var<uniform> frame: Frame;
@group(0) @binding(5) var<uniform> camera: Camera;
@group(0) @binding(6) var<uniform> lights: Lights;

const RAIN_BOUNDS_MIN: vec3<f32> = vec3<f32>(-5.8, 0.18, -19.2);
const RAIN_BOUNDS_SIZE: vec3<f32> = vec3<f32>(11.6, 8.2, 25.2);

struct ProxyHit {
    t: f32,
    radiance: vec3<f32>,
    normal: vec3<f32>,
    hit: f32,
};

fn hash_blue_noise(pixel: vec2<u32>, frame_index: u32) -> f32 {
    var state = pixel.x * 1973u + pixel.y * 9277u + frame_index * 26699u + 0x68bc21ebu;
    state = (state ^ (state >> 16u)) * 0x7feb352du;
    state = (state ^ (state >> 15u)) * 0x846ca68bu;
    state = state ^ (state >> 16u);
    return f32(state & 0x00ffffffu) / 16777215.0;
}

fn safe_normalize(value: vec3<f32>) -> vec3<f32> {
    let len = length(value);
    return select(vec3<f32>(0.0), value / len, len > 0.0001);
}

fn reconstruct_world_position(uv: vec2<f32>, scene_depth: f32) -> vec3<f32> {
    let clip_position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, scene_depth, 1.0);
    let world_h = camera.inverse_view_projection * clip_position;
    return world_h.xyz / max(world_h.w, 0.00001);
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

fn rain_wetness(world_position: vec3<f32>, normal: vec3<f32>, surface: vec4<f32>) -> f32 {
    let time = camera.position_time.w;
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

fn spot_cone_attenuation(light_index: u32, light_to_surface: vec3<f32>) -> f32 {
    let flags = lights.local_inner_shadow[light_index];
    if flags.w < 0.5 {
        return 1.0;
    }
    let direction_outer = lights.local_direction_outer[light_index];
    let spot_direction = safe_normalize(direction_outer.xyz);
    let cone = clamp((dot(light_to_surface, spot_direction) - direction_outer.w) / max(flags.x - direction_outer.w, 0.0001), 0.0, 1.0);
    return cone * cone * (3.0 - 2.0 * cone);
}

fn practical_spot_fill(light_index: u32, cone: f32) -> f32 {
    let flags = lights.local_inner_shadow[light_index];
    if flags.w < 0.5 {
        return 1.0;
    }
    return mix(0.42, 1.0, clamp(cone, 0.0, 1.0));
}

fn proxy_emissive_radiance(position: vec3<f32>) -> vec3<f32> {
    var emission = vec3<f32>(0.0);
    if abs(position.x - 4.88) < 0.20 && abs(position.z + 6.7) < 1.38 && abs(position.y - 2.65) < 0.24 {
        emission += vec3<f32>(9.5, 2.2, 0.28);
    }
    if abs(position.x + 4.88) < 0.20 && abs(position.z + 5.4) < 1.78 && abs(position.y - 2.8) < 0.22 {
        emission += vec3<f32>(0.65, 6.2, 9.5);
    }
    if abs(position.z + 18.38) < 0.22 && abs(position.x + 1.7) < 2.08 && abs(position.y - 3.25) < 0.24 {
        emission += vec3<f32>(0.55, 5.8, 8.8);
    }
    return emission;
}

fn proxy_direct_radiance(position: vec3<f32>, normal: vec3<f32>, albedo: vec3<f32>) -> vec3<f32> {
    var radiance = albedo * lights.ambient.rgb * (0.52 + max(normal.y, 0.0) * 0.34);
    let moon_direction = safe_normalize(lights.directional_direction_count.xyz);
    if lights.directional_color_intensity.w > 0.0 && length(moon_direction) > 0.0 {
        let ndotl = max(dot(normal, moon_direction), 0.0);
        radiance += albedo
            * lights.directional_color_intensity.rgb
            * lights.directional_color_intensity.w
            * ndotl
            * 0.55;
    }

    let local_count = u32(clamp(lights.directional_direction_count.w, 0.0, 4.0));
    for (var light_index = 0u; light_index < local_count; light_index += 1u) {
        let position_radius = lights.local_position_radius[light_index];
        let to_light = position_radius.xyz - position;
        let distance_to_light = length(to_light);
        let light_direction = to_light / max(distance_to_light, 0.0001);
        let light_to_surface = -light_direction;
        let attenuation = pow(clamp(1.0 - distance_to_light / position_radius.w, 0.0, 1.0), 2.0);
        let cone = spot_cone_attenuation(light_index, light_to_surface);
        let color_intensity = lights.local_color_intensity[light_index];
        radiance += albedo
            * color_intensity.rgb
            * color_intensity.w
            * attenuation
            * practical_spot_fill(light_index, cone)
            * max(dot(normal, light_direction), 0.0)
            * 0.72;
    }
    return radiance + proxy_emissive_radiance(position);
}

fn proxy_hit(position: vec3<f32>, normal: vec3<f32>, albedo: vec3<f32>, t: f32) -> ProxyHit {
    return ProxyHit(t, proxy_direct_radiance(position, normal, albedo), normal, 1.0);
}

fn trace_alley_proxy(origin: vec3<f32>, ray: vec3<f32>) -> ProxyHit {
    var best = ProxyHit(1000000.0, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0), 0.0);

    if abs(ray.y) > 0.0001 {
        let t = (-0.06 - origin.y) / ray.y;
        let p = origin + ray * t;
        if t > 0.04 && t < best.t && abs(p.x) < 3.65 && p.z > -18.6 && p.z < 8.5 {
            best = proxy_hit(p, vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.045, 0.052, 0.055), t);
        }
    }

    if abs(ray.x) > 0.0001 {
        let left_t = (-5.0 - origin.x) / ray.x;
        let left_p = origin + ray * left_t;
        if left_t > 0.04 && left_t < best.t && left_p.y > 0.0 && left_p.y < 7.0 && left_p.z > -18.6 && left_p.z < 8.5 {
            best = proxy_hit(left_p, vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.18, 0.125, 0.10), left_t);
        }

        let right_t = (5.0 - origin.x) / ray.x;
        let right_p = origin + ray * right_t;
        if right_t > 0.04 && right_t < best.t && right_p.y > 0.0 && right_p.y < 7.0 && right_p.z > -18.6 && right_p.z < 8.5 {
            best = proxy_hit(right_p, vec3<f32>(-1.0, 0.0, 0.0), vec3<f32>(0.18, 0.125, 0.10), right_t);
        }
    }

    if abs(ray.z) > 0.0001 {
        let end_t = (-18.45 - origin.z) / ray.z;
        let end_p = origin + ray * end_t;
        if end_t > 0.04 && end_t < best.t && abs(end_p.x) < 5.1 && end_p.y > 0.0 && end_p.y < 7.0 {
            best = proxy_hit(end_p, vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.17, 0.12, 0.10), end_t);
        }
    }

    return best;
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let output_size = textureDimensions(output_reflection);
    if any(invocation.xy >= output_size) {
        return;
    }

    let source_size = textureDimensions(normals);
    let source_pixel = min(
        vec2<u32>(vec2<f32>(invocation.xy) * vec2<f32>(source_size) / vec2<f32>(output_size)),
        source_size - vec2<u32>(1u)
    );
    let scene_depth = textureLoad(depth, vec2<i32>(source_pixel), 0);
    let surface = textureLoad(pbr, vec2<i32>(source_pixel), 0);
    if scene_depth >= 0.99999 {
        textureStore(output_reflection, vec2<i32>(invocation.xy), vec4<f32>(0.0));
        return;
    }

    let normal = normalize(textureLoad(normals, vec2<i32>(source_pixel), 0).xyz);
    let uv = (vec2<f32>(source_pixel) + 0.5) / vec2<f32>(source_size);
    let world_position = reconstruct_world_position(uv, scene_depth);
    let wetness = rain_wetness(world_position, normal, surface);
    let effective_roughness = mix(surface.x, 0.055, wetness * 0.72);
    if effective_roughness > 0.68 && wetness < 0.08 {
        textureStore(output_reflection, vec2<i32>(invocation.xy), vec4<f32>(0.0));
        return;
    }

    let view_ray = safe_normalize(world_position - camera.position_time.xyz);
    let reflected_base = reflect(view_ray, normal);
    let tangent_seed = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(normal.y) < 0.92);
    let tangent = safe_normalize(cross(tangent_seed, normal));
    let bitangent = safe_normalize(cross(normal, tangent));
    let noise_a = hash_blue_noise(invocation.xy, frame.counters.x + 17u) - 0.5;
    let noise_b = hash_blue_noise(invocation.yx + vec2<u32>(13u, 29u), frame.counters.x + 71u) - 0.5;
    let rough_jitter = effective_roughness * effective_roughness * 0.22;
    let reflected = safe_normalize(reflected_base + (tangent * noise_a + bitangent * noise_b) * rough_jitter);
    let sky_mix = clamp(reflected.y * 0.5 + 0.5, 0.0, 1.0);
    let sky = mix(lights.sky.rgb * 0.35, lights.sky.rgb * 2.0 + lights.ambient.rgb * 0.5, sky_mix);
    let moon_direction = safe_normalize(lights.directional_direction_count.xyz);
    let moon = pow(max(dot(reflected, moon_direction), 0.0), 256.0)
        * lights.directional_color_intensity.rgb
        * lights.directional_color_intensity.w
        * 0.85;
    var local_reflection = vec3<f32>(0.0);
    let local_count = u32(clamp(lights.directional_direction_count.w, 0.0, 4.0));
    for (var light_index = 0u; light_index < local_count; light_index += 1u) {
        let position_radius = lights.local_position_radius[light_index];
        let to_light = position_radius.xyz - world_position;
        let distance_to_light = length(to_light);
        let light_direction = to_light / max(distance_to_light, 0.0001);
        let light_to_surface = -light_direction;
        let attenuation = pow(clamp(1.0 - distance_to_light / position_radius.w, 0.0, 1.0), 2.0);
        let alignment = pow(max(dot(reflected, light_direction), 0.0), 28.0);
        let color_intensity = lights.local_color_intensity[light_index];
        let cone = spot_cone_attenuation(light_index, light_to_surface);
        local_reflection += color_intensity.rgb
            * color_intensity.w
            * attenuation
            * practical_spot_fill(light_index, cone)
            * alignment;
    }
    let proxy_hit_result = trace_alley_proxy(world_position + normal * 0.045, reflected);
    let proxy_reflection = proxy_hit_result.radiance
        * mix(1.0, 0.62, effective_roughness)
        * (1.0 / (1.0 + proxy_hit_result.t * 0.025))
        * proxy_hit_result.hit;
    let wet_puddle = smoothstep(0.14, 0.78, wetness);
    let view_fresnel = 0.025 + pow(1.0 - max(dot(normal, -view_ray), 0.0), 5.0) * 0.58;
    let response = clamp(
        (1.0 - effective_roughness) * (view_fresnel + surface.y * 0.34) * (0.35 + wet_puddle * 0.65),
        0.0,
        0.72
    );
    textureStore(
        output_reflection,
        vec2<i32>(invocation.xy),
        vec4<f32>((sky * 0.55 + moon + local_reflection + proxy_reflection) * response, effective_roughness)
    );
}
