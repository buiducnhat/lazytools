[English](README.md) | [Tiếng Việt](README.vi.md)

# lazytools

A terminal utility belt — offline, keyboard-first. Like
[it-tools](https://github.com/CorentinTh/it-tools) but as a TUI, while also
working directly in shell pipelines.

```console
$ echo -n "hello world" | lazytools hash --algo md5
5eb63bbbe01eeed093cb22bb8f5acdc3

$ echo -n "userIdFromDB" | lazytools case --style kebab
user-id-from-db

$ lazytools uuid
2bc10bd9-f274-45ac-ba91-2e875e385330

$ lazytools timestamp 1700000000 --json | jq -r .relative
2 years ago

$ lazytools data-format --from json --to yaml config.json > config.yaml
```

Run `lazytools` with no arguments to open the interface:

```
┌ Tools ───────────────┐┌ Hash Text ───────────────────────────────────────────────────┐
│Crypto                ││┌ Input ─────────────────────────────────────────────────────┐│
│  Hash Text           │││hello world                                                 ││
│  HMAC                │││                                                            ││
│  Bcrypt              │││                                                            ││
│Convert               │││                                                            ││
│  Base64              │││                                                            ││
│  URL Encode          │││                                                            ││
│  Hex                 ││└────────────────────────────────────────────────────────────┘│
│  JSON Format         ││┌ Algorithm ─────────────────────────────────────────────────┐│
│  Data Format         │││‹ md5 ›                                                     ││
│  Number Base         ││└────────────────────────────────────────────────────────────┘│
│  Unicode Escape      ││┌ Digest ────────────────────────────────────────────────────┐│
│Generate              │││5eb63bbbe01eeed093cb22bb8f5acdc3                            ││
│  Password            ││└────────────────────────────────────────────────────────────┘│
│  UUID                ││                                                              │
│  ULID                ││                                                              │
│  Random Token        ││                                                              │
│  Lorem Ipsum         ││                                                              │
│Text                  ││                                                              │
│  Change Case         ││                                                              │
│  Text Stats          ││                                                              │
│Web                   ││                                                              │
│  JWT Decode          ││                                                              │
│  Timestamp           ││                                                              │
│  Cron Explainer      ││                                                              │
│  URL Parser          ││                                                              │
│  JSON Diff           ││                                                              │
└──────────────────────┘└──────────────────────────────────────────────────────────────┘
[Tab] next field [^P] palette [^O] open file [^S] save file [y] copy [?] help [q] quit
```

## Install

**Homebrew** (macOS, Linux)

```bash
brew install buiducnhat/tap/lazytools
```

**Shell installer** (macOS, Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/buiducnhat/lazytools/releases/latest/download/lazytools-installer.sh | sh
```

**PowerShell** (Windows)

```powershell
powershell -c "irm https://github.com/buiducnhat/lazytools/releases/latest/download/lazytools-installer.ps1 | iex"
```

**Cargo** — requires Rust 1.97 or newer (edition 2024)

```bash
cargo install lazytools
```

Or grab a prebuilt binary from the [releases page](https://github.com/buiducnhat/lazytools/releases).

### From source

```bash
git clone https://github.com/buiducnhat/lazytools
cd lazytools
cargo install --path crates/lazytools
```

Or run it in place: `cargo run -p lazytools`.

## Tool catalog

22 tools across five categories — the same grouping the TUI sidebar uses.

**Crypto**

| Command | Description |
|---|---|
| `hash` | Hash text with MD5 / SHA-1 / SHA-256 / SHA-512 |
| `hmac` | HMAC with a secret key (SHA-1 / SHA-256 / SHA-512) |
| `bcrypt` | Hash a password, or check whether a hash matches |

**Convert**

| Command | Description |
|---|---|
| `base64` | Text ⇄ Base64, with an optional URL-safe alphabet |
| `url` | Percent-encode / decode a URL string |
| `hex` | Text ⇄ hex |
| `json-format` | Format or minify JSON, preserving key order |
| `data-format` | Convert between JSON, YAML, TOML, and CSV |
| `number-base` | Convert a number between binary, octal, decimal, and hex |
| `unicode` | Escape text to Unicode sequences, or decode them back |

**Generate**

| Command | Description |
|---|---|
| `password` | Generate a random password |
| `uuid` | Generate random UUIDs (v4 or time-ordered v7) |
| `ulid` | Generate lexicographically sortable ULIDs |
| `token` | Generate a random token of N bytes |
| `lorem` | Generate placeholder lorem ipsum text |

**Text**

| Command | Description |
|---|---|
| `case` | Convert text between camel, snake, kebab, and other cases |
| `stats` | Count characters, words, lines, and bytes in text |

**Web**

| Command | Description |
|---|---|
| `jwt-decode` | Decode a JWT and optionally verify its HMAC signature |
| `timestamp` | Convert between Unix timestamps and human-readable dates |
| `cron` | Explain a cron expression and list its next runs |
| `url-parse` | Break a URL into its parts |
| `json-diff` | Compare two JSON documents structurally |

`lazytools <command> --help` shows the full set of options — that help text is
**generated directly from the tool's declaration**, so it never drifts from
actual behavior.

## Using it in a pipeline

- A single output → printed **raw**, no label, no decoration.
- Multiple outputs → one `key=value` pair per line.
- `--json` → prints the whole output as JSON.
- Input is read from stdin when a positional argument is missing, or when `-` is passed.

## Keyboard shortcuts

| Key | Action |
|---|---|
| `Tab` | Switch pane / move to next field |
| `j` `k` / `↑` `↓` | Move within the sidebar |
| `Ctrl+P` | Tool-finder palette (fuzzy match on name, keywords, description) |
| `y` | Copy the currently focused output |
| `Ctrl+O` / `Ctrl+S` | Open a file into the input / save output to a file (`Ctrl+O` is hidden for tools that take no input) |
| `Ctrl+R` | Run / regenerate, from any field. `Enter` does the same, except in a multiline field where it inserts a line break |
| `?` | Help |
| `q` | Quit |

Remap keys via `~/.config/lazytools/keys.toml`:

```toml
palette = "ctrl+k"
quit = "q"
help = "?"
```

A broken config **does not block startup** — lazytools still opens with the
default keys and clearly reports which entries were skipped, so you can go fix them.

## Adding a new tool

This is the most important part for long-term maintainability, so it's designed
to be cheap: **one new file + one line in `register_all()`**. No touching
ratatui, no touching clap.

Create `crates/lazytools-core/src/tools/text/reverse.rs`:

```rust
use crate::{error::ToolError, registry::Tool, spec::*, value::*};

pub struct ReverseTool { spec: ToolSpec }

impl Default for ReverseTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.reverse", "Reverse Text", Category::Text)
                .describe("Reverse a piece of text")
                .keywords(&["reverse", "flip"])
                .input(Field::text("text").multiline().label("Input"))
                .output(Field::text("result").label("Result")),
        }
    }
}

