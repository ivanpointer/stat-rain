# stat-rain

`stat-rain` is a low-overhead Matrix-style terminal status pane for macOS and Linux.

The first version is a passive visual monitor for tmux-style panes. It maps metrics to rain attributes such as speed, density, hotness, brightness, fade length, glyph churn, and message reveal intensity.

Metric-driven effect changes are smoothed over a 10 second window by default. Use `--effect-smoothing-ms` to tune that window, or set it to `0` for immediate changes.

The built-in `cpu` metric is aggregate CPU usage across the machine. `cpu.total` is also exposed as an explicit alias for configs that should make that aggregate behavior obvious.

## Development

Use `make` as the command surface:

```sh
make build
make test
make bench
make run
make fmt
```

`devbox` provides the project toolchain when available:

```sh
devbox shell
```

## Current Status

The project is in initial scaffold development. See `docs/superpowers/specs/2026-05-20-stat-rain-design.md`.
