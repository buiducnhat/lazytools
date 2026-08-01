use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::{Sha256, Sha384, Sha512};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct JwtDecodeTool {
    spec: ToolSpec,
}

impl Default for JwtDecodeTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.jwt-decode", "JWT Decode", Category::Web)
                .describe("Decode a JWT and optionally verify its HMAC signature")
                .keywords(&["jwt", "token", "jose", "claims", "bearer", "auth"])
                .input(Field::text("token").multiline().mono().label("Token"))
                .option(
                    Field::secret("secret")
                        .label("Secret")
                        .help("HMAC secret — leave empty to skip verification"),
                )
                .output(Field::text("header").multiline().mono().label("Header"))
                .output(Field::text("payload").multiline().mono().label("Payload"))
                .output(Field::text("verification").label("Verification")),
        }
    }
}

/// One wrapper per digest, same reason as `crypto::hmac::mac`: `Hmac<D>` is a
/// distinct type per algorithm, so the three arms can't share a variable.
fn verify<D>(key: &[u8], msg: &[u8], sig: &[u8]) -> bool
where
    D: hmac::EagerHash,
    Hmac<D>: Mac + hmac::digest::KeyInit,
{
    let mut mac = <Hmac<D> as hmac::digest::KeyInit>::new_from_slice(key)
        .expect("HMAC accepts a key of any length");
    mac.update(msg);
    mac.verify_slice(sig).is_ok()
}

/// Decodes one base64url segment into pretty-printed JSON.
fn decode_segment(part: &str, index: usize, what: &str) -> Result<String, ToolError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|_| ToolError::invalid("token", format!("part {index} is not valid base64url")))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| ToolError::invalid("token", format!("{what} is not valid JSON")))?;
    serde_json::to_string_pretty(&value)
        .map_err(|e| ToolError::invalid("token", format!("{what} could not be re-serialized: {e}")))
}

impl Tool for JwtDecodeTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let token = i.text("token").trim();
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(ToolError::invalid(
                "token",
                format!("expected 3 dot-separated parts, got {}", parts.len()),
            ));
        }

        let header = decode_segment(parts[0], 0, "header")?;
        let payload = decode_segment(parts[1], 1, "payload")?;

        // Deliberately no `exp` / `nbf` check: that needs a clock, and this tool is
        // otherwise a pure function of its input. Expiry checking belongs with the
        // clock-dependent Web tools, not here.
        let alg = serde_json::from_str::<serde_json::Value>(&header)
            .ok()
            .and_then(|h| h.get("alg").and_then(|a| a.as_str()).map(str::to_owned))
            .unwrap_or_else(|| "none".to_string());

        let secret = i.text("secret");
        let verification = if secret.is_empty() {
            "not verified (no secret provided)".to_string()
        } else {
            // The signature covers the raw `<header>.<payload>` base64 text, not the
            // decoded JSON — re-encoding would produce a different byte string.
            let signed = format!("{}.{}", parts[0], parts[1]);
            let (msg, key) = (signed.as_bytes(), secret.as_bytes());
            let sig = URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| {
                ToolError::invalid("token", "part 2 is not valid base64url".to_string())
            })?;

            let ok = match alg.as_str() {
                "HS256" => verify::<Sha256>(key, msg, &sig),
                "HS384" => verify::<Sha384>(key, msg, &sig),
                "HS512" => verify::<Sha512>(key, msg, &sig),
                other => {
                    return Ok(outputs(
                        header,
                        payload,
                        format!("unsupported algorithm: {other} (decoded only)"),
                    ));
                }
            };
            if ok {
                "valid signature"
            } else {
                "INVALID signature"
            }
            .to_string()
        };

        Ok(outputs(header, payload, verification))
    }
}

fn outputs(header: String, payload: String, verification: String) -> Outputs {
    let mut o = Outputs::new();
    o.set("header", header);
    o.set("payload", payload);
    o.set("verification", verification);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(token: &str, secret: &str) -> Result<Outputs, ToolError> {
        JwtDecodeTool::default().run(&Inputs::new().with("token", token).with("secret", secret))
    }

    fn field(out: &Outputs, key: &str) -> String {
        out.get(key).unwrap().as_display()
    }

    /// Builds a token so the test doesn't depend on a hardcoded vector that could
    /// silently rot if the signing input ever changed.
    fn sign_hs256(header: &str, payload: &str, secret: &str) -> String {
        let h = URL_SAFE_NO_PAD.encode(header);
        let p = URL_SAFE_NO_PAD.encode(payload);
        let signed = format!("{h}.{p}");
        let mut mac = <Hmac<Sha256> as hmac::digest::KeyInit>::new_from_slice(secret.as_bytes())
            .expect("HMAC accepts a key of any length");
        mac.update(signed.as_bytes());
        format!(
            "{signed}.{}",
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        )
    }

    #[test]
    fn valid_hs256_signature() {
        let token = sign_hs256(r#"{"alg":"HS256","typ":"JWT"}"#, r#"{"sub":"1234"}"#, "key");
        let out = run(&token, "key").unwrap();
        assert_eq!(field(&out, "verification"), "valid signature");
        assert!(field(&out, "payload").contains("\"sub\""), "{out:?}");
        assert!(field(&out, "header").contains("HS256"), "{out:?}");
    }

    #[test]
    fn wrong_secret_reports_invalid() {
        let token = sign_hs256(r#"{"alg":"HS256"}"#, r#"{"sub":"1234"}"#, "key");
        let out = run(&token, "wrong").unwrap();
        assert_eq!(field(&out, "verification"), "INVALID signature");
    }

    #[test]
    fn no_secret_decodes_without_verifying() {
        let token = sign_hs256(r#"{"alg":"HS256"}"#, r#"{"sub":"1234"}"#, "key");
        let out = run(&token, "").unwrap();
        assert_eq!(
            field(&out, "verification"),
            "not verified (no secret provided)"
        );
    }

    #[test]
    fn two_part_token_names_the_field() {
        let err = run("aaa.bbb", "").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "token", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn broken_base64_names_the_field() {
        let err = run("!!!.bbb.ccc", "").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "token", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn non_json_segment_names_the_field() {
        let token = format!(
            "{}.{}.sig",
            URL_SAFE_NO_PAD.encode("not json"),
            URL_SAFE_NO_PAD.encode("{}")
        );
        let err = run(&token, "").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "token", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn unsupported_alg_still_decodes() {
        let token = format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256"}"#),
            URL_SAFE_NO_PAD.encode(r#"{"sub":"x"}"#),
            URL_SAFE_NO_PAD.encode("signature-bytes")
        );
        let out = run(&token, "key").unwrap();
        assert_eq!(
            field(&out, "verification"),
            "unsupported algorithm: RS256 (decoded only)"
        );
        assert!(field(&out, "payload").contains("\"sub\""), "{out:?}");
    }

    #[test]
    fn missing_alg_is_treated_as_none() {
        let token = format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode("{}"),
            URL_SAFE_NO_PAD.encode(r#"{"sub":"x"}"#),
            URL_SAFE_NO_PAD.encode("sig")
        );
        let out = run(&token, "key").unwrap();
        assert_eq!(
            field(&out, "verification"),
            "unsupported algorithm: none (decoded only)"
        );
    }
}
