//! JSON ⇄ YAML ⇄ TOML ⇄ CSV, dùng `serde_json::Value` làm dạng trung gian.
//!
//! Mỗi định dạng có giới hạn thật sự khác nhau. Nguyên tắc ở đây: **báo lỗi rõ
//! ràng thay vì sinh output sai âm thầm** — một file cấu hình bị chuyển sai mà
//! không ai biết thì tệ hơn nhiều so với một thông báo lỗi.

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
                .describe("Chuyển đổi giữa JSON, YAML, TOML và CSV")
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
        "json" => serde_json::from_str(text).map_err(|e| bad(format!("JSON không hợp lệ: {e}"))),
        "yaml" => serde_yaml_ng::from_str(text).map_err(|e| bad(format!("YAML không hợp lệ: {e}"))),
        "toml" => toml::from_str(text).map_err(|e| bad(format!("TOML không hợp lệ: {e}"))),
        "csv" => parse_csv(text),
        other => Err(ToolError::invalid(
            "from",
            format!("định dạng không hỗ trợ: {other}"),
        )),
    }
}

/// CSV → mảng các object, khóa lấy từ dòng header.
fn parse_csv(text: &str) -> Result<Json, ToolError> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| ToolError::invalid("text", format!("CSV không hợp lệ: {e}")))?
        .clone();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record =
            record.map_err(|e| ToolError::invalid("text", format!("CSV không hợp lệ: {e}")))?;
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
            .map_err(|e| ToolError::Failed(format!("không sinh được JSON: {e}"))),
        "yaml" => serde_yaml_ng::to_string(value)
            .map_err(|e| ToolError::Failed(format!("không sinh được YAML: {e}"))),
        "toml" => render_toml(value),
        "csv" => render_csv(value),
        other => Err(ToolError::invalid(
            "to",
            format!("định dạng không hỗ trợ: {other}"),
        )),
    }
}

/// TOML không có `null` và không cho giá trị vô hướng ở gốc.
fn render_toml(value: &Json) -> Result<String, ToolError> {
    if !value.is_object() {
        return Err(ToolError::invalid(
            "to",
            "TOML chỉ biểu diễn được bảng ở gốc — dữ liệu này có gốc là mảng hoặc giá trị đơn",
        ));
    }
    if contains_null(value) {
        return Err(ToolError::invalid(
            "to",
            "TOML không có khái niệm `null`; hãy bỏ hoặc thay các giá trị null trước",
        ));
    }
    toml::to_string_pretty(value)
        .map_err(|e| ToolError::invalid("to", format!("không biểu diễn được bằng TOML: {e}")))
}

fn contains_null(value: &Json) -> bool {
    match value {
        Json::Null => true,
        Json::Array(a) => a.iter().any(contains_null),
        Json::Object(o) => o.values().any(contains_null),
        _ => false,
    }
}

/// CSV chỉ biểu diễn được mảng các object phẳng.
fn render_csv(value: &Json) -> Result<String, ToolError> {
    let rows = value.as_array().ok_or_else(|| {
        ToolError::invalid(
            "to",
            "CSV cần dữ liệu là một mảng các object — dữ liệu này không phải mảng",
        )
    })?;

    if rows.is_empty() {
        return Ok(String::new());
    }

    let first = rows[0]
        .as_object()
        .ok_or_else(|| ToolError::invalid("to", "CSV cần mỗi phần tử của mảng là một object"))?;
    let headers: Vec<String> = first.keys().cloned().collect();

    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record(&headers)
        .map_err(|e| ToolError::Failed(format!("không ghi được CSV: {e}")))?;

    for (idx, row) in rows.iter().enumerate() {
        let obj = row.as_object().ok_or_else(|| {
            ToolError::invalid("to", format!("phần tử thứ {idx} không phải object"))
        })?;
        let mut record = Vec::with_capacity(headers.len());
        for key in &headers {
            let cell = obj.get(key).unwrap_or(&Json::Null);
            record.push(match cell {
                Json::Null => String::new(),
                Json::String(s) => s.clone(),
                // Object/mảng lồng nhau không có ô CSV nào chứa nổi — nói thẳng.
                Json::Array(_) | Json::Object(_) => {
                    return Err(ToolError::invalid(
                        "to",
                        format!(
                            "CSV chỉ chứa được object phẳng; khóa `{key}` ở phần tử {idx} là giá trị lồng nhau"
                        ),
                    ));
                }
                other => other.to_string(),
            });
        }
        writer
            .write_record(&record)
            .map_err(|e| ToolError::Failed(format!("không ghi được CSV: {e}")))?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| ToolError::Failed(format!("không ghi được CSV: {e}")))?;
    String::from_utf8(bytes).map_err(|e| ToolError::Failed(format!("CSV không phải UTF-8: {e}")))
}

impl Tool for DataFormatTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        // `from == to` vẫn hợp lệ: hoạt động như normalize/format lại.
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

    // --- giới hạn thật của từng định dạng: phải báo lỗi rõ, không sinh output sai ---

    #[test]
    fn toml_rejects_array_at_root() {
        let err = run("[1,2]", "json", "toml").unwrap_err();
        match err {
            ToolError::InvalidInput { field: "to", msg } => {
                assert!(msg.contains("gốc"), "msg phải giải thích lý do: {msg}");
            }
            other => panic!("kỳ vọng InvalidInput trên `to`, nhận {other:?}"),
        }
    }

    #[test]
    fn toml_rejects_null() {
        let err = run(r#"{"a":null}"#, "json", "toml").unwrap_err();
        match err {
            ToolError::InvalidInput { field: "to", msg } => {
                assert!(msg.contains("null"), "msg phải nhắc tới null: {msg}");
            }
            other => panic!("kỳ vọng InvalidInput trên `to`, nhận {other:?}"),
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
                assert!(msg.contains("phẳng"), "msg phải nêu giới hạn: {msg}");
            }
            other => panic!("kỳ vọng InvalidInput trên `to`, nhận {other:?}"),
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
