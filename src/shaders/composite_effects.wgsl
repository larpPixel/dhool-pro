struct Frame {
    dimensions: vec4<f32>,
    counters: vec4<u32>,
};

struct Lights {
    directional_direction_count: vec4<f32>,
    directional_color_intensity: vec4<f32>,
    ambient: vec4<f32>,
    sky: vec4<f32>,
    weather: vec4<f32>,
    local_position_radius: array<vec4<f32>, 6>,
    local_color_intensity: array<vec4<f32>, 6>,
    local_direction_outer: array<vec4<f32>, 6>,
    local_inner_shadow: array<vec4<f32>, 6>,
};

@group(0) @binding(0) var scene_color: texture_2d<f32>;
@group(0) @binding(1) var scene_depth: texture_2d<f32>;
@group(0) @binding(2) var<uniform> frame: Frame;
@group(0) @binding(3) var<uniform> lights: Lights;

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

fn hash(value: vec2<f32>) -> f32 {
    return fract(sin(dot(value, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn load_scene(uv: vec2<f32>) -> vec3<f32> {
    let size = textureDimensions(scene_color);
    let pixel = clamp(
        vec2<i32>(uv * vec2<f32>(size)),
        vec2<i32>(0),
        vec2<i32>(size) - vec2<i32>(1)
    );
    return textureLoad(scene_color, pixel, 0).rgb;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let rainfall = clamp(lights.weather.x, 0.0, 1.0);
    if rainfall <= 0.001 {
        return vec4<f32>(0.0);
    }
    let aspect = frame.dimensions.x / max(frame.dimensions.y, 1.0);
    let cells = vec2<f32>(13.0 * aspect, 13.0);
    let cell = floor(input.uv * cells);
    let local = fract(input.uv * cells) - vec2<f32>(0.5);
    let seed = hash(cell);
    let phase = fract(f32(frame.counters.x) * 0.0028 + seed);
    let drop_center = vec2<f32>((hash(cell.yx + 9.0) - 0.5) * 0.52, phase * 1.5 - 0.62);
    let drop_delta = local - drop_center;
    let radial = length(drop_delta * vec2<f32>(1.0, 1.55));
    let ring = smoothstep(0.23, 0.16, abs(radial - 0.20));
    let body = smoothstep(0.18, 0.02, radial);
    let mask = clamp((ring * 0.72 + body * 0.24) * step(0.58, seed) * rainfall, 0.0, 0.82);
    if mask <= 0.0001 {
        return vec4<f32>(0.0);
    }
    let normal = normalize(vec2<f32>(drop_delta.x, drop_delta.y * 1.55) + vec2<f32>(0.0001));
    let refracted = load_scene(clamp(input.uv + normal * 0.0065 * mask, vec2<f32>(0.0), vec2<f32>(1.0)));
    let local_light = lights.directional_color_intensity.rgb
        * lights.directional_color_intensity.w * 0.015
        + lights.sky.rgb * 0.08;
    let highlight = pow(max(dot(normalize(vec3<f32>(normal, 0.7)), normalize(vec3<f32>(-0.4, -0.5, 0.8))), 0.0), 18.0);
    let color = refracted * (0.22 + mask * 0.18) + local_light * highlight;
    return vec4<f32>(color, mask);
}
