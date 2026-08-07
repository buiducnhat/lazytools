# Scope and roadmap

Source: MVP implementation plan
(`docs/.plans/archived/260731-1641-lazytools-mvp/`, Vietnamese, archived — this
file summarizes its scope decisions in English for ongoing reference).

## Delivered in the MVP

- Two-crate Cargo workspace: `lazytools-core` (spec/logic library) +
  `lazytools` (TUI + CLI binary).
- Spec layer: `ToolSpec`, `Field`, `FieldKind`, `Value`, `Inputs`, `Outputs`,
  `ToolError`, `Registry`, the `Tool` trait.
- CLI generated entirely from the registry, with stdin/pipe support.
- TUI: category-grouped sidebar, a generic `ToolFormComponent`, a fuzzy
  `Ctrl+P` palette, an auto-generated help popup, an auto-generated command
  bar, clipboard copy.
- A hand-written `TextArea` widget (~250 lines) — Unicode grapheme-aware,
  bracketed-paste-aware.
- Full key configuration: a centralized `KeyConfig`, overridable via
  `~/.config/lazytools/keys.toml`.
- Open/save file popups for the convert-oriented tools.
- **8 tools**: `crypto.hash`, `crypto.hmac`, `crypto.bcrypt`,
  `convert.base64`, `convert.url`, `convert.hex`, `convert.json-format`,
  `convert.data-format`.
- GitHub Actions CI across macOS, Linux, and Windows.

## MVP follow-ups closed in v0.1.0

The MVP execution report left two acceptance items written but never actually
executed, because the repository had no remote and the behaviors involved
cannot be reached headlessly. Both were run for real during the v0.1.0
release, and both are now verified rather than assumed.

- **Follow-up #1 — three-platform CI, proven.** The workflow had existed since
  the MVP without ever running. First execution was green on `ubuntu-latest`,
  `macos-latest`, and `windows-latest`, reporting an identical **86 passed** on
  every platform. The anticipated friction point — `arboard` on headless
  Windows — did not materialize, as the report predicted it might not, since no
  test touches the clipboard.
- **Follow-up #2 — manual terminal QA, run by a human.** All four behaviors
  were exercised in a real terminal: bracketed paste, `y` copying to the
  *system* clipboard, clean terminal restoration after `q`, and an `OnDemand`
  tool opening instantly without auto-running.

  **This gate earned its place: it caught a real defect.** Pasting a block whose
  lines ended in CR (rather than LF) silently destroyed every line break,
  because `TextArea::insert_str` stripped `\r` instead of treating it as a line
  break. Every headless test used `\n` exclusively, so nothing caught it. Fixed
  before release, with three regression tests covering CR, CRLF, and the
  single-line flattening case — raising the suite from 86 to 89.

## Distribution

