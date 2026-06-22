#![deny(unsafe_op_in_unsafe_fn)]

pub mod asset;
pub mod asset_pipeline;
pub mod backend;
pub mod demo;
pub mod game;
pub mod gltf_import;
pub mod gpu;
pub mod graph;
pub mod math;
pub mod memory;
pub mod perf;
pub mod profiler;
pub mod quality;
pub mod renderer;
pub mod scene;
pub mod world;

pub use asset::{
    validate_asset, validate_material, validate_mesh, AssetIssue, MaterialDesc, MeshDesc,
};
pub use asset_pipeline::{AssetId, AssetPipeline, ImportReport, ImportSeverity, ImportedAsset};
pub use backend::{BackendApi, BackendCapabilities};
pub use demo::{export_demo_gallery, DemoSummary};
pub use game::{
    build_better_than_unity_unreal_demo, EngineFrame, FrameInput, GameEngine, InputState,
    ProjectSettings, TargetPlatform,
};
pub use gltf_import::{import_gltf, GltfImportError};
pub use gpu::{run_live_renderer, LiveOptions};
pub use graph::{build_baseline_frame_graph, FeatureCategory, GraphIssue, RenderGraph};
pub use profiler::{required_debug_views, EngineProfiler, FrameBudget, FrameReport, GateFailure};
pub use quality::QualityTier;
pub use renderer::{CpuRenderer, DebugView, RenderConfig, RenderOutput, RenderPath};
pub use scene::{
    build_aaa_scene, build_asset_showcase, build_demo_scene, Camera, Light, Material, Mesh,
    RenderScene, TextureImage,
};
pub use world::{CameraRig, EntityId, MeshRenderer, RigidBody, Transform, World};
