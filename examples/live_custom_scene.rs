use dhool_pro::math::{Color, Vec2, Vec3};
use dhool_pro::{
    run_live_game, FlyCameraPlugin, GameEngine, Light, LiveOptions, Material, Mesh,
    ProjectSettings, SceneBuilder, SystemContext,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scene = SceneBuilder::new();
    let cube = scene.add_material(Material::standard("cube", Color::rgb(0.12, 0.55, 0.9)));
    let floor = scene.add_material(Material::standard("floor", Color::rgb(0.16, 0.18, 0.20)));
    scene.add_mesh(Mesh::cuboid("cube", Vec3::ONE), cube);
    scene.add_mesh(Mesh::ground_plane("floor", Vec2::new(12.0, 12.0)), floor);
    scene.add_light(Light::directional(
        "sun",
        Vec3::new(0.3, -1.0, -0.4),
        Color::WHITE,
        2.5,
    ));

    let max_frames = std::env::var("DHOOL_SMOKE_FRAMES")
        .ok()
        .and_then(|value| value.parse().ok());
    let update_count = Arc::new(AtomicU64::new(0));
    let counted_updates = update_count.clone();
    let game = GameEngine::builder(ProjectSettings {
        name: "Live Custom Game".to_string(),
        ..ProjectSettings::default()
    })
    .scene(scene.build()?)
    .plugin(FlyCameraPlugin::default())
    .fixed_system("rotate-cube", |context: &mut SystemContext<'_>| {
        let cube = context
            .world
            .find_by_name("cube")
            .ok_or_else(|| "cube entity is missing".to_string())?;
        context
            .world
            .transform_mut(cube)
            .ok_or_else(|| "cube transform is missing".to_string())?
            .rotation
            .y += context.time.delta_seconds;
        Ok(())
    })
    .update_system("count-updates", move |_: &mut SystemContext<'_>| {
        counted_updates.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })
    .build()?;
    run_live_game(
        game,
        LiveOptions {
            max_frames,
            ..LiveOptions::default()
        },
    )?;
    println!(
        "live game updates: {}",
        update_count.load(Ordering::Relaxed)
    );
    Ok(())
}
