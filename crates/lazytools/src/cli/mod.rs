//! CLI sinh hoàn toàn từ `Registry`. Không có tên tool nào được viết ra ở đây —
//! thêm tool mới không cần chạm file này.

use std::ffi::OsString;
use std::io::{IsTerminal, Read, Write};
use std::process::ExitCode;

use clap::builder::PossibleValuesParser;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use lazytools_core::ToolError;
use lazytools_core::registry::Registry;
use lazytools_core::spec::{Field, FieldKind, ToolSpec};
use lazytools_core::value::{Inputs, Outputs, Value};

/// Tên flag dài của một field: `url_safe` → `--url-safe`.
fn flag_name(key: &str) -> String {
    key.replace('_', "-")
}

pub fn build_command(registry: &Registry) -> Command {
    let mut cmd = Command::new("lazytools")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Bộ tiện ích chạy trong terminal — chạy không tham số để mở TUI")
        .arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("In toàn bộ output dạng JSON"),
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
            .help(f.help.unwrap_or("`-` hoặc bỏ trống để đọc từ stdin"));
        sub = sub.arg(apply_kind(arg, f));
    }

    for f in &spec.options {
        let mut arg = Arg::new(f.key).long(flag_name(f.key));
        if let Some(h) = f.help {
            arg = arg.help(h);
        }
        sub = sub.arg(apply_kind(arg, f));
    }

    sub
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

    // Toggle dùng `SetTrue`, đã ngầm mặc định `false` — thêm `default_value` sẽ xung đột.
    match (&f.default, &f.kind) {
        (Some(v), FieldKind::Toggle) => {
            debug_assert!(
                v == &Value::Bool(false),
                "Toggle mặc định `true` chưa được hỗ trợ ở CLI"
            );
            arg
        }
        (Some(v), _) => arg.default_value(v.as_display()),
        (None, _) => arg,
    }
}

/// Lỗi ở tầng CLI, tách khỏi `ToolError` vì cách trình bày khác nhau.
enum CliError {
    /// Lỗi dùng sai lệnh — nói rõ thiếu gì.
    Usage(String),
    Tool(ToolError),
    Io(std::io::Error),
}

impl CliError {
    /// `InvalidInput` mang tên field: định vị về đúng positional hoặc đúng flag.
    /// Đây là lý do biến thể đó tách riêng trong `ToolError`.
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

/// Đọc toàn bộ stdin. Chỉ gọi khi stdin **không** phải TTY — nếu là TTY thì
/// người dùng chỉ đơn giản quên đối số, và treo chờ nhập là lỗi UX kinh điển.
fn read_stdin(key: &str) -> Result<String, CliError> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err(CliError::Usage(format!(
            "thiếu đối số `{key}` — truyền trực tiếp hoặc pipe qua stdin"
        )));
    }
    let mut buf = String::new();
    stdin.read_to_string(&mut buf)?;
    Ok(buf)
}

fn value_of(f: &Field, m: &ArgMatches) -> Option<Value> {
    match &f.kind {
        FieldKind::Toggle => Some(Value::Bool(m.get_flag(f.key))),
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
            // Giá trị tường minh — kể cả chuỗi rỗng, đó là input hợp lệ.
            Some(Value::Text(s)) if s != "-" => Value::Text(s),
            Some(v @ (Value::Num(_) | Value::Bool(_) | Value::Choice(_))) => v,
            // Thiếu hẳn, hoặc `-`: cả hai đều nghĩa là "lấy từ stdin".
            _ => {
                if stdin_taken {
                    return Err(CliError::Usage(format!(
                        "chỉ một input được đọc từ stdin; `{}` cần giá trị tường minh",
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
        // Đúng một output → in raw, không nhãn. Pipe-friendly là mục tiêu chính.
        // Newline chỉ thêm khi ra TTY, để bytes trên pipe khớp chính xác giá trị.
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
    // clap tự xử lý `--help` / `--version` / lỗi cú pháp rồi thoát với exit code riêng.
    let matches = cmd.clone().get_matches_from(args);

    let Some((name, sub_m)) = matches.subcommand() else {
        let _ = cmd.print_help();
        return ExitCode::from(2);
    };

    let Some(tool) = registry.all().find(|t| t.spec().cli_name() == name) else {
        eprintln!("error: không có tool nào tên `{name}`");
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
