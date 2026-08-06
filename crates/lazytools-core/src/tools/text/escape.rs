use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const TARGETS: &[&str] = &["json", "regex", "shell"];
const DIRECTIONS: &[&str] = &["escape", "unescape"];

pub struct EscapeTool {
    spec: ToolSpec,
}

impl Default for EscapeTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.escape", "Escape Text", Category::Text)
                .describe("Escape or unescape text for a JSON string, a regex, or a shell")
                .keywords(&[
                    "escape",
                    "unescape",
                    "quote",
                    "json",
                    "regex",
                    "shell",
                    "backslash",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("target", TARGETS)
                        .default("json")
                        .label("Target"),
                )
                .option(
                    Field::select("direction", DIRECTIONS)
                        .default("escape")
                        .label("Direction"),
                )
                .output(Field::text("result").multiline().mono().label("Result")),
        }
    }
}

/// The content of a JSON string — no surrounding quotes, since the point is to
/// paste the result *into* a string that already has them.
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_unescape(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let escape = chars.next().ok_or("the input ends in a lone backslash")?;
        match escape {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'u' => {
                let code = take_hex4(&mut chars)?;
                // A high surrogate is only half a character; JavaScript writes
                // astral code points as a pair, so the second half is required.
                let ch = if (0xd800..0xdc00).contains(&code) {
                    let (Some('\\'), Some('u')) = (chars.next(), chars.next()) else {
                        return Err(format!("\\u{code:04x} is a lone high surrogate"));
                    };
                    let low = take_hex4(&mut chars)?;
                    if !(0xdc00..0xe000).contains(&low) {
                        return Err(format!("\\u{low:04x} is not a low surrogate"));
                    }
                    let combined = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00);
                    char::from_u32(combined)
                } else {
                    char::from_u32(code)
                };
                out.push(ch.ok_or_else(|| format!("\\u{code:04x} is not a character"))?);
            }
            other => return Err(format!("`\\{other}` is not a JSON escape")),
        }
    }
    Ok(out)
}

fn take_hex4(chars: &mut std::str::Chars) -> Result<u32, String> {
    let digits: String = chars.by_ref().take(4).collect();
    if digits.chars().count() != 4 {
        return Err(format!("`\\u{digits}` needs four hex digits"));
    }
    u32::from_str_radix(&digits, 16).map_err(|_| format!("`\\u{digits}` is not hexadecimal"))
}

/// Drops one level of backslash escaping — the inverse of `regex::escape`,
/// which only ever escapes non-alphanumeric characters.
fn regex_unescape(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let escaped = chars.next().ok_or("the input ends in a lone backslash")?;
        if escaped.is_alphanumeric() {
            // `\d`, `\w`, `\b` are classes, not escaped literals — removing the
            // backslash would silently change what the pattern matches.
            return Err(format!(
                "`\\{escaped}` is a character class, not an escaped literal"
            ));
        }
        out.push(escaped);
    }
    Ok(out)
}

/// Single quotes, because they are the only shell quoting with no interior
/// expansion at all. An embedded `'` closes the quote, emits an escaped one,
/// and reopens — the standard `'\''` dance.
fn shell_escape(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn shell_unescape(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '\'' {
                        closed = true;
                        break;
                    }
                    out.push(c);
                }
                if !closed {
                    return Err("unterminated single quote".to_string());
                }
            }
            '"' => {
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => {
                            closed = true;
                            break;
                        }
                        // Inside double quotes a backslash only escapes these.
                        '\\' => match chars.next() {
                            Some(n @ ('"' | '\\' | '$' | '`')) => out.push(n),
                            Some(n) => {
                                out.push('\\');
                                out.push(n);
                            }
                            None => return Err("the input ends in a lone backslash".to_string()),
                        },
                        c => out.push(c),
                    }
                }
                if !closed {
                    return Err("unterminated double quote".to_string());
                }
            }
            '\\' => match chars.next() {
                Some(n) => out.push(n),
                None => return Err("the input ends in a lone backslash".to_string()),
            },
            c => out.push(c),
        }
    }
    Ok(out)
}

