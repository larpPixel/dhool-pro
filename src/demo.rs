use crate::renderer::{CpuRenderer, DebugView, RenderConfig};
use crate::scene::{build_aaa_scene, build_asset_showcase, build_dust2_scene};
use image::{ColorType, ImageFormat};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct DemoSummary {
    pub output_dir: PathBuf,
    pub images: usize,
    pub meshes: usize,
    pub triangles: usize,
    pub materials: usize,
}

pub fn export_demo_gallery(
    asset_path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    width: u32,
    height: u32,
    map: Option<&str>,
) -> Result<DemoSummary, String> {
    let asset_path = asset_path.as_ref();
    let scene = match map {
        Some("dust2") => build_dust2_scene(PathBuf::from("assets/dust2")),
        _ if asset_path.is_dir() => build_aaa_scene(asset_path),
        _ => build_asset_showcase(asset_path),
    }
    .map_err(|error| error.to_string())?;
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| error.to_string())?;

    let mut cards = String::new();
    let mut report = String::new();
    let mut image_count = 0;
    for view in DebugView::ALL {
        let mut config = RenderConfig {
            width,
            height,
            debug_view: view,
            ..RenderConfig::headless_1080p()
        };
        if view == DebugView::HistoryValidity {
            config.debug_view = DebugView::Beauty;
        }
        let mut renderer = CpuRenderer::new(config);
        if view == DebugView::HistoryValidity {
            renderer
                .render(&scene)
                .map_err(|error| format!("{error:?}"))?;
            renderer.config.debug_view = view;
            renderer.config.frame_index = 1;
        }
        let output = renderer
            .render(&scene)
            .map_err(|error| format!("{error:?}"))?;
        let filename = format!("{}.png", view.slug());
        let path = output_dir.join(&filename);
        let bytes: Vec<u8> = output.color.iter().flatten().copied().collect();
        image::save_buffer_with_format(
            &path,
            &bytes,
            output.width,
            output.height,
            ColorType::Rgba8,
            ImageFormat::Png,
        )
        .map_err(|error| error.to_string())?;
        writeln!(
            cards,
            "<figure><img src=\"{filename}\" alt=\"{} renderer view\"><figcaption>{}</figcaption></figure>",
            view.slug(),
            title(view.slug())
        )
        .unwrap();
        writeln!(report, "[{}]", view.slug()).unwrap();
        for pass in &output.pass_reports {
            writeln!(
                report,
                "  {:42} {:8.3} ms (budget {:5.2} ms)",
                pass.name, pass.cpu_ms, pass.budget_ms
            )
            .unwrap();
        }
        writeln!(report, "  warnings: {}", output.warnings.len()).unwrap();
        for warning in &output.warnings {
            writeln!(report, "    {warning:?}").unwrap();
        }
        writeln!(report).unwrap();
        image_count += 1;
    }

    let triangles = scene.meshes.iter().map(|mesh| mesh.indices.len()).sum();
    let scene_name = match map {
        Some("dust2") => "assets/dust2 (local CS2 de_dust2 import)",
        _ if asset_path.is_dir() => "assets/aaa (monsoon city)",
        _ => "glTF asset showcase",
    };
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Dhool Pro Showcase</title><style>{}</style></head><body><header><p class=\"eyebrow\">LIVE-SCENE DIAGNOSTICS</p><h1>Dhool Pro renderer showcase</h1><p>Vendor-agnostic CPU reference path · {} meshes · {} triangles · {} materials</p></header><main>{cards}</main><footer>Gallery scene: {scene_name}. Pass --map dust2 for the desert map, --asset path/to/model.gltf for the deterministic glTF showcase, or omit both for the default monsoon city.</footer></body></html>",
        STYLE,
        scene.meshes.len(),
        triangles,
        scene.materials.len()
    );
    fs::write(output_dir.join("index.html"), html).map_err(|error| error.to_string())?;
    fs::write(output_dir.join("timings.txt"), report).map_err(|error| error.to_string())?;

    Ok(DemoSummary {
        output_dir: output_dir.to_path_buf(),
        images: image_count,
        meshes: scene.meshes.len(),
        triangles,
        materials: scene.materials.len(),
    })
}

fn title(slug: &str) -> String {
    slug.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const STYLE: &str = r#"
:root{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif;background:#0b1018;color:#e7edf5}*{box-sizing:border-box}body{margin:0}header{padding:64px 7vw 36px;background:linear-gradient(145deg,#101722,#080c12)}h1{font-size:3.8rem;line-height:1;margin:.15em 0;max-width:900px}.eyebrow{color:#f2b84b;letter-spacing:0;font-weight:800;font-size:.72rem}header>p:last-child{color:#9eacbd}main{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:18px;padding:30px 4vw 64px}figure{margin:0;background:#121a25;border:1px solid #263243;border-radius:8px;overflow:hidden;box-shadow:0 18px 45px #0006}img{display:block;width:100%;aspect-ratio:16/10;object-fit:cover;background:#172239}figcaption{padding:14px 16px;color:#cad5e3;font-weight:700;letter-spacing:0}footer{padding:24px 5vw;border-top:1px solid #222d3c;color:#8190a3;font-size:.85rem}@media (max-width:640px){header{padding:42px 24px 28px}h1{font-size:2.35rem}main{grid-template-columns:1fr;padding:22px 18px 48px}}
"#;
