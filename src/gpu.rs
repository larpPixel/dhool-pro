use crate::math::{Vec3, Vec4};
use crate::scene::{
    build_aaa_scene, build_asset_showcase, LightKind, Material, RenderScene, TextureImage,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3 as GpuVec3};
use std::fmt;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::util::{DeviceExt, TextureDataOrder};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const PBR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const REFLECTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const CAMERA_MOVE_SPEED: f32 = 4.0;
const CAMERA_LOOK_SPEED: f32 = 1.55;
const RAIN_DROP_COUNT: u32 = 4_096;
const RAIN_PRISM_VERTEX_COUNT: u32 = 36;
const MAX_GPU_POINT_LIGHTS: usize = 4;
const SHADOW_MAP_RESOLUTION: u32 = 4_096;
const LOCAL_SHADOW_MAP_RESOLUTION: u32 = 1_024;
const BLOOM_SCALE: u32 = 4;
const EMISSIVE_SCALE: f32 = 12.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveOptions {
    pub asset: PathBuf,
    pub max_frames: Option<u64>,
    pub width: u32,
    pub height: u32,
}

pub fn run_live_renderer(options: LiveOptions) -> Result<(), LiveError> {
    let event_loop = EventLoop::new().map_err(|error| LiveError::Window(error.to_string()))?;
    let mut application = LiveApplication::new(options);
    event_loop
        .run_app(&mut application)
        .map_err(|error| LiveError::Window(error.to_string()))?;
    if let Some(error) = application.error {
        Err(error)
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum LiveError {
    Window(String),
    Scene(String),
    Surface(String),
    AdapterUnavailable,
    Device(String),
}

impl fmt::Display for LiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(message) => write!(formatter, "window error: {message}"),
            Self::Scene(message) => write!(formatter, "scene error: {message}"),
            Self::Surface(message) => write!(formatter, "surface error: {message}"),
            Self::AdapterUnavailable => write!(formatter, "no compatible GPU adapter"),
            Self::Device(message) => write!(formatter, "GPU device error: {message}"),
        }
    }
}

impl std::error::Error for LiveError {}

struct LiveApplication {
    options: LiveOptions,
    window: Option<Arc<Window>>,
    renderer: Option<GpuRenderer>,
    error: Option<LiveError>,
    frame_count: u64,
    title_clock: Instant,
    title_frames: u32,
}

impl LiveApplication {
    fn new(options: LiveOptions) -> Self {
        Self {
            options,
            window: None,
            renderer: None,
            error: None,
            frame_count: 0,
            title_clock: Instant::now(),
            title_frames: 0,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: LiveError) {
        eprintln!("{error}");
        self.error = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler for LiveApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Dhool Pro | GPU renderer starting")
            .with_inner_size(PhysicalSize::new(self.options.width, self.options.height));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, LiveError::Window(error.to_string()));
                return;
            }
        };
        let renderer = match pollster::block_on(GpuRenderer::new(
            window.clone(),
            self.options.asset.clone(),
        )) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        println!(
            "live GPU: {} ({:?}, {:?})",
            renderer.adapter.name, renderer.adapter.backend, renderer.adapter.device_type
        );
        println!(
            "live pipeline: shadow maps -> depth prepass/PBR G-buffer -> compute reflections -> HDR lighting/rain -> Kawase bloom + GT resolve -> SMAA T2x"
        );
        window.set_title(&format!(
            "Dhool Pro | {} | vendor-agnostic compute",
            renderer.adapter.name
        ));
        window.request_redraw();
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.title_clock = Instant::now();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                let pressed = event.state == ElementState::Pressed;
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) if pressed => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::Space) if pressed && !event.repeat => {
                        renderer.toggle_auto_orbit();
                    }
                    PhysicalKey::Code(code) => renderer.set_key(code, pressed),
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                renderer.update();
                match renderer.render() {
                    RenderResult::Presented => {
                        self.frame_count += 1;
                        self.title_frames += 1;
                    }
                    RenderResult::Reconfigure => renderer.resize(renderer.size),
                    RenderResult::Skipped => {}
                    RenderResult::Fatal(message) => {
                        self.fail(event_loop, LiveError::Surface(message));
                        return;
                    }
                }

                let elapsed = self.title_clock.elapsed();
                if elapsed >= Duration::from_secs(1) {
                    let fps = self.title_frames as f64 / elapsed.as_secs_f64();
                    window.set_title(&format!(
                        "Dhool Pro | {} | {:.0} FPS | compute RT + HDR bloom + 3D rain + SMAA T2x",
                        renderer.adapter.name, fps
                    ));
                    self.title_clock = Instant::now();
                    self.title_frames = 0;
                }
                if self
                    .options
                    .max_frames
                    .is_some_and(|limit| self.frame_count >= limit)
                {
                    println!("live smoke test presented {} frames", self.frame_count);
                    event_loop.exit();
                } else {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
    tangent: [f32; 4],
}

