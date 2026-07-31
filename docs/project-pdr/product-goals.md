# Product goals

## What lazytools is

A terminal utility belt — offline, keyboard-first — comparable to
[it-tools](https://github.com/CorentinTh/it-tools) but as a TUI, while also
working directly as CLI subcommands in shell pipelines. Users either run
`lazytools` with no arguments to open the interactive interface, or invoke a
specific tool directly, e.g. `echo -n "hi" | lazytools hash --algo md5`.

## Central design thesis

The tool catalog is an **open set** that is expected to keep growing. If the
cost of adding a tool scaled linearly with each addition (touching the TUI,
the CLI, and the core logic separately), the project would become
unmaintainable somewhere around the 20th tool. The whole architecture — a
`ToolSpec` + pure `run()` function per tool, with both the TUI and CLI
generated from that spec — exists specifically to keep the marginal cost of
tool #40 about the same as tool #4.

This was validated during the MVP build: adding all 7 remaining tools in
Phase 4 changed zero lines in `crates/lazytools/src/` (the entire UI/CLI
layer) — only `tools/mod.rs` registration, per-tool files, and per-tool
`Cargo.toml` dependencies changed. See
[docs/architecture/spec-driven-tools.md](../architecture/spec-driven-tools.md)
for how that's achieved.

## Pipeline-first CLI contract

- A single output prints **raw**, with no label — designed to be piped
  directly into the next command.
- Multiple outputs print one `key=value` pair per line.
- `--json` prints the whole output as a JSON object, for programmatic
  consumption.
- Input is read from stdin whenever a positional argument is omitted or `-`
  is passed, so tools compose naturally in shell pipelines.

## Reliability expectations baked into the product

- A broken `keys.toml` must never block the app from starting — it opens
  with defaults and reports exactly which entries were skipped.
- A panic inside any tool (including third-party crate code) must never
  corrupt the user's terminal — it's caught and converted into a normal error.
- Overwriting a file is the only irreversible action in the whole app, so it
  is the only action requiring a mandatory second confirmation.
- CLI help text and TUI labels must never drift from actual behavior — both
  are generated from the same source (`ToolSpec` for tool info,
  `Component::commands()` for keybinding hints), rather than maintained by
  hand in two places.
