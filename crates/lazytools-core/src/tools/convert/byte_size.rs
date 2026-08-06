use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct ByteSizeTool {
    spec: ToolSpec,
}

impl Default for ByteSizeTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.byte-size", "Byte Size", Category::Convert)
                .describe("Convert a byte count between raw, binary (KiB), and decimal (kB) units")
                .keywords(&[
                    "byte",
                    "size",
                    "kb",
                    "mb",
                    "gb",
                    "kib",
                    "mib",
                    "gib",
                    "human",
                    "humanize",
                    "file size",
                ])
                .input(
                    Field::text("value")
                        .label("Size")
                        .help("A number with an optional unit: `1536`, `1.5 GiB`, `700 MB`"),
                )
                .output(Field::text("bytes").mono().label("Bytes"))
                .output(Field::text("binary").mono().label("Binary (1024)"))
                .output(Field::text("decimal").mono().label("Decimal (1000)"))
                .output(Field::text("bits").mono().label("Bits")),
        }
    }
}

const BINARY_UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
const DECIMAL_UNITS: &[&str] = &["B", "kB", "MB", "GB", "TB", "PB", "EB"];

/// Multiplier for a unit suffix, or `None` if it isn't one.
///
/// A bare `K`/`M`/`G` — no `i`, no `B` — is read as **binary**, which is what
/// `ls -h`, `du -h`, and `dd bs=1M` all mean by it. The explicit `kB` form is
/// the only way to ask for 1000, and that is the point of having both outputs.
fn multiplier(suffix: &str) -> Option<u128> {
    let s = suffix.trim().to_ascii_lowercase();
    let (power, binary) = match s.as_str() {
        "" | "b" | "byte" | "bytes" => return Some(1),
        "k" | "kib" => (1, true),
        "m" | "mib" => (2, true),
        "g" | "gib" => (3, true),
        "t" | "tib" => (4, true),
        "p" | "pib" => (5, true),
        "e" | "eib" => (6, true),
        "kb" => (1, false),
        "mb" => (2, false),
        "gb" => (3, false),
        "tb" => (4, false),
        "pb" => (5, false),
        "eb" => (6, false),
        _ => return None,
    };
    let base: u128 = if binary { 1024 } else { 1000 };
    Some(base.pow(power))
}

/// Splits `1.5 GiB` into `("1.5", "GiB")`. The number ends at the first
/// character that can't continue one.
fn split_number(input: &str) -> (&str, &str) {
    let end = input
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '_'))
        .unwrap_or(input.len());
    (&input[..end], &input[end..])
}

/// Largest unit in which the value is at least 1, rendered with at most two
/// decimals and no trailing zeros — `1.5 KiB`, not `1.50 KiB`.
fn humanize(bytes: u128, units: &[&str], base: u128) -> String {
    let mut value = bytes as f64;
    let mut index = 0;
    let step = base as f64;
    while value >= step && index + 1 < units.len() {
        value /= step;
        index += 1;
    }
    if index == 0 {
        return format!("{bytes} {}", units[0]);
    }
    let rendered = format!("{value:.2}");
    let trimmed = rendered.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed} {}", units[index])
}

impl Tool for ByteSizeTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let raw = i.text("value").trim();
        if raw.is_empty() {
            return Err(ToolError::invalid("value", "size must not be empty"));
        }
        if raw.starts_with('-') {
            return Err(ToolError::invalid("value", "a size cannot be negative"));
        }

        let (number, suffix) = split_number(raw);
        // `_` is a digit separator, as in `1_048_576`.
        let number: String = number.chars().filter(|c| *c != '_').collect();
        let amount: f64 = number
            .parse()
            .map_err(|_| ToolError::invalid("value", format!("`{number}` is not a number")))?;
        let unit = multiplier(suffix).ok_or_else(|| {
            ToolError::invalid("value", format!("`{}` is not a unit", suffix.trim()))
        })?;

        // f64 keeps 53 bits of mantissa, so a fractional input above 2^53 bytes
        // would report a number it cannot actually represent.
        let exact = amount * unit as f64;
        if !exact.is_finite() || exact >= (u128::MAX as f64) {
            return Err(ToolError::invalid(
                "value",
                "size is too large to represent",
            ));
        }
        let bytes = exact.round() as u128;
        let bits = bytes
            .checked_mul(8)
            .ok_or_else(|| ToolError::invalid("value", "size is too large to represent"))?;

        let mut out = Outputs::new();
        out.set("bytes", bytes.to_string());
        out.set("binary", humanize(bytes, BINARY_UNITS, 1024));
        out.set("decimal", humanize(bytes, DECIMAL_UNITS, 1000));
        out.set("bits", bits.to_string());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(value: &str) -> Result<Outputs, ToolError> {
        ByteSizeTool::default().run(&Inputs::new().with("value", value))
    }

    fn field(value: &str, key: &str) -> String {
        run(value).unwrap().get(key).unwrap().as_display()
    }

    #[test]
    fn a_bare_number_is_bytes() {
        assert_eq!(field("1536", "bytes"), "1536");
        assert_eq!(field("1536", "binary"), "1.5 KiB");
        assert_eq!(field("1536", "decimal"), "1.54 kB");
        assert_eq!(field("1536", "bits"), "12288");
    }

    /// The two scales are the reason the tool has two outputs: 1 GB of disk is
    /// not 1 GiB of memory, and the gap is what people come here to settle.
    #[test]
    fn binary_and_decimal_units_are_not_the_same_size() {
        assert_eq!(field("1 GiB", "bytes"), "1073741824");
        assert_eq!(field("1 GB", "bytes"), "1000000000");
    }

    /// `ls -h` and `dd bs=1M` both mean 1024 by a bare `M`.
    #[test]
    fn a_bare_suffix_is_binary() {
        assert_eq!(field("1M", "bytes"), "1048576");
        assert_eq!(field("1MB", "bytes"), "1000000");
    }

    #[test]
    fn fractions_separators_and_case_are_accepted() {
        assert_eq!(field("1.5 gib", "bytes"), "1610612736");
        assert_eq!(field("1_048_576", "binary"), "1 MiB");
    }

    #[test]
    fn small_values_stay_in_bytes_without_a_decimal_point() {
        assert_eq!(field("0", "binary"), "0 B");
        assert_eq!(field("999", "decimal"), "999 B");
    }

    #[test]
    fn an_unknown_unit_names_the_field() {
        let err = run("12 parsecs").unwrap_err();
        match err {
            ToolError::InvalidInput {
                field: "value",
                msg,
            } => {
                assert!(msg.contains("parsecs"), "{msg}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn empty_negative_and_unparsable_input_all_name_the_field() {
        for value in ["", "-1", "abc"] {
            let err = run(value).unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidInput { field: "value", .. }),
                "{value:?}: {err:?}"
            );
        }
    }

    #[test]
    fn a_size_beyond_representation_says_so_rather_than_wrapping() {
        // Past u128, which is where the exact byte count stops existing.
        let err = run(&"9".repeat(40)).unwrap_err();
        match err {
            ToolError::InvalidInput {
                field: "value",
                msg,
            } => {
                assert!(msg.contains("too large"), "{msg}");
            }
            other => panic!("{other:?}"),
        }
    }
}
