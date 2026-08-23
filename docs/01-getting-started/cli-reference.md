# CLI Reference

The `dhool-pro` binary accepts command-line arguments for the live renderer and
diagnostic gallery.

## Modes

Use one of these flags to select a mode.

| Flag | Action |
|------|--------|
| `--live` | Start the interactive GPU renderer window |
| `--gallery <dir>` | Export diagnostic images to a directory |
| `--dump-ppm <path>` | Write one rendered frame to a PPM file |

## Live renderer options

Use these with `--live`.

| Flag | Type | Default | Purpose |
|------|------|---------|---------|
| `--asset <path>` | String | `assets/aaa` | Scene asset directory |
| `--map <name>` | String | auto | Scene preset (`dust2` or default) |
| `--frames <N>` | Integer | unlimited | Quit after N rendered frames |
| `--capture <path>` | String | none | Write frame 30 to a PNG file |
| `--static-camera` | Flag | off | Lock the camera at scene default |
| `--profile-gpu` | Flag | off | Print GPU timestamp queries |

### Exposure flags

| Flag | Type | Purpose |
|------|------|---------|
| `--exposure-min <f32>` | Float | Minimum auto-exposure value |
| `--exposure-max <f32>` | Float | Maximum auto-exposure value |
| `--exposure-compensation <f32>` | Float | EV compensation offset |
| `--white-balance-strength <f32>` | Float | Temperature correction factor |

### Velocity flags

| Flag | Type | Default | Purpose |
|------|------|---------|---------|
| `--velocity-strength <f32>` | Float | 0.65 | Temporal velocity blending weight |
| `--velocity-cutoff <f32>` | Float | 8.0 | Clamp velocity magnitude |

### Display flags

| Flag | Type | Default | Purpose |
|------|------|---------|---------|
| `--display-gamma <f32>` | Float | 2.2 | Display gamma curve |
| `--display-brightness <f32>` | Float | 1.0 | Display brightness multiplier |

## Environment variables

| Variable | Purpose |
|----------|---------|
| `DHOOL_GPU` | Select a specific GPU adapter by name substring |

## Examples

Run the monsoon city scene in the live window:

```sh
cargo run --release --features demo-content -- --live
```

Run Dust2 with a fixed camera and export frame 40:

```sh
cargo run --release --features demo-content -- --live --map dust2     --static-camera --frames 40 --capture /tmp/dust2.png
```

Select a specific GPU:

```sh
DHOOL_GPU=NVIDIA cargo run --release --features demo-content -- --live
```

Export the full diagnostic gallery:

```sh
cargo run --release --features demo-content -- --gallery demo-output
```
