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
msg_popup -> help_popup -> file_open -> file_save -> theme_popup -> palette -> sidebar -> tool_form
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
shortcuts (`palette`, `theme`, `help`, `copy`, `open_file`, `save_file`,
`quit`, `focus_next`/`focus_prev`, `focus_sidebar`) via `key_match` against the
active `KeyConfig`.

## Focus and layout

`App` tracks a two-way `Focus` enum (`Sidebar` / `Workspace`). The sidebar
width is responsive (`App::sidebar_width`):

- `< 60` cols: sidebar hidden entirely, workspace takes the full width
- `60..80` cols: sidebar shrinks to an icon-only rail
- `>= 80` cols: full sidebar with tool names

### Both panes scroll, and both remember how far

Vertically, neither pane assumes its content fits. The catalog outgrew an
ordinary terminal at 29 tools, and `web.ip` alone declares twelve fields.

- **Sidebar.** `ListState` is stored on the component (behind a `RefCell`,
  since `DrawableComponent::draw` only has `&self`) rather than rebuilt each
  frame. Ratatui nudges the offset just far enough to bring the selection into
  view, so a state that starts at offset 0 every frame re-derives the scroll
  from scratch and pins the selection to the bottom row for the whole lower
  half of the list.
- **Tool form.** `ToolFormComponent` scrolls by **whole widgets**, not rows: a
  field is a bordered box, and half a box sliced off at the top of the pane
  reads as a rendering bug. The error box and the status line are reserved out
  of the pane *before* the fields are laid out, since appending them after the
  last field pushed them off the bottom of exactly the tall forms that need
  them. When fields don't all fit, the status line says how many are off
  screen — counting any field squeezed below its `desired_height()`, because a
  field whose border is cut is not one the user can read.

This is the same rule as everywhere else in the layer: the UI must not decide
how many fields a tool is allowed to declare.

### Focus, and getting back out of a tall form

`Tab` walks fields and then hands control back to `App`, which flips the pane.
That is fine at three fields and tedious at twelve, so `focus_sidebar` (`Esc`)
jumps straight to the tool list from wherever focus is. It is a *set*, not a
toggle: pressing it on the sidebar leaves you there rather than bouncing into
the form.

`Esc` can be spent this way because every popup and the palette sit ahead of the
panes in the routing order and consume it first, and no field widget binds it —
so `App` only ever sees it when nothing is layered on top.

The command bar and the help popup deliberately disagree about what to show, and
`app_commands(force_all)` is where that is decided — the same `force_all`
convention `Component::commands` already uses:

- **Out of reach from this pane** (`copy`, `save file`, `tools`, `switch pane`) —
  the key works, you are just standing in the wrong place. The one-line bar hides
  it; the help popup lists it, because a key missing from the help screen is a
  key nobody finds.
- **Does nothing for this tool** (`open file` on a tool with no input) — hidden
  in both. A help entry whose only outcome is the error "this tool has no input"
  is worse than no entry.

## Internal event queue

Components don't mutate `App` state directly. Instead they push an
`InternalEvent` (`crates/lazytools/src/queue.rs`) onto a shared `Queue`
(cloneable, `Rc<RefCell<VecDeque<_>>>`-backed), and `App::process_queue` drains
it once per loop iteration, translating each event into state changes and a
`NeedsUpdate` bitflag. This decouples "a component decided something happened"
from "the app decided what to do about it" — e.g. `Sidebar` pushes
`SelectTool(id)`, and only `App` knows how to load that into `tool_form`.

The theme picker is the clearest case for the indirection. It pushes
`PreviewTheme(id)` on every cursor move and `ApplyTheme(id)` on confirm, and
never resolves a theme itself: what those ids turn into also depends on the
per-color overrides in `config.toml`, which belong to `Settings`. See
[configuration-and-state.md](configuration-and-state.md).

## Run loop and debounce

`crates/lazytools/src/tui.rs` runs a fixed 16ms poll loop
(`event::poll(TICK)`), rather than blocking indefinitely on `read()`. This is
what makes the input-changed debounce work: `App::tick()` is called every loop
iteration regardless of whether a key was pressed, so a `Live`-mode tool
reruns automatically ~80ms after the user stops typing, with no extra keypress
needed. Input larger than 256KB downgrades a `Live` tool to run-on-demand so
pasting a large file can't hang the UI.

An `OnDemand` tool never runs by itself — not on a keystroke, and not when the
tool is opened or a file is loaded into it. That matters on open: auto-running
`bcrypt` there would block the UI thread for ~200ms hashing an empty password
before the user had typed anything. Whenever a tool is effectively on-demand
(natively, or downgraded by large input), a badge under the fields names the run
key, so an empty output never looks broken.

## Two keys can run a tool, and why

`keys.run` (`Ctrl+R`) runs the tool from **any** field, read-only outputs
included. `keys.confirm` (`Enter`) also runs it, but only where that key isn't
already claimed by the focused widget — `ToolFormComponent::event` asks
`FieldWidget::wants_confirm_key()` before treating `Enter` as a run request, and
an editable multiline text field answers `true` because it needs `Enter` for
line breaks.

That indirection exists to fix a real collision, not as a generality. Because
`runnable` is derived from the *effective* mode, the 256KB downgrade used to
repurpose `Enter` from "newline" to "run" — inside the very multiline field the
user was still editing. It was reachable in every `Live` tool with a multiline
input (twelve of them) simply by opening a large file, since the open popup
allows up to `MAX_FILE_BYTES` (10MB), forty times the downgrade threshold.

Two consequences worth knowing:

- The badge and the `commands()` hint both name `keys.run`, never `confirm`.
  `Ctrl+R` is true from every field; "Enter" would be a lie the moment focus sat
  on a multiline input, and a hint that drifts from behavior is worse than none.
- `wants_confirm_key()` returns `self.multiline && !self.readonly`. The
  `!readonly` half is load-bearing: `web.json-diff` and `web.jwt-decode` declare
  multiline *outputs*, and `TextWidget::event` bails out early when readonly — a
  read-only field claiming the key would leave `Enter` doing nothing at all.

A `Generate` tool sits between the two: it auto-runs like `Live`, but the
confirm key *also* re-runs it, producing a new random value. Its badge reads
"regenerate" rather than "run". Two constraints follow from how the loop works
and are easy to trip over when adding a generator:

- `Enter` only fires a run request while focus is on an **editable** field, so a
  tool with no inputs *and* no options could never be re-triggered through it —
  it would generate once and be stuck. Every generator therefore declares at
  least one option. (`keys.run` has no such restriction, but the convention
  stands: it also keeps the form from being a single read-only box.)
- The 256KB downgrade never applies to `Generate`, because such a tool has only
  small options and can't reach the threshold. `is_downgraded()` deliberately
  still tests `mode == Live` only.

On Windows, crossterm reports both key-press and key-release events; only
`KeyEventKind::Press` is handled, to avoid double-processing every keystroke.

`ratatui::init()` installs a panic hook that restores the terminal before the
panic message prints, so a bug during rendering doesn't leave the user's shell
in raw mode.
