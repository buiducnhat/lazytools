# Documentation Summary

**lazytools** — an offline, keyboard-first terminal utility belt: a TUI plus a
set of CLI subcommands sharing one spec-driven tool registry, built in Rust
(edition 2024) with `ratatui` + `clap` on top of a UI-framework-free core crate.

## Agent Context Guide

Before planning or implementing, read this `docs/SUMMARY.md` file first. Load only the detail docs relevant to the current task, and prioritize `Code Standard` docs for implementation conventions. If docs conflict with code or user intent, use the available question tool before making broad changes.

## Architecture

System design, component interactions, data flows, deployment, and external integrations.

| File | Description |
| ---- | ------------ |
| [spec-driven-tools.md](architecture/spec-driven-tools.md) | How `ToolSpec` + the `Tool` trait let the CLI and TUI both generate themselves from one declaration per tool |
| [tui-event-loop.md](architecture/tui-event-loop.md) | `Component`/`DrawableComponent` pattern, event routing order, focus/layout, the internal event queue, and the 16ms poll/debounce loop |
| [file-io.md](architecture/file-io.md) | Open/save file popups: size limits, overwrite confirmation, missing-parent-directory handling |

## Codebase

Directory structure, entry points, API patterns, and key modules.

| File | Description |
| ---- | ------------ |
| [directory-layout.md](codebase/directory-layout.md) | Full workspace tree (`lazytools-core` vs `lazytools`), entry point (`main.rs`), and why app logic lives in `lib.rs` |

## Code Standard

Conventions, naming rules, tech stack versions, and development workflows.

| File | Description |
| ---- | ------------ |
| [conventions.md](code-standard/conventions.md) | Language (English-only code/comments/UI text), edition/toolchain, crate boundary rule, error-handling conventions, naming |
| [adding-a-tool.md](code-standard/adding-a-tool.md) | Step-by-step for adding a new tool, `ToolSpec` content conventions, and the invariants CI enforces |
| [testing-conventions.md](code-standard/testing-conventions.md) | The four test layers (tool unit tests, spec invariants, CLI end-to-end, TUI/snapshot) and when to use each |

## Project PDR

Product goals, use cases, business rules, and constraints.

| File | Description |
| ---- | ------------ |
| [product-goals.md](project-pdr/product-goals.md) | Central design thesis (open tool catalog, cost-of-addition must stay flat), pipeline-first CLI contract, reliability expectations |
| [scope-and-roadmap.md](project-pdr/scope-and-roadmap.md) | What shipped in the MVP vs. what's explicitly deferred to v0.2 |

## Other

| File | Description |
| ---- | ------------ |
| [.brainstorms/260731-1635-lazytools-tui-architecture/](.brainstorms/260731-1635-lazytools-tui-architecture/SUMMARY.md) | Pre-implementation brainstorm (Vietnamese, archived): gitui research, architecture design, dependency verification |
| [.plans/archived/260731-1641-lazytools-mvp/](.plans/archived/260731-1641-lazytools-mvp/SUMMARY.md) | Full MVP implementation plan and phase-by-phase execution log (Vietnamese, archived) |
