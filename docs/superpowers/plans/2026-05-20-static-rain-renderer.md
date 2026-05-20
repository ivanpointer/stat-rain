# Static Rain Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `stat-rain run` render a deterministic, config-driven Matrix-style animation with terminal lifecycle controls and benchmarkable frame generation.

**Architecture:** Keep rendering testable by separating the pure rain engine from terminal I/O. `effect` generates frame cells from an `EffectState`; `terminal` converts cells into ANSI output and owns lifecycle escape sequences; `config` maps metrics into `EffectState`; `main` only coordinates runtime.

**Tech Stack:** Rust std, existing Clap/Serde/TOML/Anyhow/Criterion stack, Make/devbox.

---

## File Structure

- Modify `src/effect.rs`: add deterministic rain engine, frame buffer, and tests.
- Modify `src/mapping.rs`: add a small arithmetic expression evaluator for metric fields and constants.
- Modify `src/config.rs`: add TOML/file/inline/CLI merge helpers and conversion to `EffectState`.
- Modify `src/terminal.rs`: add ANSI lifecycle commands and frame rendering to a writer.
- Modify `src/cli.rs`: add run flags for bounded testable runs and terminal overrides.
- Modify `src/main.rs`: wire `stat-rain run` to config, effect, and terminal.
- Modify `benches/render_loop.rs`: benchmark the actual engine frame step.
- Modify `examples/stat-rain.toml`: keep examples compatible with the evaluator.
- Commit after each completed task.

## Task 1: Deterministic Rain Engine

- [ ] Write failing tests in `src/effect.rs` for deterministic frame generation and bounded cell count.
- [ ] Run `devbox run -- cargo test effect::tests -- --nocapture`; expect failures because the engine types do not exist.
- [ ] Implement `RainEngine`, `Frame`, `FrameCell`, and `step`.
- [ ] Run `devbox run -- cargo test effect::tests -- --nocapture`; expect pass.
- [ ] Commit with `git commit -m "Add deterministic rain engine"`.

## Task 2: Mapping Evaluation Into Effect State

- [ ] Write failing tests in `src/mapping.rs` for arithmetic over raw/normalized metric fields.
- [ ] Write failing tests in `src/config.rs` for applying profile mappings to an `EffectState`.
- [ ] Run focused tests; expect failures because evaluation is missing.
- [ ] Implement the minimal expression evaluator and config-to-effect conversion.
- [ ] Run focused tests; expect pass.
- [ ] Commit with `git commit -m "Evaluate visual mappings"`.

## Task 3: ANSI Terminal Renderer

- [ ] Write failing tests in `src/terminal.rs` for lifecycle escape sequences and rendering changed cells to a writer.
- [ ] Run focused tests; expect failures because renderer functions are missing.
- [ ] Implement ANSI lifecycle helpers and frame writer.
- [ ] Run focused tests; expect pass.
- [ ] Commit with `git commit -m "Add ANSI terminal renderer"`.

## Task 4: Wire `stat-rain run`

- [ ] Write failing CLI/runtime tests where practical through unit-testable helpers.
- [ ] Add run flags: `--frames`, `--frame-delay-ms`, `--width`, `--height`, `--no-alt-screen`, `--ascii`, and existing config/profile/mapping flags.
- [ ] Implement `run` coordination with clean terminal restore and bounded runs for verification.
- [ ] Run `devbox run -- cargo run -- run --frames 2 --width 20 --height 6 --no-alt-screen --frame-delay-ms 0`; expect visible ANSI/rain output and clean exit.
- [ ] Commit with `git commit -m "Wire static rain runtime"`.

## Task 5: Benchmarks And Verification

- [ ] Update `benches/render_loop.rs` to benchmark `RainEngine::step`.
- [ ] Run `devbox run -- make fmt`.
- [ ] Run `devbox run -- make check`.
- [ ] Run `devbox run -- make test`.
- [ ] Run `devbox run -- cargo bench --no-run`.
- [ ] Commit final formatting/benchmark updates if needed.
- [ ] Push `main` after verification passes.
