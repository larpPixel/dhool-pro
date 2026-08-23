use dhool_pro::math::{Color, Vec2, Vec3};
use dhool_pro::{
    FrameInput, GameEngine, Light, Material, Mesh, ProjectSettings, RenderConfig, SceneBuilder,
    SystemContext,
};

#[derive(Default)]
struct Score(u32);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut scene = SceneBuilder::new();
    let player_material =
        scene.add_material(Material::standard("player", Color::rgb(0.9, 0.25, 0.08)));
    let floor_material =
        scene.add_material(Material::standard("floor", Color::rgb(0.12, 0.18, 0.14)));
    scene.add_mesh(Mesh::cuboid("player", Vec3::ONE), player_material);
    scene.add_mesh(
        Mesh::ground_plane("floor", Vec2::new(10.0, 10.0)),
        floor_material,
    );
    scene.add_light(Light::directional(
        "sun",
        Vec3::new(0.2, -1.0, -0.4),
        Color::WHITE,
        2.5,
    ));

    let mut game = GameEngine::builder(ProjectSettings {
        name: "My Game".to_string(),
        native_width: 320,
        native_height: 180,
        ..ProjectSettings::default()
    })
    .scene(scene.build()?)
    .render_config(RenderConfig {
        width: 320,
        height: 180,
        ..RenderConfig::headless_1080p()
    })
    .resource(Score::default())
    .fixed_system("spin-player", |context: &mut SystemContext<'_>| {
        let player = context
            .world
            .find_by_name("player")
            .ok_or_else(|| "player entity is missing".to_string())?;
        context
            .world
            .transform_mut(player)
            .ok_or_else(|| "player transform is missing".to_string())?
            .rotation
            .y += context.time.delta_seconds;
        Ok(())
    })
    .update_system("score", |context: &mut SystemContext<'_>| {
        context
            .resources
            .get_mut::<Score>()
            .ok_or_else(|| "score resource is missing".to_string())?
            .0 += 1;
        Ok(())
    })
    .build()?;

    for _ in 0..3 {
        let frame = game.frame(FrameInput::default())?;
        println!(
            "frame {}: {} fixed tick, {:.3} average luminance",
            frame.frame_index,
            frame.fixed_ticks,
            frame.render_output.average_luminance()
        );
    }
    println!("score: {}", game.resources.get::<Score>().expect("score").0);
    Ok(())
}