impl GpuVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32x4,
        4 => Float32x4
    ];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_projection: [[f32; 4]; 4],
    inverse_view_projection: [[f32; 4]; 4],
    position_time: [f32; 4],
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    surface: [f32; 4],
    emission: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct FrameUniform {
    dimensions: [f32; 4],
    counters: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct AaUniform {
    previous_view_projection: [[f32; 4]; 4],
    viewport: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct LightUniform {
    directional_direction_count: [f32; 4],
    directional_color_intensity: [f32; 4],
    ambient: [f32; 4],
    sky: [f32; 4],
    weather: [f32; 4],
    local_position_radius: [[f32; 4]; MAX_GPU_POINT_LIGHTS],
    local_color_intensity: [[f32; 4]; MAX_GPU_POINT_LIGHTS],
    local_direction_outer: [[f32; 4]; MAX_GPU_POINT_LIGHTS],
    local_inner_shadow: [[f32; 4]; MAX_GPU_POINT_LIGHTS],
}

impl LightUniform {
    fn from_scene(scene: &RenderScene) -> Self {
        let mut uniform = Self {
            ambient: [
                scene.ambient_color.r,
                scene.ambient_color.g,
                scene.ambient_color.b,
                1.0,
            ],
            sky: [scene.sky_color.r, scene.sky_color.g, scene.sky_color.b, 1.0],
            weather: [1.0, 1.0, 0.18, 0.0],
            ..Self::zeroed()
        };
        let mut local_count = 0_usize;
        for light in &scene.lights {
            match light.kind {
                LightKind::Directional { direction }
                    if uniform.directional_color_intensity[3] == 0.0 =>
                {
                    let surface_to_light =
                        GpuVec3::new(-direction.x, -direction.y, -direction.z).normalize_or_zero();
                    uniform.directional_direction_count = [
                        surface_to_light.x,
                        surface_to_light.y,
                        surface_to_light.z,
                        0.0,
                    ];
                    uniform.directional_color_intensity =
                        [light.color.r, light.color.g, light.color.b, light.intensity];
                }
                LightKind::Local { position, radius } if local_count < MAX_GPU_POINT_LIGHTS => {
                    uniform.local_position_radius[local_count] =
                        [position.x, position.y, position.z, radius];
                    uniform.local_color_intensity[local_count] =
                        [light.color.r, light.color.g, light.color.b, light.intensity];
                    local_count += 1;
                }
                LightKind::Spot {
                    position,
                    direction,
                    radius,
                    inner_angle_radians,
                    outer_angle_radians,
                } if local_count < MAX_GPU_POINT_LIGHTS => {
                    let direction =
                        GpuVec3::new(direction.x, direction.y, direction.z).normalize_or_zero();
                    uniform.local_position_radius[local_count] =
                        [position.x, position.y, position.z, radius];
                    uniform.local_color_intensity[local_count] =
                        [light.color.r, light.color.g, light.color.b, light.intensity];
                    uniform.local_direction_outer[local_count] = [
                        direction.x,
                        direction.y,
                        direction.z,
                        outer_angle_radians.cos(),
                    ];
                    uniform.local_inner_shadow[local_count] = [
                        inner_angle_radians.cos(),
                        if light.casts_shadow { 1.0 } else { 0.0 },
                        local_count as f32,
                        1.0,
                    ];
                    local_count += 1;
                }
                _ => {}
            }
        }
        uniform.directional_direction_count[3] = local_count as f32;
        uniform
    }

    fn realtime(mut self, elapsed: f32) -> Self {
        self.weather = [1.0, 1.0, 0.18 + elapsed.sin() * 0.025, elapsed];
        let moon_flicker = 0.96 + (elapsed * 0.17).sin() * 0.025;
        self.directional_color_intensity[3] *= moon_flicker;
        let local_count =
            self.directional_direction_count[3].clamp(0.0, MAX_GPU_POINT_LIGHTS as f32) as usize;
        for light_index in 0..local_count {
            let phase = elapsed * (7.0 + light_index as f32 * 2.35) + light_index as f32 * 1.71;
            let flicker = 0.88 + phase.sin() * 0.08 + (phase * 2.13).sin() * 0.035;
            self.local_color_intensity[light_index][3] *= flicker.clamp(0.72, 1.12);
        }
        self
    }

    fn surface_to_directional_light(self) -> GpuVec3 {
        GpuVec3::new(
            self.directional_direction_count[0],
            self.directional_direction_count[1],
            self.directional_direction_count[2],
        )
        .normalize_or_zero()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct LocalShadowUniform {
    view_projection: [[[f32; 4]; 4]; MAX_GPU_POINT_LIGHTS],
}

#[derive(Clone, Copy, Debug, Default)]
struct CameraKeys {
    move_forward: bool,
    move_back: bool,
    move_left: bool,
    move_right: bool,
    look_left: bool,
    look_right: bool,
    look_up: bool,
    look_down: bool,
}

fn apply_camera_keys_to_pose(
    camera_origin: &mut GpuVec3,
    camera_target: &mut GpuVec3,
    keys: CameraKeys,
    dt: f32,
) {
    let forward = (*camera_target - *camera_origin).normalize_or_zero();
    let flat_forward = GpuVec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let flat_right = flat_forward.cross(GpuVec3::Y).normalize_or_zero();
    let forward_axis =
        (if keys.move_forward { 1.0 } else { 0.0 }) - (if keys.move_back { 1.0 } else { 0.0 });
    let strafe_axis =
        (if keys.move_right { 1.0 } else { 0.0 }) - (if keys.move_left { 1.0 } else { 0.0 });
    let movement = flat_forward * forward_axis + flat_right * strafe_axis;
    if movement.length_squared() > 0.0 {
        let delta = movement.normalize() * (CAMERA_MOVE_SPEED * dt);
        *camera_origin += delta;
        *camera_target += delta;
    }

    let yaw = ((if keys.look_right { 1.0 } else { 0.0 })
        - (if keys.look_left { 1.0 } else { 0.0 }))
        * CAMERA_LOOK_SPEED
        * dt;
    let pitch = ((if keys.look_up { 1.0 } else { 0.0 }) - (if keys.look_down { 1.0 } else { 0.0 }))
        * CAMERA_LOOK_SPEED
        * dt;
    if yaw != 0.0 || pitch != 0.0 {
        let distance = (*camera_target - *camera_origin).length().max(0.001);
        let mut direction = (*camera_target - *camera_origin).normalize_or_zero();
        if yaw != 0.0 {
            direction = Quat::from_axis_angle(GpuVec3::Y, yaw).mul_vec3(direction);
        }
        if pitch != 0.0 {
            let right = direction.cross(GpuVec3::Y).normalize_or_zero();
            let pitched = Quat::from_axis_angle(right, pitch)
                .mul_vec3(direction)
                .normalize_or_zero();
            if pitched.dot(GpuVec3::Y).abs() < 0.97 {
                direction = pitched;
            }
        }
        *camera_target = *camera_origin + direction * distance;
    }
}

struct DrawBatch {
    indices: Range<u32>,
    material_index: usize,
    prepass_eligible: bool,
    casts_shadow: bool,
}

struct GpuMaterial {
    _uniform: wgpu::Buffer,
    _textures: Vec<wgpu::Texture>,
    bind_group: wgpu::BindGroup,
}

struct RainTextures {
    bind_group: wgpu::BindGroup,
}

struct RainPassResources<'a> {
    camera_buffer: &'a wgpu::Buffer,
    light_buffer: &'a wgpu::Buffer,
    shadow_view: &'a wgpu::TextureView,
    shadow_sampler: &'a wgpu::Sampler,
    shadow_camera_buffer: &'a wgpu::Buffer,
}

struct GBuffer {
    _albedo: wgpu::Texture,
    _normal: wgpu::Texture,
    _pbr: wgpu::Texture,
    _depth: wgpu::Texture,
    _reflection: wgpu::Texture,
    _lighting_hdr: wgpu::Texture,
    _bloom_a: wgpu::Texture,
    _bloom_b: wgpu::Texture,
    _tone_mapped: wgpu::Texture,
    _aa_history_a: wgpu::Texture,
    _aa_history_b: wgpu::Texture,
    albedo: wgpu::TextureView,
    normal: wgpu::TextureView,
    pbr: wgpu::TextureView,
    depth: wgpu::TextureView,
    lighting_hdr: wgpu::TextureView,
    bloom_a: wgpu::TextureView,
    bloom_b: wgpu::TextureView,
    tone_mapped: wgpu::TextureView,
    aa_history_a: wgpu::TextureView,
    aa_history_b: wgpu::TextureView,
    reflection_bind_group: wgpu::BindGroup,
    lighting_bind_group: wgpu::BindGroup,
    bloom_extract_bind_group: wgpu::BindGroup,
    bloom_blur_a_bind_group: wgpu::BindGroup,
    bloom_blur_b_bind_group: wgpu::BindGroup,
    tonemap_bind_group: wgpu::BindGroup,
    aa_bind_group_a: wgpu::BindGroup,
    aa_bind_group_b: wgpu::BindGroup,
    reflection_width: u32,
    reflection_height: u32,
}

struct GpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    adapter: wgpu::AdapterInfo,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    batches: Vec<DrawBatch>,
    materials: Vec<GpuMaterial>,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    frame_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    aa_buffer: wgpu::Buffer,
    shadow_camera_buffer: wgpu::Buffer,
    shadow_camera_bind_group: wgpu::BindGroup,
    _shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    local_shadow_buffer: wgpu::Buffer,
    _local_shadow_texture: wgpu::Texture,
    local_shadow_array_view: wgpu::TextureView,
    local_shadow_views: Vec<wgpu::TextureView>,
    _local_shadow_camera_buffers: Vec<wgpu::Buffer>,
    local_shadow_bind_groups: Vec<wgpu::BindGroup>,
    shadowed_local_light_indices: Vec<usize>,
    depth_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    geometry_pipeline: wgpu::RenderPipeline,
    reflection_pipeline: wgpu::ComputePipeline,
    lighting_pipeline: wgpu::RenderPipeline,
    bloom_extract_pipeline: wgpu::RenderPipeline,
    bloom_blur_horizontal_pipeline: wgpu::RenderPipeline,
    bloom_blur_vertical_pipeline: wgpu::RenderPipeline,
    tonemap_pipeline: wgpu::RenderPipeline,
    aa_pipeline: wgpu::RenderPipeline,
    rain_pipeline: wgpu::RenderPipeline,
    rain: RainTextures,
    reflection_layout: wgpu::BindGroupLayout,
    lighting_layout: wgpu::BindGroupLayout,
    bloom_layout: wgpu::BindGroupLayout,
    tonemap_layout: wgpu::BindGroupLayout,
    aa_layout: wgpu::BindGroupLayout,
    bloom_format: wgpu::TextureFormat,
    gbuffer: GBuffer,
    started: Instant,
    last_update: Instant,
    frame_index: u32,
    current_view_projection: Mat4,
    previous_view_projection: Mat4,
    auto_orbit: bool,
    camera_origin: GpuVec3,
    camera_target: GpuVec3,
    keys: CameraKeys,
    base_light_uniform: LightUniform,
}

enum RenderResult {
    Presented,
    Reconfigure,
    Skipped,
    Fatal(String),
}

impl GpuRenderer {
    async fn new(window: Arc<Window>, asset: PathBuf) -> Result<Self, LiveError> {
        let scene = if asset.is_dir() {
            build_aaa_scene(&asset)
        } else {
            build_asset_showcase(&asset)
        }
        .map_err(|error| LiveError::Scene(error.to_string()))?;
        let camera_origin = GpuVec3::new(
            scene.camera.position.x,
            scene.camera.position.y,
            scene.camera.position.z,
        );
        let camera_target = GpuVec3::new(
            scene.camera.target.x,
            scene.camera.target.y,
            scene.camera.target.z,
        );
        let size = window.inner_size();
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())),
        );
        let surface = instance
            .create_surface(window)
            .map_err(|error| LiveError::Surface(error.to_string()))?;
        let adapter = select_adapter(&instance, &surface).await?;
        let adapter_info = adapter.get_info();
        let optional_features = wgpu::Features::RG11B10UFLOAT_RENDERABLE;
        let required_features = adapter.features() & optional_features;
        let bloom_format = if required_features.contains(wgpu::Features::RG11B10UFLOAT_RENDERABLE) {
            wgpu::TextureFormat::Rg11b10Ufloat
        } else {
            HDR_FORMAT
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features,
                ..wgpu::DeviceDescriptor::default()
            })
            .await
            .map_err(|error| LiveError::Device(error.to_string()))?;
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| LiveError::Surface("adapter cannot present to window".to_string()))?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-layout"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let material_layout = material_layout(&device);
        let reflection_layout = reflection_layout(&device);
        let lighting_layout = lighting_layout(&device);
        let bloom_layout = bloom_layout(&device);
        let tonemap_layout = tonemap_layout(&device);
        let aa_layout = aa_layout(&device);
        let rain_layout = rain_layout(&device);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera-uniform"),
            contents: bytemuck::bytes_of(&CameraUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let frame_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame-uniform"),
            contents: bytemuck::bytes_of(&FrameUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let aa_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("anti-alias-uniform"),
            contents: bytemuck::bytes_of(&AaUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let (vertices, indices, batches) = flatten_scene(&scene);
        let light_uniform = LightUniform::from_scene(&scene);
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene-light-uniform"),
            contents: bytemuck::bytes_of(&light_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shadow_camera = shadow_camera_for_scene(light_uniform, &vertices);
        let shadow_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shadow-camera-uniform"),
            contents: bytemuck::bytes_of(&shadow_camera),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let shadow_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow-camera-bind-group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_camera_buffer.as_entire_binding(),
            }],
        });
        let (shadow_texture, shadow_view, shadow_sampler) =
            create_shadow_map(&device, SHADOW_MAP_RESOLUTION);
        let local_shadow_setup = local_shadow_setup_for_scene(&scene);
        let local_shadow_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("local-shadow-matrices"),
            contents: bytemuck::bytes_of(&local_shadow_setup.uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let (local_shadow_texture, local_shadow_array_view, local_shadow_views) =
            create_local_shadow_map_array(&device, LOCAL_SHADOW_MAP_RESOLUTION);
        let mut local_shadow_camera_buffers = Vec::with_capacity(MAX_GPU_POINT_LIGHTS);
        let mut local_shadow_bind_groups = Vec::with_capacity(MAX_GPU_POINT_LIGHTS);
        for camera in local_shadow_setup.cameras {
            let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("local-shadow-camera-uniform"),
                contents: bytemuck::bytes_of(&camera),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("local-shadow-camera-bind-group"),
                layout: &camera_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
            });
            local_shadow_camera_buffers.push(camera_buffer);
            local_shadow_bind_groups.push(bind_group);
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene-vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene-indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let materials = scene
            .materials
            .iter()
            .map(|material| create_material(&device, &queue, &material_layout, material))
            .collect();
        let rain_resources = RainPassResources {
            camera_buffer: &camera_buffer,
            light_buffer: &light_buffer,
            shadow_view: &shadow_view,
            shadow_sampler: &shadow_sampler,
            shadow_camera_buffer: &shadow_camera_buffer,
        };
        let rain = create_rain_resources(&device, &rain_layout, rain_resources);

        let depth_pipeline = create_depth_pipeline(&device, &camera_layout);
        let shadow_pipeline = create_shadow_pipeline(&device, &camera_layout);
        let geometry_pipeline =
            create_geometry_pipeline(&device, &camera_layout, &material_layout, config.format);
        let reflection_pipeline = create_reflection_pipeline(&device, &reflection_layout);
        let lighting_pipeline = create_lighting_pipeline(&device, &lighting_layout);
        let bloom_extract_pipeline =
            create_bloom_pipeline(&device, &bloom_layout, bloom_format, "fs_extract");
        let bloom_blur_horizontal_pipeline =
            create_bloom_pipeline(&device, &bloom_layout, bloom_format, "fs_blur_horizontal");
        let bloom_blur_vertical_pipeline =
            create_bloom_pipeline(&device, &bloom_layout, bloom_format, "fs_blur_vertical");
        let tonemap_pipeline = create_tonemap_pipeline(&device, &tonemap_layout, config.format);
        let rain_pipeline = create_rain_pipeline(&device, &rain_layout, HDR_FORMAT);
        let aa_pipeline = create_aa_pipeline(&device, &aa_layout, config.format);
        let gbuffer = GBuffer::new(
            &device,
            config.width,
            config.height,
            config.format,
            bloom_format,
            &reflection_layout,
            &lighting_layout,
            &bloom_layout,
            &tonemap_layout,
            &aa_layout,
            &frame_buffer,
            &camera_buffer,
            &aa_buffer,
            &shadow_view,
            &shadow_sampler,
            &shadow_camera_buffer,
            &light_buffer,
            &local_shadow_array_view,
            &local_shadow_buffer,
        );

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            adapter: adapter_info,
            vertex_buffer,
            index_buffer,
            batches,
            materials,
            camera_buffer,
            camera_bind_group,
            frame_buffer,
            aa_buffer,
            light_buffer,
            shadow_camera_buffer,
            shadow_camera_bind_group,
            _shadow_texture: shadow_texture,
            shadow_view,
            shadow_sampler,
            local_shadow_buffer,
            _local_shadow_texture: local_shadow_texture,
            local_shadow_array_view,
            local_shadow_views,
            _local_shadow_camera_buffers: local_shadow_camera_buffers,
            local_shadow_bind_groups,
            shadowed_local_light_indices: local_shadow_setup.shadowed_indices,
            depth_pipeline,
            shadow_pipeline,
            geometry_pipeline,
            reflection_pipeline,
            lighting_pipeline,
            bloom_extract_pipeline,
            bloom_blur_horizontal_pipeline,
            bloom_blur_vertical_pipeline,
            tonemap_pipeline,
            aa_pipeline,
            rain_pipeline,
            rain,
            reflection_layout,
            lighting_layout,
            bloom_layout,
            tonemap_layout,
            aa_layout,
            bloom_format,
            gbuffer,
            started: Instant::now(),
            last_update: Instant::now(),
            frame_index: 0,
            current_view_projection: Mat4::IDENTITY,
            previous_view_projection: Mat4::IDENTITY,
            auto_orbit: true,
            camera_origin,
            camera_target,
            keys: CameraKeys::default(),
            base_light_uniform: light_uniform,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.gbuffer = GBuffer::new(
            &self.device,
            size.width,
            size.height,
            self.config.format,
            self.bloom_format,
            &self.reflection_layout,
            &self.lighting_layout,
            &self.bloom_layout,
            &self.tonemap_layout,
            &self.aa_layout,
            &self.frame_buffer,
            &self.camera_buffer,
            &self.aa_buffer,
            &self.shadow_view,
            &self.shadow_sampler,
            &self.shadow_camera_buffer,
            &self.light_buffer,
            &self.local_shadow_array_view,
            &self.local_shadow_buffer,
        );
    }

    fn toggle_auto_orbit(&mut self) {
        if self.auto_orbit {
            self.enter_manual_control();
        } else {
            self.auto_orbit = true;
        }
    }

    fn set_key(&mut self, code: KeyCode, pressed: bool) {
        let handled = match code {
            KeyCode::KeyW => {
                self.keys.move_forward = pressed;
                true
            }
            KeyCode::KeyS => {
                self.keys.move_back = pressed;
                true
            }
            KeyCode::KeyA => {
                self.keys.move_left = pressed;
                true
            }
            KeyCode::KeyD => {
                self.keys.move_right = pressed;
                true
            }
            KeyCode::ArrowLeft => {
                self.keys.look_left = pressed;
                true
            }
            KeyCode::ArrowRight => {
                self.keys.look_right = pressed;
                true
            }
            KeyCode::ArrowUp => {
                self.keys.look_up = pressed;
                true
            }
            KeyCode::ArrowDown => {
                self.keys.look_down = pressed;
                true
            }
            _ => false,
        };
        if handled && pressed {
            self.enter_manual_control();
        }
    }

    fn enter_manual_control(&mut self) {
        if self.auto_orbit {
            let elapsed = self.started.elapsed().as_secs_f32();
            let (eye, target) = self.camera_pose(elapsed);
            self.camera_origin = eye;
            self.camera_target = target;
            self.auto_orbit = false;
        }
    }

    fn camera_pose(&self, elapsed: f32) -> (GpuVec3, GpuVec3) {
        if self.auto_orbit {
            let sway = (elapsed * 0.22).sin() * 0.7;
            (
                self.camera_origin + GpuVec3::new(sway, (elapsed * 0.13).sin() * 0.035, 0.0),
                self.camera_target + GpuVec3::new(sway * 0.18, 0.0, 0.0),
            )
        } else {
            (self.camera_origin, self.camera_target)
        }
    }

    fn apply_manual_camera(&mut self, dt: f32) {
        if self.auto_orbit {
            return;
        }

        apply_camera_keys_to_pose(
            &mut self.camera_origin,
            &mut self.camera_target,
            self.keys,
            dt,
        );
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_update)
            .as_secs_f32()
            .clamp(0.0, 0.05);
        self.last_update = now;
        let elapsed = self.started.elapsed().as_secs_f32();
        self.apply_manual_camera(dt);
        let (eye, target) = self.camera_pose(elapsed);
        let view = Mat4::look_at_rh(eye, target, GpuVec3::Y);
        let projection = Mat4::perspective_rh(
            55.0_f32.to_radians(),
            self.config.width as f32 / self.config.height as f32,
            0.05,
            40.0,
        );
        let view_projection = projection * view;
        self.previous_view_projection = self.current_view_projection;
        self.current_view_projection = view_projection;
        let viewport = [
            self.config.width as f32,
            self.config.height as f32,
            1.0 / self.config.width as f32,
            1.0 / self.config.height as f32,
        ];
        let camera = CameraUniform {
            view_projection: view_projection.to_cols_array_2d(),
            inverse_view_projection: view_projection.inverse().to_cols_array_2d(),
            position_time: [eye.x, eye.y, eye.z, elapsed],
            viewport,
        };
        let frame = FrameUniform {
            dimensions: [
                self.config.width as f32,
                self.config.height as f32,
                self.gbuffer.reflection_width as f32,
                self.gbuffer.reflection_height as f32,
            ],
            counters: [self.frame_index, 0, 0, 0],
        };
        let aa = AaUniform {
            previous_view_projection: self.previous_view_projection.to_cols_array_2d(),
            viewport,
            params: [
                if self.frame_index == 0 { 0.0 } else { 0.16 },
                0.045,
                0.42,
                0.0,
            ],
        };
        let lights = self.base_light_uniform.realtime(elapsed);
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera));
        self.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&frame));
        self.queue
            .write_buffer(&self.aa_buffer, 0, bytemuck::bytes_of(&aa));
        self.queue
            .write_buffer(&self.light_buffer, 0, bytemuck::bytes_of(&lights));
    }

    fn render(&mut self) -> RenderResult {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return RenderResult::Skipped
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return RenderResult::Reconfigure
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return RenderResult::Fatal("surface acquisition validation failure".to_string())
            }
        };
        let target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dhool-frame"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cached-directional-shadow-map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.shadow_camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for batch in &self.batches {
                if batch.casts_shadow {
                    pass.draw_indexed(batch.indices.clone(), 0, 0..1);
                }
            }
        }

        for &light_index in &self.shadowed_local_light_indices {
            let Some(view) = self.local_shadow_views.get(light_index) else {
                continue;
            };
            let Some(bind_group) = self.local_shadow_bind_groups.get(light_index) else {
                continue;
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cached-local-shadow-map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for batch in &self.batches {
                if batch.casts_shadow {
                    pass.draw_indexed(batch.indices.clone(), 0, 0..1);
                }
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("partial-depth-prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gbuffer.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.depth_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for batch in &self.batches {
                if batch.prepass_eligible {
                    pass.draw_indexed(batch.indices.clone(), 0, 0..1);
                }
            }
        }

        {
            let color_attachments = [
                color_attachment(&self.gbuffer.albedo, wgpu::Color::BLACK),
                color_attachment(&self.gbuffer.normal, wgpu::Color::BLACK),
                color_attachment(
                    &self.gbuffer.pbr,
                    wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 1.0,
                        a: 0.0,
                    },
                ),
            ];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("base-pass-gbuffer"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gbuffer.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.geometry_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for batch in &self.batches {
                pass.set_bind_group(1, &self.materials[batch.material_index].bind_group, &[]);
                pass.draw_indexed(batch.indices.clone(), 0, 0..1);
            }
        }

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("vendor-agnostic-compute-reflections"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.reflection_pipeline);
            pass.set_bind_group(0, &self.gbuffer.reflection_bind_group, &[]);
            pass.dispatch_workgroups(
                self.gbuffer.reflection_width.div_ceil(8),
                self.gbuffer.reflection_height.div_ceil(8),
                1,
            );
        }

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.gbuffer.lighting_hdr,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hdr-deferred-lighting"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.lighting_pipeline);
            pass.set_bind_group(0, &self.gbuffer.lighting_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.gbuffer.lighting_hdr,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world-space-depth-tested-rain"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gbuffer.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.rain_pipeline);
            pass.set_bind_group(0, &self.rain.bind_group, &[]);
            pass.draw(0..RAIN_PRISM_VERTEX_COUNT, 0..RAIN_DROP_COUNT);
        }

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.gbuffer.bloom_a,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kawase-bloom-extract"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.bloom_extract_pipeline);
            pass.set_bind_group(0, &self.gbuffer.bloom_extract_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.gbuffer.bloom_b,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kawase-bloom-horizontal"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.bloom_blur_horizontal_pipeline);
            pass.set_bind_group(0, &self.gbuffer.bloom_blur_a_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.gbuffer.bloom_a,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kawase-bloom-vertical"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.bloom_blur_vertical_pipeline);
            pass.set_bind_group(0, &self.gbuffer.bloom_blur_b_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &self.gbuffer.tone_mapped,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gt-tonemap-bloom-resolve"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.tonemap_pipeline);
            pass.set_bind_group(0, &self.gbuffer.tonemap_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let (aa_bind_group, history_write) = if self.frame_index & 1 == 0 {
                (&self.gbuffer.aa_bind_group_a, &self.gbuffer.aa_history_b)
            } else {
                (&self.gbuffer.aa_bind_group_b, &self.gbuffer.aa_history_a)
            };
            let color_attachments = [
                Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: history_write,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                }),
            ];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("smaa-t2x-edge-persistence-resolve"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.aa_pipeline);
            pass.set_bind_group(0, aa_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        self.frame_index = self.frame_index.wrapping_add(1);
        RenderResult::Presented
    }
}

impl GBuffer {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        surface_format: wgpu::TextureFormat,
        bloom_format: wgpu::TextureFormat,
        reflection_layout: &wgpu::BindGroupLayout,
        lighting_layout: &wgpu::BindGroupLayout,
        bloom_layout: &wgpu::BindGroupLayout,
        tonemap_layout: &wgpu::BindGroupLayout,
        aa_layout: &wgpu::BindGroupLayout,
        frame_buffer: &wgpu::Buffer,
        camera_buffer: &wgpu::Buffer,
        aa_buffer: &wgpu::Buffer,
        shadow_view: &wgpu::TextureView,
        shadow_sampler: &wgpu::Sampler,
        shadow_camera_buffer: &wgpu::Buffer,
        light_buffer: &wgpu::Buffer,
        local_shadow_view: &wgpu::TextureView,
        local_shadow_buffer: &wgpu::Buffer,
    ) -> Self {
        let (albedo_texture, albedo) =
            render_texture(device, "gbuffer-albedo", width, height, ALBEDO_FORMAT);
        let (normal_texture, normal) =
            render_texture(device, "gbuffer-normal-flags", width, height, NORMAL_FORMAT);
        let (pbr_texture, pbr) =
            render_texture(device, "gbuffer-pbr-material", width, height, PBR_FORMAT);
        let (depth_texture, depth) = depth_texture(device, width, height);
        let (lighting_texture, lighting_hdr) =
            render_texture(device, "hdr-lighting-rgba16f", width, height, HDR_FORMAT);
        let bloom_width = width.div_ceil(BLOOM_SCALE).max(1);
        let bloom_height = height.div_ceil(BLOOM_SCALE).max(1);
        let (bloom_texture_a, bloom_a) = render_texture(
            device,
            "kawase-bloom-a",
            bloom_width,
            bloom_height,
            bloom_format,
        );
        let (bloom_texture_b, bloom_b) = render_texture(
            device,
            "kawase-bloom-b",
            bloom_width,
            bloom_height,
            bloom_format,
        );
        let (tone_texture, tone_mapped) =
            render_texture(device, "tone-mapped-rgb10a2", width, height, surface_format);
        let (history_texture_a, aa_history_a) =
            render_texture(device, "smaa-t2x-history-a", width, height, surface_format);
        let (history_texture_b, aa_history_b) =
            render_texture(device, "smaa-t2x-history-b", width, height, surface_format);
        let reflection_width = ((width * 7) / 10).max(1);
        let reflection_height = ((height * 7) / 10).max(1);
        let reflection_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("software-rt-reflection-70-percent"),
            size: wgpu::Extent3d {
                width: reflection_width,
                height: reflection_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: REFLECTION_FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let reflection = reflection_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let reflection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("reflection-bind-group"),
            layout: reflection_layout,
            entries: &[
                texture_entry(0, &normal),
                texture_entry(1, &pbr),
                texture_entry(2, &depth),
                texture_entry(3, &reflection),
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: frame_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });
        let lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lighting-bind-group"),
            layout: lighting_layout,
            entries: &[
                texture_entry(0, &albedo),
                texture_entry(1, &normal),
                texture_entry(2, &pbr),
                texture_entry(3, &depth),
                texture_entry(4, &reflection),
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: camera_buffer.as_entire_binding(),
                },
                texture_entry(6, shadow_view),
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: shadow_camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: light_buffer.as_entire_binding(),
                },
                texture_entry(10, local_shadow_view),
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: local_shadow_buffer.as_entire_binding(),
                },
            ],
        });
        let bloom_extract_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom-extract-bind-group"),
            layout: bloom_layout,
            entries: &[texture_entry(0, &lighting_hdr)],
        });
        let bloom_blur_a_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom-blur-a-bind-group"),
            layout: bloom_layout,
            entries: &[texture_entry(0, &bloom_a)],
        });
        let bloom_blur_b_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom-blur-b-bind-group"),
            layout: bloom_layout,
            entries: &[texture_entry(0, &bloom_b)],
        });
        let tonemap_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tonemap-bind-group"),
            layout: tonemap_layout,
            entries: &[texture_entry(0, &lighting_hdr), texture_entry(1, &bloom_a)],
        });
        let aa_bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa-t2x-bind-group-history-a"),
            layout: aa_layout,
            entries: &[
                texture_entry(0, &tone_mapped),
                texture_entry(1, &aa_history_a),
                texture_entry(2, &depth),
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: aa_buffer.as_entire_binding(),
                },
            ],
        });
        let aa_bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa-t2x-bind-group-history-b"),
            layout: aa_layout,
            entries: &[
                texture_entry(0, &tone_mapped),
                texture_entry(1, &aa_history_b),
                texture_entry(2, &depth),
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: aa_buffer.as_entire_binding(),
                },
            ],
        });
        Self {
            _albedo: albedo_texture,
            _normal: normal_texture,
            _pbr: pbr_texture,
            _depth: depth_texture,
            _reflection: reflection_texture,
            _lighting_hdr: lighting_texture,
            _bloom_a: bloom_texture_a,
            _bloom_b: bloom_texture_b,
            _tone_mapped: tone_texture,
            _aa_history_a: history_texture_a,
            _aa_history_b: history_texture_b,
            albedo,
            normal,
            pbr,
            depth,
            lighting_hdr,
            bloom_a,
            bloom_b,
            tone_mapped,
            aa_history_a,
            aa_history_b,
            reflection_bind_group,
            lighting_bind_group,
            bloom_extract_bind_group,
            bloom_blur_a_bind_group,
            bloom_blur_b_bind_group,
            tonemap_bind_group,
            aa_bind_group_a,
            aa_bind_group_b,
            reflection_width,
            reflection_height,
        }
    }
}

