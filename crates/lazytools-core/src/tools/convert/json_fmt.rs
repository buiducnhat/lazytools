//! Key order is **preserved** here, guaranteed by `serde_json`'s `preserve_order`
//! feature (see the workspace `Cargo.toml`). For a formatter, silently resorting the
//! user's keys is a bug.
//!
//! `web::json_diff` deliberately does the opposite and sorts keys before comparing,
//! because there a key swap must *not* register as a change. Both are correct for
//! their own tool — don't "unify" them.

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const MODES: &[&str] = &["pretty", "minify"];

pub struct JsonFormatTool {
    spec: ToolSpec,
}

impl Default for JsonFormatTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.json-format", "JSON Format", Category::Convert)
                .describe("Format or minify JSON")
                .keywords(&["json", "format", "pretty", "minify", "beautify", "indent"])
                .input(Field::text("text").multiline().label("Input"))
                .option(Field::select("mode", MODES).default("pretty").label("Mode"))
                .option(
                    Field::number("indent", 1, 8)
                        .default(2i64)
                        .label("Indent")
                        .help("Number of spaces per level (only used in pretty mode)"),
                )
                .output(Field::text("result").multiline().mono().label("Result")),
        }
    }
}

impl Tool for JsonFormatTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        // Malformed JSON → serde_json reports the exact line/column, which is very
        // useful to surface to the user.
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| ToolError::invalid("text", format!("invalid JSON: {e}")))?;

        let result = if i.choice("mode") == "minify" {
            serde_json::to_string(&value)
                .map_err(|e| ToolError::Failed(format!("failed to serialize: {e}")))?
        } else {
            let indent = i.num("indent").clamp(1, 8) as usize;
            let pad = vec![b' '; indent];
            let mut buf = Vec::new();
            let formatter = serde_json::ser::PrettyFormatter::with_indent(&pad);
            let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
            serde::Serialize::serialize(&value, &mut ser)
                .map_err(|e| ToolError::Failed(format!("failed to serialize: {e}")))?;
            String::from_utf8(buf)
                .map_err(|e| ToolError::Failed(format!("output is not valid UTF-8: {e}")))?
        };

        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn run(text: &str, mode: &str, indent: i64) -> Result<Outputs, ToolError> {
        JsonFormatTool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("mode", mode)
                .with("indent", Value::Num(indent)),
        )
    }

    fn ok(text: &str, mode: &str, indent: i64) -> String {
        run(text, mode, indent)
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn minify_strips_whitespace() {
        assert_eq!(
            ok("{ \"a\" : 1 , \"b\" : [ 2 ] }", "minify", 2),
            r#"{"a":1,"b":[2]}"#
        );
    }

    #[test]
    fn pretty_respects_indent() {
        assert_eq!(ok(r#"{"a":1}"#, "pretty", 2), "{\n  \"a\": 1\n}");
        assert_eq!(ok(r#"{"a":1}"#, "pretty", 4), "{\n    \"a\": 1\n}");
    }

    #[test]
    fn invalid_json_reports_position_on_the_field() {
        let err = run("{ oops }", "pretty", 2).unwrap_err();
        match err {
            ToolError::InvalidInput { field: "text", msg } => {
                assert!(
                    msg.contains("line") || msg.contains("column"),
                    "msg should mention a position: {msg}"
                );
            }
            other => panic!("expected InvalidInput on `text`, got {other:?}"),
        }
    }
}
