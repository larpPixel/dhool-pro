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

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn clamp_pixel(pixel: vec2<i32>, size: vec2<u32>) -> vec2<i32> {
    return clamp(pixel, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1));
}

fn load_color(pixel: vec2<i32>, size: vec2<u32>) -> vec3<f32> {
    return textureLoad(current_color, clamp_pixel(pixel, size), 0).rgb;
}

fn load_depth(pixel: vec2<i32>, size: vec2<u32>) -> f32 {
    return textureLoad(scene_depth, clamp_pixel(pixel, size), 0);
}

fn edge_amount(pixel: vec2<i32>, size: vec2<u32>, center: vec3<f32>, center_depth: f32) -> f32 {
    let left = load_color(pixel + vec2<i32>(-1, 0), size);
    let right = load_color(pixel + vec2<i32>(1, 0), size);
    let up = load_color(pixel + vec2<i32>(0, -1), size);
    let down = load_color(pixel + vec2<i32>(0, 1), size);
    let center_luma = luminance(center);
    let color_edge = max(
        max(abs(center_luma - luminance(left)), abs(center_luma - luminance(right))),
        max(abs(center_luma - luminance(up)), abs(center_luma - luminance(down)))
    );
    let depth_edge = max(
        max(abs(center_depth - load_depth(pixel + vec2<i32>(-1, 0), size)), abs(center_depth - load_depth(pixel + vec2<i32>(1, 0), size))),
        max(abs(center_depth - load_depth(pixel + vec2<i32>(0, -1), size)), abs(center_depth - load_depth(pixel + vec2<i32>(0, 1), size)))
    ) * 18.0;
    let threshold = aa.params.y;
    return smoothstep(threshold, threshold * 2.5, max(color_edge, depth_edge));
}

fn smaa_like_resolve(pixel: vec2<i32>, size: vec2<u32>, center: vec3<f32>, edge: f32) -> vec3<f32> {
    if edge <= 0.0 {
        return center;
    }

    let left = load_color(pixel + vec2<i32>(-1, 0), size);
    let right = load_color(pixel + vec2<i32>(1, 0), size);
    let up = load_color(pixel + vec2<i32>(0, -1), size);
    let down = load_color(pixel + vec2<i32>(0, 1), size);
    let horizontal_gradient = abs(luminance(left) - luminance(right));
    let vertical_gradient = abs(luminance(up) - luminance(down));
    let horizontal_blend = (left + right) * 0.5;
    let vertical_blend = (up + down) * 0.5;
    let blend = select(vertical_blend, horizontal_blend, horizontal_gradient > vertical_gradient);
    return mix(center, blend, edge * aa.params.z);
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

fn history_uv(pixel_uv: vec2<f32>, depth: f32) -> vec2<f32> {
    let current_clip = vec4<f32>(pixel_uv.x * 2.0 - 1.0, 1.0 - pixel_uv.y * 2.0, depth, 1.0);
    let world_h = camera.inverse_view_projection * current_clip;
    let world = vec4<f32>(world_h.xyz / max(world_h.w, 0.00001), 1.0);
    let previous_clip = aa.previous_view_projection * world;
    let previous_ndc = previous_clip.xyz / max(previous_clip.w, 0.00001);
    return previous_ndc.xy * vec2<f32>(0.5, -0.5) + 0.5;
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    let size = textureDimensions(current_color);
    let pixel_u = min(vec2<u32>(input.position.xy), size - vec2<u32>(1u));
    let pixel = vec2<i32>(pixel_u);
    let center = textureLoad(current_color, pixel, 0).rgb;
    let depth = textureLoad(scene_depth, pixel, 0);
    let edge = edge_amount(pixel, size, center, depth);
    var resolved = smaa_like_resolve(pixel, size, center, edge);

    if aa.params.x > 0.0 && depth < 0.99999 {
        let uv = (vec2<f32>(pixel_u) + 0.5) / vec2<f32>(size);
        let previous_uv = history_uv(uv, depth);
        if all(previous_uv >= vec2<f32>(0.0)) && all(previous_uv <= vec2<f32>(1.0)) {
            let history_size = textureDimensions(history_color);
            let history_pixel = clamp(
                vec2<i32>(previous_uv * vec2<f32>(history_size)),
                vec2<i32>(0),
                vec2<i32>(history_size) - vec2<i32>(1)
            );
            let history_sample = textureLoad(history_color, history_pixel, 0).rgb;
            let low = neighborhood_min(pixel, size, resolved) - vec3<f32>(0.015);
            let high = neighborhood_max(pixel, size, resolved) + vec3<f32>(0.015);
            let clamped_history = clamp(history_sample, low, high);
            let history_weight = aa.params.x * (1.0 - edge * 0.7);
            resolved = mix(resolved, clamped_history, history_weight);
        }
    }

    let output = vec4<f32>(resolved, 1.0);
    return FragmentOutput(output, output);
}
