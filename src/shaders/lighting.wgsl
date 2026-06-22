struct Camera {
    view_projection: mat4x4<f32>,
    inverse_view_projection: mat4x4<f32>,
    position_time: vec4<f32>,
    viewport: vec4<f32>,
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

fn point_light(
    world_position: vec3<f32>,
    normal: vec3<f32>,
    base: vec3<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
    radius: f32,
    intensity: f32
) -> vec3<f32> {
    let to_light = position - world_position;
    let distance_to_light = length(to_light);
    let light_direction = to_light / max(distance_to_light, 0.0001);
    let attenuation = pow(clamp(1.0 - distance_to_light / radius, 0.0, 1.0), 2.0);
    return base * color * max(dot(normal, light_direction), 0.0) * attenuation * intensity;
}

fn shadow_visibility(world_position: vec3<f32>) -> f32 {
    let light_clip = light_camera.view_projection * vec4<f32>(world_position, 1.0);
    let light_ndc = light_clip.xyz / max(light_clip.w, 0.00001);
    let uv = light_ndc.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || light_ndc.z >= 1.0 {
        return 1.0;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map));
    var visibility = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            visibility += textureSampleCompare(
                shadow_map,
                shadow_sampler,
                uv + vec2<f32>(f32(x), f32(y)) * texel,
                light_ndc.z - 0.0012
            );
        }
    }
    return visibility / 9.0;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let size = textureDimensions(albedo);
    let pixel = min(vec2<u32>(input.position.xy), size - vec2<u32>(1u));
    let scene_depth = textureLoad(depth, vec2<i32>(pixel), 0);
    if scene_depth >= 0.99999 {
        let horizon = pow(clamp(input.uv.y, 0.0, 1.0), 1.8);
        let sky = mix(vec3<f32>(0.003, 0.007, 0.018), vec3<f32>(0.018, 0.045, 0.09), horizon);
        let moon = pow(max(0.0, 1.0 - distance(input.uv, vec2<f32>(0.76, 0.16)) * 18.0), 8.0);
        return vec4<f32>(gt_tonemap(sky + moon * vec3<f32>(1.3, 1.45, 1.7)), 1.0);
    }

    let base = textureLoad(albedo, vec2<i32>(pixel), 0).rgb;
    let normal = normalize(textureLoad(normals, vec2<i32>(pixel), 0).xyz);
    let surface = textureLoad(pbr, vec2<i32>(pixel), 0);
    let clip_position = vec4<f32>(input.uv.x * 2.0 - 1.0, 1.0 - input.uv.y * 2.0, scene_depth, 1.0);
    let world_h = camera.inverse_view_projection * clip_position;
    let world_position = world_h.xyz / max(world_h.w, 0.00001);
    let shadow_pixel = min(pixel + vec2<u32>(3u, 2u), size - vec2<u32>(1u));
    let blocker_depth = textureLoad(depth, vec2<i32>(shadow_pixel), 0);
    let contact_shadow = select(1.0, 0.58, blocker_depth + 0.0015 < scene_depth);
    let reflection_size = textureDimensions(reflections);
    let reflection_pixel = min(
        vec2<u32>(vec2<f32>(pixel) * vec2<f32>(reflection_size) / vec2<f32>(size)),
        reflection_size - vec2<u32>(1u)
    );
    let reflected = textureLoad(reflections, vec2<i32>(reflection_pixel), 0).rgb;

    let light_direction = normalize(vec3<f32>(0.45, 0.78, 0.36));
    let view_direction = normalize(camera.position_time.xyz - world_position);
    let half_direction = normalize(light_direction + view_direction);
    let ndotl = max(dot(normal, light_direction), 0.0);
    let ndoth = max(dot(normal, half_direction), 0.0);
    let roughness = max(surface.x, 0.04);
    let burley = ndotl * (0.68 + roughness * 0.32);
    let specular_power = mix(180.0, 7.0, roughness);
    let f0 = mix(vec3<f32>(0.04), base, vec3<f32>(surface.y));
    let fresnel = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - max(dot(normal, view_direction), 0.0), 5.0);
    let specular = pow(ndoth, specular_power) * fresnel * ndotl;
    let ambient = vec3<f32>(0.038, 0.052, 0.075) * (0.55 + max(normal.y, 0.0) * 0.45);
    let sun_shadow = shadow_visibility(world_position);
    let direct = vec3<f32>(0.58, 0.72, 1.0) * burley * 1.65 * contact_shadow * sun_shadow;
    var hdr = base * (vec3<f32>(1.0 - surface.y)) * (ambient + direct) * surface.z;
    hdr += specular * 2.2 + reflected;
    hdr += point_light(
        world_position,
        normal,
        base,
        vec3<f32>(-3.8, 2.8, -5.4),
        vec3<f32>(0.05, 0.75, 1.0),
        8.0,
        3.8
    );
    hdr += point_light(
        world_position,
        normal,
        base,
        vec3<f32>(3.6, 2.4, -8.0),
        vec3<f32>(1.0, 0.16, 0.025),
        7.0,
        4.2
    );
    hdr += base * surface.a * 6.0;
    let bloom_offsets = array<vec2<i32>, 12>(
        vec2<i32>(-12, 0), vec2<i32>(12, 0), vec2<i32>(0, -12), vec2<i32>(0, 12),
        vec2<i32>(-7, -7), vec2<i32>(7, -7), vec2<i32>(-7, 7), vec2<i32>(7, 7),
        vec2<i32>(-3, 0), vec2<i32>(3, 0), vec2<i32>(0, -3), vec2<i32>(0, 3)
    );
    var bloom = vec3<f32>(0.0);
    for (var sample_index = 0u; sample_index < 12u; sample_index += 1u) {
        let neighbor = clamp(
            vec2<i32>(pixel) + bloom_offsets[sample_index],
            vec2<i32>(0),
            vec2<i32>(size) - vec2<i32>(1)
        );
        let neighbor_emission = textureLoad(pbr, neighbor, 0).a;
        bloom += textureLoad(albedo, neighbor, 0).rgb * neighbor_emission;
    }
    hdr += bloom * 0.16;
    let fog = smoothstep(0.78, 1.0, scene_depth);
    hdr = mix(hdr, vec3<f32>(0.045, 0.09, 0.16), fog * 0.45);
    let rain_column = floor(input.position.x * 0.31);
    let rain_phase = fract(sin(rain_column * 91.17) * 43758.5453 + camera.position_time.w * 1.9);
    let rain_y = fract(input.uv.y * 18.0 + rain_phase);
    let rain_x = abs(fract(input.position.x * 0.073 + rain_phase) - 0.5);
    let rain = select(0.0, 1.0, rain_x < 0.018 && rain_y < 0.11) * (1.0 - fog * 0.7);
    hdr += vec3<f32>(0.22, 0.38, 0.58) * rain * 0.24;
    let vignette_distance = distance(input.uv, vec2<f32>(0.5));
    let vignette = 1.0 - smoothstep(0.42, 0.78, vignette_distance) * 0.42;
    return vec4<f32>(gt_tonemap(max(hdr * vignette, vec3<f32>(0.0))), 1.0);
}
