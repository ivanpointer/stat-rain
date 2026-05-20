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
make run-socket
make run-fake-hot
make stress-cpu
make fmt
```

For visual tuning, `stat-rain run` can override live metrics with synthetic
values:

```sh
cargo run -- run \
  --simulate-metric cpu=1.0 \
  --simulate-metric memory=0.9 \
  --simulate-metric thermal_zone=95:0.95
```

Use `name=normalized` for metrics that only need a normalized value, or
`name=raw:normalized` when a mapping uses both. The `cpu` override also updates
the `cpu.total` alias.

To create real CPU pressure while watching another pane, run:

```sh
cargo run -- stress-cpu --threads 8 --duration-seconds 30
```

To change synthetic metrics while the rain is running, use two terminals. In the
rain pane:

```sh
cargo run -- run --socket /tmp/stat-rain.sock \
  --message-fade-in-ms 1500 \
  --message-stay-ms 3000 \
  --message-wash-ms 3000
```

In another pane:

```sh
cargo run -- send --socket /tmp/stat-rain.sock --metric cpu --value 0.02
cargo run -- send --socket /tmp/stat-rain.sock --metric cpu --value 0.99
cargo run -- send --socket /tmp/stat-rain.sock --metric cpu --value 0.50
cargo run -- send --socket /tmp/stat-rain.sock --message "BUILD OK" --ttl-ms 10000
cargo run -- send --socket /tmp/stat-rain.sock --message "BUILD FAILED" --class error
cargo run -- send --socket /tmp/stat-rain.sock --metric thermal_zone --stale --reason "sensor timeout"
cargo run -- send --socket /tmp/stat-rain.sock --metric thermal_zone --error --reason "read failed"
cargo run -- send --socket /tmp/stat-rain.sock --metric thermal_zone --clear-status
```

The pushed `cpu` metric also updates `cpu.total`, and external values stay
authoritative over built-in CPU samples until another value is pushed.
Pushed messages render centered as a short-lived bright overlay. Characters
resolve in randomized order during fade-in, hold stable, then wash away as rain
and embers overwrite the message cells. Message classes render with explicit
status colors: `info` is blue, `success` is green, `warning` is yellow, and
`error` is red. Class changes also tune brightness while preserving the same
message lifecycle. `--ttl-ms` controls how long a message stays fully visible
before wash-away begins.

Mapped metric stale/error states degrade the rain field only when the metric is
used by the active visual mappings. Stale and error both shift rain/embers
toward greyscale; error also adds a subtle red pulse. The derived health text is
persistent while the mapped health condition remains active, but user-sent
messages can temporarily occupy the center before the health text returns.

The Make helpers accept the same classes:

```sh
make send-message MSG="DEPLOY STARTED" CLASS=info
make send-success MSG="BUILD OK"
make send-warning MSG="CPU HOT"
make send-error MSG="BUILD FAILED"
make send-stale METRIC=thermal_zone REASON="sensor timeout"
make send-metric-error METRIC=thermal_zone REASON="read failed"
make clear-status METRIC=thermal_zone
```

`devbox` provides the project toolchain when available:

```sh
devbox shell
```

## Current Status

The project is in initial scaffold development. See `docs/superpowers/specs/2026-05-20-stat-rain-design.md`.
