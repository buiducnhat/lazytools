use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const DIRECTIONS: &[&str] = &["encode", "decode"];

pub struct UrlTool {
    spec: ToolSpec,
}

impl Default for UrlTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.url", "URL Encode", Category::Convert)
                .describe("Percent-encode / decode chuỗi URL")
                .keywords(&["url", "percent", "escape", "uri", "encode", "decode"])
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

/// `urlencoding::decode` chỉ báo lỗi khi kết quả không phải UTF-8 — chuỗi
/// percent hỏng như `%ZZ` bị nó cho đi qua nguyên văn. Với một tool decode thì
/// im lặng như vậy là bẫy: người dùng tưởng đã decode xong. Kiểm trước.
fn check_percent_sequences(text: &str) -> Result<(), ToolError> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }
        let valid = matches!((bytes.get(i + 1), bytes.get(i + 2)),
            (Some(a), Some(b)) if a.is_ascii_hexdigit() && b.is_ascii_hexdigit());
        if !valid {
            return Err(ToolError::invalid(
                "text",
                format!("chuỗi percent hỏng ở vị trí {i}: `%` phải kèm đúng 2 chữ số hex"),
            ));
        }
        i += 3;
    }
    Ok(())
}

impl Tool for UrlTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        let result = match i.choice("direction") {
            "decode" => {
                check_percent_sequences(text)?;
                urlencoding::decode(text)
                    .map_err(|e| {
                        ToolError::invalid(
                            "text",
                            format!("giải mã ra bytes không phải UTF-8: {e}"),
                        )
                    })?
                    .into_owned()
            }
            _ => urlencoding::encode(text).into_owned(),
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, direction: &str) -> Result<Outputs, ToolError> {
        UrlTool::default().run(
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
            ("hello world", "hello%20world"),
            ("a+b=c&d", "a%2Bb%3Dc%26d"),
            ("xin chào", "xin%20ch%C3%A0o"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(ok(plain, "encode"), encoded, "encode {plain:?}");
            assert_eq!(ok(encoded, "decode"), plain, "decode {encoded:?}");
        }
    }

    #[test]
    fn invalid_percent_sequence_names_the_field() {
        let err = run("%ZZ", "decode").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "text", .. }),
            "{err:?}"
        );
    }
}
