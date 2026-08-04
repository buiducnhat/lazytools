# Documentation Summary

**lazytools** — an offline, keyboard-first terminal utility belt: a TUI plus a
set of CLI subcommands sharing one spec-driven tool registry, built in Rust
(edition 2024) with `ratatui` + `clap` on top of a UI-framework-free core crate.
The catalog holds **29 tools** across five categories (Crypto, Convert,
Generate, Text, Web), each declared in a single file and registered with a
single line.

## Agent Context Guide

Before planning or implementing, read this `docs/SUMMARY.md` file first. Load only the detail docs relevant to the current task, and prioritize `Code Standard` docs for implementation conventions. If docs conflict with code or user intent, use the available question tool before making broad changes.

## Architecture

System design, component interactions, data flows, deployment, and external integrations.

| File | Description |
| ---- | ------------ |
| [spec-driven-tools.md](architecture/spec-driven-tools.md) | How `ToolSpec` + the `Tool` trait let the CLI and TUI both generate themselves from one declaration per tool; the three `RunMode`s and the two deliberate exceptions to `run()` purity |
| [tui-event-loop.md](architecture/tui-event-loop.md) | `Component`/`DrawableComponent` pattern, event routing order, focus/layout, the internal event queue, and the 16ms poll/debounce loop |
| [file-io.md](architecture/file-io.md) | Open/save file popups: size limits, overwrite confirmation, missing-parent-directory handling, and why tools with no inputs refuse "open file" |

## Codebase

Directory structure, entry points, API patterns, and key modules.

| File | Description |
| ---- | ------------ |
| [directory-layout.md](codebase/directory-layout.md) | Full workspace tree (`lazytools-core` vs `lazytools`), the per-category `tools/` layout, entry point (`main.rs`), and why app logic lives in `lib.rs` |

## Code Standard

Conventions, naming rules, tech stack versions, and development workflows.

| File | Description |
| ---- | ------------ |
| [conventions.md](code-standard/conventions.md) | Language (English-only code/comments/UI text), edition/toolchain, crate boundary rule, error-handling conventions, naming |
| [adding-a-tool.md](code-standard/adding-a-tool.md) | Step-by-step for adding a new tool, `ToolSpec` content conventions, the extra rules for generators, and the invariants CI enforces |
| [testing-conventions.md](code-standard/testing-conventions.md) | The four test layers (tool unit tests, spec invariants, CLI end-to-end, TUI/snapshot), how to test tools that aren't pure, and why adding a tool is expected to break six snapshots |
| [releasing.md](code-standard/releasing.md) | How a release is cut: dist config, the tag-driven pipeline, crates.io publish ordering, and why crates.io goes last |

## Project PDR

Product goals, use cases, business rules, and constraints.

| File | Description |
| ---- | ------------ |
| [product-goals.md](project-pdr/product-goals.md) | Central design thesis (open tool catalog, cost-of-addition must stay flat) and the two times it was measured, pipeline-first CLI contract, reliability expectations |
| [scope-and-roadmap.md](project-pdr/scope-and-roadmap.md) | What shipped in the MVP, the three v0.2 catalog batches and their design decisions, the v0.2.x interaction-debt line, and what remains explicitly out of scope |

## Other

| File | Description |
| ---- | ------------ |
| [.brainstorms/260731-1635-lazytools-tui-architecture/](.brainstorms/260731-1635-lazytools-tui-architecture/SUMMARY.md) | Pre-implementation brainstorm (Vietnamese, archived): gitui research, architecture design, dependency verification |
| [.plans/archived/260731-1641-lazytools-mvp/](.plans/archived/260731-1641-lazytools-mvp/SUMMARY.md) | Full MVP implementation plan and phase-by-phase execution log (Vietnamese, archived) |
| [.plans/archived/260802-0012-lazytools-v02-catalog/](.plans/archived/260802-0012-lazytools-v02-catalog/SUMMARY.md) | v0.2 catalog expansion 8 → 22 tools, plus `RunMode::Generate` (Vietnamese, archived) — see its [EXECUTION-REPORT.md](.plans/archived/260802-0012-lazytools-v02-catalog/EXECUTION-REPORT.md) |
