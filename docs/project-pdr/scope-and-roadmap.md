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

## Explicitly out of scope

Now that the v0.2 catalog work is delivered, what remains deferred is:

- **Cross-session persistence** — the app does not remember which tool was
  open or preserve input values between runs.
- **`exp` / `nbf` validation in `web.jwt-decode`** — deliberately omitted so the
  tool stays a pure function of its input; decoding and expiry-checking are
  different jobs.
- **`Toggle` options defaulting to `true` in the CLI** — `cli::apply_kind` maps
  `Toggle` to `ArgAction::SetTrue`, which cannot express `--no-x`. Tools work
  around it with a `Select` (see `generate.password`); lifting the limit is a
  CLI-layer project of its own.
- **`Enter` in a multiline field vs. `Enter` as the run key** — no shipped tool
  triggers the conflict (no `OnDemand`/`Generate` tool has a multiline field),
  but the ambiguity is real and unresolved.
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
