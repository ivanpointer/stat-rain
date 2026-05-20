# stat-rain Design Spec

Date: 2026-05-20
Status: Approved for implementation planning

## Purpose

`stat-rain` is a low-overhead terminal visualizer for tmux-style panes. It renders a Matrix-style rain effect that acts as an at-a-glance system status indicator. Visual attributes such as speed, density, color hotness, brightness, fade length, glyph churn, and message reveal intensity can be mapped to metrics.

Performance is a first-order requirement. For a normal `80x24` pane at default intensity, v1 should stay under `2%` of one CPU core and under `20 MB` RSS.

## Target Platforms

v1 supports macOS and Linux from the first release. OS-specific metric collection must live behind provider interfaces so future platforms can be added without changing renderer or mapping logic.

The app should work well in modern terminals and tmux, while degrading cleanly for simpler terminals.

## Product Shape

The project name is `stat-rain`.

v1 is primarily a passive visual monitor, not an interactive control surface. It starts, renders, responds to configured metrics and external messages, signals stale/error states visually, and exits cleanly.

The install target is a single Rust binary. The binary exposes subcommands:

- `stat-rain run`: start the renderer, built-in metric providers, config evaluator, and Unix socket ingestion.
- `stat-rain init`: generate a commented TOML starter config.
- `stat-rain init --examples`: additionally generate example wrappers/adapters.
- `stat-rain send`: reference sender for the binary protocol.

Adapters may bridge stdin JSON, JSON files, or polling scripts into the binary socket protocol, but the renderer hot path should remain binary and long-lived.

## Architecture

`stat-rain` is a Rust single-binary application with clear internal module boundaries:

- `terminal`: alternate screen, cursor visibility, capability detection, resize handling, terminal restore, and changed-cell output.
- `effect`: Matrix rain simulation, drops, glyph churn, trails, thermal coloring, and message reveal.
- `metrics`: built-in macOS/Linux providers, metric registry, stale/error state, and raw/normalized metric values.
- `protocol`: compact versioned binary messages over Unix domain sockets.
- `mapping`: small arithmetic expression evaluation for mapping metrics to visual attributes.
- `config`: TOML loading, inline TOML, profile selection, CLI overrides, and deterministic merge precedence.

No dynamically loaded native plugins are part of v1. Extension happens through external metric/message producers that communicate over the Unix socket. The protocol is versioned from day one but may change before `1.0`.

## Runtime Model

`stat-rain run` starts in this order:

1. Load configuration from built-in defaults, TOML, selected profile, inline TOML, and CLI overrides.
2. Detect terminal capability from environment and terminal behavior where practical.
3. Start built-in metric providers for macOS/Linux.
4. Start a Unix domain socket listener for external metric and message events.
5. Enter the render loop.

The render loop uses fixed-size buffers for the current terminal dimensions. It avoids per-frame heap allocation, updates only changed cells where practical, and keeps behavior deterministic enough to benchmark. On resize, it clears the screen, reallocates buffers, and restarts visual state.

Default lifecycle behavior is alternate screen with cursor hidden during rendering and restored on exit. This is configurable for wrappers or unusual terminal use.

Runtime logs stay quiet unless a log file is configured. Fatal startup errors may print before terminal takeover or after shutdown.

## Terminal Capability

The app should automatically detect practical capabilities:

- truecolor, 256-color, or 16-color output
- Unicode glyph support with ASCII fallback
- alternate-screen support
- tmux-related hints

Config and CLI flags can override detection.

## Metrics

The core renderer consumes metric values, not provider-specific concepts like CPU directly. Built-in providers are first-class, but external metric feeds use the same registry and mapping path.

Metric values should expose both raw and normalized forms when available:

- `metric.raw`: source-specific raw value, such as temperature in Celsius.
- `metric.normalized`: provider-defined normalized value, usually `0.0..1.0`.

Some metrics, such as temperature, cannot be normalized universally without bad assumptions. Providers should normalize when they can do so meaningfully and expose raw values when raw interpretation is important.

If a metric source fails or goes stale, rendering continues and the visual effect signals the stale/error condition.

## Visual Effect

The default effect combines three ideas:

- Classic rain: dense falling columns with bright heads and fading trails.
- Thermal status: color ramps that can shift from cool to hot based on mapped metrics.
- Signal reveal: dynamic text/messages emerge briefly from the rain.

v1 exposes a medium set of mappable visual attributes:

- `speed`
- `density`
- `color_hotness`
- `brightness`
- `fade_length`
- `glyph_churn`
- `message_reveal_intensity`

Each visual attribute should usually map to one clear metric signal. This preserves at-a-glance readability. The system does not need multi-metric blending per attribute in v1.

Dynamic text injection is required in v1. Text is pushed through the protocol at runtime and rendered into the rain effect. Static quote lists are not a v1 requirement.

## Configuration

Configuration format is TOML.

Configuration inputs merge in deterministic precedence:

1. Built/default values.
2. TOML config file.
3. Selected profile via `--profile`.
4. Inline TOML via `--config-inline`.
5. Repeated CLI override flags, such as `--map speed='cpu.normalized * 8'`.

Profiles are user-defined TOML sections. Built-in profiles should be generated as commented starter examples rather than hidden hard-coded presets.

Example mapping shape:

```toml
[profiles.default.map]
speed = "cpu.normalized * 8 + 1"
color_hotness = "thermal_zone.raw / 100"
density = "memory.normalized * 0.5 + 0.2"
```

The expression language is intentionally small: arithmetic over metric fields and constants. It is not a general scripting language.

## Protocol

The external input path is a compact binary protocol over Unix domain sockets.

The protocol is versioned from day one, with no compatibility promise before `1.0`. v1 message categories should cover:

- metric definition/update
- metric stale/error state
- dynamic text injection

The exact representation of text injection may be a distinct message type or a reserved protocol lane, whichever best fits performance and extensibility. It must be efficient and first-class.

Reference helpers include `stat-rain send` and example JSON/stdin/file-poll adapters that translate into the binary protocol.

## Development Tooling

Use `devbox` for project-scoped development dependencies. Include Rust plus benchmarking/profiling tools from the start.

Use `make` as the main project command surface. Expected targets include:

- `make build`
- `make test`
- `make bench`
- `make profile`
- `make run`
- `make fmt`

No CI files are required until the repository host and workflow are settled.

## Packaging And License

The initial packaging target is a single installable binary from Cargo or release artifacts.

The license is MIT.

## Initial Implementation Scope

After this design is approved, implementation planning should begin by scaffolding:

- local git repository and GitHub remote
- MIT license
- devbox configuration
- Makefile
- Rust crate layout
- config examples
- core module boundaries
- initial benchmark harness for render-loop and protocol parsing measurements

The first working prototype should prioritize the render loop, terminal lifecycle, config loading, and benchmark visibility before expanding provider and adapter coverage.
