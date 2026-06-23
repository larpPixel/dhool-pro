@group(0) @binding(0) var source_tex: texture_2d<f32>;

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

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn load_clamped(pixel: vec2<i32>) -> vec3<f32> {
    let size = textureDimensions(source_tex);
    let clamped = clamp(pixel, vec2<i32>(0), vec2<i32>(size) - vec2<i32>(1));
    return textureLoad(source_tex, clamped, 0).rgb;
}

@fragment
fn fs_extract(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(input.position.xy);
    let base = pixel * 4;
    var color = vec3<f32>(0.0);
    for (var y = 0; y < 4; y += 1) {
        for (var x = 0; x < 4; x += 1) {
            color += load_clamped(base + vec2<i32>(x, y));
        }
    }
    color *= 1.0 / 16.0;
    let bright = smoothstep(0.62, 1.75, luminance(color));
    return vec4<f32>(color * bright, 1.0);
}

fn kawase_blur(pixel: vec2<i32>, axis: vec2<i32>) -> vec3<f32> {
    let offsets = array<i32, 7>(-5, -3, -1, 0, 1, 3, 5);
    let weights = array<f32, 7>(0.045, 0.095, 0.185, 0.35, 0.185, 0.095, 0.045);
    var color = vec3<f32>(0.0);
    for (var i = 0u; i < 7u; i += 1u) {
        color += load_clamped(pixel + axis * offsets[i]) * weights[i];
    }
    return color;
}

@fragment
fn fs_blur_horizontal(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(input.position.xy);
    return vec4<f32>(kawase_blur(pixel, vec2<i32>(1, 0)), 1.0);
}

@fragment
fn fs_blur_vertical(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(input.position.xy);
    return vec4<f32>(kawase_blur(pixel, vec2<i32>(0, 1)), 1.0);
}
