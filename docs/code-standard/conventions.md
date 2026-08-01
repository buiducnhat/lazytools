# Coding conventions

## Language

All code, comments, doc comments, and user-facing strings (UI labels, CLI help
text, tool descriptions, error messages) are written in **English**. This
includes `.describe(...)`/`.keywords(...)`/`.label(...)`/`.help(...)` calls on
`ToolSpec`/`Field` — those strings render directly in the TUI and
`--help` output.

A Vietnamese translation of the documentation lives locally under `docs-vi/`
(generated, not tracked in git — see the root `.gitignore` entry for that
directory) and `README.vi.md` (tracked, linked from `README.md`'s language
switcher) — code and its inline comments are not translated.

## Rust edition and toolchain

- Edition 2024, `resolver = "3"` (declared once in the workspace `Cargo.toml`;
  member crates inherit `edition.workspace = true` / `license.workspace = true`).
- Requires Rust 1.97 or newer, declared as `rust-version` in
  `[workspace.package]` and pinned in `rust-toolchain.toml`. Raising the MSRV
  means changing both — see [releasing.md](releasing.md#toolchain).
- Shared dependency versions live in `[workspace.dependencies]` at the root;
  member `Cargo.toml` files reference them with `foo.workspace = true` rather
  than pinning their own versions.

## Crate boundary (architectural rule, not just a convention)

`lazytools-core` must never depend on `ratatui`, `crossterm`, or `clap` — this
is called out explicitly in its `Cargo.toml`. If you find yourself wanting to
import one of those in `lazytools-core`, the logic you're writing belongs in
the `lazytools` binary crate instead.

## Error handling

- Tool-level errors are `lazytools_core::ToolError` (`InvalidInput { field,
  msg }`, `Failed(String)`, `Io`). Always prefer `InvalidInput` with the
  specific field name when the error can be attributed to one input/option —
  the CLI layer uses that field name to print `--flag-name: <msg>` instead of
  a generic message.
- App-level (TUI) errors use `anyhow::Result`.
- Never let a tool panic escape into the TUI event loop uncaught —
  `Registry::run()` already wraps every `tool.run()` call in
  `catch_unwind` and converts a panic into `ToolError::Failed`, precisely so a
  bug in one tool (or a third-party crate it depends on) can't corrupt the
  terminal while it's in raw mode. Don't add a second `catch_unwind` elsewhere;
  the registry boundary is the only place this should happen.

## Formatting/linting gate

`cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings` must
both pass clean — clippy warnings are treated as errors in CI, not advisory.

## Naming

- Tool ids: `<category>.<name>` in `snake_case` after the dot (e.g.
  `crypto.hash`, `convert.data_format`). The CLI name is derived by stripping
  the category prefix — don't hand-roll a different CLI name in the tool spec.
  `_` in a field key becomes `-` in its CLI flag (`url_safe` -> `--url-safe`),
  handled once in `cli::flag_name`, not per tool.
- Component structs implementing the `Component`/`DrawableComponent` traits
  are named `<Thing>` for always-visible panels (`Sidebar`, `CommandBar`) and
  `<Thing>Popup` for anything that opens on demand (`HelpPopup`, `MsgPopup`,
  `FileOpenPopup`, `FileSavePopup`).
