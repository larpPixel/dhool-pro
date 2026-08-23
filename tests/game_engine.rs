use dhool_pro::asset::ShadowCasterClass;
use dhool_pro::asset_pipeline::AssetPipeline;
use dhool_pro::math::Vec3;
use dhool_pro::profiler::{required_debug_views, EngineProfiler};
use dhool_pro::world::{RigidBody, Transform};
use dhool_pro::{build_demo_scene, FrameInput, GameEngine, ProjectSettings, RenderConfig, World};

#[test]
fn game_engine_runs_world_assets_renderer_and_profiler() {
    let mut engine = GameEngine::new(ProjectSettings {
        native_width: 96,
        native_height: 64,
        ..ProjectSettings::default()
    })
    .expect("valid settings");
    let import_report = engine.load_demo_project();
    engine.set_render_config(RenderConfig::tiny_test());

    let frame = engine.frame(FrameInput::default()).expect("engine frame");

    assert!(!import_report.has_errors());
    assert!(engine.world.entity_count() >= 7);
    assert_eq!(frame.render_output.width, 96);
    assert_eq!(frame.render_output.height, 64);
    assert!(frame.render_output.average_luminance() > 0.01);
    assert!(frame.world_issues.is_empty());
    assert_eq!(
        frame.frame_report.pass_count,
        frame.render_output.pass_reports.len()
    );
}

#[test]
fn world_physics_uses_generational_entities_and_builds_render_scene() {
    let mut world = World::from_render_scene(build_demo_scene());
    let body = world.spawn(
        "falling body",
        Transform::from_position(Vec3::new(0.0, 1.0, 0.0)),
    );
    assert!(world.set_rigid_body(
        body,
        RigidBody {
            velocity: Vec3::ZERO,
            mass: 1.0,
            gravity_scale: 1.0,
            dynamic: true
        },
    ));

    world.fixed_update(1.0 / 60.0).expect("fixed update");
    let y_after = world.transform(body).expect("body transform").position.y;
    assert!(y_after < 1.0);
    assert!(world.destroy(body));
    assert!(!world.is_alive(body));

    let render_scene = world.build_render_scene();
    assert!(render_scene
        .meshes
        .iter()
        .filter(|mesh| mesh.is_floor_or_ground)
        .all(|mesh| mesh.shadow_class == ShadowCasterClass::None));
}

#[test]
fn scene_cloth_uses_the_shared_fixed_step_and_preserves_pins() {
    let scene = build_demo_scene();
    let mut expected_cloth = scene
        .meshes
        .iter()
        .find(|mesh| mesh.name == "cloth_validation_banner")
        .expect("demo cloth")
        .clone();
    let definition = expected_cloth.cloth.clone().expect("cloth definition");
    expected_cloth.bake_transform();
    let pinned_positions: Vec<_> = definition
        .pins
        .iter()
        .map(|pin| expected_cloth.vertices[pin.vertex].position)
        .collect();
    let free_vertex = expected_cloth.vertices.len() - 1;
    let initial_free_position = expected_cloth.vertices[free_vertex].position;

    let mut world = World::from_render_scene(scene);
    for _ in 0..30 {
        world.fixed_update(1.0 / 60.0).expect("cloth step");
    }
    let rendered = world.build_render_scene();
    let cloth = rendered
        .meshes
        .iter()
        .find(|mesh| mesh.name == "cloth_validation_banner")
        .expect("simulated cloth");

    for (pin, expected) in definition.pins.iter().zip(pinned_positions) {
        assert_eq!(cloth.vertices[pin.vertex].position, expected);
    }
    assert_ne!(cloth.vertices[free_vertex].position, initial_free_position);
    assert_ne!(
        cloth.vertices[free_vertex].position,
        cloth.vertices[free_vertex].previous_position
    );
    assert!(cloth.vertices.iter().all(|vertex| [
        vertex.position.x,
        vertex.position.y,
        vertex.position.z,
        vertex.previous_position.x,
        vertex.previous_position.y,
        vertex.previous_position.z,
        vertex.normal.x,
        vertex.normal.y,
        vertex.normal.z,
        vertex.tangent.x,
        vertex.tangent.y,
        vertex.tangent.z,
    ]
    .into_iter()
    .all(f32::is_finite)));
}

#[test]
fn world_round_trip_composes_existing_mesh_transforms() {
    let scene = build_demo_scene();
    let expected_environment = scene.environment_map.clone();
    let expected = scene
        .meshes
        .iter()
        .find(|mesh| mesh.name == "prepassed_reflective_panel")
        .expect("placed mesh")
        .transform;
    let rebuilt = World::from_render_scene(scene).build_render_scene();
    let actual = rebuilt
        .meshes
        .iter()
        .find(|mesh| mesh.name == "prepassed_reflective_panel")
        .expect("rebuilt placed mesh")
        .transform;

    assert_eq!(actual, expected);
    assert_eq!(rebuilt.environment_map, expected_environment);
}

#[test]
fn asset_pipeline_rejects_non_equirectangular_environment() {
    let mut scene = build_demo_scene();
    scene.environment_map = dhool_pro::EnvironmentMap::from_image(
        dhool_pro::TextureImage {
            width: 4,
            height: 4,
            pixels: vec![dhool_pro::math::Color::WHITE; 16],
        },
        dhool_pro::EnvironmentEncoding::Srgb,
        0.0,
        0.0,
    );

    let (_pipeline, report) = AssetPipeline::build_from_scene(&scene, dhool_pro::QualityTier::High);
    assert!(report
        .errors()
        .any(|message| message.text.contains("must be 2:1 equirectangular")));
}

#[test]
fn asset_pipeline_rejects_reflective_mesh_without_proxy() {
    let mut scene = build_demo_scene();
    let reflective = scene
        .meshes
        .iter_mut()
        .find(|mesh| mesh.name == "prepassed_reflective_panel")
        .expect("reflective panel");
    reflective.reflection_proxy = false;

    let (_pipeline, report) = AssetPipeline::build_from_scene(&scene, dhool_pro::QualityTier::High);
    assert!(report.has_errors());
    assert!(report
        .errors()
        .any(|msg| msg.text.contains("reflection proxy")));
}

#[test]
fn profiler_tracks_required_debug_coverage() {
    let required = required_debug_views();
    assert!(required.iter().any(|view| view.name == "msaa-edge-stencil"));
    assert!(required.iter().any(|view| view.name == "quad-utilization"));

    let profiler = EngineProfiler::default();
    let missing = profiler.evaluate_debug_coverage();
    assert!(missing
        .iter()
        .any(|failure| format!("{failure:?}").contains("quad-utilization")));
}

#[test]
fn profiler_accepts_complete_debug_view_coverage() {
    let mut profiler = EngineProfiler::default();
    for view in dhool_pro::DebugView::ALL {
        profiler.mark_debug_view_rendered(view);
    }
    assert_eq!(profiler.evaluate_debug_coverage(), Vec::new());
}
