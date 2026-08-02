//! CLI generated entirely from `Registry`. No tool name is hardcoded here —
//! adding a new tool doesn't need to touch this file.

use std::ffi::OsString;
use std::io::{IsTerminal, Read, Write};
use std::process::ExitCode;

use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use lazytools_core::ToolError;
use lazytools_core::registry::Registry;
use lazytools_core::spec::{Field, FieldKind, ToolSpec};
use lazytools_core::value::{Inputs, Outputs, Value};

/// Long flag name for a field: `url_safe` → `--url-safe`.
fn flag_name(key: &str) -> String {
    key.replace('_', "-")
}

/// clap id (and long flag) for the negated half of a `Toggle`: `--no-url-safe`.
/// Distinct from the field key, so `toggle_value` can read both halves separately.
fn negated_name(key: &str) -> String {
    format!("no-{}", flag_name(key))
}

pub fn build_command(registry: &Registry) -> Command {
    let mut cmd = Command::new("lazytools")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A terminal utility toolbox — run with no arguments to open the TUI")
        .arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Print the entire output as JSON"),
        );

    for tool in registry.all() {
        cmd = cmd.subcommand(build_subcommand(tool.spec()));
    }
    cmd
}

fn build_subcommand(spec: &ToolSpec) -> Command {
    let mut sub = Command::new(spec.cli_name()).about(spec.description);

    for f in &spec.inputs {
        let arg = Arg::new(f.key)
            .value_name(f.key.to_uppercase())
            .required(false)
            .help(f.help.unwrap_or("`-` or leave empty to read from stdin"));
        sub = sub.arg(apply_kind(arg, f));
    }

    for f in &spec.options {
        let arg = Arg::new(f.key).long(flag_name(f.key));

        if matches!(f.kind, FieldKind::Toggle) {
            // Every `Toggle` gets a `--no-x` twin. Without one, a field declaring
            // `default(true)` would be impossible to switch off, which is why such a
            // default used to be forbidden outright and why `generate.password`
            // reaches for a `Select` where a `Toggle` is the honest field type.
            //
            // The twin is generated for *every* toggle, not only those defaulting to
            // `true`: a symmetric `--help` reads better than one where the negation
            // appears only sometimes, and `--no-x` against a `false` default is
            // simply explicit rather than wrong.
            let (yes, no) = toggle_help(f);
            sub = sub.arg(apply_kind(arg.help(yes), f));
            sub = sub.arg(
                Arg::new(negated_name(f.key))
                    .long(negated_name(f.key))
                    .action(ArgAction::SetTrue)
                    // POSIX-style last-one-wins, so `--x --no-x` resolves instead of
                    // erroring out.
                    .overrides_with(f.key)
                    .help(no),
            );
        } else {
            let arg = match f.help {
                Some(h) => arg.help(h),
                None => arg,
            };
            sub = sub.arg(apply_kind(arg, f));
        }
    }

    sub
}

/// Help text for both halves of a toggle.
///
/// Which way a toggle already points isn't visible from a bare flag, and clap prints
/// `[default: ...]` only for args that take a value — so the default is spelled out
/// here instead of leaving the user to guess.
fn toggle_help(f: &Field) -> (String, String) {
    let on_by_default = matches!(f.default, Some(Value::Bool(true)));
    let yes = match (f.help, on_by_default) {
        (Some(h), true) => format!("{h} [default]"),
        (Some(h), false) => h.to_string(),
        (None, true) => "Turn this on [default]".to_string(),
        (None, false) => "Turn this on".to_string(),
    };
    let no = format!(
        "Turn off --{}{}",
        flag_name(f.key),
        if on_by_default { "" } else { " [default]" }
    );
    (yes, no)
}

/// Resolves a toggle from its two flags, falling back to the declared default.
///
/// The default fallback is the whole point: `SetTrue` on its own can only ever say
/// "absent", so a field declaring `default(true)` would silently arrive as `false`.
fn toggle_value(f: &Field, m: &ArgMatches) -> bool {
    // `try_get_one` rather than `get_flag`: a `Toggle` declared as an *input* is
    // positional and has no `--no-x` twin registered, so the id genuinely may not exist.
    let negated = m
        .try_get_one::<bool>(&negated_name(f.key))
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false);

    if negated {
        false
    } else if m.get_flag(f.key) {
        true
    } else {
        matches!(f.default, Some(Value::Bool(true)))
    }
}