`v0.1.0` ships through three channels, all produced from one tag by
[dist](https://github.com/axodotdev/cargo-dist):

- **GitHub Releases** — prebuilt archives for five targets
  (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`,
  `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`), with checksums, plus
  shell and PowerShell installer scripts.
- **Homebrew** — `brew install buiducnhat/tap/lazytools`, with the formula
  pushed automatically to `buiducnhat/homebrew-tap` on each release.
- **crates.io** — `cargo install lazytools`.

See [releasing.md](../code-standard/releasing.md) for how a release is cut and
why the publish order matters.

## Delivered in v0.2 — catalog expansion

The v0.2 line fills the three categories `Category` had declared but never
populated (`Generate`, `Text`, `Web`), in three batches of increasing cost. All
three shipped under the single **`v0.2.0`** tag; the batch numbers below are
planning units, not releases.

**Batch 1 — 5 zero-dependency tools, 8 → 13.**

- `web.jwt-decode` — decode a JWT, optionally verify its HMAC signature.
  Deliberately does **not** check `exp`/`nbf`: that needs a clock.
- `convert.number-base` — one input, four fixed outputs (binary / octal /
  decimal / hexadecimal) rather than a `to` option, so the TUI shows all four
  bases at once.
- `convert.unicode` — escape/unescape via `\uXXXX`, `U+XXXX`, or `&#NNNN;`,
  with JavaScript-style surrogate pairs for astral characters.
- `text.case` — camel / pascal / snake / kebab / constant / title / lower /
  upper.
- `text.stats` — grapheme, code point, word, line, and byte counts.

This batch doubled as a machine-checkable test of the central design thesis in
[product-goals.md](product-goals.md): it added five tools with an **empty**
`git diff` over `crates/lazytools/src/`. The cost of the 13th tool really is
the cost of the 4th.

**Batch 2 — the `Generate` category, 13 → 18.**

`generate.password`, `generate.uuid`, `generate.ulid`, `generate.token`,
`generate.lorem`. These are the first tools with **no inputs** and the first
whose `run()` is not pure, which is why the architectural groundwork landed
first (see `RunMode::Generate` in
[spec-driven-tools.md](../architecture/spec-driven-tools.md)):

- A third `RunMode` — `Generate` — so a generator runs on open *and* re-runs on
  the confirm key. `Live` offers no way to ask for a different value; `OnDemand`
  opens blank.
- A fix for `set_primary_input`, which wrote into `widgets.first_mut()`
  unconditionally and so dumped file contents into the first *option* of any
  tool without inputs. "Open file" is now also hidden from the command bar for
  those tools rather than being advertised and doing nothing.

`generate.ulid` uses `ulid::Generator` rather than bare `Ulid::generate()`: the
latter only orders by millisecond, so a batch generated inside one millisecond
comes out visibly unsorted — unacceptable for a tool that sells sortability.

**Batch 3 — the `Web` category, 18 → 22.**

`web.timestamp`, `web.cron`, `web.url-parse`, `web.json-diff`. With these the
roadmap's v0.2 commitment is complete: all five categories declared in
`spec::Category` are populated.

The interesting decisions here were about honesty and about not contradicting
existing tools:

- **One date library, not two.** `cron 0.17` depends on `chrono`, so using the
  more modern `jiff` for `web.timestamp` would have shipped *both*. `cargo tree`
  settled it before any code was written: chrono for both tools.
- **`web.cron` normalizes field counts.** The `cron` crate parses 7 fields
  (seconds first, year last); a crontab line has 5. Both forms — and 6 — are
  accepted and normalized before parsing.
- **`web.cron` describes rather than guesses.** Common field shapes (`*`, `*/n`,
  a literal, a list, a range) get a plain-English sentence; anything else falls
  back to listing the fields verbatim. `minute: */15, hour: *` is more useful
  than a confidently wrong sentence.
- **`web.json-diff` sorts keys; `convert.json-format` must not.** These are
  deliberately opposite, and both files carry a comment saying so. A formatter
  that reorders keys is broken; a differ that reports a key swap as a change is
  useless. Array order stays significant in both.

## The v0.2.x line — interaction debt

With every declared category populated, the catalog stopped being the
constraint. What was left was *interaction debt*, and it splits in two:

- **Constraints leaking into tool specs.** The UI/CLI layer was forcing tools to
  declare fields dishonestly, or forbidding field combinations outright. These
  matter more than they look: [product-goals.md](product-goals.md) says a tool
  needing UI changes is a design smell to be fixed at the abstraction, and this
  is the same smell pointing the other way — the abstraction bending the tool.
- **Additive comfort and portability.** Themes, session persistence, an OSC52
  clipboard fallback for SSH.

The leaks are being paid down first, each as its own patch release.

### Delivered in `v0.2.1` — `Toggle` options can default to `true`

`cli::apply_kind` mapped `Toggle` to `ArgAction::SetTrue` and carried a
`debug_assert!` forbidding a `true` default outright, because `SetTrue` can only
ever report "absent" — a field declaring `true` would have silently arrived as
`false`, and there was no way to switch it back off.

Every `Toggle` option now also generates a `--no-x` twin, the two declared
`overrides_with` each other so passing both resolves POSIX-style (last one wins)
instead of erroring. `cli::toggle_value` resolves the pair against the declared
default. The twin is generated for *every* toggle rather than only those
defaulting to `true`: a symmetric `--help` reads better than one where the
negation appears only sometimes, and `--no-x` against a `false` default is
merely explicit.

Two decisions worth recording:

- **`generate.password` keeps `--charset`.** Reverting it to three real
  `Toggle`s was the obvious demonstration that the fix works — and it would have
  removed `--charset` from a shipped CLI inside a *patch* release. The tool's
  spec change waits for a version bump; the comment in `password.rs` now says
  the workaround is compatibility, not capability.
- **The invariant is pinned against a synthetic spec.** No shipped tool declares
  a `true`-defaulting toggle yet, so `cli::tests` builds a throwaway `ToolSpec`
  with one and asserts the resolution through `build_command` +
  `collect_inputs`. End-to-end coverage rides on `convert.base64`'s `url_safe`,
  the catalog's only real toggle.

### Delivered in `v0.2.2` — `Enter` belongs to the field that needs it

This item was on the deferred list as an *ambiguity*: "no shipped tool triggers
the conflict, since no `OnDemand`/`Generate` tool has a multiline field." That
reasoning was wrong, and this file asserted it twice — the second time, during
the `v0.2.1` release, in a strengthened form. It is a **reachable bug**, and the
mistake is worth recording because of the shape of it.

The claim was true of a tool's *declared* mode. But `effective_mode()` downgrades
`Live` → `OnDemand` above 256KB, and `event()` derives `runnable` from the
effective mode — so the downgrade also flipped `Enter` from "insert newline" to
"run tool", inside the multiline field the user was still editing. Reachable in
the **twelve** `Live` tools with a multiline input, by exactly the workflow the
open-file popup exists for: `MAX_FILE_BYTES` is 10MB, forty times the threshold.

The existing test made it worse. `large_input_downgrades_to_on_demand` pressed
`Enter` on that field and asserted the tool ran, commented *"Only pressing Enter
runs it"* — so the suite appeared to bless the behavior. In fact the test never
verified its own claim: `set_tool` queues a run on open while the input is still
empty, and the test pasted before any `tick()`, so the stale deadline fired and
the digest was already on screen before `Enter` was pressed. It passed both
before and after the fix, for reasons unrelated to `Enter`.

The fix gives the focused widget first refusal:

- `FieldWidget::wants_confirm_key()` — defaulted `false`, overridden only by
  `TextWidget` as `multiline && !readonly`. The `!readonly` half matters:
  `web.json-diff` and `web.jwt-decode` have multiline *outputs*, whose `event()`
  returns early, so a read-only claim would leave `Enter` inert.
- A new `keys.run` (`Ctrl+R`, overridable) runs the tool from any field,
  outputs included — something `Enter` never could, since it required an
  editable field.
- `Enter` still runs the tool wherever it isn't spoken for, so `crypto.bcrypt`
  and the generators behave exactly as before.

This also lifts the ban it was blamed for: an `OnDemand` or `Generate` tool may
now declare a multiline field.

Two verification notes:

- **Zero snapshot churn**, against an expectation of ~6. That figure applies to
  adding a *tool* (the sidebar shifts in every layout snapshot); this added
  none, and the run hint is gated on `runnable`, which is off for the `Live`
  tools every snapshot renders.
- The rewritten `large_input_downgrades_to_on_demand` was checked against the
  unfixed code and **fails** there, so it is a real regression test rather than
  one that merely passes.

## v0.3 — the second catalog expansion, 22 → 29

The v0.2 line filled every category `spec::Category` had declared. This batch is
about *depth* rather than coverage: the seven jobs a terminal utility belt gets
asked for that the catalog still had no answer to.

- `convert.color` — hex / `rgb()` / `hsl()` in, hex + RGB + HSL + HSV + CMYK out.
- `convert.html-entity` — escape for HTML, or decode entities back.
- `text.lines` — sort, deduplicate, trim, drop empties, number.
- `text.diff` — the general-purpose sibling of `web.json-diff`, at line, word,
  or character granularity.
- `text.regex` — pattern, flags, every match with its capture groups, and a
  substitution.
- `web.ip` — a CIDR block broken into network, mask, range, and host counts,
  for both address families.
- `crypto.totp` — an RFC 6238 code from a base32 secret.

One new dependency: `regex`, with default features off. Everything else reuses
what was already in the tree — `similar` (from `web.json-diff`), `chrono` (from
`web.timestamp`), `hmac`/`sha1`/`sha2` (from `crypto.hmac`), and `std::net`.

### Decisions worth recording

- **Base32 is ~20 hand-written lines, not a dependency.** `crypto.totp` needs
  decode only, and the secret printed beside a QR code is the only base32 this
  program will ever see. `data-encoding` would have been a whole crate for one
  loop.
- **`hotp()` takes a counter, not a clock.** The tool reads `Utc::now()`; the
  function under it does not. That is what makes the RFC 6238 vectors assertable
  — a tool whose only entry point reads "now" is one whose correctness can't be
  tested. The secret is also the tool's *primary input*, so it arrives over a
  pipe rather than sitting in `ps`-visible argv.
- **`text.lines` is the first tool with a `true`-defaulting `Toggle`.** `v0.2.1`
  built the `--no-x` machinery and could only pin it against a synthetic spec,
  because no shipped tool used it. `--no-trim` is now the real end-to-end case.
- **`convert.color` renders HSL at whole degrees and percents**, which is what
  CSS is written in and is therefore lossy. The exactness test targets the
  conversion itself at full precision; a separate test bounds the *rendered*
  round trip at ±3 of an 8-bit channel, which is the arithmetic of the rounding
  rather than a tolerance picked to make a test pass.
- **`text.regex` caps the match *listing*, not the search.** `count` is always
  the true total. A `.` against a file opened through `Ctrl+O` (`MAX_FILE_BYTES`
  is 10MB) would otherwise render a listing far larger than the input.
- **`convert.html-entity` passes unknown entities through verbatim.** A decoder
  that mangled `AT&T` would corrupt exactly the text people paste in to check.
  The scan for a terminating `;` is capped so a stray `&` can't swallow the
  document.
- **`web.ip` reports IPv4 host conventions and IPv6's absence of them.** The
  network and broadcast addresses aren't assignable — except in a `/31`
  (RFC 3021) and a `/32`, where they are. IPv6 has no broadcast address and no
  such carve-out, and the tool says so rather than inventing one.

### The interaction debt this batch surfaced

Two layout assumptions held only because the catalog was small, and both broke
the moment it wasn't. Each was fixed at the abstraction rather than by trimming
a tool's spec, and each carries a test verified to **fail** against the unfixed
code:

- **The sidebar rebuilt its `ListState` every frame.** At 29 tools the list is
  taller than a 30-row terminal for the first time. Ratatui only nudges the
  offset far enough to reveal the selection, so re-deriving it from zero each
  frame pinned the selection to the bottom row for the whole lower half of the
  catalog. The state now lives on the component.
- **The tool form did not scroll at all.** It drew top-down and stopped at the
  bottom of the pane, so `web.ip`'s twelve fields put everything past the fold
  out of reach — the UI dictating how many fields a tool may declare, which
  [product-goals.md](product-goals.md) names as the smell to fix at the
  abstraction. The form now scrolls by whole widgets, reserves the error box and
  status line up front, and reports how many fields are off screen.

Snapshot churn: the six layout snapshots shifted for the sidebar (expected when
adding tools), and `layout_tiny_50_cols` — a deliberately degenerate 50×16 — now
shows `↕ 1 more field(s)` where it used to show a `Digest` box with two border
rows and no content. That is the honest rendering of the same situation.

### Two bindings changed, both for the same reason

Neither is a new capability; both are keys that were doing the wrong thing once a
form got big enough to notice.

- **`Esc` jumps to the tool list** (`focus_sidebar`). `Tab` walks fields and only
  reaches the sidebar after the last one — twelve presses in `web.ip`.
- **Quit moved from `q` to `Ctrl+Q`.** Routing gives the focused widget the key
  first, so a text field swallowed a bare `q` as a character — but a `Select`,
  `Toggle`, `Number`, or read-only output does not, and `q` fell straight
  through to "quit". The same keystroke typed a letter one field away and ended
  the session here. `Ctrl+Q` is XON/XOFF on a cooked terminal; raw mode clears
  `IXON`, which is the same reason `save_file` can already be `Ctrl+S`.

Both are `KeyConfig` entries, so `quit = "q"` in `keys.toml` restores the old
behavior for anyone who wants it.

## v0.4 — the rest of the interaction debt

v0.2.x named two kinds of debt and paid down only the first (constraints leaking
into tool specs). This line closes the second: *additive comfort and
portability* — the three items the previous roadmap left on the deferred list.
None of them adds a tool; the catalog stays at 29.

- **An OSC 52 clipboard fallback**, so `y` works over SSH.
- **Cross-session persistence** — the last open tool and its values.
- **A configurable theme.**

The last two share one new file, `~/.config/lazytools/config.toml`, with a
`[session]` and a `[theme]` section. `keys.toml` keeps its own file: it shipped
in the MVP, and moving it would have broken every existing install to save one
inode. Mechanics are in
[configuration-and-state.md](../architecture/configuration-and-state.md); what
follows is why the choices are what they are.

### The clipboard chooses by session, not by preference

Two backends — `arboard` (the machine the process runs on) and OSC 52 (the
terminal emulator, which over SSH is the machine the user is sitting at). Which
one is *correct* is a fact about where the terminal is, so it isn't a setting.

Over SSH, OSC 52 is tried **first**, not as a fallback. Falling back only on
failure sounds safer and is wrong: a remote host may have a perfectly working X
clipboard, so `arboard` would succeed and put the text somewhere the user cannot
paste from. Success that lands in the wrong place is worse than an error.

The two refusals are both consequences of the same property — OSC 52 has no
reply, so a failed copy is undetectable:

- **Over 64KB is refused with the limit stated.** Terminal limits vary and none
  of them reports truncation, and a clipboard silently holding half a document
  is worse than a copy that says it didn't happen.
- **GNU screen is refused outright.** Unlike tmux, screen has no passthrough a
  program can switch on for itself, so the bytes would vanish while the app
  flashed "copied". Under tmux the sequence is DCS-wrapped and works with
  `allow-passthrough` on.

The flash names the backend that took the text, because the two put it in
different places and only the user knows which they wanted.

### Persistence keeps options by default, not inputs

The deferred item read "preserve input values between runs", and the shipped
default deliberately does not. `restore` has three modes — `off`, `options`
(default), `all` — because options and inputs are different kinds of thing. An
option is how you like a tool configured; an input is the data you were working
on, and in *this* catalog that is routinely a JWT, an API token, or a payload
pasted in to decode. `all` is one line of config away for anyone who wants it;
the reverse default would be a surprise noticed only after it mattered.

`off` **deletes** any file an earlier setting left behind rather than just
ignoring it. Turning persistence off has to mean the data is gone.

Three rules hold in every mode, and each is a test rather than a comment:

- **A `Secret` is never written** — enforced by `FieldKind`, so a tool declaring
  a new secret inherits it. The end-to-end test types into `crypto.hmac`'s key
  field in the *most* permissive mode and asserts the string is absent from the
  bytes on disk.
- **Outputs are never written.** They are derived; a restored one could
  contradict the form above it.
- **A restored value is re-validated against the spec it goes into.** A session
  file outlives the catalog that wrote it, so a removed `Select` option, a
  narrowed `Number` range, or a changed type is dropped rather than forced in.

Writing happens on the way out, *after* the terminal is restored, so a failure
can be reported on stderr instead of into a screen that no longer exists.

Within a run, switching tools still resets the form to its defaults. Restoring
across runs and restoring across a tool switch are different promises, and
making the second one too would leave no way to get a clean form back.

### The bug this line surfaced

`InternalEvent::SelectTool` rebuilt the form without moving the sidebar
highlight. Picking a tool from the `Ctrl+P` palette therefore left the list
pointing at one tool while the form showed another — reachable since the palette
shipped in the MVP, and invisible in tests because `TestBackend::to_string()`
throws styles away and the highlight *is* a style. The regression test reads the
selection off the buffer's background colors instead, and was verified to fail
against the unfixed code.

Session restore needed the same `Sidebar::select_tool`, which is how the bug
turned up at all.

### Theme colors are named by default on purpose

Eight slots, three notations: a name (`cyan`, `dark-gray`, `reset`), `#rrggbb`,
or a `0`–`255` palette index. Named colors remain the defaults because they
follow the user's own terminal theme — a hard-coded `#1e1e2e` is wrong the
moment someone switches to a light background. `[theme]` is a plain map rather
than a typed struct so one bad color costs one entry instead of the whole file.

Snapshot churn: **none**. Every snapshot renders the default theme, and this
line changed no layout, no text, and no key binding.

## v0.5 — the theme picker, and the third catalog expansion, 29 → 36

Two threads, and the first one reverses a decision this file used to state as
settled (see "Explicitly out of scope" below): themes *do* get presets and a
picker. What changed is not the reasoning but the evidence — v0.4 shipped nine
configurable colors and, in doing so, made it obvious that nobody hand-writes
nine hex values to try Nord. The cost was in the wrong place: the feature
existed, and reaching it required work only its author would do.

### The picker previews, and that is the whole design

`Ctrl+T` opens a list of eleven themes and re-themes **the entire app behind
the popup** as the cursor moves. `Enter` keeps the theme, `Esc` restores the
one in force when it opened. Mechanics are in
[configuration-and-state.md](../architecture/configuration-and-state.md); three
decisions are worth recording here.

- **A swatch cannot answer the question being asked.** "Is this readable" is
  about the tool you actually use, at your terminal's font and contrast, not
  about five colored cells. Previewing live costs one `Cell<Theme>`; a swatch
  would have been cheaper and would not have answered anything.
- **`SharedTheme` became `Rc<ThemeHandle>`.** The v0.4 comment in `app.rs` read
  "a theme is read on each draw and never changes while the app runs", and
  every component was handed an `Rc<Theme>` on that basis. Live preview makes
  the second half false, so the shared value is now a `Cell<Theme>` and one
  write re-themes every holder. A `Cell` rather than a `RefCell` because
  `Theme` is `Copy`: reading during a draw takes no borrow and cannot panic
  re-entrantly.
- **The ninth slot, `background`, is what made presets possible.** Eight
  foreground colors on the terminal's own background is a color scheme, not a
  theme — Dracula's palette on a white terminal is not Dracula. It is painted
  once over the frame in `App::draw`, and again under each popup, since a popup
  `Clear`s the cells beneath it. The default `Color::Reset` makes both a no-op,
  which is why nothing else in the draw path had to learn about it.

### Where a pick is stored, and the conflict that had to be resolved

`paths.rs` states the rule the app must not break: config is what a *person*
writes, state is what the *app* writes. So `Ctrl+T` cannot write `[theme] name`
back into `config.toml`, and the pick goes to `~/.local/state/lazytools/theme.toml`.

That creates one genuinely new question: both files can now name a theme. The
answer is "whichever is newer", and the interesting part is that it needs no
clock. `theme.toml` records the *config* theme in force at the moment of the
pick; if `config.toml` still says that, the pick is current and wins, and if it
says anything else, the config has been edited since and wins instead. Editing
the config or deleting the state file both hand control back, which is the
behavior a user would expect from either action without being told.

A run that never opens the picker writes **no file at all**. Creating one
"just in case" would silently start shadowing config edits for someone who
never chose a theme — the cost of the wrong default here is invisible, which is
exactly why it is worth spelling out.

`[session] restore = "off"` does not switch the theme off. Persistence modes
are about the data you were working on; the theme is a preference, and it lives
in its own file for that reason.

### The catalog: seven tools, in the same one-file-one-line shape

`convert.base32`, `convert.byte-size`, `convert.duration`, `text.slug`,
`text.escape`, `web.jwt-encode`, `web.http-status`. No new dependencies: base32
is the ~40 lines `crypto.totp` already had, `regex::escape` was already in the
tree, and `web.jwt-encode` reuses `hmac`/`sha2`/`base64` from its decoding
counterpart.

- **`crypto.totp`'s private base32 decoder moved into `convert.base32`.** Its
  comment said "~20 lines used by exactly one tool", which was a fair reason to
  hand-write it and stopped being true the moment a second tool needed the same
  alphabet. One codec now, with the RFC 4648 vectors asserted against it.
- **`web.jwt-encode` is checked against its own decoder, not only a vector.**
  The published jwt.io token pins the header spelling and the compact payload;
  a second test signs with each algorithm and asserts `web.jwt-decode` reports
  "valid signature", because two tools disagreeing about one token is the
  failure this pair can actually have.
- **`text.escape` refuses to unescape `\d`.** Stripping the backslash from a
  character class would silently turn a digit class into the letter `d` — a
  changed meaning, not a formatting difference, so it is an error.
- **`convert.byte-size` reads a bare `M` as 1024, and says so.** `ls -h`,
  `du -h`, and `dd bs=1M` all mean binary by it; `MB` is the only spelling that
  asks for 1000. Both scales are always shown, since the gap between them is
  what people open the tool to settle.
- **`convert.duration` refuses `1h30`.** It reads as 30 minutes to some people
  and 30 seconds to others, and a tool that picks one silently is worse than
  one that asks for `1h30m`.
- **`web.http-status` reports the class for an unassigned code in range.** 499
  is not registered; inventing a reason phrase for it would be a confident lie,
  and "treat it as the generic status of its class" is what a client actually
  does.

Snapshot churn: the expected six — the sidebar shifts in every layout snapshot
when tools are added, and the command bar and help popup gained `^T theme`.

### Delivered — mouse parity

On the v0.5 branch: click and scroll work everywhere a key does. Click the
sidebar to select a tool; click a form field to focus it; click a popup's list
row to activate it; scroll to move selections; click outside a popup to dismiss
it. The implementation is hardcoded (no `[mouse]` config block) and lives
entirely in the existing component `event()` arms — no new trait methods, no
new dependency. `MouseEventKind::Moved` is filtered out in the run loop to
avoid wasted redraws.

Known limitations: click-to-position cursor inside text/text-area fields is not
implemented (focus only). Double-click is only recognized in the Theme popup
(single click = preview, double-click = apply). Right-click and drag are
ignored.

## Explicitly out of scope

Still deferred:

- **`exp` / `nbf` validation in `web.jwt-decode`** — deliberately omitted so the
  tool stays a pure function of its input; decoding and expiry-checking are
  different jobs.
- **A theme *editor*, and live config reload.** The presets and the picker
  shipped in v0.5 — this item used to bundle all three, and the bundling was
  the mistake: choosing between eleven themes is a one-keystroke job, while
  editing individual colors is rare enough that `config.toml` plus a restart of
  a program that opens instantly is not a burden worth building a UI to avoid.
  `config.toml` is still read once at startup.
- **Restoring a form when switching tools within one run** — see above; this is
  a deliberate non-goal, not an unfinished one.
- Image conversion, document conversion, any tool requiring network access, or a
  plugin runtime.

## Reading this alongside the archived plan

The archived plan
(`docs/.plans/archived/260731-1641-lazytools-mvp/SUMMARY.md`) additionally
records a phase-by-phase execution log (P0 core foundation through P5 file
I/O + CI) with concrete verification evidence at each step (test counts,
`grep`-verified absence of hard-coded tool names, `cargo tree` checks that
`lazytools-core` never pulls in `ratatui`/`crossterm`/`clap`). That log is
kept as the historical record of *how* the MVP was verified; this file exists
so the scope decisions are readable without needing to read Vietnamese.
