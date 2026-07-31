# TUI architecture: components, event routing, and the run loop

The TUI (`crates/lazytools/src/`) is modeled after gitui's component pattern:
small, focus-aware widgets that each own their own key handling and command
advertisement, composed by a top-level `App`.

## `Component` and `DrawableComponent`

Defined in `crates/lazytools/src/components/mod.rs`:

- `DrawableComponent::draw(&self, f, rect)` — anything that can render itself.
- `Component` — anything that also receives key events and declares the
  commands it responds to:
  - `event(&mut self, ev) -> Result<EventState>` — `Consumed` or `NotConsumed`
  - `commands(&self, out, force_all) -> CommandBlocking` — appends its
    `CommandInfo` entries; `Blocking` stops the pump from asking components
    further down the chain
  - `focused()` / `set_focused()` / `is_visible()` / `show()` / `hide()`

`CommandInfo` (key hint, label, group) is the single source of truth for both
the bottom command bar and the `?` help popup — the pump that builds the help
popup's content (`App::all_commands`) reuses exactly the same `commands()`
calls as the command bar, so the two can never drift apart.

## Routing order

`App::event` (`crates/lazytools/src/app.rs`) builds an explicit list of
components in priority order and calls `event_pump`:

```
msg_popup -> help_popup -> file_open -> file_save -> palette -> sidebar -> tool_form
```

`event_pump` stops at the first component that reports `Consumed`. Popups are
listed first because they're modal — if `msg_popup` is showing, no other
component should react to a keypress. `tool_form` is last, so plain characters
typed into a text field aren't intercepted by app-level shortcuts. This is also
why `y` (copy) works as both a literal character inside an editable field and a
command in read-only widgets: text/textarea fields consume `y` as input before
it ever reaches the app-level handler; read-only output fields don't consume
it, so it falls through to the copy shortcut.

If no component consumes the event, `App::event` checks its own app-level
shortcuts (`palette`, `help`, `copy`, `open_file`, `save_file`, `quit`,
`focus_next`/`focus_prev`) via `key_match` against the active `KeyConfig`.

## Focus and layout

`App` tracks a two-way `Focus` enum (`Sidebar` / `Workspace`). The sidebar
width is responsive (`App::sidebar_width`):

- `< 60` cols: sidebar hidden entirely, workspace takes the full width
- `60..80` cols: sidebar shrinks to an icon-only rail
- `>= 80` cols: full sidebar with tool names

## Internal event queue

Components don't mutate `App` state directly. Instead they push an
`InternalEvent` (`crates/lazytools/src/queue.rs`) onto a shared `Queue`
(cloneable, `Rc<RefCell<VecDeque<_>>>`-backed), and `App::process_queue` drains
it once per loop iteration, translating each event into state changes and a
`NeedsUpdate` bitflag. This decouples "a component decided something happened"
from "the app decided what to do about it" — e.g. `Sidebar` pushes
`SelectTool(id)`, and only `App` knows how to load that into `tool_form`.

## Run loop and debounce

`crates/lazytools/src/tui.rs` runs a fixed 16ms poll loop
(`event::poll(TICK)`), rather than blocking indefinitely on `read()`. This is
what makes the input-changed debounce work: `App::tick()` is called every loop
iteration regardless of whether a key was pressed, so a `Live`-mode tool
reruns automatically ~80ms after the user stops typing, with no extra keypress
needed. Input larger than 256KB downgrades a `Live` tool to run-on-demand (a
badge prompts pressing Enter) so pasting a large file can't hang the UI.

On Windows, crossterm reports both key-press and key-release events; only
`KeyEventKind::Press` is handled, to avoid double-processing every keystroke.

`ratatui::init()` installs a panic hook that restores the terminal before the
panic message prints, so a bug during rendering doesn't leave the user's shell
in raw mode.
