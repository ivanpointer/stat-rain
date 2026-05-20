# Stale/Error Message Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add mapped-metric stale/error health rendering, persistent health text, queued user messages, and millisecond-based message TTLs.

**Architecture:** Metric status lives in `metrics`, active mappings expose referenced metric names, and a small health builder derives `HealthState` from only those mapped metrics. A new message queue owns user message selection and TTL/coalescing, while the rain effect keeps direct control over text interaction and health field rendering.

**Tech Stack:** Rust, Clap, TOML config, Unix socket binary protocol, Make/devbox command surface.

---

### Task 1: Metric Status And Health State

**Files:**
- Modify: `src/metrics.rs`
- Modify: `src/mapping.rs`
- Modify: `src/config.rs`
- Create: `src/health.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests for status transitions in `src/metrics.rs`, referenced metric collection in `src/mapping.rs`, active mapping references in `src/config.rs`, and mapped-only health derivation in `src/health.rs`.

- [ ] **Step 2: Verify red**

Run: `devbox run -- cargo test metrics mapping config health`

Expected: failures for missing status APIs, referenced metric APIs, and health module.

- [ ] **Step 3: Implement minimal code**

Add `MetricStatus`, reason storage, clear-on-update semantics, `MappingExpression::referenced_metrics`, `AppConfig::referenced_metrics`, and `HealthState::from_mapped_metrics`.

- [ ] **Step 4: Verify green**

Run: `devbox run -- cargo test metrics mapping config health`

Expected: all focused tests pass.

### Task 2: Protocol And CLI Status/TTL Support

**Files:**
- Modify: `src/protocol.rs`
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write failing tests**

Add protocol round-trip tests for `MetricStale { reason }`, `MetricError`, `MetricStatusClear`, and `TextInjection { ttl_ms }`. Add main CLI conversion tests for `--stale`, `--error`, `--reason`, `--clear-status`, and `--ttl-ms`.

- [ ] **Step 2: Verify red**

Run: `devbox run -- cargo test protocol main`

Expected: compile/test failures for missing fields and variants.

- [ ] **Step 3: Implement minimal code**

Extend protocol variants and CLI args. Normal metric updates clear status through `MetricRegistry::set`; status messages mark external overrides stale/error/clear.

- [ ] **Step 4: Verify green**

Run: `devbox run -- cargo test protocol main`

Expected: all focused tests pass.

### Task 3: Message Queue And Millisecond Timings

**Files:**
- Create: `src/text.rs`
- Modify: `src/effect.rs`
- Modify: `src/main.rs`
- Modify: `src/cli.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests for FIFO queueing, duplicate coalescing by `(text, class)`, TTL refresh for active/queued messages, and millisecond timing conversion.

- [ ] **Step 2: Verify red**

Run: `devbox run -- cargo test text message_overlay main`

Expected: failures for missing queue and duration APIs.

- [ ] **Step 3: Implement minimal code**

Introduce `MessageQueue`, `QueuedMessage`, and duration-based `MessageTiming`. Convert run flags to `--message-fade-in-ms`, `--message-stay-ms`, and `--message-wash-ms`. Keep frame counters internal by converting with current `frame_delay_ms`.

- [ ] **Step 4: Verify green**

Run: `devbox run -- cargo test text message_overlay main`

Expected: all focused tests pass.

### Task 4: Health Rendering In Rain

**Files:**
- Modify: `src/effect.rs`
- Modify: `src/terminal.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write failing tests**

Add tests that degraded health changes rain/ember field color data without greyscaling message colors, error adds pulse metadata, health text persists, user messages preempt health text, and health returns after user message completion.

- [ ] **Step 2: Verify red**

Run: `devbox run -- cargo test health message_overlay terminal`

Expected: failures for missing health rendering behavior.

- [ ] **Step 3: Implement minimal code**

Feed `HealthState` into frame generation, add render-cell health buckets, apply greyscale/error tint in terminal color paths, and select user text before persistent health text.

- [ ] **Step 4: Verify green**

Run: `devbox run -- cargo test health message_overlay terminal`

Expected: all focused tests pass.

### Task 5: Docs, Examples, And Verification

**Files:**
- Modify: `README.md`
- Modify: `examples/stat-rain.toml`
- Modify: `Makefile` if helper commands need updated flags

- [ ] **Step 1: Update docs and helpers**

Document millisecond message flags, `send --ttl-ms`, metric stale/error/clear commands, and the health visual behavior.

- [ ] **Step 2: Run full verification**

Run:

```bash
devbox run -- make fmt
devbox run -- make check
devbox run -- make test
devbox run -- cargo bench --no-run
devbox run -- cargo run -- run --frames 4 --width 30 --height 8 --no-alt-screen --frame-delay-ms 0 --ascii --message-fade-in-ms 100 --message-stay-ms 300 --message-wash-ms 300
```

Expected: all commands pass.

- [ ] **Step 3: Commit**

Commit the implementation with message: `Implement mapped health message lifecycle`.
