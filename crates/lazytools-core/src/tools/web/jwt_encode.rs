use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::{Sha256, Sha384, Sha512};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const ALGOS: &[&str] = &["HS256", "HS384", "HS512"];
const SAMPLE_PAYLOAD: &str = r#"{"sub": "1234567890", "name": "Jane Doe"}"#;

pub struct JwtEncodeTool {
    spec: ToolSpec,
}

impl Default for JwtEncodeTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.jwt-encode", "JWT Encode", Category::Web)
                .describe("Sign a JSON payload into an HMAC-signed JWT")
                .keywords(&[
                    "jwt", "sign", "encode", "jose", "hs256", "token", "bearer", "auth",
                ])
                .input(
                    Field::text("payload")
                        .multiline()
                        .mono()
                        .label("Payload")
                        .default(SAMPLE_PAYLOAD),
                )
                .option(
                    Field::select("alg", ALGOS)
                        .default("HS256")
                        .label("Algorithm"),
                )
                .option(
                    Field::secret("secret")
                        .label("Secret")
                        .help("The HMAC key. An empty key is legal, and signs nothing useful."),
                )
                .output(Field::text("token").multiline().mono().label("Token"))
                .output(Field::text("header").mono().label("Header")),
        }
    }
}

/// One wrapper per digest — `Hmac<D>` is a distinct type per algorithm, the
/// same reason `crypto::hmac` and `web.jwt-decode` each have one.
fn sign<D>(key: &[u8], msg: &[u8]) -> Vec<u8>
where
    D: hmac::EagerHash,
    Hmac<D>: Mac + hmac::digest::KeyInit,
{
    let mut mac = <Hmac<D> as hmac::digest::KeyInit>::new_from_slice(key)
        .expect("HMAC accepts a key of any length");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

impl Tool for JwtEncodeTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let payload = i.text("payload").trim();
        if payload.is_empty() {
            return Err(ToolError::invalid("payload", "payload must not be empty"));
        }
        // Parsed rather than passed through: a JWT whose payload isn't JSON is
        // one the next tool in the chain will reject, and better here than
        // after it has been pasted into a header somewhere.
        let value: serde_json::Value = serde_json::from_str(payload)
            .map_err(|e| ToolError::invalid("payload", format!("not valid JSON: {e}")))?;
        if !value.is_object() {
            return Err(ToolError::invalid(
                "payload",
                "a JWT payload must be a JSON object",
            ));
        }
        // Compact, as a JWT is transmitted — the pretty form would only inflate
        // the token.
        let compact = serde_json::to_string(&value)
            .map_err(|e| ToolError::invalid("payload", format!("could not re-serialize: {e}")))?;

        let alg = match i.choice("alg") {
            "" => "HS256",
            other => other,
        };
        let header = format!(r#"{{"alg":"{alg}","typ":"JWT"}}"#);

        let signing_input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(&header),
            URL_SAFE_NO_PAD.encode(&compact)
        );
        let key = i.text("secret").as_bytes();
        let signature = match alg {
            "HS384" => sign::<Sha384>(key, signing_input.as_bytes()),
            "HS512" => sign::<Sha512>(key, signing_input.as_bytes()),
            "HS256" => sign::<Sha256>(key, signing_input.as_bytes()),
            other => {
                return Err(ToolError::invalid(
                    "alg",
                    format!("unsupported algorithm: {other}"),
                ));
            }
        };

        let mut out = Outputs::new();
        out.set(
            "token",
            format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(signature)),
        );
        out.set("header", header);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(payload: &str, alg: &str, secret: &str) -> Result<Outputs, ToolError> {
        JwtEncodeTool::default().run(
            &Inputs::new()
                .with("payload", payload)
                .with("alg", alg)
                .with("secret", secret),
        )
    }

    fn token(payload: &str, alg: &str, secret: &str) -> String {
        run(payload, alg, secret)
            .unwrap()
            .get("token")
            .unwrap()
            .as_display()
    }

    /// The canonical jwt.io example: the one token every JWT library is checked
    /// against, so it pins the header spelling, the compact payload, and the
    /// signature all at once.
    #[test]
    fn matches_the_published_hs256_vector() {
        let out = token(
            r#"{"sub":"1234567890","name":"John Doe","iat":1516239022}"#,
            "HS256",
            "your-256-bit-secret",
        );
        assert_eq!(
            out,
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
             eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.\
             SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        );
    }

    /// The other half of the pair: what this tool signs, `web.jwt-decode` must
    /// verify. Anything else and the two tools disagree about the same token.
    #[test]
    fn the_decoder_verifies_what_the_encoder_signs() {
        use crate::tools::web::jwt_decode::JwtDecodeTool;

        for alg in ALGOS {
            let signed = token(r#"{"sub":"x"}"#, alg, "hunter2");
            let decoded = JwtDecodeTool::default()
                .run(
                    &Inputs::new()
                        .with("token", signed)
                        .with("secret", "hunter2"),
                )
                .unwrap();
            assert_eq!(
                decoded.get("verification").unwrap().as_display(),
                "valid signature",
                "{alg}"
            );
        }
    }

    #[test]
    fn a_different_secret_produces_a_different_signature() {
        let a = token(r#"{"sub":"x"}"#, "HS256", "one");
        let b = token(r#"{"sub":"x"}"#, "HS256", "two");
        assert_ne!(a, b);
        // The first two segments are the same — only the signature changed.
        assert_eq!(a.rsplit_once('.').unwrap().0, b.rsplit_once('.').unwrap().0);
    }

    #[test]
    fn the_algorithm_reaches_the_header() {
        let out = run(r#"{"a":1}"#, "HS512", "k").unwrap();
        assert_eq!(
            out.get("header").unwrap().as_display(),
            r#"{"alg":"HS512","typ":"JWT"}"#
        );
    }

    #[test]
    fn a_payload_that_is_not_a_json_object_names_the_field() {
        for payload in ["", "not json", "[1,2,3]", "\"a string\""] {
            let err = run(payload, "HS256", "k").unwrap_err();
            assert!(
                matches!(
                    err,
                    ToolError::InvalidInput {
                        field: "payload",
                        ..
                    }
                ),
                "{payload:?}: {err:?}"
            );
        }
    }
}
