# Reusable game engine surface ledger

User-visible behavior: another Rust project can depend on `dhool-pro`, define its
own scene and game systems, and run the engine without Dhool's showcase assets or
demo-specific constructors.

| Surface | Applies | Source | Required change | Check | State |
|---|---|---|---|---|---|
| Entry points | yes | `src/lib.rs`, `src/main.rs`, `examples/` | Export an engine builder and keep the repository CLI as a demo client | Consumer examples and CLI compilation | proved |
| Clients | yes | External Rust crates, bundled CLI | Use only public contracts in the examples and integration tests | Default-feature and all-feature builds | proved |
| Providers | yes | CPU reference renderer, live wgpu renderer | Accept a consumer-owned world and stream system mutations into the live renderer | Three CPU frames and 20 live game-driven Vulkan frames | proved |
| Contracts | yes | Settings, schedules, systems, resources, events, scenes, errors | Add typed public contracts with validation and useful errors | Focused contract tests | proved |
| Reverse state | yes | World/resource/event mutation and frame reports | Let systems read back mutations and diagnostics after a frame | Lifecycle, render-free advance, event-drain, and 20-update live smoke checks | proved |
| Connection modes | yes | Headless CPU and interactive GPU | Keep both paths; GPU runtime needs a display and adapter | Headless run plus AMD Vulkan live run | proved |
| Tests | yes | Unit and integration suites | Add a downstream-style game and regress existing renderer behavior | 59 passed, one documented heavy import ignored | proved |
| Documents | yes | `README.md`, Cargo metadata, ledgers | Lead with dependency and custom-game usage; move demo content to an optional feature | Package and example verification | proved |

The mobile, browser, editor, network, and console clients do not apply. This
repository has no implementation or contract for those targets. They are not
claimed by this change.
