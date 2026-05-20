# stat-rain Initial Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Rust project scaffold for `stat-rain`: repo metadata, devbox, Makefile, CLI shell, module boundaries, starter config, protocol/config tests, and benchmark harness.

**Architecture:** This scaffold creates one Rust binary crate with focused modules for CLI, config, mapping, metrics, protocol, terminal, and effect. It keeps runtime behavior minimal while establishing typed interfaces and tests that later tasks can build on.

**Tech Stack:** Rust 2021, Cargo, Clap, Serde/TOML, Criterion, Make, devbox.

---

## File Structure

- `LICENSE`: MIT license text.
- `README.md`: concise project overview and local commands.
- `devbox.json`: project-scoped Rust, Cargo, Make, and profiling tools.
- `Makefile`: main command surface for build/test/bench/profile/run/fmt.
- `Cargo.toml`: binary crate metadata, dependencies, dev dependencies, bench config.
- `src/main.rs`: CLI entrypoint.
- `src/cli.rs`: subcommand and flag parsing.
- `src/config.rs`: typed config model, defaults, profile merge, inline TOML merge, CLI mapping overrides.
- `src/mapping.rs`: expression string validation boundary and visual attribute enum.
- `src/metrics.rs`: metric value and registry types.
- `src/protocol.rs`: versioned binary message codec skeleton.
- `src/effect.rs`: visual attribute state and renderer boundary.
- `src/terminal.rs`: terminal capability and lifecycle boundary.
- `src/lib.rs`: module exports for tests and benches.
- `benches/render_loop.rs`: initial Criterion benchmark harness.
- `examples/stat-rain.toml`: generated-style commented starter config.
- `examples/adapters/stdin-text-to-socket.sh`: executable wrapper that forwards each stdin line to `stat-rain send --message`.

## Task 1: Repo Metadata And Tooling

**Files:**
- Create: `LICENSE`
- Create: `README.md`
- Create: `devbox.json`
- Create: `Makefile`
- Modify: `.gitignore`

- [ ] **Step 1: Add repo metadata and tool config**

Create `LICENSE` with MIT license text using `Ivan Pointer` as copyright holder.

Create `README.md`:

```markdown
# stat-rain

`stat-rain` is a low-overhead Matrix-style terminal status pane for macOS and Linux.

The first version is a passive visual monitor for tmux-style panes. It maps metrics to rain attributes such as speed, density, hotness, brightness, fade length, glyph churn, and message reveal intensity.

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
```

Create `devbox.json`:

```json
{
  "$schema": "https://raw.githubusercontent.com/jetify-com/devbox/0.14.2/.schema/devbox.schema.json",
  "packages": [
    "rustup@latest",
    "cargo-nextest@latest",
    "cargo-criterion@latest",
    "hyperfine@latest",
    "valgrind@latest",
    "gnumake@latest"
  ],
  "shell": {
    "init_hook": [
      "rustup default stable >/dev/null 2>&1 || true"
    ]
  }
}
```

Create `Makefile`:

```makefile
.PHONY: build test bench profile run fmt check clean

CARGO ?= cargo

build:
	$(CARGO) build

test:
	$(CARGO) test

bench:
	$(CARGO) bench

profile:
	hyperfine --warmup 3 '$(CARGO) run -- --help'

run:
	$(CARGO) run -- run

fmt:
	$(CARGO) fmt --all

check:
	$(CARGO) check --all-targets

clean:
	$(CARGO) clean
```

Update `.gitignore`:

```gitignore
.superpowers/
target/
dist/
*.log
.DS_Store
```

- [ ] **Step 2: Commit repo metadata and tooling**

Run:

```bash
git add LICENSE README.md devbox.json Makefile .gitignore
git commit -m "Add project tooling"
```

Expected: commit succeeds.

## Task 2: Rust Crate And CLI Shell

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/cli.rs`

- [ ] **Step 1: Add failing CLI tests through Cargo build expectations**

Create `Cargo.toml`:

```toml
[package]
name = "stat-rain"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Low-overhead Matrix-style terminal status pane"
repository = "https://github.com/ivanpointer/stat-rain"

[dependencies]
anyhow = "1.0"
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

[dev-dependencies]
criterion = "0.5"
tempfile = "3.10"

