# Adding a new tool

This is the workflow the whole spec-driven architecture
(see [docs/architecture/spec-driven-tools.md](../architecture/spec-driven-tools.md))
is optimized for: **one new file + one line**, no changes to the TUI or CLI layers.

## Steps

1. Create a new file under `crates/lazytools-core/src/tools/<category>/<name>.rs`
   (categories today: `crypto/`, `convert/`; add a new subdirectory for a new
   category and a matching variant in `spec::Category` if needed).
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
   CLI harness needed, since `run()` is a pure function.

Done — the tool now shows up in the sidebar, the `Ctrl+P` palette, and
`lazytools --help`/`lazytools <name> --help`, with no other file touched.

## Conventions for `ToolSpec` content

- `id` is `"<category>.<name>"` (e.g. `"crypto.hash"`); the CLI subcommand name
  is derived automatically by stripping the category prefix
  (`ToolSpec::cli_name()`).
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
