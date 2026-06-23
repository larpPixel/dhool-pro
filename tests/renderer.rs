use dhool_pro::perf::{gt_tonemap_rgba8, kawase_blur_rgba32f};
use dhool_pro::renderer::RenderWarning;
use dhool_pro::scene::LightKind;
use dhool_pro::{
    build_aaa_scene, build_asset_showcase, build_demo_scene, CpuRenderer, DebugView, RenderConfig,
};

#[test]
fn demo_scene_renders_non_black_frame() {
    let scene = build_demo_scene();
    let mut renderer = CpuRenderer::new(RenderConfig::tiny_test());
    let output = renderer.render(&scene).expect("render demo scene");

    assert_eq!(output.width, 96);
    assert_eq!(output.height, 64);
    assert_eq!(output.color.len(), 96 * 64);
    assert!(output.average_luminance() > 0.01);
    assert!(output
        .pass_reports
        .iter()
        .any(|p| p.name == "partial-depth-prepass"));
    assert!(output
        .pass_reports
        .iter()
        .any(|p| p.name == "software-rt-reflection-supplement"));
    assert!(output
        .pass_reports
        .iter()
        .any(|p| p.name == "smaa-persistence-resolve"));
    assert!(output.ppm_bytes().starts_with(b"P6\n96 64\n255\n"));
    assert!(!output.warnings.iter().any(|warning| matches!(
        warning,
        RenderWarning::OverwrittenExpensiveMaterialArea { .. }
    )));
}

#[test]
fn debug_views_are_renderable() {
    let scene = build_demo_scene();
    let mut config = RenderConfig::tiny_test();
    config.debug_view = DebugView::Normals;
    let mut renderer = CpuRenderer::new(config);
    let normals = renderer.render(&scene).expect("normal debug render");

    let mut config = RenderConfig::tiny_test();
    config.debug_view = DebugView::MaterialId;
    let mut renderer = CpuRenderer::new(config);
    let material_ids = renderer.render(&scene).expect("material id debug render");

    assert_ne!(normals.color, material_ids.color);
}

#[test]
fn postprocess_cpp_kernels_are_callable_from_rust() {
    let hdr = [[2.0, 1.0, 0.25, 1.0], [0.0, 0.0, 0.0, 1.0]];
    let ldr = gt_tonemap_rgba8(&hdr, 1.0).expect("tone map");
    assert!(ldr[0][0] > ldr[0][2]);
    assert_eq!(ldr[1], [0, 0, 0, 255]);

    let blurred = kawase_blur_rgba32f(&hdr, 2, 1, 1).expect("kawase blur");
    assert_eq!(blurred.len(), 2);
    assert!(blurred[0][0] > 0.0);
}

#[test]
fn cc0_gltf_showcase_imports_geometry_and_texture() {
    let scene = build_asset_showcase("assets/demo/suzanne/Suzanne.gltf").expect("showcase scene");
    let imported = scene
        .meshes
        .iter()
        .find(|mesh| mesh.name.starts_with("web_asset_"))
        .expect("imported mesh");
    assert!(imported.indices.len() > 1_000);
    let material = &scene.materials[imported.material_index];
    assert!(material.base_color_texture.is_some());

    let mut renderer = CpuRenderer::new(RenderConfig::tiny_test());
    let output = renderer.render(&scene).expect("render imported asset");
    assert!(output.average_luminance() > 0.01);
}

#[test]
fn every_declared_debug_view_renders() {
    let scene = build_demo_scene();
    for view in DebugView::ALL {
        let mut config = RenderConfig::tiny_test();
        config.debug_view = view;
        let mut renderer = CpuRenderer::new(config);
        renderer.render(&scene).expect("debug view render");
    }
}

#[test]
fn aaa_scene_loads_cc0_pbr_assets() {
    let scene = build_aaa_scene("assets/aaa").expect("AAA scene");
    assert!(scene.meshes.len() >= 30);
    assert!(scene
        .meshes
        .iter()
        .any(|mesh| mesh.name.contains("trash_can")));
    assert!(scene
        .meshes
        .iter()
        .any(|mesh| mesh.name.contains("street_lamp")));
    assert!(scene.materials.iter().any(|material| {
        material.base_color_texture.is_some()
            && material.normal_texture.is_some()
            && material.pbr_texture.is_some()
    }));
    assert!(scene
        .meshes
        .iter()
        .filter(|mesh| mesh.is_floor_or_ground)
        .all(|mesh| mesh.shadow_class == dhool_pro::asset::ShadowCasterClass::None));
}

