use crate::asset_pipeline::{AssetPipeline, ImportReport};
use crate::cloth::ClothError;
use crate::math::{Vec2, Vec3};
use crate::profiler::{EngineProfiler, FrameReport};
use crate::quality::QualityTier;
use crate::renderer::{CpuRenderer, RenderConfig, RenderError, RenderOutput};
use crate::scene::{build_demo_scene, RenderScene};
use crate::world::{World, WorldIssue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetPlatform {
    Windows,
    Linux,
}

impl TargetPlatform {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "windows" => Self::Windows,
            _ => Self::Linux,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSettings {
    pub name: String,
    pub target_platforms: Vec<TargetPlatform>,
    pub quality: QualityTier,
    pub fixed_timestep: f32,
    pub native_width: u32,
    pub native_height: u32,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            name: "Dhool Pro Project".to_string(),
            target_platforms: vec![TargetPlatform::Windows, TargetPlatform::Linux],
            quality: QualityTier::High,
            fixed_timestep: 1.0 / 60.0,
            native_width: 1920,
            native_height: 1080,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputState {
    pub move_axis: Vec3,
    pub look_delta: Vec2,
    pub primary_action: bool,
    pub secondary_action: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInput {
    pub delta_seconds: f32,
    pub input: InputState,
}

impl Default for FrameInput {
    fn default() -> Self {
        Self {
            delta_seconds: 1.0 / 60.0,
            input: InputState::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EngineFrame {
    pub frame_index: u64,
    pub fixed_ticks: u32,
    pub render_output: RenderOutput,
    pub frame_report: FrameReport,
    pub world_issues: Vec<WorldIssue>,
    pub import_report: ImportReport,
}

impl EngineFrame {
    pub fn passed_production_gates(&self) -> bool {
        self.world_issues.is_empty()
            && !self.import_report.has_errors()
            && self.frame_report.passed()
    }
}

#[derive(Debug)]
pub enum EngineError {
    Render(RenderError),
    Cloth(ClothError),
    NoSceneLoaded,
}

impl From<RenderError> for EngineError {
    fn from(value: RenderError) -> Self {
        Self::Render(value)
    }
}

impl From<ClothError> for EngineError {
    fn from(value: ClothError) -> Self {
        Self::Cloth(value)
    }
}

pub struct GameEngine {
    pub settings: ProjectSettings,
    pub world: World,
    pub assets: AssetPipeline,
    pub profiler: EngineProfiler,
    renderer: CpuRenderer,
    accumulator: f32,
    frame_index: u64,
    scene_loaded: bool,
}

impl GameEngine {
    pub fn new(settings: ProjectSettings) -> Self {
        let render_config = RenderConfig {
            width: settings.native_width,
            height: settings.native_height,
            quality: settings.quality,
            ..RenderConfig::headless_1080p()
        };
        Self {
            assets: AssetPipeline::new(settings.quality),
            profiler: EngineProfiler::default(),
            world: World::default(),
            renderer: CpuRenderer::new(render_config),
            settings,
            accumulator: 0.0,
            frame_index: 0,
            scene_loaded: false,
        }
    }

    pub fn load_scene(&mut self, scene: RenderScene) -> ImportReport {
        let (assets, report) = AssetPipeline::build_from_scene(&scene, self.settings.quality);
        self.assets = assets;
        self.world = World::from_render_scene(scene);
        self.scene_loaded = true;
        report
    }

    pub fn load_demo_project(&mut self) -> ImportReport {
        self.load_scene(build_demo_scene())
    }

    pub fn set_render_config(&mut self, config: RenderConfig) {
        self.renderer = CpuRenderer::new(config);
    }

    pub fn frame(&mut self, input: FrameInput) -> Result<EngineFrame, EngineError> {
        if !self.scene_loaded {
            return Err(EngineError::NoSceneLoaded);
        }
        let delta = input.delta_seconds.clamp(0.0, 0.25);
        self.accumulator += delta;
        let mut fixed_ticks = 0;
        while self.accumulator >= self.settings.fixed_timestep {
            self.world.fixed_update(self.settings.fixed_timestep)?;
            self.accumulator -= self.settings.fixed_timestep;
            fixed_ticks += 1;
        }

        let mut render_scene = self.world.build_render_scene();
        apply_camera_input(&mut render_scene, input.input, delta);
        let render_output = self.renderer.render(&render_scene)?;
        let frame_report = self.profiler.evaluate(&render_output);
        let world_issues = self.world.validate();
        let import_report = self.assets.validate();
        let frame = EngineFrame {
            frame_index: self.frame_index,
            fixed_ticks,
            render_output,
            frame_report,
            world_issues,
            import_report,
        };
        self.frame_index = self.frame_index.saturating_add(1);
        Ok(frame)
    }
}

pub fn build_better_than_unity_unreal_demo() -> GameEngine {
    let mut engine = GameEngine::new(ProjectSettings::default());
    engine.load_demo_project();
    engine
}

fn apply_camera_input(scene: &mut RenderScene, input: InputState, dt: f32) {
    let forward = (scene.camera.target - scene.camera.position).normalize_or_zero();
    let right = forward.cross(scene.camera.up).normalize_or_zero();
    let up = scene.camera.up.normalize_or_zero();
    let movement = right * input.move_axis.x + up * input.move_axis.y + forward * input.move_axis.z;
    let delta = movement * (dt * 4.0);
    scene.camera.position += delta;
    scene.camera.target += delta;
    if input.look_delta.x != 0.0 || input.look_delta.y != 0.0 {
        let look = right * (input.look_delta.x * 0.01) + up * (input.look_delta.y * 0.01);
        scene.camera.target += look;
    }
}
