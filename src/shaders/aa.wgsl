struct Camera {
    view_projection: mat4x4<f32>,
    inverse_view_projection: mat4x4<f32>,
    position_time: vec4<f32>,
    viewport: vec4<f32>,
};

struct AntiAlias {
    previous_view_projection: mat4x4<f32>,
    viewport: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var current_color: texture_2d<f32>;
@group(0) @binding(1) var history_color: texture_2d<f32>;
@group(0) @binding(2) var scene_depth: texture_depth_2d;
@group(0) @binding(3) var<uniform> camera: Camera;
@group(0) @binding(4) var<uniform> aa: AntiAlias;
@group(0) @binding(5) var motion_vectors: texture_2d<f32>;
@group(0) @binding(6) var smaa_blend: texture_2d<f32>;
@group(0) @binding(7) var normal_flags: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FragmentOutput {
    @location(0) presented: vec4<f32>,
    @location(1) history: vec4<f32>,
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

fn clamp_pixel(pixel: vec2<i32>, size: vec2<u32>) -> vec2<i32> {
    return clamp(pixel, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1));
}

fn load_color(pixel: vec2<i32>, size: vec2<u32>) -> vec3<f32> {
    return textureLoad(current_color, clamp_pixel(pixel, size), 0).rgb;
}

fn neighborhood_min(pixel: vec2<i32>, size: vec2<u32>, center: vec3<f32>) -> vec3<f32> {
    var low = center;
    low = min(low, load_color(pixel + vec2<i32>(-1, 0), size));
    low = min(low, load_color(pixel + vec2<i32>(1, 0), size));
    low = min(low, load_color(pixel + vec2<i32>(0, -1), size));
    low = min(low, load_color(pixel + vec2<i32>(0, 1), size));
    return low;
}

fn neighborhood_max(pixel: vec2<i32>, size: vec2<u32>, center: vec3<f32>) -> vec3<f32> {
    var high = center;
    high = max(high, load_color(pixel + vec2<i32>(-1, 0), size));
    high = max(high, load_color(pixel + vec2<i32>(1, 0), size));
    high = max(high, load_color(pixel + vec2<i32>(0, -1), size));
    high = max(high, load_color(pixel + vec2<i32>(0, 1), size));
    return high;
}

fn smaa_resolve(pixel: vec2<i32>, size: vec2<u32>, center: vec3<f32>) -> vec3<f32> {
    let blend = textureLoad(smaa_blend, clamp_pixel(pixel, size), 0).rgb;
    if blend.z <= 0.0001 {
        return center;
    }
    let left = load_color(pixel + vec2<i32>(-1, 0), size);
    let right = load_color(pixel + vec2<i32>(1, 0), size);
    let up = load_color(pixel + vec2<i32>(0, -1), size);
    let down = load_color(pixel + vec2<i32>(0, 1), size);
    let horizontal = mix(center, (left + right) * 0.5, blend.x);
    let vertical = mix(center, (up + down) * 0.5, blend.y);
    return mix(horizontal, vertical, blend.y / max(blend.x + blend.y, 0.0001));
}

fn fps_glyph_mask(glyph: u32) -> u32 {
    switch glyph {
        case 0u: { return 0x7b6fu; }
        case 1u: { return 0x749au; }
        case 2u: { return 0x73e7u; }
        case 3u: { return 0x79e7u; }
        case 4u: { return 0x49edu; }
        case 5u: { return 0x79cfu; }
        case 6u: { return 0x7bcfu; }
        case 7u: { return 0x24a7u; }
        case 8u: { return 0x7befu; }
        case 9u: { return 0x79efu; }
        case 10u: { return 0x13cfu; }
        case 11u: { return 0x12ebu; }
        case 12u: { return 0x79cfu; }
        default: { return 0u; }
    }
}

fn fps_overlay_glyph(slot: i32, fps: u32) -> u32 {
    if slot == 0 {
        return 10u;
    }
    if slot == 1 {
        return 11u;
    }
    if slot == 2 {
        return 12u;
    }
    if slot == 4 {
        return fps / 100u;
    }
    if slot == 5 {
        return (fps / 10u) % 10u;
    }
    if slot == 6 {
        return fps % 10u;
    }
    return 99u;
}

fn fps_overlay_coverage(pixel: vec2<i32>) -> f32 {
    let scale = 3;
    let glyph_size = vec2<i32>(3 * scale, 5 * scale);
    let gap = 2 * scale;
    let origin = vec2<i32>(14, 14);
    let local = pixel - origin;
    if any(local < vec2<i32>(0)) {
        return 0.0;
    }
    let stride = glyph_size.x + gap;
    let slot = local.x / stride;
    if slot < 0 || slot > 6 || local.y >= glyph_size.y {
        return 0.0;
    }
    let glyph_local = vec2<i32>(local.x - slot * stride, local.y);
    if glyph_local.x >= glyph_size.x {
        return 0.0;
    }
    let cell = glyph_local / scale;
    let glyph = fps_overlay_glyph(slot, u32(clamp(aa.params.w + 0.5, 0.0, 999.0)));
    let mask = fps_glyph_mask(glyph);
    let bit_index = u32(cell.y * 3 + cell.x);
    return f32((mask >> bit_index) & 1u);
}

fn apply_fps_overlay(color: vec3<f32>, pixel: vec2<i32>) -> vec3<f32> {
    let panel_origin = vec2<i32>(9, 9);
    let panel_size = vec2<i32>(96, 27);
    let in_panel = all(pixel >= panel_origin) && all(pixel < panel_origin + panel_size);
    var output = color;
    if in_panel {
        output = mix(output, vec3<f32>(0.012, 0.016, 0.012), 0.58);
    }
    let glyph = fps_overlay_coverage(pixel);
    return mix(output, vec3<f32>(1.0, 0.82, 0.34), glyph);
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    let size = textureDimensions(current_color);
    let pixel_u = min(vec2<u32>(input.position.xy), size - vec2<u32>(1u));
    let pixel = vec2<i32>(pixel_u);
    let center = textureLoad(current_color, pixel, 0).rgb;
    let depth = textureLoad(scene_depth, pixel, 0);
    let spatial_resolve = smaa_resolve(pixel, size, center);
    var resolved = spatial_resolve;
    let edge_strength = textureLoad(smaa_blend, pixel, 0).b;
    let material_flags = u32(round(textureLoad(normal_flags, pixel, 0).w));
    let responsive = (material_flags & 2u) != 0u;

    if aa.params.x > 0.0 && edge_strength > 0.0001 && depth > 0.00001 && !responsive {
        let uv = (vec2<f32>(pixel_u) + 0.5) / vec2<f32>(size);
        let motion = textureLoad(motion_vectors, pixel, 0).rg;
        let previous_uv = uv - motion;
        if all(previous_uv >= vec2<f32>(0.0)) && all(previous_uv <= vec2<f32>(1.0)) {
            let history_size = textureDimensions(history_color);
            let history_pixel = clamp(
                vec2<i32>(previous_uv * vec2<f32>(history_size)),
                vec2<i32>(0),
                vec2<i32>(history_size) - vec2<i32>(1)
            );
            let history_sample = textureLoad(history_color, history_pixel, 0).rgb;
            let low = neighborhood_min(pixel, size, spatial_resolve) - vec3<f32>(0.018);
            let high = neighborhood_max(pixel, size, spatial_resolve) + vec3<f32>(0.018);
            let clamped_history = clamp(history_sample, low, high);
            let motion_rejection = smoothstep(0.012, 0.09, length(motion));
            let history_weight = aa.params.x * edge_strength * (1.0 - motion_rejection * 0.65);
            resolved = mix(resolved, clamped_history, history_weight);
        }
    }

    let presented = apply_fps_overlay(resolved, pixel);
    // History is the current frame's spatial SMAA result, never the temporally
    // blended presentation. This keeps T2x to two frames instead of silently
    // turning it into an accumulation buffer.
    return FragmentOutput(vec4<f32>(presented, 1.0), vec4<f32>(spatial_resolve, edge_strength));
}
