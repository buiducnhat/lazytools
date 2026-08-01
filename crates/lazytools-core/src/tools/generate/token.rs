use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use rand::RngExt;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

const ENCODINGS: &[&str] = &["hex", "base64", "base64url"];

pub struct TokenTool {
    spec: ToolSpec,
}

impl Default for TokenTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("generate.token", "Random Token", Category::Generate)
                .describe("Generate a random token of N bytes")
                .keywords(&[
                    "token", "random", "bytes", "secret", "api-key", "nonce", "entropy",
                ])
                .option(Field::number("bytes", 8, 256).default(32i64).label("Bytes"))
                .option(
                    Field::select("encoding", ENCODINGS)
                        .default("hex")
                        .label("Encoding"),
                )
                .output(Field::text("result").mono().label("Token"))
                .mode(RunMode::Generate),
        }
    }
}

impl Tool for TokenTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let len = i.num("bytes").clamp(8, 256) as usize;
        let mut buf = vec![0u8; len];
        rand::rng().fill(&mut buf[..]);

        let result = match i.choice("encoding") {
            "base64" => STANDARD.encode(&buf),
            "base64url" => URL_SAFE_NO_PAD.encode(&buf),
            _ => hex::encode(&buf),
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(bytes: i64, encoding: &str) -> String {
        TokenTool::default()
            .run(
                &Inputs::new()
                    .with("bytes", bytes)
                    .with("encoding", encoding),
            )
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn hex_is_two_characters_per_byte() {
        let out = ok(32, "hex");
        assert_eq!(out.len(), 64);
        assert!(out.chars().all(|c| c.is_ascii_hexdigit()), "{out}");
    }

    #[test]
    fn every_encoding_round_trips_to_the_requested_byte_count() {
        assert_eq!(hex::decode(ok(32, "hex")).unwrap().len(), 32);
        assert_eq!(STANDARD.decode(ok(32, "base64")).unwrap().len(), 32);
        assert_eq!(
            URL_SAFE_NO_PAD.decode(ok(32, "base64url")).unwrap().len(),
            32
        );
    }

    #[test]
    fn base64url_avoids_characters_that_need_escaping() {
        let out = ok(64, "base64url");
        for bad in ['+', '/', '='] {
            assert!(!out.contains(bad), "{bad:?} must not appear in {out}");
        }
    }

    #[test]
    fn consecutive_calls_differ() {
        assert_ne!(ok(32, "hex"), ok(32, "hex"));
    }
}
