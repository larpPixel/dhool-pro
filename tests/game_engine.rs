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
    });
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

    world.fixed_update(1.0 / 60.0);
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
