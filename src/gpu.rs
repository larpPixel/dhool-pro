use crate::math::{Vec3, Vec4};
use crate::scene::{build_aaa_scene, build_asset_showcase, Material, RenderScene, TextureImage};
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
const CAMERA_MOVE_SPEED: f32 = 4.0;
const CAMERA_LOOK_SPEED: f32 = 1.55;

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
            "live pipeline: shadow map -> depth prepass/PBR G-buffer -> compute reflections -> deferred GT resolve -> SMAA T2x"
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
                        "Dhool Pro | {} | {:.0} FPS | compute RT + SMAA T2x",
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

struct GBuffer {
    _albedo: wgpu::Texture,
    _normal: wgpu::Texture,
    _pbr: wgpu::Texture,
    _depth: wgpu::Texture,
    _reflection: wgpu::Texture,
    _tone_mapped: wgpu::Texture,
    _aa_history_a: wgpu::Texture,
    _aa_history_b: wgpu::Texture,
    albedo: wgpu::TextureView,
    normal: wgpu::TextureView,
    pbr: wgpu::TextureView,
    depth: wgpu::TextureView,
    tone_mapped: wgpu::TextureView,
    aa_history_a: wgpu::TextureView,
    aa_history_b: wgpu::TextureView,
    reflection_bind_group: wgpu::BindGroup,
    lighting_bind_group: wgpu::BindGroup,
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
    aa_buffer: wgpu::Buffer,
    shadow_camera_buffer: wgpu::Buffer,
    shadow_camera_bind_group: wgpu::BindGroup,
    _shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    depth_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    geometry_pipeline: wgpu::RenderPipeline,
    reflection_pipeline: wgpu::ComputePipeline,
    lighting_pipeline: wgpu::RenderPipeline,
    aa_pipeline: wgpu::RenderPipeline,
    reflection_layout: wgpu::BindGroupLayout,
    lighting_layout: wgpu::BindGroupLayout,
    aa_layout: wgpu::BindGroupLayout,
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
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
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
        let aa_layout = aa_layout(&device);

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
        let light_eye = GpuVec3::new(12.0, 19.0, 8.0);
        let light_target = GpuVec3::new(0.0, 0.0, -6.0);
        let light_view = Mat4::look_at_rh(light_eye, light_target, GpuVec3::Y);
        let light_projection = Mat4::orthographic_rh(-17.0, 17.0, -17.0, 17.0, 0.1, 55.0);
        let light_view_projection = light_projection * light_view;
        let shadow_camera = CameraUniform {
            view_projection: light_view_projection.to_cols_array_2d(),
            inverse_view_projection: light_view_projection.inverse().to_cols_array_2d(),
            position_time: [light_eye.x, light_eye.y, light_eye.z, 0.0],
            viewport: [2048.0, 2048.0, 1.0 / 2048.0, 1.0 / 2048.0],
        };
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
        let (shadow_texture, shadow_view, shadow_sampler) = create_shadow_map(&device, 2048);

        let (vertices, indices, batches) = flatten_scene(&scene);
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

        let depth_pipeline = create_depth_pipeline(&device, &camera_layout);
        let shadow_pipeline = create_shadow_pipeline(&device, &camera_layout);
        let geometry_pipeline =
            create_geometry_pipeline(&device, &camera_layout, &material_layout, config.format);
        let reflection_pipeline = create_reflection_pipeline(&device, &reflection_layout);
        let lighting_pipeline = create_lighting_pipeline(&device, &lighting_layout, config.format);
        let aa_pipeline = create_aa_pipeline(&device, &aa_layout, config.format);
        let gbuffer = GBuffer::new(
            &device,
            config.width,
            config.height,
            config.format,
            &reflection_layout,
            &lighting_layout,
            &aa_layout,
            &frame_buffer,
            &camera_buffer,
            &aa_buffer,
            &shadow_view,
            &shadow_sampler,
            &shadow_camera_buffer,
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
            shadow_camera_buffer,
            shadow_camera_bind_group,
            _shadow_texture: shadow_texture,
            shadow_view,
            shadow_sampler,
            depth_pipeline,
            shadow_pipeline,
            geometry_pipeline,
            reflection_pipeline,
            lighting_pipeline,
            aa_pipeline,
            reflection_layout,
            lighting_layout,
            aa_layout,
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
            &self.reflection_layout,
            &self.lighting_layout,
            &self.aa_layout,
            &self.frame_buffer,
            &self.camera_buffer,
            &self.aa_buffer,
            &self.shadow_view,
            &self.shadow_sampler,
            &self.shadow_camera_buffer,
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
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera));
        self.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&frame));
        self.queue
            .write_buffer(&self.aa_buffer, 0, bytemuck::bytes_of(&aa));
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
                view: &self.gbuffer.tone_mapped,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gt-tonemap-color-resolve"),
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
        reflection_layout: &wgpu::BindGroupLayout,
        lighting_layout: &wgpu::BindGroupLayout,
        aa_layout: &wgpu::BindGroupLayout,
        frame_buffer: &wgpu::Buffer,
        camera_buffer: &wgpu::Buffer,
        aa_buffer: &wgpu::Buffer,
        shadow_view: &wgpu::TextureView,
        shadow_sampler: &wgpu::Sampler,
        shadow_camera_buffer: &wgpu::Buffer,
    ) -> Self {
        let (albedo_texture, albedo) =
            render_texture(device, "gbuffer-albedo", width, height, ALBEDO_FORMAT);
        let (normal_texture, normal) =
            render_texture(device, "gbuffer-normal-flags", width, height, NORMAL_FORMAT);
        let (pbr_texture, pbr) =
            render_texture(device, "gbuffer-pbr-material", width, height, PBR_FORMAT);
        let (depth_texture, depth) = depth_texture(device, width, height);
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
            ],
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
            _tone_mapped: tone_texture,
            _aa_history_a: history_texture_a,
            _aa_history_b: history_texture_b,
            albedo,
            normal,
            pbr,
            depth,
            tone_mapped,
            aa_history_a,
            aa_history_b,
            reflection_bind_group,
            lighting_bind_group,
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

fn create_material(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    material: &Material,
) -> GpuMaterial {
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
            material.emission.luminance(),
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
            cull_mode: Some(wgpu::Face::Back),
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
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
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/lighting.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lighting-pipeline-layout"),
        bind_group_layouts: &[Some(lighting_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fullscreen-deferred-lighting-gt-tonemap"),
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
        label: Some("directional-shadow-map-2048"),
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

fn color_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: None,
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