async fn select_adapter(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
) -> Result<wgpu::Adapter, LiveError> {
    if let Ok(requested) = std::env::var("DHOOL_GPU") {
        let requested = requested.to_lowercase();
        if let Some(adapter) = instance
            .enumerate_adapters(wgpu::Backends::all())
            .await
            .into_iter()
            .find(|adapter| {
                adapter.is_surface_supported(surface)
                    && adapter.get_info().name.to_lowercase().contains(&requested)
            })
        {
            return Ok(adapter);
        }
        return Err(LiveError::Device(format!(
            "DHOOL_GPU={requested} did not match a present-capable adapter"
        )));
    }
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(surface),
        })
        .await
        .map_err(|_| LiveError::AdapterUnavailable)
}

fn flatten_scene(scene: &RenderScene) -> (Vec<GpuVertex>, Vec<u32>, Vec<DrawBatch>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut batches = Vec::new();
    for mesh in &scene.meshes {
        let base_vertex = vertices.len() as u32;
        for vertex in &mesh.vertices {
            let position = mesh.transform.mul_vec4(Vec4::new(
                vertex.position.x,
                vertex.position.y,
                vertex.position.z,
                1.0,
            ));
            let normal = mesh.transform.mul_vec4(Vec4::new(
                vertex.normal.x,
                vertex.normal.y,
                vertex.normal.z,
                0.0,
            ));
            let normal = Vec3::new(normal.x, normal.y, normal.z).normalize_or_zero();
            let tangent = mesh.transform.mul_vec4(Vec4::new(
                vertex.tangent.x,
                vertex.tangent.y,
                vertex.tangent.z,
                0.0,
            ));
            let tangent = Vec3::new(tangent.x, tangent.y, tangent.z).normalize_or_zero();
            vertices.push(GpuVertex {
                position: [position.x, position.y, position.z],
                normal: [normal.x, normal.y, normal.z],
                uv: [vertex.uv.x, vertex.uv.y],
                color: [
                    vertex.color.r,
                    vertex.color.g,
                    vertex.color.b,
                    vertex.color.a,
                ],
                tangent: [tangent.x, tangent.y, tangent.z, vertex.tangent_sign],
            });
        }
        let first_index = indices.len() as u32;
        for triangle in &mesh.indices {
            indices.extend([
                triangle[0] + base_vertex,
                triangle[1] + base_vertex,
                triangle[2] + base_vertex,
            ]);
        }
        batches.push(DrawBatch {
            indices: first_index..indices.len() as u32,
            material_index: mesh.material_index,
            prepass_eligible: mesh.prepass_eligible
                && !scene.materials[mesh.material_index].flags.alpha_tested,
            casts_shadow: !mesh.is_floor_or_ground
                && mesh.shadow_class != crate::asset::ShadowCasterClass::None,
        });
    }
    (vertices, indices, batches)
}

