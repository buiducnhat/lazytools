# Testing conventions

Four distinct test layers exist, each targeting a different seam. Match the
layer to what you're actually verifying — don't reach for a TUI test when a
plain unit test would cover it.

## 1. Tool unit tests (`crates/lazytools-core/src/tools/**/*.rs`)

Each tool file has a `#[cfg(test)] mod tests` block calling `run()` directly
with hand-built `Inputs`. No frontend, no I/O. This is the default and
cheapest layer — cover known vectors, edge cases, and every `ToolError`
variant a tool can return here.

## 2. Spec invariants (`crates/lazytools-core/tests/spec_invariants.rs`)

Integration test that walks `Registry::new()` and checks properties that must
hold for *every* tool (unique ids, unique field keys, valid CLI names, default
values matching `FieldKind`, declared outputs actually produced). This is what
catches a copy-paste mistake in a new `ToolSpec` before it becomes a runtime
bug in the CLI or TUI.

## 3. CLI end-to-end tests (`crates/lazytools/tests/cli.rs`)

Uses `assert_cmd` to spawn the actual `lazytools` binary as a subprocess per
test and assert on stdout/stderr/exit code. Used for behavior that only
exists at the CLI layer: stdin piping, `--json`, exit code conventions (2 for
clap usage errors, 1 for `ToolError::InvalidInput` reaching `run()`), and
`--help` content.

## 4. TUI tests (`crates/lazytools/tests/file_io.rs`, `snapshots.rs`)

Construct `App` directly (importable because the app logic lives in
`lib.rs`, not just `main.rs`) and drive it with simulated
`ratatui::crossterm::event::Event`s against a `ratatui::backend::TestBackend`
— no real terminal needed. Two flavors:

- **Behavioral** (`file_io.rs`): asserts on file-write side effects and
  specific rendered strings (e.g. "must ask for confirmation"). Uses a
  `TempDir` helper that only ever touches `std::env::temp_dir()`, never files
  inside the repo.
- **Snapshot** (`snapshots.rs`): uses [`insta`](https://insta.rs) to assert the
  full rendered screen buffer matches a committed `.snap` file. First run
  produces a `.snap.new`; review and accept with `cargo insta review` (or
  `cargo insta test --accept` to accept all pending changes non-interactively).
  **Regenerate snapshots whenever you intentionally change UI text or
  layout** — a snapshot diff is the point, not a failure to work around.

Debounce-dependent tests (anything exercising `RunMode::Live`) sleep past the
~80ms debounce window and call `app.tick()` explicitly rather than polling —
keep that pattern for new tests in this file rather than adding real-time
polling loops.

## Running everything

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

These three commands are exactly what CI runs, on Linux + macOS + Windows —
run them locally before opening a PR. `cargo insta review` is not part of CI;
snapshot changes must already be committed.
