# Directory layout

Two-crate Cargo workspace (`Cargo.toml` at the repo root, `resolver = "3"`,
edition 2024).

The subdirectory names under `tools/` are the five `spec::Category` variants.
Adding a category means adding a directory here *and* a variant there — those
two are expected to stay in lockstep.

```
crates/
  lazytools-core/         # pure tool logic — no ratatui/crossterm/clap
    src/
      lib.rs              # re-exports: error, registry, spec, tools, value
      error.rs            # ToolError (InvalidInput / Failed / Io)
      registry.rs          # Tool trait, Registry (lookup + panic-safe run())
      spec.rs              # ToolSpec, Field, FieldKind, Category, RunMode
      value.rs              # Inputs/Outputs/Value — the data tools exchange
      tools/                # one subdirectory per Category, one file per tool
        mod.rs              # register_all() — the ONLY place every tool is listed
        crypto/             # hash.rs, hmac.rs, bcrypt.rs, totp.rs
        convert/            # base64.rs, base32.rs, url.rs, hex.rs, json_fmt.rs,
                            #   data_format.rs, number_base.rs, unicode.rs,
                            #   color.rs, html_entity.rs, byte_size.rs,
                            #   duration.rs
        generate/           # password.rs, uuid.rs, ulid.rs, token.rs, lorem.rs
        text/               # case.rs, stats.rs, lines.rs, diff.rs, regex.rs,
                            #   slug.rs, escape.rs
        web/                # jwt_decode.rs, jwt_encode.rs, timestamp.rs,
                            #   cron.rs, url_parse.rs, json_diff.rs, ip.rs,
                            #   http_status.rs
    tests/
      spec_invariants.rs    # registry-wide invariants (see code-standard/testing-conventions.md)

  lazytools/               # binary crate: TUI + CLI + main()
    src/
      main.rs               # entry point: argv.len() > 1 -> CLI, else -> TUI
      lib.rs                # re-exports app/cli/clipboard/components/keys/paths/
                            #   popups/queue/session/settings/tui/ui
      app.rs                # App — top-level state, layout, event routing, queue draining
      tui.rs                # terminal setup/teardown, 16ms poll loop, debounce tick
      queue.rs               # InternalEvent, NeedsUpdate, Queue (Rc<RefCell<VecDeque<_>>>)
      clipboard.rs            # native (arboard) + OSC 52 clipboard, text-only
      paths.rs                # config vs state directories, XDG-aware
      settings.rs             # config.toml — [session] and [theme]
      session.rs              # session.toml — last tool + values, secrets excluded
      theme_state.rs          # theme.toml — the theme picked in the app, and
                              #   which config it was picked against
      cli/
        mod.rs                # builds the whole clap Command tree from Registry
      keys/
        key_list.rs           # KeysList — every default key binding, one struct
        key_config.rs         # KeyConfig — TOML loading/merging over defaults, hint()
        mod.rs                # key_match(), typed_char()
      components/
        mod.rs                # Component / DrawableComponent traits, CommandInfo, pumps
        sidebar.rs             # tool list grouped by Category, responsive width
        palette.rs             # Ctrl+P fuzzy tool finder (nucleo)
        tool_form.rs           # renders/edits a ToolSpec's fields, runs the tool
        cmdbar.rs               # bottom hint bar, built from CommandInfo
        field/                 # one file per FieldKind widget:
                                # text.rs, textarea.rs, number.rs, secret.rs,
                                # select.rs, toggle.rs, filepath.rs
      popups/
        mod.rs
        help.rs                 # `?` — shortcuts list generated from commands()
        msg.rs                  # info/error popup
        file_open.rs             # Ctrl+O — directory browser, size-limited
        file_save.rs              # Ctrl+S — path entry + overwrite confirmation
        theme.rs                   # Ctrl+T — theme picker, previews as it moves
      ui/
        mod.rs                  # centered_rect() and other layout helpers
        style.rs                 # Theme (nine colors), ThemeHandle/SharedTheme
        themes.rs                 # the built-in theme presets
    tests/
      cli.rs                    # end-to-end CLI tests (assert_cmd, one process per test)
      file_io.rs                 # App-level file open/save tests (simulated key events)
      session.rs                  # App-level persistence tests (restore, capture, secrets)
      theme.rs                     # colors and the Ctrl+T picker, asserted on rendered cells
      snapshots.rs                  # insta snapshot tests over TestBackend renders
      snapshots/*.snap               # committed snapshot fixtures
```

## Entry point

`crates/lazytools/src/main.rs` is intentionally tiny: it builds a `Registry`,
and dispatches purely on argument count — any argument at all routes to the
CLI (`cli::run`), otherwise it starts the TUI
(`tui::run_with_user_config`). The CLI reads no config file and writes no
session; the TUI reads `keys.toml` and `config.toml` if present, and reads and
writes `session.toml` and `theme.toml` — see
[architecture/configuration-and-state.md](../architecture/configuration-and-state.md).

## Why the app logic lives in `lib.rs`, not just `main.rs`

`crates/lazytools/src/lib.rs` re-exports `app`, `cli`, `clipboard`,
`components`, `keys`, `paths`, `popups`, `queue`, `session`, `settings`,
`theme_state`, `tui`, `ui` from a library target specifically so that
`crates/lazytools/tests/*.rs` (integration tests, which can only import from a
library crate) can construct `App` directly and drive it with simulated
key events — this is what makes `file_io.rs`, `session.rs`, `theme.rs`, and
`snapshots.rs` possible without spawning a real terminal.
