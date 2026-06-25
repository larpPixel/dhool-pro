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
fn aaa_scene_uses_generated_puddle_breakup_maps() {
    let scene = build_aaa_scene("assets/aaa").expect("AAA scene");
    let puddle_material_index = scene
        .materials
        .iter()
        .position(|material| material.name == "foreground_clear_puddle")
        .expect("puddle material");
    let puddle_material = &scene.materials[puddle_material_index];

    assert!(puddle_material.normal_texture.is_some());
    assert!(puddle_material.pbr_texture.is_some());
    assert!(puddle_material.flags.glass);
    assert!(puddle_material.base_color.a < 0.5);
    assert_eq!(puddle_material.texture_sample_count, 3);
    assert!(std::path::Path::new("assets/aaa/puddles/generated_puddle_detail.png").is_file());
    assert!(std::path::Path::new("assets/aaa/puddles/generated_puddle_normal.png").is_file());
    assert!(std::path::Path::new("assets/aaa/puddles/generated_puddle_arm.png").is_file());

    let puddles: Vec<_> = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.material_index == puddle_material_index)
        .collect();
    assert!(puddles.len() >= 3);
    assert!(puddles
        .iter()
        .all(|mesh| mesh.is_floor_or_ground && !mesh.reflection_proxy));
}

#[test]
fn aaa_prepass_is_limited_to_selected_large_occluders() {
    let scene = build_aaa_scene("assets/aaa").expect("AAA scene");
    let prepassed: Vec<_> = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.prepass_eligible)
        .map(|mesh| mesh.name.as_str())
        .collect();

    for required in [
        "left_aged_brick",
        "right_aged_brick",
        "alley_end_brick",
        "left_loading_shutter",
        "right_loading_shutter",
        "left_recessed_service_bay",
        "far_scaffold_shadow",
    ] {
        assert!(
            prepassed.contains(&required),
            "{required} should be prepassed"
        );
    }
    assert!(prepassed.len() < scene.meshes.len() / 3);
    for required_proxy in [
        "rain_darkened_asphalt",
        "left_aged_brick",
        "center_dangling_cable_sheet",
        "far_scaffold_shadow",
    ] {
        let mesh = scene
            .meshes
            .iter()
            .find(|mesh| mesh.name == required_proxy)
            .expect("AAA reflection proxy mesh");
        assert!(
            mesh.reflection_proxy,
            "{required_proxy} should feed reflection proxies"
        );
    }
    assert!(scene
        .meshes
        .iter()
        .filter(|mesh| mesh.name.contains("barrel") || mesh.name.contains("street_lamp"))
        .all(|mesh| !mesh.prepass_eligible));
    assert!(scene
        .meshes
        .iter()
        .filter(|mesh| mesh.is_floor_or_ground)
        .all(|mesh| !mesh.prepass_eligible));
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
fn live_rain_path_uses_generated_detail_atlas_not_flat_overlay_assets() {
    let gpu = include_str!("../src/gpu.rs");
    let rain_shader = include_str!("../src/shaders/rain3d.wgsl");

    assert!(std::path::Path::new("assets/aaa/rain/generated_streak_detail.png").is_file());
    assert!(gpu.contains("GENERATED_RAIN_STREAK_DETAIL"));
    assert!(gpu.contains("rain-streak-detail"));
    assert!(rain_shader.contains("var rain_streak_detail: texture_2d<f32>"));
    assert!(rain_shader.contains("fn generated_streak_detail"));
    assert!(!gpu.contains("rain_overlay"));
    assert!(!gpu.contains("ripples_"));
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
    let geometry_shader = include_str!("../src/shaders/geometry.wgsl");
    let rain_shader = include_str!("../src/shaders/rain3d.wgsl");
    let lighting_shader = include_str!("../src/shaders/lighting.wgsl");
    let reflection_shader = include_str!("../src/shaders/reflections.wgsl");
    let ssr_shader = include_str!("../src/shaders/ssr.wgsl");

    assert!(rain_shader.contains("const RAIN_GRID_X: u32 = 64u"));
    assert!(lighting_shader.contains("const RAIN_GRID_X: u32 = 64u"));
    assert!(rain_shader.contains("fn rain_wind_direction"));
    assert!(lighting_shader.contains("fn rain_wind_direction"));
    assert!(reflection_shader.contains("fn rain_wind_direction"));
    assert!(reflection_shader.contains("@group(0) @binding(7) var<uniform> lights: Lights"));
    assert!(ssr_shader.contains("fn trace_screen_space_reflection"));
    assert!(reflection_shader.contains("fn temporal_reconstruct_reflection"));
    assert!(reflection_shader.contains("var reflection_history: texture_2d<f32>"));
    assert!(reflection_shader.contains("var motion_vectors: texture_2d<f32>"));
    assert!(reflection_shader.contains("var<uniform> reflection_proxies: ReflectionProxies"));
    assert!(reflection_shader.contains("var half_resolution_ssr: texture_2d<f32>"));
    assert!(reflection_shader.contains("fn trace_scene_proxy"));
    assert!(gpu.contains("uniform_entry(7, wgpu::ShaderStages::COMPUTE)"));
    assert!(gpu.contains("reflection_proxy_uniform(&scene)"));
    assert!(gpu.contains("uniform_entry(10, wgpu::ShaderStages::COMPUTE)"));
    assert!(geometry_shader.contains("let material_flag = material.surface.w"));
    assert!(
        geometry_shader.contains("output.normal_flags = vec4<f32>(mapped_normal, material_flag)")
    );
    assert!(gpu.contains("translucent_albedo_target(ALBEDO_FORMAT)"));
    assert!(lighting_shader.contains("glass_mask"));
    assert!(reflection_shader.contains("glass_mask"));
    assert!(reflection_shader.contains("water_glass_mask"));
    assert!(reflection_shader.contains("upright_glass_mask"));
    assert!(reflection_shader.contains("source_pixel.x & 1u"));
    assert!(lighting_shader.contains("reflection_energy_cap"));
}

