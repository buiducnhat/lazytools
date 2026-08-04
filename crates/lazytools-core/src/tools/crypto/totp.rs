use chrono::Utc;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

const ALGOS: &[&str] = &["sha1", "sha256", "sha512"];

pub struct TotpTool {
    spec: ToolSpec,
}

impl Default for TotpTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("crypto.totp", "TOTP Code", Category::Crypto)
                .describe("Generate a time-based one-time password from a base32 secret")
                .keywords(&[
                    "totp",
                    "otp",
                    "2fa",
                    "mfa",
                    "authenticator",
                    "hotp",
                    "rfc6238",
                    "token",
                ])
                .input(
                    Field::secret("secret").label("Secret").help(
                        "Base32, as printed next to the QR code (spaces and `=` are ignored)",
                    ),
                )
                .option(
                    Field::select("algo", ALGOS)
                        .default("sha1")
                        .label("Algorithm")
                        .help("Authenticator apps use SHA-1 unless the issuer says otherwise"),
                )
                .option(Field::number("digits", 6, 10).default(6).label("Digits"))
                .option(
                    Field::number("period", 5, 300)
                        .default(30)
                        .label("Period (seconds)"),
                )
                .output(Field::text("code").mono().label("Code"))
                .output(Field::text("expires_in").label("Expires in"))
                .output(Field::text("next").mono().label("Next code"))
                .output(Field::text("counter").label("Counter"))
                // The code is a function of the clock, so it goes stale on its own. This
                // mode runs it on open and lets the confirm key ask for the current one.
                .mode(RunMode::Generate),
        }
    }
}

/// RFC 4648 base32, decode only. `data-encoding` would do this in one line, but it is a
/// whole dependency for ~20 lines used by exactly one tool — and the secret shown next
/// to a QR code is the only base32 anything in this program will ever see.
fn base32_decode(input: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let mut bits: u32 = 0;
    let mut width: u32 = 0;
    let mut out = Vec::new();

    for c in input.chars() {
        // Padding and the spaces authenticator apps print for readability are noise.
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

    if out.is_empty() {
        return Err("secret decodes to zero bytes".to_string());
    }
    Ok(out)
}

/// The HOTP truncation of RFC 4226, evaluated at an explicit counter.
///
/// Kept separate from `run()` — and taking the counter rather than reading the clock —
/// so the RFC 6238 test vectors can be asserted directly. A tool whose only entry point
/// reads "now" is a tool whose correctness cannot be tested.
fn hotp(key: &[u8], counter: u64, digits: u32, algo: &str) -> Result<u32, String> {
    fn digest<D>(key: &[u8], msg: &[u8]) -> Vec<u8>
    where
        D: hmac::EagerHash,
        Hmac<D>: Mac + hmac::digest::KeyInit,
    {
        let mut mac = <Hmac<D> as hmac::digest::KeyInit>::new_from_slice(key)
            .expect("HMAC accepts a key of any length");
        mac.update(msg);
        mac.finalize().into_bytes().to_vec()
    }

    let msg = counter.to_be_bytes();
    let hash = match algo {
        "sha256" => digest::<Sha256>(key, &msg),
        "sha512" => digest::<Sha512>(key, &msg),
        "sha1" => digest::<Sha1>(key, &msg),
        other => return Err(format!("unsupported algorithm: {other}")),
    };

    // Dynamic truncation: the low nibble of the last byte picks the offset.
    let offset = (hash[hash.len() - 1] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        hash[offset] & 0x7f,
        hash[offset + 1],
        hash[offset + 2],
        hash[offset + 3],
    ]);
    Ok(binary % 10u32.pow(digits))
}

