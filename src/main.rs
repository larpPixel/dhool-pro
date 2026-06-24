use dhool_pro::{
    backend::supported_backends_for_target, build_baseline_frame_graph, export_demo_gallery,
    run_live_renderer, FrameInput, GameEngine, LiveOptions, ProjectSettings, RenderConfig,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--live") {
        let asset = option_value(&args, "--asset").unwrap_or_else(|| "assets/aaa".to_string());
        let max_frames = option_value(&args, "--frames").and_then(|value| value.parse().ok());
        if let Err(error) = run_live_renderer(LiveOptions {
            asset: asset.into(),
            max_frames,
            width: 1280,
            height: 720,
        }) {
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
    });
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
        let summary = export_demo_gallery(asset, output, 480, 300).unwrap_or_else(|error| {
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
