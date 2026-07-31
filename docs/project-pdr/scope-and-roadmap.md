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