fn apply_kind(arg: Arg, f: &Field) -> Arg {
    let arg = match &f.kind {
        FieldKind::Select { options } => {
            arg.value_parser(PossibleValuesParser::new(options.to_vec()))
        }
        FieldKind::Toggle => arg.action(ArgAction::SetTrue),
        FieldKind::Number { min, max } => arg.value_parser(value_parser!(i64).range(*min..=*max)),
        FieldKind::Text { .. } | FieldKind::Secret | FieldKind::FilePath { .. } => arg,
    };

    // `Toggle` uses `SetTrue`, which takes no value, so `default_value` would conflict
    // with it. Its declared default is applied later, in `toggle_value`, alongside the
    // `--no-x` twin.
    match (&f.default, &f.kind) {
        (_, FieldKind::Toggle) => arg,
        (Some(v), _) => arg.default_value(v.as_display()),
        (None, _) => arg,
    }
}

/// CLI-layer error, separate from `ToolError` because the presentation differs.
enum CliError {
    /// Usage error — states clearly what's missing.
    Usage(String),
    Tool(ToolError),
    Io(std::io::Error),
}

impl CliError {
    /// `InvalidInput` carries the field name: it can be pinpointed to the right
    /// positional argument or the right flag. This is why that variant is kept
    /// separate in `ToolError`.
    fn render(&self, spec: &ToolSpec) -> String {
        match self {
            Self::Usage(msg) => msg.clone(),
            Self::Io(e) => e.to_string(),
            Self::Tool(ToolError::InvalidInput { field, msg }) => {
                let is_option = spec.options.iter().any(|f| f.key == *field);
                if is_option {
                    format!("--{}: {msg}", flag_name(field))
                } else {
                    format!("{field}: {msg}")
                }
            }
            Self::Tool(e) => e.to_string(),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Reads all of stdin. Only called when stdin is **not** a TTY — if it is a TTY,
/// the user simply forgot the argument, and hanging waiting for input is a classic UX bug.
fn read_stdin(key: &str) -> Result<String, CliError> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err(CliError::Usage(format!(
            "missing argument `{key}` — pass it directly or pipe it through stdin"
        )));
    }
    let mut buf = String::new();
    stdin.read_to_string(&mut buf)?;
    Ok(buf)
}

fn value_of(f: &Field, m: &ArgMatches) -> Option<Value> {
    match &f.kind {
        FieldKind::Toggle => Some(Value::Bool(toggle_value(f, m))),
        FieldKind::Number { .. } => m.get_one::<i64>(f.key).copied().map(Value::Num),
        FieldKind::Select { .. } => m.get_one::<String>(f.key).cloned().map(Value::Choice),
        FieldKind::Text { .. } | FieldKind::Secret | FieldKind::FilePath { .. } => {
            m.get_one::<String>(f.key).cloned().map(Value::Text)
        }
    }
}

fn collect_inputs(spec: &ToolSpec, m: &ArgMatches) -> Result<Inputs, CliError> {
    let mut inputs = Inputs::new();
    let mut stdin_taken = false;

    for f in &spec.inputs {
        let value = match value_of(f, m) {
            // Explicit value — even an empty string, that's valid input.
            Some(Value::Text(s)) if s != "-" => Value::Text(s),
            Some(v @ (Value::Num(_) | Value::Bool(_) | Value::Choice(_))) => v,
            // Missing entirely, or `-`: both mean "read from stdin".
            _ => {
                if stdin_taken {
                    return Err(CliError::Usage(format!(
                        "only one input can be read from stdin; `{}` needs an explicit value",
                        f.key
                    )));
                }
                stdin_taken = true;
                Value::Text(read_stdin(f.key)?)
            }
        };
        inputs.set(f.key, value);
    }

    for f in &spec.options {
        if let Some(v) = value_of(f, m) {
            inputs.set(f.key, v);
        }
    }

    Ok(inputs)
}

fn print_outputs(spec: &ToolSpec, outputs: &Outputs, as_json: bool) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();

    if as_json {
        let map: serde_json::Map<String, serde_json::Value> = spec
            .outputs
            .iter()
            .filter_map(|f| outputs.get(f.key).map(|v| (f.key.to_string(), to_json(v))))
            .collect();
        writeln!(out, "{}", serde_json::Value::Object(map))?;
    } else if spec.outputs.len() == 1 {
        // Exactly one output → print raw, unlabeled. Pipe-friendliness is the main goal.
        // Newline is only added when writing to a TTY, so bytes on a pipe match the value exactly.
        let value = spec.outputs.first().and_then(|f| outputs.get(f.key));
        let text = value.map(Value::as_display).unwrap_or_default();
        if out.is_terminal() {
            writeln!(out, "{text}")?;
        } else {
            write!(out, "{text}")?;
        }
    } else {
        for f in &spec.outputs {
            if let Some(v) = outputs.get(f.key) {
                writeln!(out, "{}={}", f.key, v.as_display())?;
            }
        }
    }

    out.flush()
}

fn to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Text(s) | Value::Choice(s) => serde_json::Value::String(s.clone()),
        Value::Num(n) => serde_json::Value::from(*n),
        Value::Bool(b) => serde_json::Value::Bool(*b),
    }
}

