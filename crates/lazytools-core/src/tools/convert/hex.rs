use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const DIRECTIONS: &[&str] = &["encode", "decode"];

pub struct HexTool {
    spec: ToolSpec,
}

impl Default for HexTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.hex", "Hex", Category::Convert)
                .describe("Chuyển văn bản ⇄ hex")
                .keywords(&["hex", "hexadecimal", "base16", "encode", "decode"])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("direction", DIRECTIONS)
                        .default("encode")
                        .label("Direction"),
                )
                .output(Field::text("result").mono().label("Result")),
        }
    }
}

impl Tool for HexTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        let result = match i.choice("direction") {
            "decode" => {
                // Bỏ khoảng trắng để chấp nhận cả dạng "48 65 6c".
                let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                let bytes = hex::decode(&cleaned)
                    .map_err(|e| ToolError::invalid("text", format!("hex không hợp lệ: {e}")))?;
                String::from_utf8(bytes).map_err(|e| {
                    ToolError::invalid("text", format!("giải mã ra bytes không phải UTF-8: {e}"))
                })?
            }
            _ => hex::encode(text.as_bytes()),
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, direction: &str) -> Result<Outputs, ToolError> {
        HexTool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("direction", direction),
        )
    }

    fn ok(text: &str, direction: &str) -> String {
        run(text, direction)
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn round_trip() {
        let cases = [
            ("hello", "68656c6c6f"),
            ("", ""),
            ("xin chào", "78696e206368c3a06f"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(ok(plain, "encode"), encoded, "encode {plain:?}");
            assert_eq!(ok(encoded, "decode"), plain, "decode {encoded:?}");
        }
    }

    #[test]
    fn decode_ignores_whitespace() {
        assert_eq!(ok("68 65 6c 6c 6f", "decode"), "hello");
    }

    #[test]
    fn invalid_hex_names_the_field() {
        let err = run("zz", "decode").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "text", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn non_utf8_result_is_reported_not_panicked() {
        let err = run("ff", "decode").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "text", .. }),
            "{err:?}"
        );
    }
}
