# Dhool Pro Documentation — Table of Contents

## Overview

- [Documentation Home](index.md)
- [Table of Contents](SUMMARY.md)
- [Reference Manual](MANUAL.md)

## How-To Guides

- [How to Create a Custom Scene](08-how-to-guides/how-to-create-a-custom-scene.md)
  - Create a SceneBuilder
  - Define materials
  - Create meshes
  - Add lights
  - Set the camera
  - Configure the environment
  - Build the scene
  - Use the scene
- [How to Implement Game Systems](08-how-to-guides/how-to-implement-game-systems.md)
  - Understand system schedules
  - Write a system function
  - Register a system
  - Implement a movement system
  - Handle input
  - Use the FlyCameraPlugin
  - Dispatch and handle events
  - Register custom resources
  - Combine systems into a plugin
  - Validate and test
- [How to Add Cloth Simulation](08-how-to-guides/how-to-add-cloth-simulation.md)
  - Understand cloth types
  - Configure cloth settings
  - Define pinned vertices
  - Create the cloth definition
  - Attach cloth to a mesh entity
  - Add colliders
  - Run the fixed step
  - Build a render scene with cloth
  - Cloth error handling
- [How to Profile and Benchmark](08-how-to-guides/how-to-profile-and-benchmark.md)
  - Export the diagnostic gallery
  - Open the gallery index
  - Read the timing report
  - Interpret pass reports
  - Run with frame limit
  - Set performance budgets
  - Check debug view coverage
  - Render specific debug views
  - Frame report fields
  - Gate failures reference

## Shaders

- [Shaders](06-shaders/) — WGSL source files for the GPU render path

## API Reference

- Scene (`dhool_pro::scene`)
  - `RenderScene`, `Camera`, `Material`, `Mesh`, `Vertex`
  - `Light`, `LightKind`
  - `EnvironmentMap`, `EnvironmentProfile`, `ExposureProfile`
  - `EnvironmentEncoding`, `MaterialClass`, `MaterialFlags`
  - `TextureImage`
- Scene Builder (`dhool_pro::scene_builder`)
  - `SceneBuilder`, `MaterialId`, `SceneBuildError`
- Game Engine (`dhool_pro::game`)
  - `GameEngine`, `GameEngineBuilder`
  - `EnginePlugin`, `EngineFrame`, `EngineUpdate`
  - `FrameInput`, `InputState`
  - `ProjectSettings`, `TargetPlatform`
  - `FlyCameraPlugin`
  - `EngineBuildError`, `EngineError`
- Runtime (`dhool_pro::runtime`)
  - `GameSystem`, `SystemContext`, `SystemResult`
  - `SystemFailure`, `Schedule`
  - `Resources`, `Events`, `Time`
- World (`dhool_pro::world`)
  - `World`, `EntityId`, `Transform`
  - `MeshRenderer`, `CameraRig`, `LightComponent`
  - `RigidBody`, `Collider`, `PhysicsSettings`
  - `ScriptState`, `Name`, `WorldIssue`
- Renderer (`dhool_pro::renderer`)
  - `CpuRenderer`, `RenderConfig`, `RenderOutput`
  - `RenderPath`, `DebugView`, `PassReport`
  - `RenderWarning`, `RenderError`
- Cloth (`dhool_pro::cloth`)
  - `ClothState`, `ClothDefinition`, `ClothSettings`
  - `ClothPin`, `ClothColliderSnapshot`, `ClothError`
- Profiler (`dhool_pro::profiler`)
  - `EngineProfiler`, `FrameBudget`, `FrameReport`
  - `GateFailure`, `DebugViewRequirement`
  - `required_debug_views`
- Asset (`dhool_pro::asset`)
  - `MaterialDesc`, `MeshDesc`, `ShadowCasterClass`
  - `TextureDesc`, `TextureFormat`, `TextureRole`
  - `AssetIssue`, `validate_asset`, `validate_material`, `validate_mesh`
- Asset Pipeline (`dhool_pro::asset_pipeline`)
  - `AssetPipeline`, `AssetId`, `ImportReport`
  - `ImportSeverity`, `ImportedAsset`
- GPU (`dhool_pro::gpu`)
  - `run_live_game`, `run_live_renderer`
  - `LiveOptions`, `LiveError`
- glTF Import (`dhool_pro::gltf_import`)
  - `import_gltf`, `GltfImportError`
- Backend (`dhool_pro::backend`)
  - `BackendApi`, `BackendCapabilities`
- Quality (`dhool_pro::quality`)
  - `QualityTier`
- Demo (`dhool_pro::demo`)
  - `export_demo_gallery`, `DemoSummary`
- Render Graph (`dhool_pro::graph`)
  - `RenderGraph`, `GraphIssue`, `FeatureCategory`
  - `build_baseline_frame_graph`
- Math (`dhool_pro::math`)
  - `Vec2`, `Vec3`, `Vec4`, `Mat4`, `Color`, `Quat`
- Performance (`dhool_pro::perf`)
  - `reduce_hiz_rgba16`, `edge_stencil_4x4`
  - `blue_noise_hash`, `gt_tonemap_rgba8`
  - `kawase_blur_rgba32f`