pub fn run<I, T>(registry: &Registry, args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut cmd = build_command(registry);
    // clap handles `--help` / `--version` / syntax errors itself and exits with its own exit code.
    let matches = cmd.clone().get_matches_from(args);

    let Some((name, sub_m)) = matches.subcommand() else {
        let _ = cmd.print_help();
        return ExitCode::from(2);
    };

    let Some(tool) = registry.all().find(|t| t.spec().cli_name() == name) else {
        eprintln!("error: no tool named `{name}`");
        return ExitCode::from(1);
    };
    let spec = tool.spec();
    let as_json = sub_m.get_flag("json");

    let result = collect_inputs(spec, sub_m)
        .and_then(|inputs| registry.run(spec.id, &inputs).map_err(CliError::Tool));

    match result {
        Ok(outputs) => match print_outputs(spec, &outputs, as_json) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        },
        Err(e) => {
            eprintln!("error: {}", e.render(spec));
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use lazytools_core::registry::Tool;
    use lazytools_core::spec::Category;

    use super::*;

    /// A tool carrying a `Toggle` that defaults to **`true`** — the shape the CLI
    /// layer used to forbid with a `debug_assert!`. No shipped tool has one yet
    /// (`generate.password` still works around the old limit with a `Select`, and
    /// changing its spec would break `--charset` in a patch release), so the
    /// invariant is pinned against a synthetic spec instead of the real catalog.
    struct ToggleTool {
        spec: ToolSpec,
    }

    impl Default for ToggleTool {
        fn default() -> Self {
            Self {
                spec: ToolSpec::new("test.toggle", "Toggle", Category::Convert)
                    .option(Field::toggle("symbols").default(true))
                    .option(Field::toggle("digits").default(false))
                    .output(Field::text("result")),
            }
        }
    }

    impl Tool for ToggleTool {
        fn spec(&self) -> &ToolSpec {
            &self.spec
        }

        fn run(&self, _: &Inputs) -> Result<Outputs, ToolError> {
            Ok(Outputs::one("result", ""))
        }
    }

    /// Parses `args` and returns the resolved `(symbols, digits)` pair. Deliberately
    /// stops at `collect_inputs` — the point under test is how flags become `Value`s,
    /// not what a tool does with them. The spec has no inputs, so nothing reads stdin.
    fn resolve(args: &[&str]) -> (bool, bool) {
        let registry = Registry::from_tools(vec![Box::new(ToggleTool::default())]);
        let matches = build_command(&registry)
            .try_get_matches_from(args)
            .unwrap_or_else(|e| panic!("{args:?} must parse: {e}"));
        let (_, sub) = matches.subcommand().expect("a subcommand must match");

        let spec = registry.get("test.toggle").expect("registered").spec();
        let inputs = collect_inputs(spec, sub).ok().expect("inputs must collect");
        (inputs.bool("symbols"), inputs.bool("digits"))
    }

    #[test]
    fn toggles_fall_back_to_their_declared_default() {
        // The regression that motivated this: `SetTrue` alone can only report
        // "absent", so `symbols` used to arrive as `false` despite declaring `true`.
        assert_eq!(resolve(&["lazytools", "toggle"]), (true, false));
    }

    #[test]
    fn negation_flag_turns_a_true_default_off() {
        assert_eq!(
            resolve(&["lazytools", "toggle", "--no-symbols"]),
            (false, false)
        );
    }

    #[test]
    fn positive_flag_turns_a_false_default_on() {
        assert_eq!(resolve(&["lazytools", "toggle", "--digits"]), (true, true));
    }

    /// `--x` and `--no-x` override each other POSIX-style, so passing both is a
    /// last-one-wins resolution rather than a usage error.
    #[test]
    fn passing_both_halves_lets_the_last_one_win() {
        assert!(resolve(&["lazytools", "toggle", "--no-symbols", "--symbols"]).0);
        assert!(!resolve(&["lazytools", "toggle", "--symbols", "--no-symbols"]).0);
    }

    /// Both halves must reach `--help`, and the reader must be able to tell which
    /// way the field already points without running anything.
    #[test]
    fn help_shows_both_halves_and_marks_the_default() {
        let registry = Registry::from_tools(vec![Box::new(ToggleTool::default())]);
        let mut cmd = build_command(&registry);
        let sub = cmd
            .get_subcommands_mut()
            .find(|c| c.get_name() == "toggle")
            .expect("subcommand exists");
        let help = sub.render_help().to_string();

        for flag in ["--symbols", "--no-symbols", "--digits", "--no-digits"] {
            assert!(help.contains(flag), "--help is missing `{flag}`:\n{help}");
        }
        assert!(
            help.contains("Turn this on [default]"),
            "the true-defaulting toggle must say so:\n{help}"
        );
        assert!(
            help.contains("Turn off --digits [default]"),
            "the false-defaulting toggle must say so on its negation:\n{help}"
        );
    }
}
