use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const DIRECTIONS: &[&str] = &["encode", "decode"];

pub struct Base64Tool {
    spec: ToolSpec,
}

impl Default for Base64Tool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.base64", "Base64", Category::Convert)
                .describe("Chuyển văn bản ⇄ Base64")
                .keywords(&["base64", "b64", "encode", "decode", "url safe"])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("direction", DIRECTIONS)
                        .default("encode")
                        .label("Direction"),
                )
                .option(
                    Field::toggle("url_safe")
                        .default(false)
                        .label("URL safe")
                        .help("Dùng bảng chữ cái an toàn cho URL (-_ thay vì +/)"),
                )
                .output(Field::text("result").mono().label("Result")),
        }
    }
}

impl Tool for Base64Tool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        // `Engine` không dyn-compatible (method có generic), nên chọn engine
        // bằng giá trị chứ không qua trait object.
        let engine = if i.bool("url_safe") {
            URL_SAFE
        } else {
            STANDARD
        };
        let text = i.text("text");

        let result = match i.choice("direction") {
            "decode" => {
                let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                let bytes = engine
                    .decode(cleaned.as_bytes())
                    .map_err(|e| ToolError::invalid("text", format!("base64 không hợp lệ: {e}")))?;
                String::from_utf8(bytes).map_err(|e| {
                    ToolError::invalid("text", format!("giải mã ra bytes không phải UTF-8: {e}"))
                })?
            }
            _ => engine.encode(text.as_bytes()),
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn run(text: &str, direction: &str, url_safe: bool) -> Result<Outputs, ToolError> {
        Base64Tool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("direction", direction)
                .with("url_safe", Value::Bool(url_safe)),
        )
    }

    fn ok(text: &str, direction: &str, url_safe: bool) -> String {
        run(text, direction, url_safe)
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn round_trip_standard() {
        for plain in ["hello", "", "xin chào"] {
            let encoded = ok(plain, "encode", false);
            assert_eq!(ok(&encoded, "decode", false), plain, "round trip {plain:?}");
        }
        assert_eq!(ok("hello", "encode", false), "aGVsbG8=");
    }

    #[test]
    fn url_safe_uses_different_alphabet() {
        // Byte 0xfb 0xff sinh `+` / `/` ở bảng chuẩn và `-` / `_` ở bảng URL-safe.
        let plain = "~~~?";
        let std = ok(plain, "encode", false);
        let url = ok(plain, "encode", true);
        assert_ne!(std, url, "hai bảng chữ cái phải cho kết quả khác nhau");
        assert_eq!(ok(&url, "decode", true), plain);
    }

    #[test]
    fn invalid_base64_names_the_field() {
        let err = run("!!!not base64!!!", "decode", false).unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "text", .. }),
            "{err:?}"
        );
    }
}