[[bench]]
name = "render_loop"
harness = false
```

Create `src/lib.rs`:

```rust
pub mod cli;
```

Create `src/main.rs`:

```rust
use anyhow::Result;
use stat_rain::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    match cli.command {
        Command::Run(_) => {
            println!("stat-rain run scaffold");
        }
        Command::Init(_) => {
            println!("stat-rain init scaffold");
        }
        Command::Send(_) => {
            println!("stat-rain send scaffold");
        }
    }

    Ok(())
}
```

Create `src/cli.rs`:

```rust
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "stat-rain")]
#[command(about = "Low-overhead Matrix-style terminal status pane")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(RunArgs),
    Init(InitArgs),
    Send(SendArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub profile: Option<String>,

    #[arg(long = "config-inline")]
    pub config_inline: Option<String>,

    #[arg(long = "map")]
    pub mappings: Vec<String>,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub examples: bool,

    #[arg(long, default_value = "stat-rain.toml")]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct SendArgs {
    #[arg(long)]
    pub socket: PathBuf,

    #[arg(long)]
    pub metric: Option<String>,

    #[arg(long)]
    pub value: Option<f64>,

    #[arg(long)]
    pub message: Option<String>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
```

- [ ] **Step 2: Run CLI build check**

Run:

```bash
make check
```

Expected: command succeeds.

- [ ] **Step 3: Commit crate and CLI shell**

Run:

```bash
git add Cargo.toml src/main.rs src/lib.rs src/cli.rs
git commit -m "Add Rust CLI scaffold"
```

Expected: commit succeeds.

## Task 3: Config, Mapping, And Metric Types

**Files:**
- Modify: `src/lib.rs`
- Create: `src/mapping.rs`
- Create: `src/metrics.rs`
- Create: `src/config.rs`

- [ ] **Step 1: Add typed modules and unit tests**

Replace `src/lib.rs`:

```rust
pub mod cli;
pub mod config;
pub mod mapping;
pub mod metrics;
```

Create `src/mapping.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualAttribute {
    Speed,
    Density,
    ColorHotness,
    Brightness,
    FadeLength,
    GlyphChurn,
    MessageRevealIntensity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingExpression(String);

impl MappingExpression {
    pub fn new(value: impl Into<String>) -> Result<Self, MappingError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(MappingError::EmptyExpression);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    EmptyExpression,
}

impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExpression => write!(f, "mapping expression cannot be empty"),
        }
    }
}

impl std::error::Error for MappingError {}

impl FromStr for VisualAttribute {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "speed" => Ok(Self::Speed),
            "density" => Ok(Self::Density),
            "color_hotness" => Ok(Self::ColorHotness),
            "brightness" => Ok(Self::Brightness),
            "fade_length" => Ok(Self::FadeLength),
            "glyph_churn" => Ok(Self::GlyphChurn),
            "message_reveal_intensity" => Ok(Self::MessageRevealIntensity),
            _ => Err(format!("unknown visual attribute: {value}")),
        }
    }
}
```

Create `src/metrics.rs`:

```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricValue {
    pub raw: Option<f64>,
    pub normalized: Option<f64>,
}