#[test]
fn live_lighting_uses_real_local_shadows_and_post_bloom() {
    let gpu = include_str!("../src/gpu.rs");
    let lighting_shader = include_str!("../src/shaders/lighting.wgsl");
    let reflection_shader = include_str!("../src/shaders/reflections.wgsl");
    let ssr_shader = include_str!("../src/shaders/ssr.wgsl");
    let bloom_shader = include_str!("../src/shaders/bloom.wgsl");
    let tonemap_shader = include_str!("../src/shaders/tonemap.wgsl");

    assert!(lighting_shader.contains("texture_depth_2d_array"));
    assert!(lighting_shader.contains("local_shadow_cameras"));
    assert!(lighting_shader.contains("PRACTICAL_SPOT_MIN_FILL"));
    assert!(lighting_shader.contains("let fd90 = 0.5 + 2.0 * ldoth * ldoth * roughness"));
    assert!(lighting_shader.contains("specular_occlusion"));
    assert!(lighting_shader.contains("distance_to_light >= position_radius.w"));
    assert!(lighting_shader.contains("outer_halo"));
    assert!(lighting_shader.contains("spot_presence"));
    assert!(lighting_shader.contains("local_shadow_weight > 0.001"));
    assert!(ssr_shader.contains("fn surface_direct_radiance"));
    assert!(!reflection_shader.contains("fn trace_alley_proxy"));
    assert!(!reflection_shader.contains("proxy_emissive_radiance"));
    assert!(reflection_shader.contains("rough_jitter"));
    assert!(!lighting_shader.contains("bloom_offsets"));
    assert!(gpu.contains("kawase-bloom-extract"));
    assert!(gpu.contains("gt-tonemap-bloom-resolve"));
    assert!(bloom_shader.contains("fs_extract"));
    assert!(tonemap_shader.contains("gt_tonemap"));
}

#[test]
fn live_gpu_uses_motion_smaa_stages_and_shadow_cache_flags() {
    let gpu = include_str!("../src/gpu.rs");
    let geometry_shader = include_str!("../src/shaders/geometry.wgsl");
    let aa_shader = include_str!("../src/shaders/aa.wgsl");
    let depth_copy_shader = include_str!("../src/shaders/depth_copy.wgsl");
    let tonemap_shader = include_str!("../src/shaders/tonemap.wgsl");

    assert!(gpu.contains("motion-vectors-rg16"));
    assert!(gpu.contains("smaa-edge-detect"));
    assert!(gpu.contains("smaa-blend-weights"));
    assert!(gpu.contains("depth-copy-r32f"));
    assert!(gpu.contains("depth-copy-r32f-fullscreen"));
    assert!(gpu.contains("depth_copy_pipeline: wgpu::RenderPipeline"));
    assert!(!gpu.contains("depth-copy-r32f-compute"));
    assert!(depth_copy_shader.contains("fn vs_main"));
    assert!(depth_copy_shader.contains("fn fs_main"));
    assert!(!depth_copy_shader.contains("@compute"));
    assert!(gpu.contains("hiz-generation-min-max-discontinuity"));
    assert!(gpu.contains("blue-noise-volumetric-fog"));
    assert!(gpu.contains("shadow_cache_ready"));
    assert!(gpu.contains("local_shadow_cache_ready"));
    assert!(gpu.contains("load_color_attachment(&self.gbuffer.albedo)"));
    assert!(geometry_shader.contains("@location(3) motion_vectors"));
    assert!(aa_shader.contains("@group(0) @binding(5) var motion_vectors"));
    assert!(aa_shader.contains("@group(0) @binding(6) var smaa_blend"));
    assert!(tonemap_shader.contains("@group(0) @binding(2) var volumetric_fog"));
    assert!(gpu.contains("reflection-proxy-uniform"));
    assert!(gpu.contains("MAX_REFLECTION_PROXIES"));
}

#[test]
fn gallery_default_points_at_live_aaa_scene() {
    let main = include_str!("../src/main.rs");
    let demo = include_str!("../src/demo.rs");
    let readme = include_str!("../README.md");

    assert!(main.contains(".unwrap_or(\"assets/aaa\")"));
    assert!(demo.contains("build_aaa_scene(asset_path)"));
    assert!(readme.contains("same `assets/aaa` industrial alley scene used by `--live`"));
}
