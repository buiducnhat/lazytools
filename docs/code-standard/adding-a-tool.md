# Adding a new tool

This is the workflow the whole spec-driven architecture
(see [docs/architecture/spec-driven-tools.md](../architecture/spec-driven-tools.md))
is optimized for: **one new file + one line**, no changes to the TUI or CLI layers.

## Steps

1. Create a new file under `crates/lazytools-core/src/tools/<category>/<name>.rs`
   (categories today: `crypto/`, `convert/`, `text/`, `web/`; add a new
   subdirectory for a new category and a matching variant in `spec::Category`
   if needed).
2. Define a struct holding a `ToolSpec`, built in `Default::default()`:

   ```rust
   use crate::{error::ToolError, registry::Tool, spec::*, value::*};

   pub struct ReverseTool { spec: ToolSpec }

   impl Default for ReverseTool {
       fn default() -> Self {
           Self {
               spec: ToolSpec::new("text.reverse", "Reverse Text", Category::Text)
                   .describe("Reverse a piece of text")
                   .keywords(&["reverse", "flip"])
                   .input(Field::text("text").multiline().label("Input"))
                   .output(Field::text("result").label("Result")),
           }
       }
   }

   impl Tool for ReverseTool {
       fn spec(&self) -> &ToolSpec { &self.spec }

       fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
           Ok(Outputs::one("result", i.text("text").chars().rev().collect::<String>()))
       }
   }
   ```

3. Register it — the single line that makes it exist everywhere at once, in
   `crates/lazytools-core/src/tools/mod.rs`:

   ```rust
   Box::new(text::reverse::ReverseTool::default()),
   ```

4. Add a `#[cfg(test)] mod tests` block in the same file exercising `run()`
   directly (see [testing-conventions.md](testing-conventions.md)) — no TUI or
   CLI harness needed, since `run()` takes `Inputs` and returns `Outputs` with
   nothing else in the loop. (Most tools are also pure, so a fixed expected
   value works; generators and clock-reading tools are asserted by property
   instead — see "Writing a generator" below.)

Done — the tool now shows up in the sidebar, the `Ctrl+P` palette, and
`lazytools --help`/`lazytools <name> --help`, with no other file touched.

## Conventions for `ToolSpec` content

- `id` is `"<category>.<name>"` (e.g. `"crypto.hash"`); the CLI subcommand name
  is derived automatically by stripping the category prefix
  (`ToolSpec::cli_name()`). The part after the dot must be **kebab-case**
  (`convert.data-format`, not `convert.data_format`) — `cli_name` may only
  contain `[a-z0-9-]`, and `spec_invariants` fails CI otherwise.
- `.describe(...)`, `.keywords(...)`, and every `.label(...)` are **user-facing
  text shown in the TUI and CLI help** — write them in English, since that's
  the language of the rest of the interface (`lazytools --help`, the sidebar,
  the palette). Keep descriptions to one short sentence.
- Field order matters: `inputs` first (the first one is the `primary_input()`,
  used for both stdin-piping in the CLI and "open file" in the TUI), then
  `options`, then `outputs`.
- Use `RunMode::OnDemand` for anything that takes long enough to feel laggy if
  run on every keystroke (the existing precedent is bcrypt hashing, ~250ms at
  cost 12). This is a judgment call made at declaration time, not something
  measured after the fact — the whole point of putting it in the spec is to
  force that judgment call up front.
- Error messages returned via `ToolError::invalid(field, msg)` should name the
  specific problem (not just "invalid input") — the CLI layer prefixes it with
  `--flag-name:` or the field name automatically, so the message itself only
  needs to describe what's wrong.

## Writing a generator

Tools that produce a random value (the `Generate` category) have four extra
rules. Each exists because of a constraint elsewhere in the codebase, so they
are not stylistic:

- **Use `RunMode::Generate`.** It runs the tool on open *and* makes the confirm
  key produce a fresh value. `Live` gives the user no way to ask for a different
  result; `OnDemand` opens showing nothing.
- **Declare at least one option.** `Enter` only fires a run request while focus
  is on an *editable* field, so a tool with no inputs and no options could never
  be re-triggered through it — it would generate exactly once and be stuck
  there. (`keys.run` / `Ctrl+R` works from any field including outputs, so this
  is no longer a hard lock, but the convention stands: a form that is one
  read-only box is a poor tool.)
- **A multiline field is allowed on any `RunMode`.** It used to be impossible on
  `OnDemand`/`Generate` — the form swallowed `Enter` as "run" before the field
  saw it. The form now asks `FieldWidget::wants_confirm_key()` first, so a
  multiline field keeps `Enter` for line breaks and `Ctrl+R` runs the tool.
- **A `Toggle` may default to `true`.** Every toggle option gets a generated
  `--no-x` twin (`cli::build_subcommand`), and `cli::toggle_value` resolves the
  pair against the declared default, so `--symbols` / `--no-symbols` both work
  regardless of which way the field points. `generate.password` still uses a
  `charset` select rather than three toggles, but only for CLI compatibility —
  no longer because the CLI can't express it.
- **Return `Ok` for the default inputs.** `spec_invariants::declared_outputs_are_actually_produced`
  tolerates an `InvalidInput` rejection but not a `Failed` or a panic, and a
  generator has no reason to reject its own defaults.

Test generators by **property**, never by fixed value: assert the length, the
alphabet, the line count, the ordering — not the bytes.

## Invariants enforced by tests

`crates/lazytools-core/tests/spec_invariants.rs` walks the whole registry and
will fail CI if a new tool violates any of:

- tool ids must be unique
- field keys must be unique within a single tool
- ids must map to unique, valid CLI subcommand names
- default values must match their field's `FieldKind`
- every field declared as an output must actually be produced by `run()` for
  at least one exercised input

Run `cargo test -p lazytools-core --test spec_invariants` after adding a tool
to check these before opening a PR.
