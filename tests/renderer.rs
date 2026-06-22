use dhool_pro::perf::{gt_tonemap_rgba8, kawase_blur_rgba32f};
use dhool_pro::renderer::RenderWarning;
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
