# Terminal Lifecycle Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `stat-rain run` safe and ergonomic as a long-running tmux pane app by adding terminal size resolution, resize restart, Ctrl-C restore, ASCII mode, and diff rendering.

**Architecture:** Keep terminal behavior isolated in `terminal`, glyph mode in `effect`, and runtime coordination in `main`. Use small testable helpers for size selection and diff rendering; use a signal flag in runtime so cleanup still flows through the existing restore path.

**Tech Stack:** Rust std, existing project crates, plus direct `libc` and `signal-hook` dependencies.

---

## Task 1: Terminal Size Resolution

- [ ] Add tests in `src/terminal.rs` for `TerminalSize::resolve(Some(width), Some(height), fallback)` and fallback behavior.
- [ ] Run `devbox run -- cargo test terminal::tests::resolves_terminal_size -- --nocapture`; expect missing type/function failure.
- [ ] Implement `TerminalSize`, `resolve_terminal_size`, and macOS/Linux `ioctl` size detection with fallback.
- [ ] Run the focused terminal tests; expect pass.
- [ ] Commit `git commit -m "Resolve terminal size for runtime"`.

## Task 2: ASCII Glyph Mode

- [ ] Add tests in `src/effect.rs` showing ASCII mode emits only ASCII visible glyphs and Unicode mode can emit Matrix glyphs.
- [ ] Run focused effect tests; expect failures because glyph mode is not part of engine state.
- [ ] Add `GlyphSet` to `EffectState` and update `RainEngine` glyph selection.
- [ ] Wire `--ascii` to `EffectState`.
- [ ] Run focused tests; expect pass.
- [ ] Commit `git commit -m "Support ASCII glyph mode"`.

## Task 3: Diff Frame Rendering

- [ ] Add tests in `src/terminal.rs` showing a stateful renderer writes all cells for the first frame and only changed cells for the next frame.
- [ ] Run focused terminal tests; expect missing renderer failure.
- [ ] Implement `FrameRenderer` with previous-frame tracking and clear-on-resize behavior.
- [ ] Run focused terminal tests; expect pass.
- [ ] Commit `git commit -m "Render only changed cells"`.

## Task 4: Runtime Resize And Ctrl-C Restore

- [ ] Add unit tests for a runtime helper that decides whether a resize should rebuild `RainEngine`.
- [ ] Add `libc` and `signal-hook` dependencies.
- [ ] Update `stat-rain run` to resolve terminal size when width/height flags are omitted, poll terminal size each frame, clear/restart engine on size change, and exit via a signal flag on Ctrl-C.
- [ ] Verify bounded run with `devbox run -- cargo run -- run --frames 2 --width 20 --height 6 --no-alt-screen --frame-delay-ms 0`.
- [ ] Commit `git commit -m "Harden terminal runtime lifecycle"`.

## Task 5: Verification And Push

- [ ] Run `devbox run -- make fmt`.
- [ ] Run `devbox run -- make check`.
- [ ] Run `devbox run -- make test`.
- [ ] Run `devbox run -- cargo bench --no-run`.
- [ ] Push `main` to `origin`.