fn shadow_camera_for_scene(lights: LightUniform, vertices: &[GpuVertex]) -> CameraUniform {
    let surface_to_light = lights.surface_to_directional_light();
    let surface_to_light = if surface_to_light.length_squared() > 0.0 {
        surface_to_light
    } else {
        GpuVec3::new(0.35, 0.75, 0.45).normalize()
    };
    let mut min = GpuVec3::splat(f32::INFINITY);
    let mut max = GpuVec3::splat(f32::NEG_INFINITY);
    for vertex in vertices {
        let position = GpuVec3::from_array(vertex.position);
        min = min.min(position);
        max = max.max(position);
    }
    if vertices.is_empty() {
        min = GpuVec3::new(-8.0, -1.0, -18.0);
        max = GpuVec3::new(8.0, 6.0, 6.0);
    }
    let center = (min + max) * 0.5;
    let radius = ((max - min).length() * 0.55).max(8.0);
    let eye = center + surface_to_light * (radius * 2.4);
    let view = Mat4::look_at_rh(eye, center, GpuVec3::Y);
    let projection = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.1, radius * 4.8);
    let view_projection = projection * view;
    CameraUniform {
        view_projection: view_projection.to_cols_array_2d(),
        inverse_view_projection: view_projection.inverse().to_cols_array_2d(),
        position_time: [eye.x, eye.y, eye.z, 0.0],
        viewport: [
            SHADOW_MAP_RESOLUTION as f32,
            SHADOW_MAP_RESOLUTION as f32,
            1.0 / SHADOW_MAP_RESOLUTION as f32,
            1.0 / SHADOW_MAP_RESOLUTION as f32,
        ],
    }
}

