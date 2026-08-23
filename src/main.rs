use dhool_pro::{
    backend::supported_backends_for_target, build_aaa_scene, build_asset_showcase,
    build_baseline_frame_graph, build_dust2_scene, export_demo_gallery, run_live_renderer,
    FrameInput, GameEngine, LiveOptions, ProjectSettings, RenderConfig,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--live") {
        normalize_live_locale_for_xkb();
        let asset = option_value(&args, "--asset").unwrap_or_else(|| "assets/aaa".to_string());
        let map = option_value(&args, "--map");
        let max_frames = option_value(&args, "--frames").and_then(|value| value.parse().ok());
        let capture_path = option_value(&args, "--capture").map(Into::into);
        let exposure_min = option_f32(&args, "--exposure-min");
        let exposure_max = option_f32(&args, "--exposure-max");
        let exposure_compensation = option_f32(&args, "--exposure-compensation");
        let white_balance_strength = option_f32(&args, "--white-balance-strength");
        let velocity_compensation_strength =
            option_f32(&args, "--velocity-strength").unwrap_or(0.65);
        let camera_velocity_cutoff = option_f32(&args, "--velocity-cutoff").unwrap_or(8.0);
        let display_gamma = option_f32(&args, "--display-gamma").unwrap_or(2.2);
        let display_brightness = option_f32(&args, "--display-brightness").unwrap_or(1.0);
        let profile_gpu = args.iter().any(|arg| arg == "--profile-gpu");
        let static_camera = args.iter().any(|arg| arg == "--static-camera");
        let scene = match map.as_deref() {
            Some("dust2") => build_dust2_scene("assets/dust2"),
            _ if std::path::Path::new(&asset).is_dir() => build_aaa_scene(&asset),
            _ => build_asset_showcase(&asset),
        }
        .unwrap_or_else(|error| {
            eprintln!("scene load failed: {error}");
            std::process::exit(2);
        });
        if let Err(error) = run_live_renderer(
            scene,
            LiveOptions {
                max_frames,
                width: 1280,
                height: 720,
                capture_path,
                exposure_min,
                exposure_max,
                exposure_compensation,
                white_balance_strength,
                velocity_compensation_strength,
                camera_velocity_cutoff,
                display_gamma,
                display_brightness,
                profile_gpu,
                static_camera,
            },
        ) {
            eprintln!("live renderer failed: {error}");
            std::process::exit(2);
        }
        return;
    }

    let graph = build_baseline_frame_graph();
    let issues = graph.validate();

    println!("dhool-pro vendor-agnostic renderer bootstrap");
    println!("target os: {}", std::env::consts::OS);
    println!(
        "available baseline backends: {:?}",
        supported_backends_for_target(std::env::consts::OS)
    );
    println!("registered passes: {}", graph.pass_count());
    println!("registered resources: {}", graph.resource_count());

    if !issues.is_empty() {
        eprintln!("baseline frame graph has validation issues:");
        for issue in issues {
            eprintln!("  - {issue:?}");
        }
        std::process::exit(1);
    }

    let mut engine = GameEngine::new(ProjectSettings {
        native_width: 320,
        native_height: 180,
        ..ProjectSettings::default()
    })
    .expect("valid demo settings");
    let import_report = engine.load_demo_project();
    engine.set_render_config(RenderConfig {
        width: 320,
        height: 180,
        ..RenderConfig::headless_1080p()
    });
    let frame = engine.frame(FrameInput::default()).expect("demo frame");
    println!("game engine entities: {}", engine.world.entity_count());
    println!("asset import messages: {}", import_report.messages.len());
    println!(
        "demo render: {}x{}",
        frame.render_output.width, frame.render_output.height
    );
    println!(
        "average hdr luminance: {:.4}",
        frame.render_output.average_luminance()
    );
    println!(
        "renderer passes executed: {}",
        frame.render_output.pass_reports.len()
    );
    println!("renderer warnings: {}", frame.render_output.warnings.len());
    println!(
        "production gate failures: {}",
        frame.frame_report.failures.len()
    );

    if let Some(path) = std::env::args()
        .skip_while(|arg| arg != "--dump-ppm")
        .nth(1)
    {
        std::fs::write(&path, frame.render_output.ppm_bytes()).expect("write ppm");
        println!("wrote {path}");
    }

    if let Some(index) = args.iter().position(|arg| arg == "--gallery") {
        let output = args
            .get(index + 1)
            .map(String::as_str)
            .unwrap_or("demo-output");
        let asset = args
            .iter()
            .position(|arg| arg == "--asset")
            .and_then(|asset_index| args.get(asset_index + 1))
            .map(String::as_str)
            .unwrap_or("assets/aaa");
        let map = args
            .iter()
            .position(|arg| arg == "--map")
            .and_then(|map_index| args.get(map_index + 1))
            .map(String::as_str);
        let summary = export_demo_gallery(asset, output, 480, 300, map).unwrap_or_else(|error| {
            eprintln!("gallery export failed: {error}");
            std::process::exit(2);
        });
        println!(
            "gallery: {} images, {} meshes, {} triangles, {} materials",
            summary.images, summary.meshes, summary.triangles, summary.materials
        );
        println!("open {}/index.html", summary.output_dir.display());
    }
}

fn option_value(args: &[String], option: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == option)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn option_f32(args: &[String], option: &str) -> Option<f32> {
    option_value(args, option).and_then(|value| value.parse().ok())
}

#[cfg(target_os = "linux")]
fn normalize_live_locale_for_xkb() {
    let lc_all = std::env::var("LC_ALL")
        .ok()
        .filter(|value| !value.is_empty());
    if lc_all
        .as_deref()
        .is_some_and(|value| !is_xkb_bad_locale(value))
    {
        return;
    }

    let has_bad_locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .any(|value| is_xkb_bad_locale(&value));

    if has_bad_locale {
        std::env::set_var("LC_ALL", "C.UTF-8");
    }
}

#[cfg(not(target_os = "linux"))]
fn normalize_live_locale_for_xkb() {}

#[cfg(target_os = "linux")]
fn is_xkb_bad_locale(locale: &str) -> bool {
    let locale = locale.to_ascii_lowercase();
    locale == "en_in" || locale.contains("iso8859") || locale.contains("iso-8859")
}
