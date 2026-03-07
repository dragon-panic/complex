# complex (cx)

## Install after verify

This project IS the `cx` tool. The verify → install → commit → push steps
should flow without pausing between them once the user has approved the work.

After tests and clippy pass (step 7), run:

```
cargo install --path .
```

This installs to `~/.cargo/bin` so all projects get the update. Do this before committing.

## Project structure

- `src/main.rs` — CLI entry point (clap)
- `src/store.rs` — persistence layer (graph, bodies, archive)
- `tests/integration.rs` — integration tests
- `AGENT.md` — instructions for autonomous agents using cx