struct LocalShadowSetup {
    uniform: LocalShadowUniform,
    cameras: [CameraUniform; MAX_GPU_POINT_LIGHTS],
    shadowed_indices: Vec<usize>,
}

fn local_shadow_setup_for_scene(scene: &RenderScene) -> LocalShadowSetup {
    let mut uniform = LocalShadowUniform::zeroed();
    let mut cameras = [CameraUniform::zeroed(); MAX_GPU_POINT_LIGHTS];
    let mut shadowed_indices = Vec::new();
    let mut local_index = 0_usize;

    for light in &scene.lights {
        match light.kind {
            LightKind::Directional { .. } => {}
            LightKind::Local { .. } => {
                local_index += 1;
            }
            LightKind::Spot {
                position,
                direction,
                radius,
                outer_angle_radians,
                ..
            } => {
                if local_index < MAX_GPU_POINT_LIGHTS && light.casts_shadow {
                    let camera =
                        spot_shadow_camera(position, direction, radius, outer_angle_radians);
                    uniform.view_projection[local_index] = camera.view_projection;
                    cameras[local_index] = camera;
                    shadowed_indices.push(local_index);
                }
                local_index += 1;
            }
        }

        if local_index >= MAX_GPU_POINT_LIGHTS {
            break;
        }
    }

    LocalShadowSetup {
        uniform,
        cameras,
        shadowed_indices,
    }
}