#[test]
fn aaa_practical_lights_are_shadowed_spots() {
    let scene = build_aaa_scene("assets/aaa").expect("AAA scene");
    for name in ["cyan_practical", "amber_practical"] {
        let light = scene
            .lights
            .iter()
            .find(|light| light.name == name)
            .expect("practical light");
        assert!(light.casts_shadow);
        let LightKind::Spot {
            direction,
            inner_angle_radians,
            outer_angle_radians,
            ..
        } = light.kind
        else {
            panic!("{name} should be a spot light");
        };
        assert!(direction.length() > 0.99);
        assert!(inner_angle_radians < outer_angle_radians);
        assert!(outer_angle_radians >= 67.0_f32.to_radians());
    }
}

#[test]
fn live_rain_path_does_not_bind_2d_overlay_assets() {
    let gpu = include_str!("../src/gpu.rs");
    let rain_shader = include_str!("../src/shaders/rain3d.wgsl");

    assert!(!gpu.contains("rain_overlay"));
    assert!(!gpu.contains("ripples_"));
    assert!(!rain_shader.contains("texture_2d<f32>"));
    assert!(!rain_shader.contains("rain_overlay"));
    assert!(!rain_shader.contains("ripples_"));
    assert!(rain_shader.contains("texture_depth_2d"));
}

#[test]
fn live_rain_uses_volumetric_prism_geometry() {
    let gpu = include_str!("../src/gpu.rs");
    let rain_shader = include_str!("../src/shaders/rain3d.wgsl");

    assert!(gpu.contains("RAIN_DROP_COUNT: u32 = 4_096"));
    assert!(gpu.contains("wgpu::PrimitiveTopology::TriangleList"));
    assert!(gpu.contains("pass.draw(0..RAIN_PRISM_VERTEX_COUNT, 0..RAIN_DROP_COUNT)"));
    assert!(rain_shader.contains("fn prism_local_vertex"));
    assert!(rain_shader.contains("@location(5) surface_normal"));
    assert!(!rain_shader.contains("to_camera"));
}

#[test]
fn rain_impacts_reflections_and_streaks_share_weather_field() {
    let gpu = include_str!("../src/gpu.rs");
    let rain_shader = include_str!("../src/shaders/rain3d.wgsl");
    let lighting_shader = include_str!("../src/shaders/lighting.wgsl");
    let reflection_shader = include_str!("../src/shaders/reflections.wgsl");

    assert!(rain_shader.contains("const RAIN_GRID_X: u32 = 64u"));
    assert!(lighting_shader.contains("const RAIN_GRID_X: u32 = 64u"));
    assert!(rain_shader.contains("fn rain_wind_direction"));
    assert!(lighting_shader.contains("fn rain_wind_direction"));
    assert!(reflection_shader.contains("fn rain_wind_direction"));
    assert!(reflection_shader.contains("@group(0) @binding(6) var<uniform> lights: Lights"));
    assert!(gpu.contains("uniform_entry(6, wgpu::ShaderStages::COMPUTE)"));
    assert!(lighting_shader.contains("reflection_energy_cap"));
}

#[test]
fn live_lighting_uses_real_local_shadows_and_post_bloom() {
    let gpu = include_str!("../src/gpu.rs");
    let lighting_shader = include_str!("../src/shaders/lighting.wgsl");
    let reflection_shader = include_str!("../src/shaders/reflections.wgsl");
    let bloom_shader = include_str!("../src/shaders/bloom.wgsl");
    let tonemap_shader = include_str!("../src/shaders/tonemap.wgsl");

    assert!(lighting_shader.contains("texture_depth_2d_array"));
    assert!(lighting_shader.contains("local_shadow_cameras"));
    assert!(lighting_shader.contains("PRACTICAL_SPOT_MIN_FILL"));
    assert!(lighting_shader.contains("let fd90 = 0.5 + 2.0 * ldoth * ldoth * roughness"));
    assert!(lighting_shader.contains("specular_occlusion"));
    assert!(reflection_shader.contains("fn trace_alley_proxy"));
    assert!(reflection_shader.contains("proxy_emissive_radiance"));
    assert!(reflection_shader.contains("rough_jitter"));
    assert!(!lighting_shader.contains("bloom_offsets"));
    assert!(gpu.contains("kawase-bloom-extract"));
    assert!(gpu.contains("gt-tonemap-bloom-resolve"));
    assert!(bloom_shader.contains("fs_extract"));
    assert!(tonemap_shader.contains("gt_tonemap"));
}