impl MetricValue {
    pub fn new(raw: Option<f64>, normalized: Option<f64>) -> Self {
        Self { raw, normalized }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MetricRegistry {
    values: BTreeMap<String, MetricValue>,
}

impl MetricRegistry {
    pub fn set(&mut self, name: impl Into<String>, value: MetricValue) {
        self.values.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<MetricValue> {
        self.values.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
```

Create `src/config.rs`:

```rust
use crate::mapping::{MappingExpression, VisualAttribute};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub active_profile: String,
    pub mappings: BTreeMap<VisualAttribute, MappingExpression>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut mappings = BTreeMap::new();
        mappings.insert(VisualAttribute::Speed, MappingExpression::new("cpu.normalized * 8 + 1").unwrap());
        mappings.insert(VisualAttribute::Density, MappingExpression::new("memory.normalized * 0.5 + 0.2").unwrap());
        mappings.insert(VisualAttribute::ColorHotness, MappingExpression::new("cpu.normalized").unwrap());

        Self {
            active_profile: "default".to_string(),
            mappings,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct TomlConfig {
    profiles: Option<BTreeMap<String, TomlProfile>>,
}

#[derive(Debug, Deserialize, Default)]
struct TomlProfile {
    map: Option<BTreeMap<String, String>>,
}

impl AppConfig {
    pub fn from_toml_profile(input: &str, profile: &str) -> Result<Self> {
        let parsed: TomlConfig = toml::from_str(input).context("failed to parse config TOML")?;
        let mut config = Self {
            active_profile: profile.to_string(),
            ..Self::default()
        };

        let Some(profiles) = parsed.profiles else {
            return Ok(config);
        };

        let Some(profile_config) = profiles.get(profile) else {
            bail!("profile not found: {profile}");
        };

        if let Some(map) = &profile_config.map {
            for (attribute, expression) in map {
                config.set_mapping(attribute, expression)?;
            }
        }

        Ok(config)
    }

    pub fn apply_mapping_override(&mut self, override_value: &str) -> Result<()> {
        let Some((attribute, expression)) = override_value.split_once('=') else {
            bail!("mapping override must use attribute=expression");
        };
        self.set_mapping(attribute.trim(), expression.trim())
    }

    fn set_mapping(&mut self, attribute: &str, expression: &str) -> Result<()> {
        let attribute = attribute
            .parse::<VisualAttribute>()
            .map_err(|message| anyhow::anyhow!(message))?;
        let expression = MappingExpression::new(expression)?;
        self.mappings.insert(attribute, expression);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_selected_profile_mapping() {
        let config = AppConfig::from_toml_profile(
            r#"
            [profiles.default.map]
            speed = "cpu.normalized * 2"
            brightness = "memory.normalized"
            "#,
            "default",
        )
        .unwrap();

        assert_eq!(config.active_profile, "default");
        assert_eq!(
            config.mappings.get(&VisualAttribute::Speed).unwrap().as_str(),
            "cpu.normalized * 2"
        );
        assert_eq!(
            config.mappings.get(&VisualAttribute::Brightness).unwrap().as_str(),
            "memory.normalized"
        );
    }

    #[test]
    fn applies_cli_mapping_override() {
        let mut config = AppConfig::default();

        config
            .apply_mapping_override("glyph_churn=cpu.normalized * 4")
            .unwrap();

        assert_eq!(
            config
                .mappings
                .get(&VisualAttribute::GlyphChurn)
                .unwrap()
                .as_str(),
            "cpu.normalized * 4"
        );
    }
}
```

- [ ] **Step 2: Run unit tests**

Run:

```bash
make test
```

Expected: tests pass.

- [ ] **Step 3: Commit config and metric types**

Run:

```bash
git add src/lib.rs src/config.rs src/mapping.rs src/metrics.rs
git commit -m "Add config and metric types"
```

Expected: commit succeeds.

## Task 4: Protocol And Runtime Boundary Skeleton

**Files:**
- Modify: `src/lib.rs`
- Create: `src/protocol.rs`
- Create: `src/effect.rs`
- Create: `src/terminal.rs`

- [ ] **Step 1: Add protocol/effect/terminal modules and tests**

Replace `src/lib.rs`:

```rust
pub mod cli;
pub mod config;
pub mod effect;
pub mod mapping;
pub mod metrics;
pub mod protocol;
pub mod terminal;
```

Create `src/protocol.rs`:

```rust
use anyhow::{bail, Result};

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolMessage {
    MetricUpdate {
        name: String,
        raw: Option<f64>,
        normalized: Option<f64>,
    },
    MetricStale {
        name: String,
    },
    TextInjection {
        text: String,
    },
}

impl ProtocolMessage {
    pub fn encode(&self, output: &mut Vec<u8>) {
        output.push(PROTOCOL_VERSION);
        match self {
            Self::MetricUpdate {
                name,
                raw,
                normalized,
            } => {
                output.push(1);
                write_string(output, name);
                write_optional_f64(output, *raw);
                write_optional_f64(output, *normalized);
            }
            Self::MetricStale { name } => {
                output.push(2);
                write_string(output, name);
            }
            Self::TextInjection { text } => {
                output.push(3);
                write_string(output, text);
            }
        }
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < 2 {
            bail!("protocol message too short");
        }
        if input[0] != PROTOCOL_VERSION {
            bail!("unsupported protocol version: {}", input[0]);
        }

        let mut cursor = Cursor::new(&input[2..]);
        match input[1] {
            1 => Ok(Self::MetricUpdate {
                name: cursor.read_string()?,
                raw: cursor.read_optional_f64()?,
                normalized: cursor.read_optional_f64()?,
            }),
            2 => Ok(Self::MetricStale {
                name: cursor.read_string()?,
            }),
            3 => Ok(Self::TextInjection {
                text: cursor.read_string()?,
            }),
            kind => bail!("unsupported protocol message kind: {kind}"),
        }
    }
}

fn write_string(output: &mut Vec<u8>, value: &str) {
    let len = value.len() as u16;
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn write_optional_f64(output: &mut Vec<u8>, value: Option<f64>) {
    match value {
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value.to_le_bytes());
        }
        None => output.push(0),
    }
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn read_string(&mut self) -> Result<String> {
        let len_bytes = self.read_exact(2)?;
        let len = u16::from_le_bytes([len_bytes[0], len_bytes[1]]) as usize;
        let bytes = self.read_exact(len)?;
        Ok(String::from_utf8(bytes.to_vec())?)
    }

    fn read_optional_f64(&mut self) -> Result<Option<f64>> {
        let tag = self.read_exact(1)?[0];
        match tag {
            0 => Ok(None),
            1 => {
                let bytes = self.read_exact(8)?;
                Ok(Some(f64::from_le_bytes(bytes.try_into().unwrap())))
            }
            _ => bail!("invalid optional f64 tag: {tag}"),
        }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.offset + len;
        if end > self.input.len() {
            bail!("protocol message truncated");
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_metric_update() {
        let message = ProtocolMessage::MetricUpdate {
            name: "cpu".to_string(),
            raw: Some(42.0),
            normalized: Some(0.42),
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }

    #[test]
    fn round_trips_text_injection() {
        let message = ProtocolMessage::TextInjection {
            text: "SYSTEM OK".to_string(),
        };
        let mut encoded = Vec::new();

        message.encode(&mut encoded);

        assert_eq!(ProtocolMessage::decode(&encoded).unwrap(), message);
    }
}
```

Create `src/effect.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectState {
    pub speed: f64,
    pub density: f64,
    pub color_hotness: f64,
    pub brightness: f64,
    pub fade_length: f64,
    pub glyph_churn: f64,
    pub message_reveal_intensity: f64,
}

impl Default for EffectState {
    fn default() -> Self {
        Self {
            speed: 1.0,
            density: 0.35,
            color_hotness: 0.0,
            brightness: 0.8,
            fade_length: 8.0,
            glyph_churn: 0.25,
            message_reveal_intensity: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCell {
    pub glyph: char,
    pub color_hotness_bucket: u8,
    pub brightness_bucket: u8,
}
```

Create `src/terminal.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
    Ansi16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphMode {
    Unicode,
    Ascii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub color_mode: ColorMode,
    pub glyph_mode: GlyphMode,
    pub alternate_screen: bool,
    pub tmux: bool,
}

impl TerminalCapabilities {
    pub fn detect_from_env(term: Option<&str>, colorterm: Option<&str>, tmux: Option<&str>) -> Self {
        let color_mode = match (colorterm, term) {
            (Some(value), _) if value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit") => {
                ColorMode::TrueColor
            }
            (_, Some(value)) if value.contains("256color") => ColorMode::Ansi256,
            _ => ColorMode::Ansi16,
        };

        Self {
            color_mode,
            glyph_mode: GlyphMode::Unicode,
            alternate_screen: true,
            tmux: tmux.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_truecolor() {
        let caps = TerminalCapabilities::detect_from_env(Some("xterm-256color"), Some("truecolor"), None);

        assert_eq!(caps.color_mode, ColorMode::TrueColor);
        assert!(!caps.tmux);
    }

    #[test]
    fn detects_tmux_and_256_color() {
        let caps = TerminalCapabilities::detect_from_env(Some("screen-256color"), None, Some("/tmp/tmux"));

        assert_eq!(caps.color_mode, ColorMode::Ansi256);
        assert!(caps.tmux);
    }
}
```

- [ ] **Step 2: Run unit tests**

Run:

```bash
make test
```

Expected: tests pass.

- [ ] **Step 3: Commit protocol and runtime skeleton**

Run:

```bash
git add src/lib.rs src/protocol.rs src/effect.rs src/terminal.rs
git commit -m "Add protocol and runtime boundaries"
```

Expected: commit succeeds.

## Task 5: Examples And Benchmark Harness

**Files:**
- Create: `examples/stat-rain.toml`
- Create: `examples/adapters/stdin-text-to-socket.sh`
- Create: `benches/render_loop.rs`

- [ ] **Step 1: Add starter config and benchmark**

Create `examples/stat-rain.toml`:

```toml
# Generated starter config for stat-rain.

[profiles.default.map]
speed = "cpu.normalized * 8 + 1"
density = "memory.normalized * 0.5 + 0.2"
color_hotness = "cpu.normalized"
brightness = "memory.normalized * 0.4 + 0.5"
fade_length = "cpu.normalized * 6 + 4"
glyph_churn = "cpu.normalized * 0.5 + 0.1"
message_reveal_intensity = "external.message_pressure.normalized"

[profiles.thermal.map]
speed = "cpu.normalized * 5 + 1"
density = "memory.normalized * 0.4 + 0.3"
color_hotness = "thermal_zone.raw / 100"
brightness = "cpu.normalized * 0.3 + 0.6"
fade_length = "memory.normalized * 8 + 4"
glyph_churn = "cpu.normalized * 0.35 + 0.1"
message_reveal_intensity = "external.message_pressure.normalized"
```

Create `examples/adapters/stdin-text-to-socket.sh`:

```sh
#!/usr/bin/env sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 /path/to/stat-rain.sock" >&2
  exit 2
fi

socket="$1"

while IFS= read -r line; do
  stat-rain send --socket "$socket" --message "$line"
done
```

Create `benches/render_loop.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use stat_rain::effect::{EffectState, RenderCell};

fn build_cells(width: usize, height: usize, state: EffectState) -> Vec<RenderCell> {
    let len = width * height;
    let hotness = (state.color_hotness.clamp(0.0, 1.0) * 255.0) as u8;
    let brightness = (state.brightness.clamp(0.0, 1.0) * 255.0) as u8;

    (0..len)
        .map(|index| RenderCell {
            glyph: if index % 7 == 0 { 'ﾊ' } else { '0' },
            color_hotness_bucket: hotness,
            brightness_bucket: brightness,
        })
        .collect()
}

fn bench_build_80x24_cells(c: &mut Criterion) {
    c.bench_function("build_80x24_cells", |b| {
        b.iter(|| {
            black_box(build_cells(
                black_box(80),
                black_box(24),
                black_box(EffectState::default()),
            ))
        })
    });
}

criterion_group!(benches, bench_build_80x24_cells);
criterion_main!(benches);
```

- [ ] **Step 2: Make adapter executable**

Run:

```bash
chmod +x examples/adapters/stdin-text-to-socket.sh
```

Expected: command succeeds.

- [ ] **Step 3: Run tests and benchmark compile**

Run:

```bash
make test
cargo bench --no-run
```

Expected: tests pass and benchmark target compiles.

- [ ] **Step 4: Commit examples and benchmark**

Run:

```bash
git add examples/stat-rain.toml examples/adapters/stdin-text-to-socket.sh benches/render_loop.rs
git commit -m "Add examples and benchmark scaffold"
```

Expected: commit succeeds.

## Task 6: Final Verification

**Files:**
- No new files.

- [ ] **Step 1: Format code**

Run:

```bash
make fmt
```

Expected: command succeeds.

- [ ] **Step 2: Run full local verification**

Run:

```bash
make check
make test
cargo bench --no-run
```

Expected: all commands succeed.

- [ ] **Step 3: Inspect git state**

Run:

```bash
git status --short
git log --oneline --max-count=6
```

Expected: working tree is clean after committing any formatting changes. Recent commits show the design spec and scaffold commits.

- [ ] **Step 4: Push only when requested**

Do not push automatically. The remote is `git@github.com:ivanpointer/stat-rain.git`, but publishing should wait for user confirmation after local scaffold verification.
