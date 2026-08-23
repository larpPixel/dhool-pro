use dhool_pro::math::{Mat4, Vec4};
use dhool_pro::perf::{gt_tonemap_rgba8, kawase_blur_rgba32f};
use dhool_pro::renderer::RenderWarning;
use dhool_pro::scene::LightKind;
use dhool_pro::{
    build_aaa_scene, build_asset_showcase, build_demo_scene, build_dust2_scene, CpuRenderer,
    DebugView, RenderConfig,
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
fn reverse_z_projection_maps_near_to_one_and_far_to_zero() {
    let near = 0.1;
    let far = 1_000.0;
    let projection = Mat4::perspective_reverse_rh(60.0_f32.to_radians(), 16.0 / 9.0, near, far);

    let near_clip = projection.mul_vec4(Vec4::new(0.0, 0.0, -near, 1.0));
    let far_clip = projection.mul_vec4(Vec4::new(0.0, 0.0, -far, 1.0));
    let near_depth = near_clip.z / near_clip.w;
    let far_depth = far_clip.z / far_clip.w;

    assert!(
        (near_depth - 1.0).abs() < 1.0e-5,
        "near depth was {near_depth}"
    );
    assert!(far_depth.abs() < 1.0e-5, "far depth was {far_depth}");
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
    assert_eq!(scene.environment.precipitation, 1.0);
    assert_eq!(scene.environment.surface_wetness, 1.0);
    assert!(scene
        .meshes
        .iter()
        .any(|mesh| { mesh.name == "monsoon_cloth_validation_awning" && mesh.cloth.is_some() }));
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
fn aaa_scene_uses_generated_decal_assets_and_deeper_set_dressing() {
    let scene = build_aaa_scene("assets/aaa").expect("AAA scene");
    for path in [
        "assets/aaa/generated/cyan_transit_panel.png",
        "assets/aaa/generated/amber_hazard_placard.png",
        "assets/aaa/generated/wet_tarp_cable_net.png",
        "assets/aaa/generated/city_shopfront_facade.png",
        "assets/aaa/generated/city_apartment_skyline.png",
        "assets/aaa/generated/monsoon_market_awning.png",
    ] {
        assert!(std::path::Path::new(path).is_file(), "{path} should exist");
    }

    for name in [
        "generated_cyan_transit_panel",
        "generated_amber_hazard_placard",
        "generated_wet_tarp_cable_net",
        "generated_city_shopfront_facade",
        "generated_city_apartment_skyline",
        "generated_monsoon_market_awning",
    ] {
        let material = scene
            .materials
            .iter()
            .find(|material| material.name == name)
            .expect("generated material");
        assert!(material.base_color_texture.is_some());
        assert_eq!(material.texture_sample_count, 1);
    }

    for mesh_name in [
        "left_generated_cyan_transit_panel",
        "right_generated_amber_hazard_placard",
        "deep_generated_cyan_transit_panel",
        "left_wet_tarp_cable_net",
        "center_hanging_tarp_cable_sheet",
        "deep_overhead_service_bridge",
        "deep_cyan_panel_reflection_pool",
        "left_city_shopfront_row_near_shell",
        "left_city_shopfront_row_near_projecting_awning_0",
        "left_city_shopfront_row_near_structural_post_1",
        "right_city_shopfront_row_near_shell",
        "right_city_shopfront_row_near_shutter_2",
        "city_cross_street_wet_asphalt",
        "left_market_lane_wet_pavers",
        "right_bazaar_lane_wet_pavers",
        "central_apartment_skyline_tower_0_shell",
        "central_apartment_skyline_tower_2_window_grid",
        "central_apartment_skyline_tower_4_roof_tank",
        "center_bazaar_overhead_tarp_slab",
        "center_bazaar_overhead_tarp_pole_2",
        "city_cross_street_reflection_lake",
    ] {
        assert!(
            scene.meshes.iter().any(|mesh| mesh.name == mesh_name),
            "{mesh_name} should be in the expanded city scene"
        );
    }

    for removed_billboard in [
        "left_city_shopfront_row_near",
        "right_city_shopfront_row_near",
        "far_left_apartment_skyline_wall",
        "far_right_apartment_skyline_wall",
        "central_apartment_skyline_tower_wall",
        "left_monsoon_market_awning_wall",
        "right_monsoon_market_awning_wall",
        "far_monsoon_market_awning_gate",
    ] {
        assert!(
            !scene
                .meshes
                .iter()
                .any(|mesh| mesh.name == removed_billboard),
            "{removed_billboard} should not remain as a flat billboard"
        );
    }

    let city_generated_materials: Vec<_> = [
        "generated_city_shopfront_facade",
        "generated_city_apartment_skyline",
        "generated_monsoon_market_awning",
    ]
    .into_iter()
    .map(|name| {
        scene
            .materials
            .iter()
            .position(|material| material.name == name)
            .expect("city generated material")
    })
    .collect();
    assert!(scene
        .meshes
        .iter()
        .filter(|mesh| city_generated_materials.contains(&mesh.material_index))
        .all(|mesh| mesh.vertices.len() > 4));

    assert!(scene.meshes.len() >= 90);
    assert!(scene.camera.far >= 160.0);
    assert!(scene.camera.position.z < -28.0);
    assert!(scene.camera.target.z < -58.0);
    assert!(scene
        .lights
        .iter()
        .any(|light| light.name == "deep_cyan_panel_fill"));
    assert!(scene
        .lights
        .iter()
        .any(|light| light.name == "deep_amber_service_fill"));
    assert!(scene
        .lights
        .iter()
        .any(|light| light.name == "city_cross_street_cyan"));
    assert!(scene
        .lights
        .iter()
        .any(|light| light.name == "market_lane_amber"));
}

#[test]
fn dust2_scene_uses_local_cs2_export_and_long_view_distance() {
    let exported_world =
        std::path::Path::new("assets/dust2/generated/cs2/maps/de_dust2/world_physics_physics.glb");
    if exported_world.is_file() {
        assert!(exported_world.metadata().expect("Dust2 physics GLB").len() > 40_000_000);
        assert!(std::path::Path::new("assets/dust2/generated/cs2/maps/de_dust2.glb").is_file());

        let texture_count = std::fs::read_dir("assets/dust2/generated/cs2/maps")
            .expect("Dust2 texture directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "png")
            })
            .count();
        assert!(texture_count >= 600);
        assert!(std::path::Path::new(
            "assets/dust2/generated/cs2/materials/skybox/sky_de_dust2.png"
        )
        .is_file());
        let manifest =
            std::fs::read_to_string("assets/dust2/generated/cs2/manifest.json").expect("manifest");
        assert!(manifest.contains("\"visual_mesh\": \"maps/de_dust2.glb\""));
        assert!(manifest.contains("\"visual_texture_count\": 648"));
        assert!(manifest.contains("\"skybox_texture\": \"materials/skybox/sky_de_dust2.png\""));

        let previous_physics_import = std::env::var_os("DHOOL_DUST2_PHYSICS_IMPORT");
        std::env::set_var("DHOOL_DUST2_PHYSICS_IMPORT", "1");
        let scene = build_dust2_scene("assets/dust2").expect("CS2 Dust2 scene");
        if let Some(previous_physics_import) = previous_physics_import {
            std::env::set_var("DHOOL_DUST2_PHYSICS_IMPORT", previous_physics_import);
        } else {
            std::env::remove_var("DHOOL_DUST2_PHYSICS_IMPORT");
        }
        let triangles: usize = scene.meshes.iter().map(|mesh| mesh.indices.len()).sum();
        let vertices: usize = scene.meshes.iter().map(|mesh| mesh.vertices.len()).sum();

        assert!(scene
            .meshes
            .iter()
            .any(|mesh| mesh.name.starts_with("cs2_dust2_physics_group_concrete")));
        assert!(scene
            .meshes
            .iter()
            .any(|mesh| mesh.name.starts_with("cs2_dust2_physics_group_metal")));
        assert!(scene
            .meshes
            .iter()
            .any(|mesh| mesh.name.starts_with("cs2_dust2_physics_group_wood")));
        assert!(!scene
            .meshes
            .iter()
            .any(|mesh| mesh.name.starts_with("cs2_dust2_visual_")));
        assert!(scene
            .materials
            .iter()
            .any(|material| material.name == "cs2_dust2_concrete_plaster"));
        assert!(scene
            .materials
            .iter()
            .any(|material| material.name == "cs2_dust2_glass"));
        assert!(
            scene
                .materials
                .iter()
                .filter(|material| material.base_color_texture.is_some())
                .count()
                >= 8
        );
        assert!(!scene
            .meshes
            .iter()
            .any(|mesh| mesh.name == "cs2_dust2_skybox_shell"));
        assert!(!scene
            .materials
            .iter()
            .any(|material| material.name == "cs2_dust2_skybox"));

        assert!(scene.meshes.len() >= 30);
        assert!(triangles >= 400_000);
        assert!(vertices >= 1_000_000);
        assert!(scene.camera.far >= 600.0);
        assert!((1.0..=3.0).contains(&scene.camera.position.y));
        assert!(scene.camera.position.z < -30.0);
        assert_eq!(scene.environment.precipitation, 0.0);
        assert_eq!(scene.environment.surface_wetness, 0.0);
        assert!(scene
            .meshes
            .iter()
            .any(|mesh| mesh.name == "dust2_cloth_validation_awning" && mesh.cloth.is_some()));
    } else {
        let import_script =
            std::fs::read_to_string("scripts/import_cs2_dust2.sh").expect("Dust2 import script");
        assert!(import_script.contains("--gltf_export_materials"));
        assert!(import_script.contains("maps/de_dust2.glb"));
        assert!(import_script.contains("sky_de_dust2"));
        assert!(import_script.contains("-colorspace sRGB -alpha off"));
        assert!(!import_script.contains("-auto-level"));
    }

    let scene_source = std::fs::read_to_string("src/scene.rs").expect("scene source");
    assert!(scene_source.contains("import_gltf_with_texture_limits_filter_budget_by_primitive"));
    assert!(scene_source.contains("CS2_DUST2_STRUCTURAL_TEXTURE_DIMENSION"));
    assert!(scene_source.contains("CS2_DUST2_STRUCTURAL_PBR_TEXTURE_DIMENSION"));
    assert!(scene_source.contains("materials/skybox/sky_de_dust2.vmat"));
    assert!(scene_source.contains("lighting/postprocessing/de_dust2_prefab/de_dust2_prefab.vpost"));
    assert!(scene_source.contains("0.925"));
    assert!(scene_source.contains("1.1"));
    assert!(scene_source.contains("211.0 / 255.0"));
    assert!(scene_source.contains("CS2_DUST2_SOURCE_SUN_COLOR"));
    assert!(scene_source.contains("cs2_dust2_visual_lights"));
    assert!(scene_source.contains("DHOOL_DUST2_PHYSICS_IMPORT"));
    assert!(scene_source.contains("DHOOL_DUST2_VISUAL_IMPORT"));
    assert!(scene_source.contains("CS2_DUST2_LIVE_IMPORT_BUDGET"));
    assert!(scene_source.contains("DHOOL_DUST2_FULL_VISUAL"));
    assert!(scene_source.contains("apply_cs2_dust2_physics_material_textures"));
    assert!(!scene_source.contains("cs2_dust2_skybox_shell"));
    assert!(scene_source.contains("far: 620.0"));

    let import_source = std::fs::read_to_string("src/gltf_import.rs").expect("glTF import source");
    assert!(import_source.contains("GltfImportBudget"));
    assert!(import_source.contains("compact_indexed_geometry"));

    let gpu_source = std::fs::read_to_string("src/gpu.rs").expect("GPU source");
    assert!(gpu_source.contains("DUST2_SKYBOX_TEXTURE"));
    assert!(gpu_source.contains("live_skybox_texture_for_map"));
    assert!(gpu_source.contains("lighting-skybox-panorama"));
    assert!(gpu_source.contains("ControlFlow::Poll"));
    assert!(gpu_source.contains("fn about_to_wait"));

    let lighting_source =
        std::fs::read_to_string("src/shaders/lighting.wgsl").expect("lighting shader");
    assert!(lighting_source.contains("var skybox: texture_2d"));
    assert!(lighting_source.contains("textureSample(skybox"));
    assert!(lighting_source.contains("atan2(ray.z, ray.x)"));
    assert!(lighting_source.contains("exp2(lights.environment.x)"));
    assert!(lighting_source.contains("lights.environment.y"));
    assert!(lighting_source.contains("INV_PI"));
    assert!(lighting_source.contains("sky_hemisphere_irradiance"));

    assert!(gpu_source.contains("scene.environment.precipitation"));
    assert!(gpu_source.contains("scene.environment.sky_exposure_ev"));
    assert!(gpu_source.contains("fn update_cloth"));
    assert!(gpu_source.contains("BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST"));
    assert!(gpu_source.contains("scene-previous-positions"));
    assert!(gpu_source.contains("GpuPreviousPosition::layout()"));

    let main_source = std::fs::read_to_string("src/main.rs").expect("main source");
    assert!(main_source.contains("normalize_live_locale_for_xkb"));
    assert!(main_source.contains("is_xkb_bad_locale"));
    assert!(main_source.contains("C.UTF-8"));
}

#[test]
#[ignore = "heavy local CS2 visual import; run manually after scripts/import_cs2_dust2.sh"]
fn dust2_live_visual_scene_constructs_from_generated_assets() {
    let visual_path = std::path::Path::new("assets/dust2/generated/cs2/maps/de_dust2.glb");
    if !visual_path.is_file() {
        eprintln!(
            "skipping local CS2 Dust2 scene test; missing {}",
            visual_path.display()
        );
        return;
    }

    let previous_visual_import = std::env::var_os("DHOOL_DUST2_VISUAL_IMPORT");
    let previous_full_visual = std::env::var_os("DHOOL_DUST2_FULL_VISUAL");
    std::env::set_var("DHOOL_DUST2_VISUAL_IMPORT", "1");
    std::env::remove_var("DHOOL_DUST2_FULL_VISUAL");
    let scene = build_dust2_scene("assets/dust2").expect("CS2 Dust2 visual scene");
    if let Some(previous_visual_import) = previous_visual_import {
        std::env::set_var("DHOOL_DUST2_VISUAL_IMPORT", previous_visual_import);
    } else {
        std::env::remove_var("DHOOL_DUST2_VISUAL_IMPORT");
    }
    if let Some(previous_full_visual) = previous_full_visual {
        std::env::set_var("DHOOL_DUST2_FULL_VISUAL", previous_full_visual);
    }
    let triangles: usize = scene.meshes.iter().map(|mesh| mesh.indices.len()).sum();
    let vertices: usize = scene.meshes.iter().map(|mesh| mesh.vertices.len()).sum();
    let base_color_textures = scene
        .materials
        .iter()
        .filter(|material| material.base_color_texture.is_some())
        .count();
    let high_res_base_color_textures = scene
        .materials
        .iter()
        .filter_map(|material| material.base_color_texture.as_ref())
        .filter(|texture| texture.width.max(texture.height) > 512)
        .count();
    let normal_textures = scene
        .materials
        .iter()
        .filter(|material| material.normal_texture.is_some())
        .count();
    let pbr_textures = scene
        .materials
        .iter()
        .filter(|material| material.pbr_texture.is_some())
        .count();

    eprintln!(
        "Dust2 visual scene: {} meshes, {triangles} triangles, {vertices} vertices, {} materials, {base_color_textures} base textures, {high_res_base_color_textures} high-res base textures, {normal_textures} normal textures, {pbr_textures} pbr textures",
        scene.meshes.len(),
        scene.materials.len()
    );
    assert!(scene
        .meshes
        .iter()
        .any(|mesh| mesh.name.starts_with("cs2_dust2_visual_")));
    for required_detail in [
        "chainlink_fence_wire",
        "palm_bark",
        "agave_plant",
        "antenna_card",
        "crates_dishes",
        "dust_weeds",
        "zebra_grass",
        "olive_branch",
        "sumac",
    ] {
        assert!(
            scene
                .meshes
                .iter()
                .any(|mesh| mesh.name.to_ascii_lowercase().contains(required_detail)),
            "missing restored CS2 Dust2 detail asset {required_detail}"
        );
    }
    assert!(
        scene
            .materials
            .iter()
            .filter(|material| material.base_color_texture.is_some())
            .count()
            >= 80
    );
    assert!(high_res_base_color_textures >= 120);
    assert!(normal_textures >= 200);
    assert!(pbr_textures >= 200);
    assert!(!scene
        .meshes
        .iter()
        .any(|mesh| mesh.name == "cs2_dust2_skybox_shell"));
    assert!(scene
        .lights
        .iter()
        .any(|light| light.name == "cs2_dust2_fixture_hanging_light"));
    assert!(scene
        .lights
        .iter()
        .any(|light| light.name == "cs2_dust2_fixture_ceiling_light"));
    assert!(scene.meshes.len() >= 3_500);
    assert!(triangles >= 4_300_000);
    assert!(vertices >= 4_100_000);
    assert!(triangles <= 4_900_000);
    assert!(vertices <= 5_000_000);
    assert!(scene.camera.far >= 600.0);
    assert!(!scene
        .meshes
        .iter()
        .any(|mesh| mesh.name.to_ascii_lowercase().contains("blocklight")));
}

#[test]
fn live_gpu_uses_scene_camera_fov_and_clip_distance() {
    let gpu = include_str!("../src/gpu.rs");

    assert!(gpu.contains("camera_fov_y: f32"));
    assert!(gpu.contains("camera_far: f32"));
    assert!(gpu.contains("let camera_far = scene.camera.far"));
    assert!(gpu.contains("display_fps"));
    assert!(gpu.contains("live FPS:"));
    assert!(gpu.contains("FrameCapture"));
    assert!(gpu.contains("copy_presented_frame_to_buffer"));
    assert!(gpu.contains("captured live frame"));
    assert!(gpu.contains("let projection = perspective_reverse_rh("));
    assert!(gpu.contains("REVERSE_Z_CLEAR_DEPTH"));
    assert!(gpu.contains("CompareFunction::GreaterEqual"));
    assert!(!gpu.contains("CompareFunction::LessEqual"));

    let aa_shader = include_str!("../src/shaders/aa.wgsl");
    let ui_shader = include_str!("../src/shaders/ui_overlay.wgsl");
    let present_shader = include_str!("../src/shaders/present.wgsl");
    assert!(!aa_shader.contains("overlay"));
    assert!(ui_shader.contains("glyph_coverage"));
    assert!(ui_shader.contains("aa.params.w"));
    assert!(present_shader.contains("display_gamma"));
}

#[test]
fn aaa_scene_uses_generated_material_texture_sets() {
    let scene = build_aaa_scene("assets/aaa").expect("AAA scene");
    for path in [
        "assets/aaa/generated/textures/wet_corrugated_metal_base.png",
        "assets/aaa/generated/textures/wet_corrugated_metal_normal.png",
        "assets/aaa/generated/textures/wet_corrugated_metal_arm.png",
        "assets/aaa/generated/textures/oily_patched_concrete_base.png",
        "assets/aaa/generated/textures/oily_patched_concrete_normal.png",
        "assets/aaa/generated/textures/oily_patched_concrete_arm.png",
    ] {
        assert!(std::path::Path::new(path).is_file(), "{path} should exist");
    }

    for name in [
        "generated_wet_corrugated_metal",
        "generated_oily_patched_concrete",
    ] {
        let material = scene
            .materials
            .iter()
            .find(|material| material.name == name)
            .expect("generated texture-set material");
        assert!(material.base_color_texture.is_some());
        assert!(material.normal_texture.is_some());
        assert!(material.pbr_texture.is_some());
        assert_eq!(material.texture_sample_count, 3);
        assert!(material.flags.uses_bent_normals);
    }

    for mesh_name in [
        "right_generated_corrugated_service_skin",
        "left_generated_corrugated_access_panel",
        "deep_generated_corrugated_rollup_door",
        "generated_oily_concrete_foreground_slab",
        "generated_oily_concrete_mid_service_slab",
        "generated_oily_concrete_far_gate_slab",
    ] {
        assert!(
            scene.meshes.iter().any(|mesh| mesh.name == mesh_name),
            "{mesh_name} should use a generated material texture set"
        );
    }
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

    assert!(gpu.contains("RAIN_DROP_COUNT: u32 = 32_768"));
    assert!(gpu.contains("wgpu::PrimitiveTopology::TriangleList"));
    assert!(gpu.contains("pass.draw(0..RAIN_PRISM_VERTEX_COUNT, 0..RAIN_DROP_COUNT)"));
    assert!(rain_shader.contains("fn prism_local_vertex"));
    assert!(rain_shader.contains("@location(5) surface_normal"));
    assert!(!rain_shader.contains("to_camera"));
    assert!(!gpu.contains("half-resolution-camera-composite-effects"));
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
    let exposure_shader = include_str!("../src/shaders/auto_exposure.wgsl");

    assert!(lighting_shader.contains("texture_depth_2d_array"));
    assert!(lighting_shader.contains("local_shadow_cameras"));
    assert!(lighting_shader.contains("PRACTICAL_SPOT_MIN_FILL"));
    assert!(lighting_shader.contains("let fd90 = 0.5 + 2.0 * ldoth * ldoth * roughness"));
    assert!(lighting_shader.contains("specular_occlusion"));
    assert!(lighting_shader.contains("distance_to_light >= position_radius.w"));
    assert!(lighting_shader.contains("outer_halo"));
    assert!(lighting_shader.contains("spot_presence"));
    assert!(lighting_shader.contains("let shadow_weight = practical_spot_shadow_weight"));
    assert!(lighting_shader.contains("fs_local_lights"));
    assert!(gpu.contains("local-light-z-fail-stencil-volumes"));
    assert!(gpu.contains("stencil-restricted-local-lighting"));
    assert!(ssr_shader.contains("fn surface_direct_radiance"));
    assert!(!reflection_shader.contains("fn trace_alley_proxy"));
    assert!(!reflection_shader.contains("proxy_emissive_radiance"));
    assert!(reflection_shader.contains("rough_jitter"));
    assert!(!lighting_shader.contains("bloom_offsets"));
    assert!(gpu.contains("diffuse-bloom-extract"));
    assert!(gpu.contains("diffuse-gaussian-bloom-horizontal"));
    assert!(gpu.contains("diffuse-gaussian-bloom-vertical"));
    assert!(gpu.contains("gt-tonemap-bloom-resolve"));
    assert!(bloom_shader.contains("fs_extract"));
    assert!(bloom_shader.contains("textureSampleLevel"));
    assert!(bloom_shader.contains("fn gaussian_blur"));
    assert!(bloom_shader.contains("BLOOM_LUMINANCE_CAP"));
    assert!(bloom_shader.contains("source_luminance - BLOOM_THRESHOLD"));
    assert!(bloom_shader.contains("fs_blur_horizontal"));
    assert!(bloom_shader.contains("fs_blur_vertical"));
    assert!(!bloom_shader.contains("array<i32, 7>"));
    assert!(tonemap_shader.contains("gt_tonemap"));
    assert!(tonemap_shader.contains("const BLOOM_INTENSITY: f32 = 0.10"));
    assert!(gpu.contains("diffuse-luminance-auto-exposure-white-balance"));
    assert!(exposure_shader.contains("texture_storage_2d<rgba16float, write>"));
    assert!(exposure_shader.contains("0.18 * compensation / max(geometric_mean"));
    assert!(exposure_shader.contains("lights.ambient.rgb + lights.sky.rgb * 0.35"));
    assert!(exposure_shader.contains("vec3<f32>(0.92)"));
    assert!(exposure_shader.contains("vec3<f32>(1.08)"));
    assert!(tonemap_shader.contains("exposure_white_balance"));
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
    assert!(gpu.contains("load_color_attachment(&self.gbuffer.albedo_ms)"));
    assert!(gpu.contains("2x-msaa-gbuffer-custom-coverage-resolve"));
    assert!(geometry_shader.contains("@location(3) motion_vectors"));
    assert!(aa_shader.contains("@group(0) @binding(5) var motion_vectors"));
    assert!(aa_shader.contains("@group(0) @binding(6) var smaa_blend"));
    assert!(aa_shader.contains("@group(0) @binding(7) var normal_flags"));
    assert!(aa_shader.contains("(material_flags & 2u) != 0u"));
    assert!(aa_shader.contains("let spatial_resolve = smaa_resolve"));
    assert!(aa_shader.contains("edge_strength > 0.0001"));
    assert!(aa_shader.contains("vec4<f32>(spatial_resolve, edge_strength)"));
    assert!(aa_shader.contains("vec4<f32>(resolved, 1.0)"));
    assert!(!aa_shader.contains("fps_overlay"));
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
    assert!(readme.contains("same `assets/aaa` monsoon city scene used by `--live`"));
}