impl Tool for EscapeTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        let escaping = i.choice("direction") != "unescape";

        let result = match (i.choice("target"), escaping) {
            ("regex", true) => regex::escape(text),
            ("regex", false) => regex_unescape(text).map_err(|e| ToolError::invalid("text", e))?,
            ("shell", true) => shell_escape(text),
            ("shell", false) => shell_unescape(text).map_err(|e| ToolError::invalid("text", e))?,
            (_, true) => json_escape(text),
            (_, false) => json_unescape(text).map_err(|e| ToolError::invalid("text", e))?,
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, target: &str, direction: &str) -> Result<Outputs, ToolError> {
        EscapeTool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("target", target)
                .with("direction", direction),
        )
    }

    fn ok(text: &str, target: &str, direction: &str) -> String {
        run(text, target, direction)
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn json_escapes_quotes_newlines_and_control_characters() {
        assert_eq!(ok("a\"b\\c", "json", "escape"), r#"a\"b\\c"#);
        assert_eq!(ok("line\nnext\t.", "json", "escape"), r"line\nnext\t.");
        assert_eq!(ok("\u{1}", "json", "escape"), "\\u0001");
    }

    #[test]
    fn json_round_trips_including_astral_characters() {
        for text in ["a\"b\\c", "line\nnext", "emoji 🦀", "tab\there"] {
            let escaped = ok(text, "json", "escape");
            assert_eq!(ok(&escaped, "json", "unescape"), text, "{text:?}");
        }
        // Written the way JavaScript would: a surrogate pair.
        assert_eq!(ok("\\ud83e\\udd80", "json", "unescape"), "\u{1f980}");
    }

    #[test]
    fn a_bad_json_escape_names_the_field() {
        for text in [r"\q", r"\u12", r"\ud83e", "trailing\\"] {
            let err = run(text, "json", "unescape").unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidInput { field: "text", .. }),
                "{text:?}: {err:?}"
            );
        }
    }

    #[test]
    fn regex_escapes_metacharacters_and_round_trips() {
        let escaped = ok("1+1=2 (really?)", "regex", "escape");
        assert!(escaped.contains(r"\+"), "{escaped}");
        assert_eq!(ok(&escaped, "regex", "unescape"), "1+1=2 (really?)");
    }

    /// Stripping the backslash from `\d` would turn a digit class into the
    /// letter `d` — a silent change of meaning, so it is refused.
    #[test]
    fn regex_unescape_refuses_to_strip_a_character_class() {
        let err = run(r"\d+", "regex", "unescape").unwrap_err();
        match err {
            ToolError::InvalidInput { field: "text", msg } => {
                assert!(msg.contains("character class"), "{msg}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn shell_quoting_survives_an_embedded_quote() {
        assert_eq!(ok("rm -rf /", "shell", "escape"), "'rm -rf /'");
        let tricky = "it's $HOME `whoami`";
        let escaped = ok(tricky, "shell", "escape");
        // The apostrophe has to close, escape, and reopen — nothing inside a
        // single-quoted run may be left for the shell to expand.
        assert_eq!(escaped, r#"'it'\''s $HOME `whoami`'"#);
        assert_eq!(ok(&escaped, "shell", "unescape"), tricky);
    }

    #[test]
    fn shell_unescape_reads_the_three_quoting_forms() {
        assert_eq!(
            ok(r#""double $quoted""#, "shell", "unescape"),
            "double $quoted"
        );
        assert_eq!(ok(r"back\ slash", "shell", "unescape"), "back slash");
        assert_eq!(ok(r#""a\"b""#, "shell", "unescape"), "a\"b");
    }

    #[test]
    fn an_unterminated_quote_names_the_field() {
        for text in ["'open", "\"open"] {
            let err = run(text, "shell", "unescape").unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidInput { field: "text", .. }),
                "{text:?}: {err:?}"
            );
        }
    }

    #[test]
    fn empty_input_is_not_an_error_in_any_mode() {
        for target in TARGETS {
            for direction in DIRECTIONS {
                assert!(run("", target, direction).is_ok(), "{target} {direction}");
            }
        }
    }
}
