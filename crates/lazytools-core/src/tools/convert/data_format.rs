//! JSON ⇄ YAML ⇄ TOML ⇄ CSV, using `serde_json::Value` as the intermediate form.
//!
//! Each format has genuinely different limitations. The principle here is:
//! **report a clear error instead of silently producing wrong output** — a
//! config file that gets silently mis-converted is far worse than an error
//! message.

use serde_json::{Map, Value as Json};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const FORMATS: &[&str] = &["json", "yaml", "toml", "csv"];

pub struct DataFormatTool {
    spec: ToolSpec,
}

impl Default for DataFormatTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.data-format", "Data Format", Category::Convert)
                .describe("Convert between JSON, YAML, TOML, and CSV")
                .keywords(&["json", "yaml", "toml", "csv", "convert", "data"])
                .input(Field::text("text").multiline().label("Input"))
                .option(Field::select("from", FORMATS).default("json").label("From"))
                .option(Field::select("to", FORMATS).default("yaml").label("To"))
                .output(Field::text("result").multiline().mono().label("Result")),
        }
    }
}

fn parse(text: &str, format: &str) -> Result<Json, ToolError> {
    let bad = |e: String| ToolError::invalid("text", e);

    match format {
        "json" => serde_json::from_str(text).map_err(|e| bad(format!("invalid JSON: {e}"))),
        "yaml" => serde_yaml_ng::from_str(text).map_err(|e| bad(format!("invalid YAML: {e}"))),
        "toml" => toml::from_str(text).map_err(|e| bad(format!("invalid TOML: {e}"))),
        "csv" => parse_csv(text),
        other => Err(ToolError::invalid(
            "from",
            format!("unsupported format: {other}"),
        )),
    }
}

/// CSV → array of objects, with keys taken from the header row.
fn parse_csv(text: &str) -> Result<Json, ToolError> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| ToolError::invalid("text", format!("invalid CSV: {e}")))?
        .clone();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| ToolError::invalid("text", format!("invalid CSV: {e}")))?;
        let obj: Map<String, Json> = headers
            .iter()
            .zip(record.iter())
            .map(|(h, v)| (h.to_string(), Json::String(v.to_string())))
            .collect();
        rows.push(Json::Object(obj));
    }
    Ok(Json::Array(rows))
}

fn render(value: &Json, format: &str) -> Result<String, ToolError> {
    match format {
        "json" => serde_json::to_string_pretty(value)
            .map_err(|e| ToolError::Failed(format!("failed to produce JSON: {e}"))),
        "yaml" => serde_yaml_ng::to_string(value)
            .map_err(|e| ToolError::Failed(format!("failed to produce YAML: {e}"))),
        "toml" => render_toml(value),
        "csv" => render_csv(value),
        other => Err(ToolError::invalid(
            "to",
            format!("unsupported format: {other}"),
        )),
    }
}

/// TOML has no `null` and doesn't allow a scalar value at the root.
fn render_toml(value: &Json) -> Result<String, ToolError> {
    if !value.is_object() {
        return Err(ToolError::invalid(
            "to",
            "TOML can only represent a table at the root — this data has an array or a scalar value at the root",
        ));
    }
    if contains_null(value) {
        return Err(ToolError::invalid(
            "to",
            "TOML has no concept of `null`; remove or replace the null values first",
        ));
    }
    toml::to_string_pretty(value)
        .map_err(|e| ToolError::invalid("to", format!("cannot be represented as TOML: {e}")))
}

fn contains_null(value: &Json) -> bool {
    match value {
        Json::Null => true,
        Json::Array(a) => a.iter().any(contains_null),
        Json::Object(o) => o.values().any(contains_null),
        _ => false,
    }
}

