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

## Explicitly out of scope (deferred to a future v0.2)

- **JWT decode/verify** — deferred.
- **Cross-session persistence** — the app does not remember which tool was
  open or preserve input values between runs.
- The **Generate** category (password, UUID, ULID, lorem ipsum generators).
- The **Web/Dev** category (URL parser, cron expression explainer, JSON
  diff, timestamp conversion).
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
