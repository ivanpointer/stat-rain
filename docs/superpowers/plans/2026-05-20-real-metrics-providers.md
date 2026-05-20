# Real Metrics Providers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace placeholder runtime metrics with lightweight built-in CPU and memory providers for macOS and Linux.

**Architecture:** Add a `MetricProvider` abstraction in `metrics`, keep OS-specific sampling behind focused modules, and have `stat-rain run` sample providers at a configurable interval before evaluating mappings into `EffectState`. Keep parser/math helpers testable without requiring a specific host OS.

**Tech Stack:** Rust std, existing `libc`, existing Make/devbox/Criterion tooling.

---

## Task 1: Metric Provider Core

- [ ] Add tests for CPU delta calculation, memory normalization, registry merge, and stale/error metric marking.
- [ ] Implement `MetricProvider`, `MetricSample`, CPU tick math, memory math, and `MetricRegistry::mark_stale`.
- [ ] Commit `git commit -m "Add metric provider core"`.

## Task 2: Linux Provider

- [ ] Add parser tests for `/proc/stat` and `/proc/meminfo` sample strings.
- [ ] Implement Linux CPU/memory provider under `cfg(target_os = "linux")`.
- [ ] Commit `git commit -m "Add Linux system metrics provider"`.

## Task 3: macOS Provider

- [ ] Add tests for macOS CPU tick math using synthetic Mach tick arrays.
- [ ] Implement macOS CPU load and memory sampling with `host_statistics64`/`sysctlbyname`.
- [ ] Commit `git commit -m "Add macOS system metrics provider"`.

## Task 4: Runtime Integration

- [ ] Add `--metric-sample-ms` flag.
- [ ] Update `stat-rain run` to sample built-in metrics, evaluate mappings every frame from the current registry, and use stale/error fallback values if sampling fails.
- [ ] Verify a bounded run still renders.
- [ ] Commit `git commit -m "Drive rain mappings from system metrics"`.

## Task 5: Benchmark And Verification

- [ ] Add a Criterion benchmark for metric sampling helpers.
- [ ] Run `devbox run -- make fmt`.
- [ ] Run `devbox run -- make check`.
- [ ] Run `devbox run -- make test`.
- [ ] Run `devbox run -- cargo bench --no-run`.
- [ ] Push `main` to `origin`.
