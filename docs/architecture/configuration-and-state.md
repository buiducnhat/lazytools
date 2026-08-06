# Configuration and state

Four files, in two directories that are deliberately not the same one.

| File | Directory | Written by | Purpose |
| ---- | --------- | ---------- | ------- |
| `keys.toml` | config | the user | key bindings |
| `config.toml` | config | the user | `[session]` and `[theme]` |
| `session.toml` | state | the app | last open tool and its values |
| `theme.toml` | state | the app | the theme picked with `Ctrl+T` |

Path resolution lives in `crates/lazytools/src/paths.rs`:

- **config** — `$XDG_CONFIG_HOME/lazytools`, else `~/.config/lazytools`
  (`%USERPROFILE%` stands in for `$HOME` on Windows). Both are *searched*, most
  preferred first: honoring `XDG_CONFIG_HOME` must not silently ignore a
  `~/.config/lazytools/keys.toml` that worked yesterday.
- **state** — `$XDG_STATE_HOME/lazytools`, else `~/.local/state/lazytools`.

The split is the point. A config directory is the kind of thing people put in a
dotfiles repository; a session file is one machine's cursor position, and it has
no business travelling with it. Deleting the whole state directory costs the
user nothing but which tool was open.

There is no `dirs` crate: two paths and one environment variable each is less
code than the dependency.

## The failure policy

**A broken config file never blocks startup.** The user has to be able to get
into the app to fix it, so every loader returns `(value, Option<Issue>)` and the
app opens with defaults plus a popup naming the file and the entries it skipped.
`App::from_user_config` collects the issues from every loader and shows them
together.

Recovery granularity follows the shape of the file:

- `keys.toml` is a flat map, so it recovers **per entry** — one unrecognized
  binding keeps its default and everything else applies.
- `config.toml`'s typed sections (`[session]`) use `deny_unknown_fields`, so a
  misspelled key is reported by name rather than silently doing nothing. Its
  `[theme]` section is a plain map and therefore recovers per entry, the same
  way `keys.toml` does. Only TOML that doesn't parse at all costs the whole
  file.
- `session.toml` and `theme.toml` report **nothing**, ever. Nobody wrote them
  by hand, so a corrupt or stale one is discarded and the app starts as if
  there had been no session and no pick. This is the one place where quiet
  fallback is the honest behavior.

## `[session]` — what carries over

```toml
[session]
restore = "options"   # "off" | "options" | "all"
```

`Restore` decides both what is written on quit and what is read on start:

- **`off`** — nothing is written, nothing is read, and any file left behind by
  an earlier setting is **deleted**. Switching persistence off has to mean the
  data is gone, not merely unread.
- **`options`** (default) — the last open tool and the values of its *options*.
- **`all`** — also the input fields.

`options` is the default because options and inputs are different kinds of
thing. An option is how you like a tool configured — which hash algorithm, which
case style. An input is the *data* you were working on, and in this catalog that
is routinely a JWT, an API token, or a payload someone pasted in to decode. A
utility keeping those on disk by default would be a surprise, and the kind of
surprise that only gets noticed after it matters.

Three rules hold in `session.rs` regardless of mode:

- **A `Secret` field is never written.** Not truncated, not hashed — excluded by
  `FieldKind`, so a tool that declares a new secret inherits the rule without
  anyone remembering to update this file. `crypto.hmac`'s key and
  `crypto.bcrypt`'s password are why it exists.
- **Outputs are never written.** They are derived from the inputs; restoring one
  risks showing a result that no longer matches the form above it.
- **A restored value must still be legal.** The file outlives the catalog that
  wrote it, so every value is re-validated against the spec it is going into: a
  `Select` whose option was removed, a `Number` outside a narrowed range, or a
  value whose type changed is dropped rather than forced in.

Values over 8KB are left out rather than truncated — `Ctrl+O` reads files up to
10MB into the primary input, and a session file is not a document store.

The file carries a `version`; a document written by another version is ignored
rather than migrated.

### When it is written

On the way out of `tui::run_with`, *after* the terminal is restored, so a write
failure can be reported on stderr rather than into a screen that no longer
exists. Not on every keystroke: the state worth keeping is the state you left.

Within a single run, switching tools still resets the form to its declared
defaults. Restoring across runs and restoring across a tool switch are different
promises, and making the second one too would leave no way to get a clean form
back without clearing every field by hand.

## `[theme]` — colors

```toml
[theme]
name = "dracula"       # one of the built-in themes
border_focus = "magenta"
title = "#ff8800"
text_dim = "244"
```

Two kinds of entry, and they compose in one direction: `name` picks a **base**,
every other key **corrects** it. `name = "nord"` with `error = "magenta"` is
Nord with one color changed, and that correction survives a later switch to a
different theme — which is why `Settings` keeps the overrides as a list rather
than folding them into the resolved theme once. `Settings::theme_for(id)` is
that composition, and the picker calls it on every keystroke.