impl Tool for ReverseTool {
    fn spec(&self) -> &ToolSpec { &self.spec }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        Ok(Outputs::one("result", i.text("text").chars().rev().collect::<String>()))
    }
}
```

Then add exactly one line to `tools/mod.rs`:

```rust
Box::new(text::reverse::ReverseTool::default()),
```

Done. The tool appears **simultaneously** in the TUI sidebar, in the palette, and
in `lazytools --help` — the input form is built automatically from `ToolSpec`,
and so is the CLI subcommand.

### Why it's built this way

`lazytools-core` has no dependency on ratatui/crossterm/clap. Each tool declares
only a `ToolSpec` (describing its fields) and one `Inputs → Outputs` function.
Both frontends **read** that spec instead of hard-coding anything:

- `ToolFormComponent` builds a widget per `FieldKind`.
- The CLI layer builds subcommands + flags from that same spec.

The consequence: there is a single source of truth, and the cost of adding the
40th tool is about the same as the 4th. That is measurable, and it has been
measured — the v0.2.0 release took the catalog from 8 tools to 22, and across
all three batches of new tools `git diff crates/lazytools/src/` came back
**empty**. An invariant test
(`crates/lazytools-core/tests/spec_invariants.rs`) walks the whole registry to
keep the property from drifting.

`RunMode` is declared per tool so behavior is decided when the tool is written
rather than discovered as UI jank later:

- `Live` re-runs on every edit, debounced. The default.
- `OnDemand` waits for the run key — for slow tools, like bcrypt at cost 12
  (~250ms).
- `Generate` runs on open *and* re-runs on the run key, so a random
  generator can hand you a different password without editing anything.

`Ctrl+R` always runs the tool. `Enter` runs it too, except in a multiline text
field, where it belongs to the field and inserts a line break.

Tools are pure functions with two deliberate exceptions: random generators, and
the tools that read the clock (`timestamp`, `cron`). Both are tested by
property — length, alphabet, ordering — rather than against fixed values.

## Development

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

CI runs exactly these three commands, on Linux + macOS + Windows.

The TUI's snapshot tests use [`insta`](https://insta.rs); when the interface
changes on purpose, review them with `cargo insta review`.

## Documentation

See [docs/SUMMARY.md](docs/SUMMARY.md) for architecture, codebase layout, code
standards, and product context. A Vietnamese translation is available locally
under `docs-vi/` (not tracked in git).

## License

MIT
