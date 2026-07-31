use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const ALGOS: &[&str] = &["sha1", "sha256", "sha512"];

pub struct HmacTool {
    spec: ToolSpec,
}

impl Default for HmacTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("crypto.hmac", "HMAC", Category::Crypto)
                .describe("Compute the HMAC of text with a secret key")
                .keywords(&["hmac", "sha", "sign", "mac", "signature", "key"])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("algo", ALGOS)
                        .default("sha256")
                        .label("Algorithm"),
                )
                .option(Field::secret("key").label("Key"))
                .output(Field::text("digest").mono().label("Digest")),
        }
    }
}

/// Repeated wrapper for each algorithm — `Hmac<D>` is a different type per
/// algorithm, so they can't be merged into a single variable.
fn mac<D>(key: &[u8], msg: &[u8]) -> String
where
    D: hmac::EagerHash,
    Hmac<D>: Mac + hmac::digest::KeyInit,
{
    let mut mac = <Hmac<D> as hmac::digest::KeyInit>::new_from_slice(key)
        .expect("HMAC accepts a key of any length");
    mac.update(msg);
    hex::encode(mac.finalize().into_bytes())
}

impl Tool for HmacTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let key = i.text("key");
        if key.is_empty() {
            return Err(ToolError::invalid("key", "key must not be empty"));
        }
        let (key, msg) = (key.as_bytes(), i.text("text").as_bytes());

        let digest = match i.choice("algo") {
            "sha1" => mac::<Sha1>(key, msg),
            "sha512" => mac::<Sha512>(key, msg),
            "sha256" => mac::<Sha256>(key, msg),
            other => {
                return Err(ToolError::invalid(
                    "algo",
                    format!("unsupported algorithm: {other}"),
                ));
            }
        };
        Ok(Outputs::one("digest", digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, algo: &str, key: &str) -> Result<Outputs, ToolError> {
        HmacTool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("algo", algo)
                .with("key", key),
        )
    }

    /// Vectors from RFC 2202 / RFC 4231, key = "key", msg = "The quick brown fox jumps over the lazy dog".
    #[test]
    fn known_vectors() {
        let msg = "The quick brown fox jumps over the lazy dog";
        let cases = [
            ("sha1", "de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9"),
            (
                "sha256",
                "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
            ),
        ];
        for (algo, expected) in cases {
            let out = run(msg, algo, "key").unwrap();
            assert_eq!(
                out.get("digest").unwrap().as_display(),
                expected,
                "algo={algo}"
            );
        }
    }

    #[test]
    fn empty_key_names_the_field() {
        let err = run("hello", "sha256", "").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "key", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn unknown_algo_names_the_field() {
        let err = run("hello", "bogus", "k").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "algo", .. }),
            "{err:?}"
        );
    }
}
