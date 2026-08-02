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
populated (`Generate`, `Text`, `Web`), in three batches of increasing cost.

**Batch 1 (`v0.1.1`) — 5 zero-dependency tools, 8 → 13.**

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

**Batch 2 (`v0.2.0`) — the `Generate` category, 13 → 18.**

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

**Batch 3 (`v0.3.0`) — the `Web` category, 18 → 22.**

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

## Explicitly out of scope

Still deferred:

- **Cross-session persistence** — the app does not remember which tool was
  open or preserve input values between runs. Planned for the `v0.2.x` line;
  note that `FieldKind::Secret` values (`crypto.hmac`'s key,
  `crypto.bcrypt`'s password) must never be written to disk.
- **`exp` / `nbf` validation in `web.jwt-decode`** — deliberately omitted so the
  tool stays a pure function of its input; decoding and expiry-checking are
  different jobs.
- **OSC52 clipboard fallback for SSH sessions** — next up in the `v0.2.x` line.
- Image conversion, document conversion, any tool requiring network access,
  a plugin runtime, or a theme editor.
- OSC52 clipboard fallback for SSH sessions.

## Reading this alongside the archived plan

The archived plan
(`docs/.plans/archived/260731-1641-lazytools-mvp/SUMMARY.md`) additionally
records a phase-by-phase execution log (P0 core foundation through P5 file
I/O + CI) with concrete verification evidence at each step (test counts,
`grep`-verified absence of hard-coded tool names, `cargo tree` checks that
`lazytools-core` never pulls in `ratatui`/`crossterm`/`clap`). That log is
kept as the historical record of *how* the MVP was verified; this file exists
so the scope decisions are readable without needing to read Vietnamese.
