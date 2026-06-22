struct Frame {
    dimensions: vec4<f32>,
    counters: vec4<u32>,
};

@group(0) @binding(0) var normals: texture_2d<f32>;
@group(0) @binding(1) var pbr: texture_2d<f32>;
@group(0) @binding(2) var depth: texture_depth_2d;
@group(0) @binding(3) var output_reflection: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var<uniform> frame: Frame;

fn hash_blue_noise(pixel: vec2<u32>, frame_index: u32) -> f32 {
    var state = pixel.x * 1973u + pixel.y * 9277u + frame_index * 26699u + 0x68bc21ebu;
    state = (state ^ (state >> 16u)) * 0x7feb352du;
    state = (state ^ (state >> 15u)) * 0x846ca68bu;
    state = state ^ (state >> 16u);
    return f32(state & 0x00ffffffu) / 16777215.0;
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
    if scene_depth >= 0.99999 || surface.x > 0.62 {
        textureStore(output_reflection, vec2<i32>(invocation.xy), vec4<f32>(0.0));
        return;
    }

    let normal = normalize(textureLoad(normals, vec2<i32>(source_pixel), 0).xyz);
    let uv = (vec2<f32>(source_pixel) + 0.5) / vec2<f32>(source_size);
    let view_ray = normalize(vec3<f32>((uv * 2.0 - 1.0) * vec2<f32>(1.7778, -1.0), -1.4));
    let reflected = reflect(view_ray, normal);
    let noise = hash_blue_noise(invocation.xy, frame.counters.x) - 0.5;
    let sky_mix = clamp(reflected.y * 0.5 + 0.5 + noise * 0.025, 0.0, 1.0);
    let sky = mix(vec3<f32>(0.025, 0.045, 0.085), vec3<f32>(0.20, 0.38, 0.62), sky_mix);
    let neon_direction = normalize(vec3<f32>(0.0, 0.72, -0.68));
    let neon = pow(max(dot(reflected, neon_direction), 0.0), 96.0)
        * vec3<f32>(5.0, 0.55, 0.12);
    let response = (1.0 - surface.x) * (0.28 + surface.y * 0.72);
    textureStore(
        output_reflection,
        vec2<i32>(invocation.xy),
        vec4<f32>((sky + neon) * response, surface.x)
    );
}