impl Tool for TotpTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let secret = i.text("secret").trim();
        if secret.is_empty() {
            return Err(ToolError::invalid("secret", "secret must not be empty"));
        }
        let key = base32_decode(secret).map_err(|e| ToolError::invalid("secret", e))?;

        let algo = i.choice("algo");
        let digits = i.num("digits").clamp(6, 10) as u32;
        let period = i.num("period").clamp(5, 300) as u64;

        // `timestamp()` is seconds since the epoch in UTC — TOTP has no timezone, and
        // reading a local clock here would produce codes that only work in one place.
        let now = Utc::now().timestamp().max(0) as u64;
        let counter = now / period;

        let code = hotp(&key, counter, digits, algo).map_err(|e| ToolError::invalid("algo", e))?;
        let next =
            hotp(&key, counter + 1, digits, algo).map_err(|e| ToolError::invalid("algo", e))?;

        let width = digits as usize;
        let mut out = Outputs::new();
        out.set("code", format!("{code:0width$}"));
        out.set("expires_in", format!("{}s", period - (now % period)));
        out.set("next", format!("{next:0width$}"));
        out.set("counter", counter.to_string());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 Appendix B. The seeds are ASCII digits repeated to each algorithm's
    /// block size, passed as raw key bytes so the vectors are exact.
    const SEED_SHA1: &[u8] = b"12345678901234567890";
    const SEED_SHA256: &[u8] = b"12345678901234567890123456789012";
    const SEED_SHA512: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

    fn at(seed: &[u8], time: u64, algo: &str) -> String {
        let code = hotp(seed, time / 30, 8, algo).unwrap();
        format!("{code:08}")
    }

    #[test]
    fn rfc6238_sha1_vectors() {
        let cases = [
            (59u64, "94287082"),
            (1111111109, "07081804"),
            (1111111111, "14050471"),
            (1234567890, "89005924"),
            (2000000000, "69279037"),
            (20000000000, "65353130"),
        ];
        for (time, want) in cases {
            assert_eq!(at(SEED_SHA1, time, "sha1"), want, "T={time}");
        }
    }

    #[test]
    fn rfc6238_sha256_and_sha512_vectors() {
        assert_eq!(at(SEED_SHA256, 59, "sha256"), "46119246");
        assert_eq!(at(SEED_SHA512, 59, "sha512"), "90693936");
        assert_eq!(at(SEED_SHA256, 1111111109, "sha256"), "68084774");
        assert_eq!(at(SEED_SHA512, 1111111109, "sha512"), "25091201");
    }

    /// Truncating to fewer digits keeps the low-order end, so a 6-digit code is the
    /// tail of the 8-digit one for the same counter.
    #[test]
    fn digit_count_truncates_from_the_left() {
        let eight = hotp(SEED_SHA1, 1, 8, "sha1").unwrap();
        let six = hotp(SEED_SHA1, 1, 6, "sha1").unwrap();
        assert_eq!(six, eight % 1_000_000);
    }

    #[test]
    fn base32_decodes_the_standard_alphabet() {
        assert_eq!(
            base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap(),
            SEED_SHA1
        );
        // "Hi!" — an odd byte count, so the last group is padded.
        assert_eq!(base32_decode("JBUSC===").unwrap(), b"Hi!");
    }

    /// Authenticator apps print the secret in lowercase, in space-separated groups,
    /// sometimes hyphenated. All three must decode to the same key.
    #[test]
    fn base32_ignores_case_spacing_and_padding() {
        let canonical = base32_decode("GEZDGNBVGY3TQOJQ").unwrap();
        for variant in [
            "gezdgnbvgy3tqojq",
            "GEZD GNBV GY3T QOJQ",
            "GEZD-GNBV-GY3T-QOJQ",
            "GEZDGNBVGY3TQOJQ====",
        ] {
            assert_eq!(base32_decode(variant).unwrap(), canonical, "{variant}");
        }
    }

    #[test]
    fn a_bad_secret_names_the_field() {
        for bad in ["", "   ", "not base32 — 1889!"] {
            let err = TotpTool::default()
                .run(
                    &Inputs::new()
                        .with("secret", bad)
                        .with("algo", "sha1")
                        .with("digits", 6i64)
                        .with("period", 30i64),
                )
                .unwrap_err();
            assert!(
                matches!(
                    err,
                    ToolError::InvalidInput {
                        field: "secret",
                        ..
                    }
                ),
                "{bad:?}: {err:?}"
            );
        }
    }

    /// The live path: a real code, of the requested width, that agrees with `hotp` at
    /// the counter the tool reports.
    #[test]
    fn the_live_code_matches_its_own_reported_counter() {
        let out = TotpTool::default()
            .run(
                &Inputs::new()
                    .with("secret", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")
                    .with("algo", "sha1")
                    .with("digits", 6i64)
                    .with("period", 30i64),
            )
            .unwrap();

        let code = out.get("code").unwrap().as_display();
        let counter: u64 = out.get("counter").unwrap().as_display().parse().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()), "{code}");
        assert_eq!(
            code,
            format!("{:06}", hotp(SEED_SHA1, counter, 6, "sha1").unwrap())
        );
        assert_eq!(
            out.get("next").unwrap().as_display(),
            format!("{:06}", hotp(SEED_SHA1, counter + 1, 6, "sha1").unwrap())
        );
    }
}
