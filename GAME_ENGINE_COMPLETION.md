# Reusable game engine completion record

The exit condition is a consumer crate that can build its own scene, register game
systems, keep game state in typed resources, advance deterministic fixed updates,
render frames, and opt into the live GPU window without depending on Dhool's bundled
demo content.

| Result | Source paths | Runtime paths | Check | State | Evidence |
|---|---|---|---|---|---|
| Public application contract | `src/game.rs`, `src/runtime.rs` | Consumer-owned game loop | External-consumer integration test | proved | Ordered startup/fixed/update systems, typed resources and events, named failures, and deterministic time passed in `tests/reusable_engine.rs` |
| Consumer-owned scene creation | `src/scene_builder.rs` | CPU and GPU renderers | Scene builder and rendered-frame tests | proved | Custom materials, cuboid, ground plane, lights, rotation, and asset validation passed without demo content |
| Bundled content is optional | `Cargo.toml`, `src/scene.rs`, `src/lib.rs` | Cargo dependency build | Default-feature build and package verification | proved | `cargo package --allow-dirty` verified 56 files at 883.7 KiB; showcase assets were excluded |
| Generic renderer entry points | `src/gpu.rs`, `src/main.rs` | Headless and live window | Headless example and live game smoke | proved | `custom_game` rendered three CPU frames; `live_custom_scene` ran game systems and presented 20 Vulkan frames on AMD Radeon Graphics |
| Public world mutation API | `src/world.rs` | Fixed/update systems | World and lifecycle tests | proved | Entity lookup, component access, camera selection, rotation, configurable gravity/floor, and no hidden demo floor passed |
| Consumer documentation and example | `README.md`, `examples/` | Downstream project workflow | Build and run examples | proved | Both examples compiled; `custom_game` ran and retained score 3 |
| Full validation | Whole crate | CPU reference and available GPU | fmt, Clippy, all tests, examples, package, live smoke | proved | fmt and strict all-feature Clippy passed; 59 tests passed and one heavy Dust2 import stayed ignored |

Final facts to record:

- Branch and commit: `main` at `4472c8d` before this work.
- Base branch or remote head: `origin/main` at `4472c8d` before this work.
- Worktree status: clean before this work.
- Final worktree status: scoped modifications are present and uncommitted.
- Authorized Git action: edit and verify only. No commit or push was requested.
- Checks not run: the ignored heavy CS2 Dust2 visual import, non-Linux targets, mobile,
  browser, console, editor, audio, and network flows.
- Publication boundary: Cargo package verification passed, but crates.io publication
  still needs the owner to choose and add a license.
- First action after a pause: choose a license before publishing; implementation work
  in this record is complete.
