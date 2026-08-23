# Dhool Pro Reference Manual

A reusable Rust game engine with deterministic simulation and vendor-agnostic rendering.

## Table of Contents

1. [Scene](#1-scene)
2. [Scene Builder](#2-scene-builder)
3. [Game Engine](#3-game-engine)
4. [Runtime](#4-runtime)
5. [World](#5-world)
6. [Renderer](#6-renderer)
7. [Cloth](#7-cloth)
8. [Profiler](#8-profiler)
9. [Asset Pipeline](#9-asset-pipeline)
10. [GPU Integration](#10-gpu-integration)
11. [glTF Import](#11-gltf-import)
12. [Math](#12-math)
13. [Performance Primitives](#13-performance-primitives)
14. [Backend](#14-backend)
15. [Quality](#15-quality)
16. [Demo](#16-demo)
17. [Memory](#17-memory)

---

## 1. Scene

Module: `dhool_pro::scene`

### `RenderScene`

The complete scene description passed to the renderer.

```rust
pub struct RenderScene {
    pub camera: Camera,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
    pub lights: Vec<Light>,
    pub ambient_color: Color,
    pub environment_map: EnvironmentMap,
    pub environment: EnvironmentProfile,
}
```

Methods:

```rust
impl RenderScene {
    pub fn asset_descriptors(&self) -> (Vec<MaterialDesc>, Vec<MeshDesc>);
}
```

### `Camera`

The scene camera with a perspective projection.

```rust
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub vertical_fov_radians: f32,
    pub near: f32,
    pub far: f32,
}
```

Methods:

```rust
impl Camera {
    pub fn view_projection(self, aspect: f32) -> Mat4;
}
```

`view_projection` computes `look_at_rh * perspective_reverse_rh`.

### `Material`

Surface appearance data.

```rust
pub struct Material {
    pub name: String,
    pub material_id: u8,
    pub class: MaterialClass,
    pub flags: MaterialFlags,
    pub base_color: Color,
    pub specular_intensity: f32,
    pub specular_color: Color,
    pub roughness: f32,
    pub metallic: f32,
    pub emissive_color: Color,
    pub ambient_occlusion: f32,
    pub texture_sample_count: u8,
}
```

Methods:

```rust
impl Material {
    pub fn standard(name: impl Into<String>, base_color: Color) -> Self;
    pub fn to_asset_desc(&self) -> MaterialDesc;
}
```

`Material::standard` creates a material with `MaterialClass::Standard`, roughness `0.6`, and specular intensity `0.5`.

### `MaterialClass`

```rust
pub enum MaterialClass {
    Standard,
    Glass,
}
```

### `MaterialFlags`

```rust
pub struct MaterialFlags {
    pub glass: bool,
    pub alpha_tested: bool,
    pub uses_bent_normals: bool,
}
```

### `Vertex`

Per-vertex data for meshes.

```rust
pub struct Vertex {
    pub position: Vec3,
    pub previous_position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub tangent_sign: f32,
    pub uv: Vec2,
    pub color: Color,
}
```

### `Mesh`

A renderable mesh with geometry data.

```rust
pub struct Mesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<[u32; 3]>,
    pub material_index: usize,
    pub transform: Mat4,
    pub shadow_class: ShadowCasterClass,
    pub is_floor_or_ground: bool,
    pub prepass_eligible: bool,
    pub reflection_proxy: bool,
    pub cloth: Option<ClothDefinition>,
}
```

Methods:

```rust
impl Mesh {
    pub fn cuboid(name: impl Into<String>, size: Vec3) -> Self;
    pub fn plane(name: impl Into<String>, width: f32, depth: f32) -> Self;
    pub fn quad(name: impl Into<String>, width: f32, height: f32) -> Self;
    pub fn sphere(name: impl Into<String>, radius: f32, sectors: u32, rings: u32) -> Self;
    pub fn to_asset_desc(&self) -> MeshDesc;
    pub fn bake_transform(&mut self);
}
```

`bake_transform` applies the mesh transform to each vertex and resets it to identity.

### `Light`

A scene light source.

```rust
pub struct Light {
    pub name: String,
    pub kind: LightKind,
    pub color: Color,
    pub intensity: f32,
    pub casts_shadow: bool,
}
```

Static constructors:

```rust
impl Light {
    pub fn directional(
        name: impl Into<String>,
        direction: Vec3,
        color: Color,
        intensity: f32,
    ) -> Self;
    pub fn local(
        name: impl Into<String>,
        position: Vec3,
        color: Color,
        intensity: f32,
        radius: f32,
    ) -> Self;
    pub fn spot(
        name: impl Into<String>,
        position: Vec3,
        direction: Vec3,
        color: Color,
        intensity: f32,
        radius: f32,
        inner_angle_radians: f32,
        outer_angle_radians: f32,
    ) -> Self;
}
```

### `LightKind`

```rust
pub enum LightKind {
    Directional { direction: Vec3 },
    Local { position: Vec3, radius: f32 },
    Spot {
        position: Vec3,
        direction: Vec3,
        radius: f32,
        inner_angle_radians: f32,
        outer_angle_radians: f32,
    },
}
```

### `EnvironmentMap`

Panorama-based sky and indirect lighting.

```rust
pub struct EnvironmentMap {
    pub panorama: Arc<TextureImage>,
    pub encoding: EnvironmentEncoding,
}
```

Methods:

```rust
impl EnvironmentMap {
    pub fn from_image(
        panorama: TextureImage,
        encoding: EnvironmentEncoding,
        exposure_ev: f32,
        yaw_radians: f32,
    ) -> Self;
    pub fn from_hdr_path(
        path: impl AsRef<Path>,
        exposure_ev: f32,
        yaw_radians: f32,
    ) -> Result<Self, String>;
    pub fn from_srgb_path(
        path: impl AsRef<Path>,
        exposure_ev: f32,
        yaw_radians: f32,
    ) -> Result<Self, String>;
    pub fn solid(color: Color) -> Self;
    pub fn with_linear_tint(self, tint: Vec3) -> Self;
    pub fn with_exposure_and_yaw(self, exposure_ev: f32, yaw_radians: f32) -> Self;
    pub fn sample_radiance(&self, direction: Vec3) -> Vec3;
    pub const fn exposure_ev(&self) -> f32;
    pub const fn yaw_radians(&self) -> f32;
    pub const fn irradiance_sh(&self) -> &[[f32; 3]; 9];
    pub fn linear_texel(&self, index: usize) -> Vec3;
    pub fn diffuse_irradiance(&self, normal: Vec3) -> Vec3;
}
```

`solid` creates a constant-color environment map.
`from_hdr_path` loads a linear HDR panorama.
`from_srgb_path` loads an sRGB panorama.

### `EnvironmentEncoding`

```rust
pub enum EnvironmentEncoding {
    LinearHdr,
    Srgb,
}
```

### `TextureImage`

```rust
pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Color>,
}
```

Methods:

```rust
impl TextureImage {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String>;
    pub fn from_encoded_bytes(bytes: &[u8]) -> Result<Self, String>;
    pub fn from_encoded_bytes_with_max_dimension(
        bytes: &[u8],
        max_dimension: u32,
    ) -> Result<Self, String>;
}
```

### `EnvironmentProfile`

Weather and exposure settings.

```rust
pub struct EnvironmentProfile {
    pub precipitation: f32,
    pub surface_wetness: f32,
    pub wind: f32,
    pub exposure: ExposureProfile,
}
```

Constructors:

```rust
impl EnvironmentProfile {
    pub const fn rainy() -> Self;
    pub const fn dry_daylight() -> Self;
}
```

### `ExposureProfile`

```rust
pub struct ExposureProfile {
    pub min: f32,
    pub max: f32,
    pub compensation: f32,
    pub white_balance_strength: f32,
}
```

---

## 2. Scene Builder

Module: `dhool_pro::scene_builder`

### `SceneBuilder`

Fluent builder for constructing a `RenderScene`.

```rust
pub struct SceneBuilder { /* private */ }
```

Methods:

```rust
impl SceneBuilder {
    pub fn new() -> Self;
    pub fn camera(mut self, camera: Camera) -> Self;
    pub fn environment(
        mut self,
        map: EnvironmentMap,
        profile: EnvironmentProfile,
        ambient_color: Color,
    ) -> Self;
    pub fn add_material(&mut self, material: Material) -> MaterialId;
    pub fn add_mesh(&mut self, mesh: Mesh, material: MaterialId);
    pub fn add_light(&mut self, light: Light);
    pub fn build(self) -> Result<RenderScene, SceneBuildError>;
}
```

Validation during `build`:
- Camera near clip must be positive, far must exceed near
- Material names must be non-empty
- Mesh names must be non-empty
- Each mesh must have vertices and indices
- Each mesh material index must be valid
- Maximum 256 materials

### `MaterialId`

Opaque handle returned by `add_material`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialId(usize);
```

Methods:

```rust
impl MaterialId {
    pub fn index(&self) -> usize;
}
```

### `SceneBuildError`

```rust
pub enum SceneBuildError {
    InvalidCameraClipPlanes,
    TooManyMaterials { count: usize },
    EmptyMaterialName { index: usize },
    EmptyMeshName { mesh: String },
    EmptyMesh { mesh: String },
    MissingMaterial { mesh: String, material: usize },
}
```

---

## 3. Game Engine

Module: `dhool_pro::game`

### `GameEngine`

The main game runtime that owns the world, systems, and renderer.

```rust
pub struct GameEngine {
    pub settings: ProjectSettings,
    /* private fields */
}
```

Methods:

```rust
impl GameEngine {
    pub fn builder(settings: ProjectSettings) -> GameEngineBuilder;
    pub fn frame(&mut self, input: FrameInput) -> Result<EngineFrame, EngineError>;
    pub fn resources(&self) -> &Resources;
    pub fn resources_mut(&mut self) -> &mut Resources;
    pub fn events(&self) -> &Events;
    pub fn events_mut(&mut self) -> &mut Events;
}
```

`frame` advances one frame: processes input, runs startup (once), fixed systems, update systems, and renders.

### `EngineFrame`

```rust
pub struct EngineFrame {
    pub world: World,
    pub output: RenderOutput,
    pub events: Events,
}
```

### `EngineUpdate`

```rust
pub struct EngineUpdate {
    pub world: World,
    pub events: Events,
}
```

### `GameEngineBuilder`

Fluent builder for `GameEngine`.

```rust
pub struct GameEngineBuilder { /* private */ }
```

Methods:

```rust
impl GameEngineBuilder {
    pub fn new(settings: ProjectSettings) -> Self;
    pub fn scene(mut self, scene: RenderScene) -> Self;
    pub fn render_config(mut self, config: RenderConfig) -> Self;
    pub fn resource<T: Send + Sync + 'static>(mut self, resource: T) -> Self;
    pub fn register_resource<T: Send + Sync + 'static>(&mut self, resource: T);
    pub fn plugin(mut self, plugin: impl EnginePlugin) -> Self;
    pub fn startup_system(
        mut self,
        name: impl Into<String>,
        system: impl GameSystem + 'static,
    ) -> Self;
    pub fn fixed_system(
        mut self,
        name: impl Into<String>,
        system: impl GameSystem + 'static,
    ) -> Self;
    pub fn update_system(
        mut self,
        name: impl Into<String>,
        system: impl GameSystem + 'static,
    ) -> Self;
    pub fn register_startup_system(
        &mut self,
        name: impl Into<String>,
        system: impl GameSystem + 'static,
    );
    pub fn register_fixed_system(
        &mut self,
        name: impl Into<String>,
        system: impl GameSystem + 'static,
    );
    pub fn register_update_system(
        &mut self,
        name: impl Into<String>,
        system: impl GameSystem + 'static,
    );
    pub fn build(self) -> Result<GameEngine, EngineBuildError>;
}
```

Call order:
1. `new` with project settings
2. `scene` with a built `RenderScene`
3. `render_config` (optional, defaults to headless 1080p)
4. `resource` for each typed resource
5. `plugin` for each plugin
6. System registrations in any order
7. `build` validates and creates the engine

### `EnginePlugin`

```rust
pub trait EnginePlugin {
    fn build(&self, builder: &mut GameEngineBuilder);
}
```

### `FlyCameraPlugin`

A built-in plugin for keyboard and mouse camera control.

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyCameraPlugin {
    pub move_speed: f32,
    pub boost_multiplier: f32,
    pub mouse_sensitivity: f32,
}
```

Default values: `move_speed = 5.0`, `boost_multiplier = 3.0`, `mouse_sensitivity = 0.003`.

### `FrameInput`

Per-frame input data from the window.

```rust
pub struct FrameInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub boost: bool,
    pub mouse_delta: (f32, f32),
    pub cursor_position: (f32, f32),
    pub left_click: bool,
    pub right_click: bool,
    pub middle_click: bool,
    pub escape: bool,
    pub space: bool,
}
```

### `InputState`

Per-frame input state. Same fields as `FrameInput`.

### `ProjectSettings`

```rust
pub struct ProjectSettings {
    pub name: String,
    pub native_width: u32,
    pub native_height: u32,
    pub quality: QualityTier,
    pub fixed_timestep_seconds: f32,
    pub max_frame_time_seconds: f32,
    pub target_platform: TargetPlatform,
}
```

Default: `name = "dhool-pro"`, `native_width = 1920`, `native_height = 1080`, `quality = QualityTier::High`, `fixed_timestep_seconds = 1.0/60.0`, `max_frame_time_seconds = 0.1`, `target_platform = TargetPlatform::Native`.

### `TargetPlatform`

```rust
pub enum TargetPlatform {
    Native,
    Web,
}
```

### `EngineBuildError`

```rust
pub enum EngineBuildError {
    InvalidScene { messages: Vec<String> },
    MissingScene,
    InvalidResolution { width: u32, height: u32 },
    InvalidFixedTimestep { timestep: f32 },
    System(SystemFailure),
}
```

### `EngineError`

```rust
pub enum EngineError {
    Render(RenderError),
    Cloth(ClothError),
    System(SystemFailure),
}
```

---

## 4. Runtime

Module: `dhool_pro::runtime`

### `GameSystem`

```rust
pub trait GameSystem: Send {
    fn run(&mut self, context: &mut SystemContext<'_>) -> SystemResult;
}
```

Implemented for closures: `FnMut(&mut SystemContext) -> SystemResult + Send`.

### `SystemResult`

```rust
pub type SystemResult = Result<(), String>;
```

### `SystemContext`

```rust
pub struct SystemContext<'a> {
    pub world: &'a mut World,
    pub resources: &'a mut Resources,
    pub events: &'a mut Events,
    pub input: InputState,
    pub time: Time,
    pub schedule: Schedule,
}
```

### `Schedule`

```rust
pub enum Schedule {
    Startup,
    Fixed,
    Update,
}
```

### `Resources`

Type-indexed map of shared game state.

```rust
pub struct Resources { /* private */ }
```

Methods:

```rust
impl Resources {
    pub fn new() -> Self;
    pub fn insert<T: Send + Sync + 'static>(&mut self, resource: T);
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T>;
    pub fn get_mut<T: Send + Sync + 'static>(&mut self) -> Option<&mut T>;
}
```

### `Events`

Type-indexed event queues.

```rust
pub struct Events { /* private */ }
```

Methods:

```rust
impl Events {
    pub fn new() -> Self;
    pub fn send<T: Send + 'static>(&mut self, event: T);
    pub fn drain<T: Send + 'static>(&mut self) -> Vec<T>;
    pub fn has_events<T: Send + 'static>(&self) -> bool;
    pub fn clear(&mut self);
}
```

### `Time`

```rust
pub struct Time {
    pub delta_seconds: f32,
    pub total_seconds: f32,
    pub total_frames: u64,
    pub fixed_delta_seconds: f32,
    pub frame_index: u64,
}
```

### `SystemFailure`

```rust
pub struct SystemFailure {
    pub system: Option<String>,
    pub message: String,
}
```

---

## 5. World

Module: `dhool_pro::world`

### `World`

ECS-style world with component storage.

```rust
pub struct World {
    pub materials: Vec<Material>,
    pub ambient_color: Color,
    pub environment_map: EnvironmentMap,
    pub environment: EnvironmentProfile,
    pub physics: PhysicsSettings,
    /* private fields */
}
```

Methods:

```rust
impl World {
    // Entity lifecycle
    pub fn spawn(&mut self, name: impl Into<String>, transform: Transform) -> EntityId;
    pub fn destroy(&mut self, id: EntityId) -> bool;
    pub fn is_alive(&self, id: EntityId) -> bool;
    pub fn entities(&self) -> impl Iterator<Item = EntityId>;

    // Transforms
    pub fn transform(&self, id: EntityId) -> Option<&Transform>;
    pub fn transform_mut(&mut self, id: EntityId) -> Option<&mut Transform>;

    // Names
    pub fn name(&self, id: EntityId) -> Option<&str>;
    pub fn find_by_name(&self, name: &str) -> Option<EntityId>;

    // Components
    pub fn set_mesh_renderer(&mut self, id: EntityId, renderer: MeshRenderer) -> bool;
    pub fn set_camera(&mut self, id: EntityId, camera: CameraRig) -> bool;
    pub fn set_light(&mut self, id: EntityId, light: LightComponent) -> bool;
    pub fn set_rigid_body(&mut self, id: EntityId, body: RigidBody) -> bool;
    pub fn set_collider(&mut self, id: EntityId, collider: Collider) -> bool;
    pub fn set_cloth(
        &mut self,
        id: EntityId,
        definition: &ClothDefinition,
    ) -> Result<bool, ClothError>;
    pub fn set_script(&mut self, id: EntityId, script: ScriptState) -> bool;

    // Component accessors
    pub fn camera(&self, id: EntityId) -> Option<&CameraRig>;
    pub fn camera_mut(&mut self, id: EntityId) -> Option<&mut CameraRig>;
    pub fn active_camera(&self) -> Option<(EntityId, &CameraRig)>;
    pub fn active_camera_mut(&mut self) -> Option<(EntityId, &mut CameraRig)>;
    pub fn set_active_camera(&mut self, id: EntityId) -> bool;
    pub fn rigid_body(&self, id: EntityId) -> Option<&RigidBody>;
    pub fn rigid_body_mut(&mut self, id: EntityId) -> Option<&mut RigidBody>;
    pub fn collider(&self, id: EntityId) -> Option<&Collider>;
    pub fn mesh_renderer(&self, id: EntityId) -> Option<&MeshRenderer>;
    pub fn script(&self, id: EntityId) -> Option<&ScriptState>;
    pub fn script_mut(&mut self, id: EntityId) -> Option<&mut ScriptState>;

    // Simulation
    pub fn tick(&self) -> u64;
    pub fn fixed_update(&mut self, dt: f32) -> Result<(), ClothError>;
    pub fn entity_count(&self) -> usize;

    // Scene conversion
    pub fn validate(&self) -> Vec<WorldIssue>;
    pub fn build_render_scene(&self) -> RenderScene;
    pub fn into_render_scene(self) -> RenderScene;
    pub fn from_render_scene(scene: RenderScene) -> Self;
}
```

### `EntityId`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId { /* private fields */ }
```

### `Transform`

```rust
pub struct Transform {
    pub position: Vec3,
    pub previous_position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
```

Methods:

```rust
impl Transform {
    pub const IDENTITY: Self;
    pub fn from_position(position: Vec3) -> Self;
    pub fn matrix(&self) -> Mat4;
}
```

### `MeshRenderer`

```rust
pub struct MeshRenderer {
    pub mesh: Mesh,
    pub visible: bool,
    pub prepass_eligible: bool,
    pub reflection_proxy: bool,
}
```

### `CameraRig`

```rust
pub struct CameraRig {
    pub camera: Camera,
    pub active: bool,
}
```

### `LightComponent`

```rust
pub struct LightComponent {
    pub kind: LightKind,
    pub color: Color,
    pub intensity: f32,
    pub casts_shadow: bool,
}
```

### `RigidBody`

```rust
pub struct RigidBody {
    pub dynamic: bool,
    pub velocity: Vec3,
    pub gravity_scale: f32,
}
```

### `PhysicsSettings`

```rust
pub struct PhysicsSettings {
    pub gravity: Vec3,
    pub floor_height: Option<f32>,
}
```

Default: `gravity = (0.0, -9.81, 0.0)`, `floor_height = None`.

### `Collider`

```rust
pub enum Collider {
    Sphere { radius: f32 },
    Aabb { half_extents: Vec3 },
}
```

### `ScriptState`

```rust
pub struct ScriptState {
    pub enabled: bool,
    pub ticks: u64,
}
```

### `Name`

```rust
pub struct Name(pub String);
```

### `WorldIssue`

Validation issues returned by `World::validate`.

```rust
pub enum WorldIssue {
    MissingActiveCamera,
    MeshMaterialOutOfRange {
        entity: EntityId,
        mesh: String,
        material_index: usize,
    },
    FloorCastsShadow {
        entity: EntityId,
        mesh: String,
    },
    ReflectiveMeshMissingProxy {
        entity: EntityId,
        mesh: String,
    },
}
```

---

## 6. Renderer

Module: `dhool_pro::renderer`

### `CpuRenderer`

Deterministic CPU rasterizer.

```rust
pub struct CpuRenderer { /* private */ }
```

Methods:

```rust
impl CpuRenderer {
    pub fn new(config: RenderConfig) -> Self;
    pub fn render(
        &self,
        scene: &RenderScene,
        debug_view: Option<DebugView>,
    ) -> RenderOutput;
}
```

### `RenderConfig`

```rust
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub quality: QualityTier,
    pub path: RenderPath,
}
```

Methods:

```rust
impl RenderConfig {
    pub const fn headless_1080p() -> Self;
}
```

### `RenderPath`

```rust
pub enum RenderPath {
    Forward,
    Deferred,
}
```

### `DebugView`

```rust
pub enum DebugView {
    Beauty,
    Albedo,
    Normals,
    Depth,
    Specular,
    Roughness,
    AmbientOcclusion,
    MotionVectors,
    ShadowMap,
    BasePass,
    MaterialIds,
    LightAccum,
    Reflections,
    PostFx,
    SmaaEdges,
    SmaaBlendWeights,
}
```

`DebugView::ALL` provides a list of all variants.

### `RenderOutput`

```rust
pub struct RenderOutput {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
    pub pass_reports: Vec<PassReport>,
    pub warnings: Vec<RenderWarning>,
    pub graph_passes: usize,
    pub graph_resources: usize,
}
```

Methods:

```rust
impl RenderOutput {
    pub fn save_ppm(&self, path: impl AsRef<Path>) -> Result<(), String>;
    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), String>;
}
```

### `PassReport`

```rust
pub struct PassReport {
    pub name: String,
    pub cpu_ms: f32,
    pub budget_ms: f32,
    pub gpu_ms: Option<f32>,
}
```

### `RenderWarning`

```rust
pub enum RenderWarning {
    SsrCostExceededSoftwareRt { ssr_ms: f32, software_rt_ms: f32 },
    EmptySmaaEdges,
    Graph(GraphIssue),
    Asset(AssetIssue),
}
```

### `RenderError`

```rust
pub enum RenderError {
    ResolutionTooSmall { width: u32, height: u32 },
    TooManyLights { count: usize, limit: usize },
    EmptyScene,
}
```

---

## 7. Cloth

Module: `dhool_pro::cloth`

### `ClothState`

Simulation state for a cloth mesh.

```rust
pub struct ClothState { /* private */ }
```

Methods:

```rust
impl ClothState {
    pub fn new(
        vertices: &[Vertex],
        triangle_indices: &[[u32; 3]],
        definition: &ClothDefinition,
    ) -> Result<Self, ClothError>;
    pub fn step(
        &mut self,
        dt: f32,
        colliders: &[ClothColliderSnapshot],
        vertices: &mut [Vertex],
    ) -> Result<(), ClothError>;
}
```

`step` runs one Verlet integration tick:
1. Applies gravity, wind, and damping
2. Resolves structural and bending distance constraints
3. Enforces pinned vertex anchors
4. Resolves collisions against all colliders
5. Recomputes vertex normals, tangents, and bitangent signs
6. Writes positions and normals back to the vertex buffer

### `ClothDefinition`

```rust
pub struct ClothDefinition {
    pub settings: ClothSettings,
    pub pins: Vec<ClothPin>,
}
```

### `ClothSettings`

```rust
pub struct ClothSettings {
    pub gravity: Vec3,
    pub wind: Vec3,
    pub damping: f32,
    pub constraint_iterations: u32,
    pub structural_stiffness: f32,
    pub bending_stiffness: f32,
    pub particle_radius: f32,
    pub collision_friction: f32,
}
```

Default values:
- `gravity`: `(0.0, -9.81, 0.0)`
- `wind`: `(0.0, 0.0, 0.0)`
- `damping`: `0.995`
- `constraint_iterations`: `8`
- `structural_stiffness`: `1.0`
- `bending_stiffness`: `0.5`
- `particle_radius`: `0.005`
- `collision_friction`: `0.2`

### `ClothPin`

```rust
pub struct ClothPin {
    pub vertex: usize,
    pub position: Option<Vec3>,
}
```

Constructors:

```rust
impl ClothPin {
    pub const fn at_initial(vertex: usize) -> Self;
    pub const fn at(vertex: usize, position: Vec3) -> Self;
}
```

`at_initial` pins the vertex at its mesh position.
`at` pins the vertex at a specific world position.

### `ClothColliderSnapshot`

A snapshot of a collider shape at a single time step.

```rust
pub enum ClothColliderSnapshot {
    Floor { height: f32 },
    Sphere { center: Vec3, radius: f32 },
    Aabb { min: Vec3, max: Vec3 },
}
```

Constructors:

```rust
impl ClothColliderSnapshot {
    pub const fn floor(height: f32) -> Self;
    pub const fn sphere(center: Vec3, radius: f32) -> Self;
    pub const fn aabb(min: Vec3, max: Vec3) -> Self;
}
```

### `ClothError`

```rust
pub enum ClothError {
    EmptyVertexBuffer,
    NonFiniteVertex(usize),
    InvalidTriangleIndex(usize, usize, u32, usize),
    InvalidPin(usize, usize),
    NonFinitePin(usize),
    InvalidSettings(&'static str, f32),
    VertexBufferLength(usize, usize),
    InvalidDeltaTime(f32),
    InvalidCollider(usize),
}
```

---

## 8. Profiler

Module: `dhool_pro::profiler`

### `EngineProfiler`

Per-frame performance analysis.

```rust
pub struct EngineProfiler {
    pub budget: FrameBudget,
    pub enforce_timing: bool,
    /* private fields */
}
```

Methods:

```rust
impl EngineProfiler {
    pub fn with_budget(budget: FrameBudget) -> Self;
    pub fn mark_debug_view_rendered(&mut self, view: DebugView);
    pub fn clear_debug_coverage(&mut self);
    pub fn evaluate(&self, output: &RenderOutput) -> FrameReport;
    pub fn evaluate_debug_coverage(&self) -> Vec<GateFailure>;
}
```

### `FrameBudget`

Per-pass timing budgets in milliseconds.

```rust
pub struct FrameBudget {
    pub total_ms: f32,
    pub visibility_ms: f32,
    pub prepass_ms: f32,
    pub geometry_ms: f32,
    pub shadows_ms: f32,
    pub reflections_ms: f32,
    pub post_ms: f32,
    pub aa_ms: f32,
}
```

Default: `total_ms = 33.33` (30 FPS), all sub-budgets default to `0.0`.

### `FrameReport`

```rust
pub struct FrameReport {
    pub total_cpu_ms: f32,
    pub pass_count: usize,
    pub graph_passes: usize,
    pub graph_resources: usize,
    pub failures: Vec<GateFailure>,
}
```

### `GateFailure`

```rust
pub enum GateFailure {
    TotalFrameBudgetExceeded { actual_ms: f32, budget_ms: f32 },
    PassBudgetExceeded { pass: String, actual_ms: f32, budget_ms: f32 },
    SsrSlowerThanSoftwareRt { ssr_ms: f32, software_rt_ms: f32 },
    MissingRequiredDebugView { name: &'static str },
    EmptySmaaEdges,
    SuspiciousGraphClear { pass: String, resource: String },
    RequiredVendorFeature { pass: String },
    RequiredTemporalSmoothing { pass: String },
    AssetImportFailure { message: String },
}
```

### `DebugViewRequirement`

```rust
pub struct DebugViewRequirement {
    pub name: &'static str,
    pub implemented_view: Option<DebugView>,
}
```

### `required_debug_views`

```rust
pub fn required_debug_views() -> Vec<DebugViewRequirement>;
```

---

## 9. Asset Pipeline

Module: `dhool_pro::asset_pipeline`

### `AssetPipeline`

```rust
pub struct AssetPipeline { /* private */ }
```

Methods:

```rust
impl AssetPipeline {
    pub fn build_from_scene(
        scene: &RenderScene,
        quality: QualityTier,
    ) -> (Self, ImportReport);
    pub fn build_from_descriptors(
        materials: &[MaterialDesc],
        meshes: &[MeshDesc],
        quality: QualityTier,
    ) -> (Self, ImportReport);
    pub fn lookup_mesh(&self, id: &AssetId) -> Option<(&MeshDesc, &[u8])>;
}
```

Module: `dhool_pro::asset`

### `MaterialDesc`

```rust
pub struct MaterialDesc {
    pub name: String,
    pub textures: Vec<TextureDesc>,
    pub texture_sample_count: u8,
    pub runtime_texture_blending: bool,
    pub requires_specular_occlusion: bool,
    pub has_bent_normals: bool,
    pub alpha_tested: bool,
}
```

### `MeshDesc`

```rust
pub struct MeshDesc {
    pub name: String,
    pub material_index: usize,
    pub is_floor_or_ground: bool,
    pub shadow_class: ShadowCasterClass,
    pub participates_in_prepass: bool,
    pub writes_motion_vectors_in_prepass: bool,
    pub long_thin_triangles: bool,
    pub dense_alpha_cards: bool,
}
```

### `ShadowCasterClass`

```rust
pub enum ShadowCasterClass {
    None,
    StaticDirectional,
    StaticLocal,
    Dynamic,
}
```

### `TextureDesc`

```rust
pub struct TextureDesc {
    pub name: String,
    pub format: TextureFormat,
    pub role: TextureRole,
}
```

### `TextureFormat`

```rust
pub enum TextureFormat {
    Bc1, Bc3, Bc4, Bc5, Bc7,
    Rgba8, Rgba16Float, Other,
}
```

### `TextureRole`

```rust
pub enum TextureRole {
    NormalsAndBentNormals,
    BaseColorAndAo,
    SpecularRoughnessHeight,
    Opacity,
}
```

### `AssetIssue`

```rust
pub enum AssetIssue {
    MissingMaterial { mesh_name: String },
    MissingTexture { material_name: String, role: TextureRole },
    TextureTooSmall { texture_name: String, width: u32, height: u32 },
    InvalidMaterialParameter { material_name: String, field: String, value: f32 },
    AlphaCardOutOfBounds { mesh_name: String },
    ShadowCasterGeometryMissing,
}
```

### Validation Functions

```rust
pub fn validate_asset(
    materials: &[MaterialDesc],
    meshes: &[MeshDesc],
    tier: QualityTier,
) -> Vec<AssetIssue>;
pub fn validate_material(material: &MaterialDesc, tier: QualityTier) -> Vec<AssetIssue>;
pub fn validate_mesh(mesh: &MeshDesc, material: &MaterialDesc) -> Vec<AssetIssue>;
```

### `ImportReport`

```rust
pub struct ImportReport { /* private */ }
```

Methods:

```rust
impl ImportReport {
    pub fn has_errors(&self) -> bool;
    pub fn errors(&self) -> impl Iterator<Item = &ImportedAsset>;
    pub fn warnings(&self) -> impl Iterator<Item = &ImportedAsset>;
}
```

### `ImportSeverity`

```rust
pub enum ImportSeverity {
    Error,
    Warning,
}
```

### `ImportedAsset`

```rust
pub struct ImportedAsset {
    pub id: AssetId,
    pub name: String,
    pub severity: ImportSeverity,
    pub text: String,
}
```

### `AssetId`

```rust
pub struct AssetId { /* private */ }
```

---

## 10. GPU Integration

Module: `dhool_pro::gpu`

### `run_live_game`

Run a game engine with the live GPU renderer.

```rust
pub fn run_live_game(
    game: GameEngine,
    options: LiveOptions,
) -> Result<(), LiveError>;
```

### `run_live_renderer`

Run a static scene with the live GPU renderer and a free camera.

```rust
pub fn run_live_renderer(
    scene: RenderScene,
    options: LiveOptions,
) -> Result<(), LiveError>;
```

### `LiveOptions`

```rust
pub struct LiveOptions {
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub gpu_name_filter: Option<String>,
    pub static_camera: bool,
    pub max_frames: Option<u64>,
    pub capture_path: Option<PathBuf>,
}
```

### `LiveError`

```rust
pub enum LiveError {
    EngineBuild(String),
    NoSuitableAdapter,
    WindowCreation(String),
    RenderLoop(String),
}
```

---

## 11. glTF Import

Module: `dhool_pro::gltf_import`

### `import_gltf`

Import a glTF 2.0 file into engine mesh data.

```rust
pub fn import_gltf(
    path: impl AsRef<Path>,
    transform: Mat4,
) -> Result<ImportReport, GltfImportError>;
```

### `GltfImportError`

```rust
pub enum GltfImportError {
    Io(std::io::Error),
    Gltf(String),
    MissingBuffers,
    UnsupportedPrimitive,
}
```

---

## 12. Math

Module: `dhool_pro::math`

Dhool re-exports `glam` types and adds its own `Color` type.

```rust
pub use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
```

### `Color`

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
```

Methods:

```rust
impl Color {
    pub const BLACK: Self;
    pub const WHITE: Self;
    pub const ZERO: Self;
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self;
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self;
    pub fn to_vec3(&self) -> Vec3;
    pub fn to_array(&self) -> [f32; 4];
}
```

---

## 13. Performance Primitives

Module: `dhool_pro::perf`

Gospol C helpers for GPU-like compute on the CPU.

```rust
pub fn reduce_hiz_rgba16(
    depth: &[f32],
    width: u32,
    height: u32,
) -> Result<Vec<[u16; 4]>, PerfError>;

pub fn edge_stencil_4x4(
    depth: &[f32],
    width: u32,
    height: u32,
    threshold: f32,
) -> Result<Vec<u8>, PerfError>;

pub fn blue_noise_hash(x: u32, y: u32, frame: u32, seed: u32) -> u32;

pub fn gt_tonemap_rgba8(
    hdr_rgba: &[[f32; 4]],
    exposure: f32,
) -> Result<Vec<[u8; 4]>, PerfError>;

pub fn kawase_blur_rgba32f(
    src_rgba: &[[f32; 4]],
    width: u32,
    height: u32,
    radius: u32,
) -> Result<Vec<[f32; 4]>, PerfError>;
```

### `PerfError`

```rust
pub enum PerfError {
    SizeOverflow,
    SizeMismatch { expected: usize, actual: usize },
    OutputRejected,
}
```

---

## 14. Backend

Module: `dhool_pro::backend`

### `BackendApi`

```rust
pub struct BackendApi { /* private */ }
```

### `BackendCapabilities`

```rust
pub struct BackendCapabilities {
    pub supports_timestamp_queries: bool,
    pub max_texture_dimension_2d: u32,
    pub max_bind_groups: u32,
    pub max_compute_workgroup_size_x: u32,
}
```

---

## 15. Quality

Module: `dhool_pro::quality`

### `QualityTier`

```rust
pub enum QualityTier {
    Low,
    Medium,
    High,
    Ultra,
}
```

---

## 16. Demo

Module: `dhool_pro::demo`

### `export_demo_gallery`

Render a gallery of debug views for a given map.

```rust
#[cfg(feature = "demo-content")]
pub fn export_demo_gallery(
    asset_path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    width: u32,
    height: u32,
    map: Option<&str>,
) -> Result<DemoSummary, String>;
```

Available map names: `"aaa"` (monsoon city), `"dust2"`, `"showcase"`, or `None` for the default scene.

### `DemoSummary`

```rust
pub struct DemoSummary {
    pub output_dir: PathBuf,
    pub images: usize,
    pub meshes: usize,
    pub triangles: usize,
    pub materials: usize,
}
```

---

## 17. Memory

Module: `dhool_pro::memory`

Memory pool infrastructure for back-end buffer management. Types and methods vary by version; see the source in `src/memory.rs` for current pool implementations.

---

## Index of Error Types

| Type | Module | Description |
|---|---|---|
| `SceneBuildError` | `scene_builder` | Scene validation failures |
| `EngineBuildError` | `game` | Engine configuration failures |
| `EngineError` | `game` | Runtime engine errors |
| `SystemFailure` | `runtime` | Named system execution failure |
| `RenderError` | `renderer` | Renderer preconditions |
| `RenderWarning` | `renderer` | Renderer diagnostics |
| `ClothError` | `cloth` | Cloth simulation errors |
| `PerfError` | `perf` | Compute primitive errors |
| `LiveError` | `gpu` | GPU runtime initialization |
| `GltfImportError` | `gltf_import` | Model file import |
| `WorldIssue` | `world` | World validation diagnostics |
| `AssetIssue` | `asset` | Asset validation diagnostics |
| `GateFailure` | `profiler` | Performance budget violations |
| `ImportSeverity` | `asset_pipeline` | Asset import severity |

## Index of Trait Types

| Trait | Module | Description |
|---|---|---|
| `GameSystem` | `runtime` | System callable trait |
| `EnginePlugin` | `game` | Plugin interface |