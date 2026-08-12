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

@group(0) @binding(0) var lighting_hdr: texture_2d<f32>;
@group(0) @binding(1) var exposure_white_balance: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> lights: Lights;

var<workgroup> log_luminance: array<f32, 64>;

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(
    @builtin(local_invocation_index) local_index: u32,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let size = textureDimensions(lighting_hdr);
    var sum = 0.0;
    var count = 0u;
    // Each lane covers a strided portion of the frame. Lighting alpha contains
    // the material-independent diffuse illuminance proxy emitted by lighting.
    for (var y = local_id.y; y < size.y; y += 8u) {
        for (var x = local_id.x; x < size.x; x += 8u) {
            let diffuse_illuminance = textureLoad(lighting_hdr, vec2<i32>(i32(x), i32(y)), 0).a;
            sum += log(max(diffuse_illuminance, 0.0001));
            count += 1u;
        }
    }
    log_luminance[local_index] = sum / f32(max(count, 1u));
    workgroupBarrier();

    var stride = 32u;
    loop {
        if local_index < stride {
            log_luminance[local_index] += log_luminance[local_index + stride];
        }
        workgroupBarrier();
        if stride == 1u { break; }
        stride /= 2u;
    }

    if local_index == 0u {
        let geometric_mean = exp(log_luminance[0] / 64.0);
        let exposure = clamp(0.18 / max(geometric_mean, 0.0001), 0.35, 3.5);

        var illuminant = lights.ambient.rgb + lights.sky.rgb * 0.35;
        illuminant += lights.directional_color_intensity.rgb
            * lights.directional_color_intensity.w;
        let local_count = u32(clamp(lights.directional_direction_count.w, 0.0, 6.0));
        for (var index = 0u; index < local_count; index += 1u) {
            illuminant += lights.local_color_intensity[index].rgb
                * lights.local_color_intensity[index].w * 0.12;
        }
        let neutral = max(luminance(illuminant), 0.0001);
        let gains = clamp(vec3<f32>(neutral) / max(illuminant, vec3<f32>(0.0001)), vec3<f32>(0.78), vec3<f32>(1.28));
        textureStore(exposure_white_balance, vec2<i32>(0), vec4<f32>(gains, exposure));
    }
}