fn spot_shadow_camera(
    position: crate::math::Vec3,
    direction: crate::math::Vec3,
    radius: f32,
    outer_angle_radians: f32,
) -> CameraUniform {
    let eye = GpuVec3::new(position.x, position.y, position.z);
    let direction = GpuVec3::new(direction.x, direction.y, direction.z).normalize_or_zero();
    let direction = if direction.length_squared() > 0.0 {
        direction
    } else {
        -GpuVec3::Z
    };
    let up = if direction.dot(GpuVec3::Y).abs() > 0.92 {
        GpuVec3::Z
    } else {
        GpuVec3::Y
    };
    let view = Mat4::look_at_rh(eye, eye + direction, up);
    let fov = (outer_angle_radians * 2.0).clamp(20.0_f32.to_radians(), 135.0_f32.to_radians());
    let projection = Mat4::perspective_rh(fov, 1.0, 0.05, radius.max(0.1));
    let view_projection = projection * view;
    CameraUniform {
        view_projection: view_projection.to_cols_array_2d(),
        inverse_view_projection: view_projection.inverse().to_cols_array_2d(),
        position_time: [eye.x, eye.y, eye.z, 0.0],
        viewport: [
            LOCAL_SHADOW_MAP_RESOLUTION as f32,
            LOCAL_SHADOW_MAP_RESOLUTION as f32,
            1.0 / LOCAL_SHADOW_MAP_RESOLUTION as f32,
            1.0 / LOCAL_SHADOW_MAP_RESOLUTION as f32,
        ],
    }
}

