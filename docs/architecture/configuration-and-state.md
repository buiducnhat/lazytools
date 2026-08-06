# Configuration and state

Three files, in two directories that are deliberately not the same one.

| File | Directory | Written by | Purpose |
| ---- | --------- | ---------- | ------- |
| `keys.toml` | config | the user | key bindings |
| `config.toml` | config | the user | `[session]` and `[theme]` |
| `session.toml` | state | the app | last open tool and its values |

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
- `session.toml` reports **nothing**, ever. Nobody wrote it by hand, so a
  corrupt or stale one is discarded and the app starts as if there had been no
  session. This is the one place where quiet fallback is the honest behavior.

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
border_focus = "magenta"
title = "#ff8800"
text_dim = "244"
```

The eight slots are the fields of `ui::Theme`: `border`, `border_focus`, `text`,
`text_dim`, `error`, `selection_fg`, `selection_bg`, `title`. Three notations
are accepted — a name (`cyan`, `dark-gray`, `reset`), `#rrggbb`, or a `0`–`255`
palette index. Named colors stay the default because they follow the terminal's
own theme; a hard-coded `#1e1e2e` looks wrong the moment someone switches to a
light background.

The theme is resolved once at startup and shared as an `Rc<Theme>` by every
component. There is no theme editor and no live reload — see
[scope-and-roadmap.md](../project-pdr/scope-and-roadmap.md).

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