The nine slots are the fields of `ui::Theme`: `background`, `border`,
`border_focus`, `text`, `text_dim`, `error`, `selection_fg`, `selection_bg`,
`title`. Three notations are accepted — a name (`cyan`, `dark-gray`, `reset`),
`#rrggbb`, or a `0`–`255` palette index.

`[theme]` stays a plain map rather than a typed struct so one bad color costs
one entry instead of the whole file; `name` is the single key in it that isn't
a color, and an unknown one is reported with the list of real ones.

### The built-in themes

`ui::themes::PRESETS` — `terminal` (the default), `dracula`, `nord`,
`gruvbox-dark`, `solarized-dark`, `catppuccin-mocha`, `tokyo-night`,
`one-dark`, `monokai`, `solarized-light`, `github-light`. Each is a straight
transcription of the project's published palette, and two rules hold, both
enforced by tests in `themes.rs`:

- **`terminal` names nothing absolute.** It is the sixteen ANSI colors, so it
  follows whatever the user's terminal already does — a hard-coded `#1e1e2e` is
  wrong the moment someone switches to a light background. Every other preset
  states exact colors and therefore stops following it.
- **A preset that paints a `background` must name every foreground too.** Half
  a theme — the terminal's own light text on an explicit dark background, or
  the reverse — is the failure mode that makes the app unusable rather than
  merely ugly.

`background` is the slot that made the presets possible at all. It is painted
once over the whole frame in `App::draw`, and again under each popup, because a
popup `Clear`s the cells beneath it and would otherwise punch a hole in it. The
default `Color::Reset` makes both a no-op.

### The picker, and why it previews

`Ctrl+T` opens `popups::theme::ThemePopup`. Moving the cursor re-themes the
**whole app behind the popup** rather than only a swatch: the question being
answered is "is this readable in the tool I actually use", and a five-cell
color chip cannot answer it. `Enter` keeps the theme, `Esc` puts back the one
that was in force when the picker opened.

This is what `SharedTheme` is for. It is an `Rc<ThemeHandle>` — a `Cell<Theme>`
— rather than an `Rc<Theme>`, so every component that was handed a clone at
startup sees the new colors on the next frame. A component given a copy of the
theme would keep drawing the old one.

The popup resolves nothing itself. It pushes `PreviewTheme` / `ApplyTheme` onto
the queue and `App` answers, because the theme that results also depends on the
per-color overrides above, which are settings rather than the picker's
business.

### `theme.toml` — where a pick goes, and what wins

The app must not write into a directory people hand-edit, so `Ctrl+T` cannot
put `name` back into `config.toml`. The pick lands in the state directory
instead, and that raises the only real question here: **which wins when both
name a theme?**

Newest — and it is decidable without a clock. `theme.toml` records the
`config.toml` theme that was in force at the moment of the pick:

```toml
name = "dracula"
from_config = "nord"
```

If `config.toml` still says `nord`, the pick was made against the config as it
stands, and the pick wins. If it now says something else — or nothing — the
config has been edited since, and the file the user actually typed in wins.
Deleting `theme.toml` hands control back to `config.toml` too. Neither can be
shadowed by a choice made months ago.

Written on the way out, next to the session, and **only if the picker was
used**: a run that never opened it writes no file at all, because creating one
would start shadowing config edits for someone who never chose a theme.
`[session] restore` governs `session.toml` only — a theme is a preference, not
session data, and `restore = "off"` does not switch it off.

## The clipboard, and why it is here

`clipboard.rs` has two backends and no configuration entry, because the right
one is a fact about the session rather than a preference:

- **Native** (`arboard`) — the clipboard of the machine the process runs on.
- **OSC 52** — an escape sequence asking the *terminal emulator* to set its own
  clipboard. Over SSH that is the machine the user is sitting at.

When `SSH_CONNECTION`/`SSH_CLIENT`/`SSH_TTY` is set, OSC 52 is tried first and
native second; otherwise the order is reversed. First rather than as a fallback:
a remote host may well have a working X clipboard, so "native succeeded" over
SSH means the text landed somewhere the user cannot paste from. The flash
message names which backend took it.

Two refusals, both because OSC 52 has no reply and therefore no way to detect a
failed copy:

- Text over 64KB is refused with the limit stated, rather than handed to a
  terminal that may silently keep half of it.
- Under GNU screen (`TERM=screen*` with no `TMUX`) the sequence is refused
  outright: screen has no passthrough a program can switch on for itself, so the
  bytes would vanish and the app would flash "copied" over nothing. Under tmux
  the sequence is DCS-wrapped instead, which works when `allow-passthrough` is
  on.
