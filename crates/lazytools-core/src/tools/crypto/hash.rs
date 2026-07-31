use md5::{Digest, Md5};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const ALGOS: &[&str] = &["md5", "sha1", "sha256", "sha512"];

pub struct HashTool {
    spec: ToolSpec,
}

impl Default for HashTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("crypto.hash", "Hash Text", Category::Crypto)
                .describe("Băm văn bản bằng MD5/SHA")
                .keywords(&[
                    "md5", "sha", "sha1", "sha256", "sha512", "checksum", "digest",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("algo", ALGOS)
                        .default("md5")
                        .label("Algorithm"),
                )
                .output(Field::text("digest").mono().label("Digest")),
        }
    }
}

impl Tool for HashTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let bytes = i.text("text").as_bytes();
        let digest = match i.choice("algo") {
            "md5" => hex::encode(Md5::digest(bytes)),
            "sha1" => hex::encode(Sha1::digest(bytes)),
            "sha256" => hex::encode(Sha256::digest(bytes)),
            "sha512" => hex::encode(Sha512::digest(bytes)),
            other => {
                return Err(ToolError::invalid(
                    "algo",
                    format!("thuật toán không hỗ trợ: {other}"),
                ));
            }
        };
        Ok(Outputs::one("digest", digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, algo: &str) -> Result<Outputs, ToolError> {
        let inputs = Inputs::new().with("text", text).with("algo", algo);
        HashTool::default().run(&inputs)
    }

    #[test]
    fn known_vectors() {
        let cases = [
            ("", "md5", "d41d8cd98f00b204e9800998ecf8427e"),
            ("hello world", "md5", "5eb63bbbe01eeed093cb22bb8f5acdc3"),
            (
                "hello world",
                "sha1",
                "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed",
            ),
            (
                "hello world",
                "sha256",
                "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
            ),
        ];

        for (text, algo, expected) in cases {
            let out = run(text, algo).expect("hash phải chạy được");
            assert_eq!(
                out.get("digest").unwrap().as_display(),
                expected,
                "text={text:?} algo={algo}"
            );
        }
    }

    #[test]
    fn unknown_algo_is_invalid_input() {
        let err = run("hello", "bogus").expect_err("algo lạ phải trả lỗi");
        assert!(
            matches!(err, ToolError::InvalidInput { field: "algo", .. }),
            "kỳ vọng InvalidInput trên field `algo`, nhận: {err:?}"
        );
    }
}