fn create_material(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    material: &Material,
) -> GpuMaterial {
    let emission_strength = material
        .emission
        .r
        .max(material.emission.g)
        .max(material.emission.b);
    let uniform = MaterialUniform {
        base_color: [
            material.base_color.r,
            material.base_color.g,
            material.base_color.b,
            material.base_color.a,
        ],
        surface: [
            material.roughness,
            material.metallic,
            material.cavity,
            emission_strength,
        ],
        emission: [
            material.emission.r,
            material.emission.g,
            material.emission.b,
            EMISSIVE_SCALE,
        ],
    };
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{}-uniform", material.name)),
        contents: bytemuck::bytes_of(&uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let white = solid_texture(crate::math::Color::WHITE);
    let flat_normal = solid_texture(crate::math::Color::new(0.5, 0.5, 1.0, 1.0));
    let default_pbr = solid_texture(crate::math::Color::new(1.0, 1.0, 0.0, 1.0));
    let (base_texture, base_view) = upload_material_texture(
        device,
        queue,
        &format!("{}-base-color", material.name),
        material.base_color_texture.as_ref().unwrap_or(&white),
        true,
    );
    let (normal_texture, normal_view) = upload_material_texture(
        device,
        queue,
        &format!("{}-normal", material.name),
        material.normal_texture.as_ref().unwrap_or(&flat_normal),
        false,
    );
    let (pbr_texture, pbr_view) = upload_material_texture(
        device,
        queue,
        &format!("{}-pbr", material.name),
        material.pbr_texture.as_ref().unwrap_or(&default_pbr),
        false,
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("material-linear-repeat"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..wgpu::SamplerDescriptor::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{}-bind-group", material.name)),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            texture_entry(1, &base_view),
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            texture_entry(3, &normal_view),
            texture_entry(4, &pbr_view),
        ],
    });
    GpuMaterial {
        _uniform: uniform_buffer,
        _textures: vec![base_texture, normal_texture, pbr_texture],
        bind_group,
    }
}

fn create_rain_resources(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    resources: RainPassResources<'_>,
) -> RainTextures {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("world-space-rain-bind-group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: resources.camera_buffer.as_entire_binding(),
            },
            texture_entry(3, resources.shadow_view),
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(resources.shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: resources.shadow_camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: resources.light_buffer.as_entire_binding(),
            },
        ],
    });
    RainTextures { bind_group }
}

fn solid_texture(color: crate::math::Color) -> TextureImage {
    TextureImage {
        width: 1,
        height: 1,
        pixels: vec![color],
    }
}

fn upload_material_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    texture: &TextureImage,
    srgb: bool,
) -> (wgpu::Texture, wgpu::TextureView) {
    let (mip_count, bytes) = build_mip_chain(texture);
    let gpu_texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        TextureDataOrder::MipMajor,
        &bytes,
    );
    let view = gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (gpu_texture, view)
}

fn build_mip_chain(texture: &TextureImage) -> (u32, Vec<u8>) {
    let mut width = texture.width.max(1);
    let mut height = texture.height.max(1);
    let mut level: Vec<[u8; 4]> = texture
        .pixels
        .iter()
        .map(|color| color.to_rgba8())
        .collect();
    let mut bytes = Vec::with_capacity(level.len() * 5);
    let mut mip_count = 0;
    loop {
        bytes.extend(level.iter().flatten());
        mip_count += 1;
        if width == 1 && height == 1 {
            break;
        }
        let next_width = (width / 2).max(1);
        let next_height = (height / 2).max(1);
        let mut next = Vec::with_capacity((next_width * next_height) as usize);
        for y in 0..next_height {
            for x in 0..next_width {
                let mut sum = [0_u32; 4];
                let mut samples = 0_u32;
                for oy in 0..2 {
                    for ox in 0..2 {
                        let source_x = (x * 2 + ox).min(width - 1);
                        let source_y = (y * 2 + oy).min(height - 1);
                        let pixel = level[(source_y * width + source_x) as usize];
                        for channel in 0..4 {
                            sum[channel] += pixel[channel] as u32;
                        }
                        samples += 1;
                    }
                }
                next.push([
                    (sum[0] / samples) as u8,
                    (sum[1] / samples) as u8,
                    (sum[2] / samples) as u8,
                    (sum[3] / samples) as u8,
                ]);
            }
        }
        level = next;
        width = next_width;
        height = next_height;
    }
    (mip_count, bytes)
}

