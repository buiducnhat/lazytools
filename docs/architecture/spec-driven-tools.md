# Spec-driven tool architecture

The core design principle of lazytools: every tool is a plain data declaration
(`ToolSpec`) plus a single `run()` function, and both frontends (TUI and CLI)
render themselves from that declaration instead of hard-coding anything about
individual tools.

## The `Tool` trait and `ToolSpec`

`crates/lazytools-core/src/registry.rs` defines the only contract a tool must
implement:

```rust
pub trait Tool: Send + Sync {
    fn spec(&self) -> &ToolSpec;
    fn run(&self, input: &Inputs) -> Result<Outputs, ToolError>;
}
```

`ToolSpec` (`crates/lazytools-core/src/spec.rs`) declares:

- `id` / `name` / `category` / `description` / `keywords` — identity and discovery metadata
- `inputs`, `options`, `outputs` — ordered lists of `Field`, each with a `FieldKind`
  (`Text`, `Secret`, `Number`, `Select`, `Toggle`, `FilePath`)
- `mode: RunMode` — one of three:
  - `Live` — re-run automatically with debounce. The default.
  - `OnDemand` — only run when the user explicitly confirms. Used by `bcrypt`,
    since cost-12 hashing takes ~250ms and would freeze the UI if run on every
    keystroke.
  - `Generate` — runs on open and on option changes like `Live`, **and** the
    confirm key re-runs it. This third variant exists because neither of the
    other two fits a random generator: under `Live` there is no way to ask for a
    *different* password without editing a field, and under `OnDemand` the tool
    opens showing nothing at all.

`run()` is an `Inputs -> Result<Outputs, ToolError>` function with no terminal
access and no CLI dependency — this is what makes tools unit testable without
any frontend involved.

It is also pure, with **two deliberate exceptions**:

- **Generators** (`Category::Generate`) draw from a random source, so the same
  inputs give a different output every call — that *is* the feature.
- **Clock-dependent Web tools** (`web.timestamp` with an empty value,
  `web.cron`'s next-run list) read the system clock.

Neither exception costs testability, because both are tested by **property**
rather than by fixed value: that a password has the requested length and draws
only from the requested character set, that consecutive ULIDs sort in ascending
order, that a cron expression's next runs increase. Where a tool has both a pure
and an impure branch — `web.timestamp` is pure for any explicit value — the pure
branch is tested against fixed vectors and only the clock branch falls back to
property assertions.

## How the two frontends consume the spec

```
                    ToolSpec (declared once per tool)
                    /                              \
        crates/lazytools/src/cli/mod.rs     crates/lazytools/src/components/tool_form.rs
        (builds clap subcommand)             (builds ratatui input widgets)
```

- **CLI** (`cli::build_subcommand`): iterates `spec.inputs`/`spec.options` and
  maps each `FieldKind` to a clap `Arg` (`Select` → `PossibleValuesParser`,
  `Toggle` → `ArgAction::SetTrue`, `Number` → a ranged `value_parser`, etc.).
  Subcommand name comes from `spec.cli_name()`, which strips the category
  prefix (`crypto.hash` -> `hash`).
- **TUI** (`ToolFormComponent`): builds one widget per field via the
  `components/field/` module (`text.rs`, `textarea.rs`, `number.rs`,
  `secret.rs`, `select.rs`, `toggle.rs`, `filepath.rs`), keyed off the same
  `FieldKind` enum.

Because both sides read the same `ToolSpec`, adding a tool means writing one
file and one line in `tools::register_all()` (see
[docs/code-standard/adding-a-tool.md](../code-standard/adding-a-tool.md)) — the
CLI help, the TUI form, and the palette entry all appear automatically.

## `Registry`

`Registry` (`crates/lazytools-core/src/registry.rs`) owns the `Vec<Box<dyn
Tool>>` and an id -> index map for O(1) lookup. `Registry::run()` wraps
`tool.run()` in `std::panic::catch_unwind` — a panic inside a third-party crate
must not corrupt the terminal while it's in raw mode, so it's converted to a
`ToolError::Failed` instead of unwinding through the TUI event loop.

## Crate boundary

`lazytools-core` has zero dependency on `ratatui`, `crossterm`, or `clap` (enforced
by convention, documented in `crates/lazytools-core/Cargo.toml`). It only knows
about tool specs and pure data transforms. All terminal/CLI concerns live in the
`lazytools` binary crate. This means the tool catalog can be tested, reused, or
embedded elsewhere without pulling in a TUI or CLI framework.
