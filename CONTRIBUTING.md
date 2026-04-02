# Contributing to complex

## Build

```bash
cargo build
```

The binary is `cx` (defined in `Cargo.toml` as `[[bin]] name = "cx"`).

## Test

```bash
cargo test
```

Integration tests live in `tests/integration.rs` and exercise the CLI
end-to-end using `assert_cmd` + `tempfile`. Each test gets its own
`.complex/` directory via `CX_DIR`.

## Lint

```bash
cargo clippy -- -D warnings
```

## Install locally

```bash
cargo install --path .
```

This puts `cx` in `~/.cargo/bin/`. After verifying tests and clippy pass,
install before committing so all projects get the update immediately.

## Project structure

```
src/
  main.rs    — CLI entry point (clap). All subcommands defined and dispatched here.
  model.rs   — Data types: Node, Edge, EdgeType, State, Comment, Graph.
  store.rs   — Persistence: load/save per-node files, issues/, comments, archive, events.
  db.rs      — In-memory SQLite: materializes the graph for queries (cx surface, etc).
  id.rs      — Base62 ID generation with collision detection and retry.
tests/
  integration.rs — End-to-end CLI tests.
```

## Commit conventions

Commit messages follow the pattern:

```
integrate(<short-id>): <task title>
```

where `<short-id>` is the cx task ID. Commit `.complex/` changes alongside
code changes so the task history travels with the repo.
