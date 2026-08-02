# File I/O: opening and saving through the TUI

lazytools tools themselves never touch the filesystem — `Tool::run()` only
sees `Inputs`/`Outputs` as in-memory text. Reading and writing files is
strictly a UI-layer concern, handled by two popups in
`crates/lazytools/src/popups/`.

## Opening a file (`file_open.rs`)

`Ctrl+O` opens `FileOpenPopup`, a directory browser. Key rules:

- `MAX_FILE_BYTES` (10MB) caps what can be loaded; `check_openable()` rejects
  anything larger with a clear "over the limit" message before any read is
  attempted — no hang, no partial load.
- On confirm, `App::open_file()` reads the file and calls
  `tool_form.set_primary_input()`, which loads the content into the currently
  open tool's first input field and triggers a rerun — exactly as if the user
  had pasted the text.
- **A tool with no inputs at all cannot receive a file.** Every generator
  (`Category::Generate`) is in that position, and `set_primary_input` used to
  write into `widgets.first_mut()` unconditionally — which for those tools is
  the first *option*, so `Ctrl+O` dumped a whole file into e.g. a `length` box.
  Three guards now cover it: `set_primary_input` returns early,
  `App::event` shows an error instead of opening the picker, and
  `App::app_commands` omits the "open file" hint entirely so the command bar
  never advertises a key that does nothing. The last one is the same
  never-let-hints-drift rule as everything else generated from
  `Component::commands()`.

## Saving a file (`file_save.rs`)

`Ctrl+S` opens `FileSavePopup` with the currently focused output value.
`Stage` is a two-step state machine:

1. `EnteringPath` — user types/pastes a destination path.
2. `ConfirmOverwrite` — entered only if the target path already exists.

Rules encoded directly in `submit()`:

- An empty path is rejected with an inline error.
- A missing parent directory is reported as an error and is **never
  auto-created** — silently creating directories on the user's behalf was
  judged too surprising for a tool meant to be safe to script around.
- **Overwriting an existing file is the only irreversible action in the whole
  app**, so it always requires a second explicit confirm keypress
  (`Stage::ConfirmOverwrite`); `Esc` from that stage returns to `EnteringPath`
  rather than closing the popup outright, so the user doesn't lose their typed
  path.

Both popups are exercised end-to-end (not just unit-tested) in
`crates/lazytools/tests/file_io.rs`, which drives `App` through simulated key
events and asserts on the rendered `TestBackend` screen content — see
[docs/code-standard/testing-conventions.md](../code-standard/testing-conventions.md).