fn create_depth_pipeline(
    device: &wgpu::Device,
    camera_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/depth_prepass.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("depth-prepass-pipeline-layout"),
        bind_group_layouts: &[Some(camera_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("partial-depth-prepass-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[GpuVertex::layout()],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}

fn create_shadow_pipeline(
    device: &wgpu::Device,
    camera_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/depth_prepass.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow-pipeline-layout"),
        bind_group_layouts: &[Some(camera_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("directional-shadow-map-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[GpuVertex::layout()],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Front),
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 1,
                slope_scale: 1.25,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}

fn create_geometry_pipeline(
    device: &wgpu::Device,
    camera_layout: &wgpu::BindGroupLayout,
    material_layout: &wgpu::BindGroupLayout,
    _surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/geometry.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("geometry-pipeline-layout"),
        bind_group_layouts: &[Some(camera_layout), Some(material_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("deferred-gbuffer-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[GpuVertex::layout()],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[
                Some(color_target(ALBEDO_FORMAT)),
                Some(color_target(NORMAL_FORMAT)),
                Some(color_target(PBR_FORMAT)),
            ],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_reflection_pipeline(
    device: &wgpu::Device,
    reflection_layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/reflections.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("reflection-pipeline-layout"),
        bind_group_layouts: &[Some(reflection_layout)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("vendor-agnostic-software-rt-compute"),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("cs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn create_lighting_pipeline(
    device: &wgpu::Device,
    lighting_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/lighting.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lighting-pipeline-layout"),
        bind_group_layouts: &[Some(lighting_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fullscreen-deferred-hdr-lighting"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(color_target(HDR_FORMAT))],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_bloom_pipeline(
    device: &wgpu::Device,
    bloom_layout: &wgpu::BindGroupLayout,
    bloom_format: wgpu::TextureFormat,
    entry_point: &'static str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/bloom.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bloom-pipeline-layout"),
        bind_group_layouts: &[Some(bloom_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("kawase-bloom-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(color_target(bloom_format))],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_tonemap_pipeline(
    device: &wgpu::Device,
    tonemap_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/tonemap.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tonemap-pipeline-layout"),
        bind_group_layouts: &[Some(tonemap_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("gt-tonemap-bloom-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(color_target(surface_format))],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_rain_pipeline(
    device: &wgpu::Device,
    rain_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/rain3d.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("world-space-rain-pipeline-layout"),
        bind_group_layouts: &[Some(rain_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("world-space-depth-tested-rain-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(alpha_blend_target(surface_format))],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_aa_pipeline(
    device: &wgpu::Device,
    aa_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/aa.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("smaa-t2x-pipeline-layout"),
        bind_group_layouts: &[Some(aa_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smaa-t2x-edge-persistence-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[
                Some(color_target(surface_format)),
                Some(color_target(surface_format)),
            ],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("material-layout"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
            sampled_texture_layout_entry(1, wgpu::ShaderStages::FRAGMENT),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            sampled_texture_layout_entry(3, wgpu::ShaderStages::FRAGMENT),
            sampled_texture_layout_entry(4, wgpu::ShaderStages::FRAGMENT),
        ],
    })
}

fn reflection_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("reflection-layout"),
        entries: &[
            unfiltered_texture_layout_entry(0, wgpu::ShaderStages::COMPUTE),
            unfiltered_texture_layout_entry(1, wgpu::ShaderStages::COMPUTE),
            depth_layout_entry(2, wgpu::ShaderStages::COMPUTE),
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: REFLECTION_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            uniform_entry(4, wgpu::ShaderStages::COMPUTE),
            uniform_entry(5, wgpu::ShaderStages::COMPUTE),
            uniform_entry(6, wgpu::ShaderStages::COMPUTE),
        ],
    })
}

fn lighting_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("lighting-layout"),
        entries: &[
            unfiltered_texture_layout_entry(0, wgpu::ShaderStages::FRAGMENT),
            unfiltered_texture_layout_entry(1, wgpu::ShaderStages::FRAGMENT),
            unfiltered_texture_layout_entry(2, wgpu::ShaderStages::FRAGMENT),
            depth_layout_entry(3, wgpu::ShaderStages::FRAGMENT),
            unfiltered_texture_layout_entry(4, wgpu::ShaderStages::FRAGMENT),
            uniform_entry(5, wgpu::ShaderStages::FRAGMENT),
            depth_layout_entry(6, wgpu::ShaderStages::FRAGMENT),
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            uniform_entry(8, wgpu::ShaderStages::FRAGMENT),
            uniform_entry(9, wgpu::ShaderStages::FRAGMENT),
            depth_array_layout_entry(10, wgpu::ShaderStages::FRAGMENT),
            uniform_entry(11, wgpu::ShaderStages::FRAGMENT),
        ],
    })
}

fn bloom_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bloom-layout"),
        entries: &[unfiltered_texture_layout_entry(
            0,
            wgpu::ShaderStages::FRAGMENT,
        )],
    })
}

fn tonemap_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tonemap-layout"),
        entries: &[
            unfiltered_texture_layout_entry(0, wgpu::ShaderStages::FRAGMENT),
            unfiltered_texture_layout_entry(1, wgpu::ShaderStages::FRAGMENT),
        ],
    })
}

fn aa_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smaa-t2x-layout"),
        entries: &[
            unfiltered_texture_layout_entry(0, wgpu::ShaderStages::FRAGMENT),
            unfiltered_texture_layout_entry(1, wgpu::ShaderStages::FRAGMENT),
            depth_layout_entry(2, wgpu::ShaderStages::FRAGMENT),
            uniform_entry(3, wgpu::ShaderStages::FRAGMENT),
            uniform_entry(4, wgpu::ShaderStages::FRAGMENT),
        ],
    })
}

fn rain_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("world-space-rain-layout"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
            depth_layout_entry(3, wgpu::ShaderStages::FRAGMENT),
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            uniform_entry(5, wgpu::ShaderStages::FRAGMENT),
            uniform_entry(6, wgpu::ShaderStages::VERTEX_FRAGMENT),
        ],
    })
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn sampled_texture_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn unfiltered_texture_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn depth_layout_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn depth_array_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}

fn render_texture(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hardware-depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn create_shadow_map(
    device: &wgpu::Device,
    resolution: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("directional-shadow-map"),
        size: wgpu::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("shadow-comparison-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        compare: Some(wgpu::CompareFunction::LessEqual),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..wgpu::SamplerDescriptor::default()
    });
    (texture, view, sampler)
}

fn create_local_shadow_map_array(
    device: &wgpu::Device,
    resolution: u32,
) -> (wgpu::Texture, wgpu::TextureView, Vec<wgpu::TextureView>) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("local-shadow-map-array"),
        size: wgpu::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: MAX_GPU_POINT_LIGHTS as u32,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("local-shadow-map-array-view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..wgpu::TextureViewDescriptor::default()
    });
    let layer_views = (0..MAX_GPU_POINT_LIGHTS)
        .map(|layer| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("local-shadow-map-layer"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer as u32,
                array_layer_count: Some(1),
                ..wgpu::TextureViewDescriptor::default()
            })
        })
        .collect();
    (texture, array_view, layer_views)
}

fn color_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn alpha_blend_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn color_attachment(
    view: &wgpu::TextureView,
    clear: wgpu::Color,
) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(clear),
            store: wgpu::StoreOp::Store,
        },
    })
}

fn texture_entry(binding: u32, view: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(view),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_lights_populate_gpu_uniform() {
        let scene = crate::scene::build_demo_scene();
        let uniform = LightUniform::from_scene(&scene);
        let expected_direction = GpuVec3::new(0.35, 0.75, 0.45).normalize();
        let actual_direction = GpuVec3::new(
            uniform.directional_direction_count[0],
            uniform.directional_direction_count[1],
            uniform.directional_direction_count[2],
        );

        assert!((actual_direction - expected_direction).length() < 0.001);
        assert_eq!(uniform.directional_direction_count[3], 1.0);
        assert_eq!(uniform.directional_color_intensity[3], 2.4);
        assert_eq!(uniform.local_position_radius[0], [0.9, 1.2, 1.1, 4.0]);
        assert_eq!(uniform.local_color_intensity[0][3], 5.0);
    }

    #[test]
    fn wasd_moves_camera_relative_to_view_direction() {
        let mut eye = GpuVec3::new(0.0, 1.5, 5.0);
        let mut target = GpuVec3::new(0.0, 1.5, 0.0);
        let keys = CameraKeys {
            move_forward: true,
            move_right: true,
            ..CameraKeys::default()
        };

        apply_camera_keys_to_pose(&mut eye, &mut target, keys, 1.0);

        assert!(eye.x > 0.0);
        assert!(eye.z < 5.0);
        assert!(((target - eye).length() - 5.0).abs() < 0.001);
        assert!(((eye - GpuVec3::new(0.0, 1.5, 5.0)).length() - CAMERA_MOVE_SPEED).abs() < 0.001);
    }

    #[test]
    fn arrow_look_rotates_target_without_translating_eye() {
        let original_eye = GpuVec3::new(0.0, 1.5, 5.0);
        let mut eye = original_eye;
        let mut target = GpuVec3::new(0.0, 1.5, 0.0);
        let keys = CameraKeys {
            look_right: true,
            ..CameraKeys::default()
        };

        apply_camera_keys_to_pose(&mut eye, &mut target, keys, 1.0 / CAMERA_LOOK_SPEED);

        assert_eq!(eye, original_eye);
        assert!(target.x.abs() > 0.1);
        assert!(((target - eye).length() - 5.0).abs() < 0.001);
    }
}