/// CSV can only represent a flat array of objects.
fn render_csv(value: &Json) -> Result<String, ToolError> {
    let rows = value.as_array().ok_or_else(|| {
        ToolError::invalid(
            "to",
            "CSV requires the data to be an array of objects — this data isn't an array",
        )
    })?;

    if rows.is_empty() {
        return Ok(String::new());
    }

    let first = rows[0].as_object().ok_or_else(|| {
        ToolError::invalid("to", "CSV requires each array element to be an object")
    })?;
    let headers: Vec<String> = first.keys().cloned().collect();

    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record(&headers)
        .map_err(|e| ToolError::Failed(format!("failed to write CSV: {e}")))?;

    for (idx, row) in rows.iter().enumerate() {
        let obj = row
            .as_object()
            .ok_or_else(|| ToolError::invalid("to", format!("element {idx} is not an object")))?;
        let mut record = Vec::with_capacity(headers.len());
        for key in &headers {
            let cell = obj.get(key).unwrap_or(&Json::Null);
            record.push(match cell {
                Json::Null => String::new(),
                Json::String(s) => s.clone(),
                // A nested object/array has no CSV cell that can hold it — say so plainly.
                Json::Array(_) | Json::Object(_) => {
                    return Err(ToolError::invalid(
                        "to",
                        format!(
                            "CSV can only hold flat objects; key `{key}` in element {idx} is a nested value"
                        ),
                    ));
                }
                other => other.to_string(),
            });
        }
        writer
            .write_record(&record)
            .map_err(|e| ToolError::Failed(format!("failed to write CSV: {e}")))?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| ToolError::Failed(format!("failed to write CSV: {e}")))?;
    String::from_utf8(bytes).map_err(|e| ToolError::Failed(format!("CSV is not valid UTF-8: {e}")))
}

impl Tool for DataFormatTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        // `from == to` is still valid: it acts as a normalize/reformat pass.
        let value = parse(i.text("text"), i.choice("from"))?;
        let result = render(&value, i.choice("to"))?;
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, from: &str, to: &str) -> Result<Outputs, ToolError> {
        DataFormatTool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("from", from)
                .with("to", to),
        )
    }

    fn ok(text: &str, from: &str, to: &str) -> String {
        run(text, from, to)
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn json_to_yaml() {
        assert_eq!(ok(r#"{"a":1}"#, "json", "yaml").trim(), "a: 1");
    }

    #[test]
    fn yaml_to_json() {
        assert_eq!(ok("a: 1", "yaml", "json"), "{\n  \"a\": 1\n}");
    }

    #[test]
    fn json_to_toml() {
        assert_eq!(
            ok(r#"{"a":1,"b":"x"}"#, "json", "toml").trim(),
            "a = 1\nb = \"x\""
        );
    }

    #[test]
    fn toml_round_trip() {
        assert_eq!(ok("a = 1", "toml", "toml").trim(), "a = 1");
    }

    #[test]
    fn json_array_to_csv() {
        let json = r#"[{"name":"an","age":3},{"name":"bo","age":4}]"#;
        assert_eq!(ok(json, "json", "csv"), "name,age\nan,3\nbo,4\n");
    }

    #[test]
    fn csv_to_json() {
        let out = ok("name,age\nan,3\n", "csv", "json");
        assert!(out.contains("\"name\": \"an\""), "{out}");
    }

    #[test]
    fn same_format_normalises() {
        assert_eq!(ok("{ \"a\" : 1 }", "json", "json"), "{\n  \"a\": 1\n}");
    }

    // --- real limitations of each format: must report a clear error, never wrong output ---

    #[test]
    fn toml_rejects_array_at_root() {
        let err = run("[1,2]", "json", "toml").unwrap_err();
        match err {
            ToolError::InvalidInput { field: "to", msg } => {
                assert!(msg.contains("root"), "msg should explain why: {msg}");
            }
            other => panic!("expected InvalidInput on `to`, got {other:?}"),
        }
    }

    #[test]
    fn toml_rejects_null() {
        let err = run(r#"{"a":null}"#, "json", "toml").unwrap_err();
        match err {
            ToolError::InvalidInput { field: "to", msg } => {
                assert!(msg.contains("null"), "msg should mention null: {msg}");
            }
            other => panic!("expected InvalidInput on `to`, got {other:?}"),
        }
    }

    #[test]
    fn csv_rejects_non_array() {
        let err = run(r#"{"a":1}"#, "json", "csv").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "to", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn csv_rejects_nested_values() {
        let err = run(r#"[{"a":{"b":1}}]"#, "json", "csv").unwrap_err();
        match err {
            ToolError::InvalidInput { field: "to", msg } => {
                assert!(
                    msg.contains("flat"),
                    "msg should state the limitation: {msg}"
                );
            }
            other => panic!("expected InvalidInput on `to`, got {other:?}"),
        }
    }

    #[test]
    fn malformed_input_names_the_text_field() {
        let err = run("{ oops", "json", "yaml").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "text", .. }),
            "{err:?}"
        );
    }
}
