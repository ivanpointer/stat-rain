# stat-rain Agent Instructions

## Open Brain Project

Use the Open Brain project `stat-rain` for this repository.

- Before non-trivial work, search Open Brain with `project_id: "stat-rain"` and `scope: "project_then_global"`.
- Prefer repo files and live tool output over Open Brain if they conflict.
- Capture durable repo-specific decisions, implementation notes, visual tuning, verification history, unresolved blockers, and next steps with `project_id: "stat-rain"`.
- Do not store secrets, raw logs, large code blocks, private data, or low-value transcript noise.
- If relevant stat-rain thoughts are found under `general`, review them and move them into `stat-rain` before relying on them.

The Open Brain project metadata should identify:

- Project id: `stat-rain`
- Repo: `git@github.com:ivanpointer/stat-rain.git`
- Local path: `/Users/ivanpointer/Source/matrix-monitor`
- Default branch: `main`

## Development

Use `make` as the repo-facing command surface. When devbox is needed, run make through:

```sh
devbox run -- make <target>
```

Keep implementation work scoped and verify before reporting completion. Typical checks:

```sh
devbox run -- make fmt
devbox run -- make check
devbox run -- make test
devbox run -- cargo bench --no-run
```

For visual changes, also run a bounded render smoke with explicit dimensions and `--no-alt-screen`.
