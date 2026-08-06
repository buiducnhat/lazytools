# Testing conventions

Four distinct test layers exist, each targeting a different seam. Match the
layer to what you're actually verifying — don't reach for a TUI test when a
plain unit test would cover it.

## 1. Tool unit tests (`crates/lazytools-core/src/tools/**/*.rs`)

Each tool file has a `#[cfg(test)] mod tests` block calling `run()` directly
with hand-built `Inputs`. No frontend, no I/O. This is the default and
cheapest layer — cover known vectors, edge cases, and every `ToolError`
variant a tool can return here.

Convention: a local `fn run(...) -> Result<Outputs, ToolError>` helper that
builds `Inputs` with `.with()`, plus a `fn ok(...) -> String` for the happy
path. (`gen` is a reserved keyword in edition 2024 — don't name a helper that.)

### Testing tools that aren't pure

Random generators and the clock-reading Web tools have no fixed expected
output, so assert **properties** instead of values: the requested length, that
every character is in the requested alphabet, the line count, the ordering.

Property tests earn their keep — they are not a weaker substitute. The
`generate.ulid` ordering test failed on first run and was right to: bare
`Ulid::generate()` only orders by millisecond, so a batch produced inside one
millisecond came out visibly unsorted, in a tool whose entire selling point is
that its values sort. Length-and-alphabet assertions alone would have shipped
that. Pick the property that would actually be embarrassing to get wrong.

For a tool with both a pure and an impure branch (`web.timestamp` is pure for
any explicit value, clock-dependent only when the input is empty), test the
pure branch against fixed vectors and reserve property assertions for the
clock branch.

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

## 4. TUI tests (`crates/lazytools/tests/file_io.rs`, `session.rs`, `theme.rs`, `snapshots.rs`)

Construct `App` directly (importable because the app logic lives in
`lib.rs`, not just `main.rs`) and drive it with simulated
`ratatui::crossterm::event::Event`s against a `ratatui::backend::TestBackend`
— no real terminal needed. Two flavors:

- **Behavioral** (`file_io.rs`, `session.rs`): asserts on file-write side
  effects and specific rendered strings (e.g. "must ask for confirmation").
  Uses a `TempDir` helper that only ever touches `std::env::temp_dir()`, never
  files inside the repo. `App::with_settings(registry, settings, session)` is
  the seam for anything config- or persistence-dependent — construct the state
  explicitly rather than writing to the user's real config directory, which a
  test must never touch.
- **Style** (`theme.rs`, and the sidebar-highlight test in `session.rs`): reads
  `terminal.backend().buffer()` and asserts on cell `fg`/`bg`. Reach for this
  whenever the behavior under test *is* a style: `TestBackend::to_string()`
  throws styles away, so a string assertion would pass no matter which row was
  highlighted. That is exactly how the palette/sidebar desync survived from the
  MVP to v0.4 unnoticed.
- **Snapshot** (`snapshots.rs`): uses [`insta`](https://insta.rs) to assert the
  full rendered screen buffer matches a committed `.snap` file. First run
  produces a `.snap.new`; review and accept with `cargo insta review` (or
  `cargo insta test --accept` to accept all pending changes non-interactively).
  **Regenerate snapshots whenever you intentionally change UI text or
  layout** — a snapshot diff is the point, not a failure to work around.

**Adding a tool breaks six snapshot tests, and that is expected.** Most tests
in `snapshots.rs` build `App::new(Registry::new())` — the *real* registry — so
any new tool changes the sidebar in every one of them. Don't "fix" this by
switching those tests to `Registry::from_tools`: they exist to catch layout
regressions in the real app, and isolating them from the registry would throw
away exactly what they protect. Read the diff, confirm it is sidebar-only, then
accept.

Use `Registry::from_tools` with a locally-defined test tool only when the test
is about a *capability* rather than the shipped catalog — covering every
`FieldKind`, or exercising `RunMode::Generate` with an `AtomicUsize` run
counter so the assertion is "it ran twice", not a random value.

When adding a regression test for a bug, prove it fails without the fix before
committing it. Temporarily revert the fix, watch the test go red, restore.
A regression test that would pass either way documents nothing.

Debounce-dependent tests (anything exercising `RunMode::Live` or
`RunMode::Generate`) sleep past the ~80ms debounce window and call `app.tick()`
explicitly rather than polling — keep that pattern for new tests in this file
rather than adding real-time polling loops.

## Running everything

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

These three commands are exactly what CI runs, on Linux + macOS + Windows —
run them locally before opening a PR. `cargo insta review` is not part of CI;
snapshot changes must already be committed.
