use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const BASES: &[&str] = &["auto", "bin", "oct", "dec", "hex"];

pub struct NumberBaseTool {
    spec: ToolSpec,
}

impl Default for NumberBaseTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.number-base", "Number Base", Category::Convert)
                .describe("Convert a number between binary, octal, decimal, and hex")
                .keywords(&[
                    "base", "radix", "binary", "octal", "hex", "decimal", "number",
                ])
                .input(Field::text("value").label("Value"))
                .option(
                    Field::select("from", BASES)
                        .default("auto")
                        .label("Input base"),
                )
                .output(Field::text("binary").mono().label("Binary"))
                .output(Field::text("octal").mono().label("Octal"))
                .output(Field::text("decimal").mono().label("Decimal"))
                .output(Field::text("hexadecimal").mono().label("Hexadecimal")),
        }
    }
}

/// Strips the prefix matching `radix`, if present. `auto` uses the prefix itself
/// to pick the radix, so both callers go through the same table.
fn strip_prefix(s: &str, radix: u32) -> &str {
    let prefix = match radix {
        2 => ["0b", "0B"],
        8 => ["0o", "0O"],
        16 => ["0x", "0X"],
        _ => return s,
    };
    prefix.iter().find_map(|p| s.strip_prefix(p)).unwrap_or(s)
}

impl Tool for NumberBaseTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        // `_` is accepted as a digit separator, so `1_000` works like in Rust literals.
        let value: String = i
            .text("value")
            .trim()
            .chars()
            .filter(|c| *c != '_')
            .collect();
        if value.is_empty() {
            return Err(ToolError::invalid("value", "value must not be empty"));
        }
        if value.starts_with('-') {
            return Err(ToolError::invalid(
                "value",
                "negative numbers are not supported",
            ));
        }

        let radix = match i.choice("from") {
            "bin" => 2,
            "oct" => 8,
            "dec" => 10,
            "hex" => 16,
            // `auto`: the prefix decides, and a bare number is decimal.
            _ => {
                let lower = value.to_ascii_lowercase();
                if lower.starts_with("0x") {
                    16
                } else if lower.starts_with("0b") {
                    2
                } else if lower.starts_with("0o") {
                    8
                } else {
                    10
                }
            }
        };

        let digits = strip_prefix(&value, radix);
        let n = u128::from_str_radix(digits, radix).map_err(|e| {
            let msg = match e.kind() {
                std::num::IntErrorKind::PosOverflow => "value exceeds 128 bits".to_string(),
                _ => format!("not a valid base-{radix} number: {digits}"),
            };
            ToolError::invalid("value", msg)
        })?;

        let mut out = Outputs::new();
        out.set("binary", format!("{n:b}"));
        out.set("octal", format!("{n:o}"));
        out.set("decimal", n.to_string());
        // Lowercase, no prefix — consistent with `convert.hex`.
        out.set("hexadecimal", format!("{n:x}"));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(value: &str, from: &str) -> Result<Outputs, ToolError> {
        NumberBaseTool::default().run(&Inputs::new().with("value", value).with("from", from))
    }

    fn all(value: &str, from: &str) -> [String; 4] {
        let out = run(value, from).unwrap();
        ["binary", "octal", "decimal", "hexadecimal"].map(|k| out.get(k).unwrap().as_display())
    }

    #[test]
    fn decimal_input_fills_all_four_bases() {
        assert_eq!(
            all("255", "auto"),
            ["11111111", "377", "255", "ff"].map(String::from)
        );
    }

    #[test]
    fn auto_detects_prefixes() {
        assert_eq!(all("0xff", "auto")[2], "255");
        assert_eq!(all("0b1010", "auto")[2], "10");
        assert_eq!(all("0o17", "auto")[2], "15");
    }

    #[test]
    fn explicit_base_accepts_bare_and_prefixed_digits() {
        assert_eq!(all("ff", "hex")[2], "255");
        assert_eq!(all("0xff", "hex")[2], "255");
    }

    #[test]
    fn underscores_are_separators() {
        assert_eq!(all("1_000", "auto")[2], "1000");
    }

    #[test]
    fn empty_input_names_the_field() {
        let err = run("", "auto").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "value", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn negative_input_names_the_field() {
        let err = run("-1", "auto").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "value", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn garbage_names_the_field() {
        let err = run("hello", "auto").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "value", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn overflow_says_so() {
        let err = run(&"9".repeat(40), "dec").unwrap_err();
        match err {
            ToolError::InvalidInput {
                field: "value",
                msg,
            } => {
                assert!(msg.contains("128 bits"), "{msg}");
            }
            other => panic!("{other:?}"),
        }
    }
}
