//! RFC 4648 base32, hand-written in ~40 lines rather than pulling in
//! `data-encoding`. This module owns the canonical codec for the whole crate:
//! `crypto.totp` decodes the secret printed next to a QR code with the same
//! `decode` below, so there is one alphabet and one set of tolerances.

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const DIRECTIONS: &[&str] = &["encode", "decode"];
const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub struct Base32Tool {
    spec: ToolSpec,
}

impl Default for Base32Tool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.base32", "Base32", Category::Convert)
                .describe("Convert text ⇄ Base32 (RFC 4648)")
                .keywords(&["base32", "b32", "rfc4648", "encode", "decode", "otp"])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("direction", DIRECTIONS)
                        .default("encode")
                        .label("Direction"),
                )
                .option(
                    Field::toggle("padding")
                        .default(true)
                        .label("Padding")
                        .help("Pad the encoded output to a multiple of 8 with `=`"),
                )
                .output(Field::text("result").mono().label("Result")),
        }
    }
}

/// Encodes five bits at a time, most significant first.
pub(crate) fn encode(bytes: &[u8], padding: bool) -> String {
    let mut out = String::new();
    let (mut bits, mut width) = (0u32, 0u32);

    for &byte in bytes {
        bits = (bits << 8) | u32::from(byte);
        width += 8;
        while width >= 5 {
            width -= 5;
            out.push(ALPHABET[((bits >> width) & 0x1f) as usize] as char);
        }
    }
    // A trailing partial group is left-aligned in its own character.
    if width > 0 {
        out.push(ALPHABET[((bits << (5 - width)) & 0x1f) as usize] as char);
    }
    if padding {
        while !out.len().is_multiple_of(8) {
            out.push('=');
        }
    }
    out
}

/// Decodes, ignoring the noise base32 is usually printed with: whitespace,
/// `=` padding, and the `-` separators authenticator apps insert for
/// readability. Any other character is an error rather than a silent skip.
pub(crate) fn decode(input: &str) -> Result<Vec<u8>, String> {
    let (mut bits, mut width) = (0u32, 0u32);
    let mut out = Vec::new();

    for c in input.chars() {
        if c.is_whitespace() || c == '=' || c == '-' {
            continue;
        }
        let upper = c.to_ascii_uppercase() as u8;
        let value = ALPHABET
            .iter()
            .position(|&a| a == upper)
            .ok_or_else(|| format!("`{c}` is not a base32 character"))?;

        bits = (bits << 5) | value as u32;
        width += 5;
        if width >= 8 {
            width -= 8;
            out.push((bits >> width) as u8);
        }
    }
    Ok(out)
}

impl Tool for Base32Tool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        let result = match i.choice("direction") {
            "decode" => {
                let bytes = decode(text).map_err(|e| ToolError::invalid("text", e))?;
                String::from_utf8(bytes).map_err(|e| {
                    ToolError::invalid("text", format!("decoded bytes are not valid UTF-8: {e}"))
                })?
            }
            _ => encode(text.as_bytes(), i.bool("padding")),
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn run(text: &str, direction: &str, padding: bool) -> Result<Outputs, ToolError> {
        Base32Tool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("direction", direction)
                .with("padding", Value::Bool(padding)),
        )
    }

    fn ok(text: &str, direction: &str, padding: bool) -> String {
        run(text, direction, padding)
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    /// RFC 4648 section 10 — the published vectors, so the alphabet and the
    /// partial-group handling are pinned to the spec rather than to itself.
    #[test]
    fn rfc4648_test_vectors() {
        let cases = [
            ("", ""),
            ("f", "MY======"),
            ("fo", "MZXQ===="),
            ("foo", "MZXW6==="),
            ("foob", "MZXW6YQ="),
            ("fooba", "MZXW6YTB"),
            ("foobar", "MZXW6YTBOI======"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(ok(plain, "encode", true), encoded, "encode {plain:?}");
            assert_eq!(ok(encoded, "decode", true), plain, "decode {encoded:?}");
        }
    }

    #[test]
    fn padding_can_be_switched_off_and_decodes_either_way() {
        assert_eq!(ok("foo", "encode", false), "MZXW6");
        assert_eq!(ok("MZXW6", "decode", true), "foo");
    }

    /// The tolerances exist because base32 is usually read off a screen: the
    /// secret beside a QR code arrives with spaces, lowercase, and dashes.
    #[test]
    fn decoding_ignores_case_spaces_and_dashes() {
        assert_eq!(ok("mzxw 6ytb-oi", "decode", true), "foobar");
    }

    #[test]
    fn a_character_outside_the_alphabet_names_the_field() {
        let err = run("MZXW6!!!", "decode", true).unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "text", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn bytes_that_are_not_text_say_so_rather_than_mangling_them() {
        // 0xff is not valid UTF-8 on its own.
        let encoded = encode(&[0xff], true);
        let err = run(&encoded, "decode", true).unwrap_err();
        match err {
            ToolError::InvalidInput { field: "text", msg } => {
                assert!(msg.contains("UTF-8"), "{msg}");
            }
            other => panic!("{other:?}"),
        }
    }
}
