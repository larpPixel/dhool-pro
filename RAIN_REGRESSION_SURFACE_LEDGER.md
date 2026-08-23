# Rain Regression Surface Ledger

User-visible behavior: live maps show dense, depth-tested world-space rain without large screen-space water blobs obscuring the scene.

| Surface | Applies | Source | Required change | Check | State |
|---|---|---|---|---|---|
| Entry points | yes | `--live`, including `--map dust2` | Use the repaired shared live renderer | Fullscreen PRIME run | Default verified; Dust II is blocked before rendering by a pre-existing WGPU out-of-memory panic |
| Clients | yes | Winit live window | Present the repaired HDR result | Fullscreen screenshot | verified |
| Providers | yes | WGPU rain and post-processing passes | Restore drop density and remove lens-water pass | Renderer contract test | verified |
| Contracts | yes | Runtime frame graph and tonemap bindings | Remove the composite resource, pass, binding, and shader | Architecture tests and GPU startup | verified |
| Reverse state | no | Rain is driven by scene weather, not a user toggle | No reverse action exists | Source trace | not applicable |
| Connection modes | no | Rendering is local and has no network or relay path | No remote variants exist | Source trace | not applicable |
| Tests | yes | `tests/architecture.rs`, `tests/renderer.rs` | Lock in dense world rain and absence of the bad pass | Focused and full test suites | verified |
| Documents | yes | `OPT_IMPLEMENTATION.md` | Describe the actual rain presentation | Diff review | verified |

Historical reference: `c4ee85d` used clean world-space streaks and did not contain the screen-space composite introduced by `c2f1e44`.
