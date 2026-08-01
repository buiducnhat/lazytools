use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const DIRECTIONS: &[&str] = &["encode", "decode"];
const FORMATS: &[&str] = &["\\uXXXX", "U+XXXX", "&#NNNN;"];

pub struct UnicodeTool {
    spec: ToolSpec,
}

impl Default for UnicodeTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.unicode", "Unicode Escape", Category::Convert)
                .describe("Escape text to Unicode sequences, or decode them back")
                .keywords(&["unicode", "escape", "codepoint", "entity", "utf", "u+"])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("direction", DIRECTIONS)
                        .default("encode")
                        .label("Direction"),
                )
                .option(
                    Field::select("format", FORMATS)
                        .default("\\uXXXX")
                        .label("Format"),
                )
                .output(Field::text("result").multiline().mono().label("Result")),
        }
    }
}

fn encode(text: &str, format: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match format {
            // `U+` and `&#` address code points directly, so astral characters stay
            // whole. Only the JavaScript-style `\u` form is limited to 16 bits.
            "U+XXXX" => out.push_str(&format!("U+{:04X}", c as u32)),
            "&#NNNN;" => out.push_str(&format!("&#{};", c as u32)),
            _ => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
        }
    }
    out
}

/// Reads exactly `len` hex digits starting at `at`. `None` if they aren't there.
fn hex_at(chars: &[char], at: usize, len: usize) -> Option<u32> {
    let slice = chars.get(at..at + len)?;
    let s: String = slice.iter().collect();
    u32::from_str_radix(&s, 16).ok()
}

fn bad_escape(what: &str) -> ToolError {
    ToolError::invalid("text", format!("invalid escape sequence: {what}"))
}

/// Decoding accepts all three formats regardless of the `format` option —
/// lenient when reading, strict when writing.
fn decode(text: &str) -> Result<String, ToolError> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;

    while i < chars.len() {
        // `\uXXXX`, including JavaScript-style surrogate pairs.
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'u') {
            let unit = hex_at(&chars, i + 2, 4).ok_or_else(|| {
                bad_escape(&chars[i..chars.len().min(i + 6)].iter().collect::<String>())
            })?;

            if (0xD800..0xDC00).contains(&unit) {
                let low = (chars.get(i + 6) == Some(&'\\') && chars.get(i + 7) == Some(&'u'))
                    .then(|| hex_at(&chars, i + 8, 4))
                    .flatten()
                    .filter(|low| (0xDC00..0xE000).contains(low))
                    .ok_or_else(|| {
                        ToolError::invalid("text", format!("unpaired surrogate \\u{unit:04X}"))
                    })?;
                let cp = 0x1_0000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                out.push(char::from_u32(cp).ok_or_else(|| bad_escape(&format!("\\u{unit:04X}")))?);
                i += 12;
                continue;
            }
            if (0xDC00..0xE000).contains(&unit) {
                return Err(ToolError::invalid(
                    "text",
                    format!("unpaired surrogate \\u{unit:04X}"),
                ));
            }
            out.push(char::from_u32(unit).ok_or_else(|| bad_escape(&format!("\\u{unit:04X}")))?);
            i += 6;
            continue;
        }

        // `U+XXXX` — 1 to 6 hex digits, longest match wins.
        if chars[i] == 'U' && chars.get(i + 1) == Some(&'+') {
            let len = (1..=6)
                .rev()
                .find(|n| hex_at(&chars, i + 2, *n).is_some())
                .ok_or_else(|| bad_escape("U+"))?;
            let cp = hex_at(&chars, i + 2, len).expect("length was just probed");
            out.push(char::from_u32(cp).ok_or_else(|| bad_escape(&format!("U+{cp:04X}")))?);
            i += 2 + len;
            continue;
        }

        // `&#NNNN;` — decimal, terminated by `;`.
        if chars[i] == '&' && chars.get(i + 1) == Some(&'#') {
            let digits: String = chars[i + 2..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            let end = i + 2 + digits.len();
            if digits.is_empty() || chars.get(end) != Some(&';') {
                return Err(bad_escape(
                    &chars[i..chars.len().min(end + 1)]
                        .iter()
                        .collect::<String>(),
                ));
            }
            let cp: u32 = digits
                .parse()
                .map_err(|_| bad_escape(&format!("&#{digits};")))?;
            out.push(char::from_u32(cp).ok_or_else(|| bad_escape(&format!("&#{digits};")))?);
            i = end + 1;
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    Ok(out)
}

impl Tool for UnicodeTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        let result = match i.choice("direction") {
            "decode" => decode(text)?,
            _ => encode(text, i.choice("format")),
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, direction: &str, format: &str) -> Result<Outputs, ToolError> {
        UnicodeTool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("direction", direction)
                .with("format", format),
        )
    }

    fn ok(text: &str, direction: &str, format: &str) -> String {
        run(text, direction, format)
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn ascii_round_trip_in_every_format() {
        for format in FORMATS {
            let encoded = ok("Hi!", "encode", format);
            assert_eq!(ok(&encoded, "decode", format), "Hi!", "format={format}");
        }
    }

    #[test]
    fn astral_round_trip_in_every_format() {
        for format in FORMATS {
            let encoded = ok("😀", "encode", format);
            assert_eq!(ok(&encoded, "decode", format), "😀", "format={format}");
        }
    }

    #[test]
    fn backslash_u_emits_a_surrogate_pair_for_astral_chars() {
        assert_eq!(ok("😀", "encode", "\\uXXXX"), "\\uD83D\\uDE00");
        assert_eq!(ok("\\uD83D\\uDE00", "decode", "\\uXXXX"), "😀");
    }

    #[test]
    fn code_point_formats_do_not_use_surrogates() {
        assert_eq!(ok("😀", "encode", "U+XXXX"), "U+1F600");
        assert_eq!(ok("😀", "encode", "&#NNNN;"), "&#128512;");
    }

    #[test]
    fn unpaired_surrogate_is_reported() {
        let err = run("\\uD83D", "decode", "\\uXXXX").unwrap_err();
        match err {
            ToolError::InvalidInput { field: "text", msg } => {
                assert!(msg.contains("unpaired surrogate"), "{msg}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn malformed_escape_names_the_field() {
        let err = run("\\uZZZZ", "decode", "\\uXXXX").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "text", .. }),
            "{err:?}"
        );
    }

    /// Text that isn't part of an escape passes through untouched, so escapes can
    /// be mixed into ordinary prose.
    #[test]
    fn plain_characters_pass_through_when_decoding() {
        assert_eq!(ok("a\\u0062c U+0044 &#69;", "decode", "\\uXXXX"), "abc D E");
    }

    #[test]
    fn decoding_accepts_any_format_regardless_of_the_option() {
        assert_eq!(ok("U+0041", "decode", "\\uXXXX"), "A");
        assert_eq!(ok("&#65;", "decode", "U+XXXX"), "A");
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(ok("", "encode", "\\uXXXX"), "");
        assert_eq!(ok("", "decode", "\\uXXXX"), "");
    }
}
