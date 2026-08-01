use rand::seq::IndexedRandom;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

const CHARSETS: &[&str] = &[
    "alphanumeric",
    "alphanumeric+symbols",
    "letters",
    "digits",
    "hex",
];

const LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?";
const HEX: &str = "0123456789abcdef";

pub struct PasswordTool {
    spec: ToolSpec,
}

impl Default for PasswordTool {
    fn default() -> Self {
        Self {
            // A `charset` select rather than three toggles: the CLI can't express a
            // `Toggle` that defaults to `true` (see `cli::apply_kind`), and one box
            // beats three in the form.
            spec: ToolSpec::new("generate.password", "Password", Category::Generate)
                .describe("Generate a random password")
                .keywords(&["password", "random", "secret", "passphrase", "credentials"])
                .option(
                    Field::number("length", 4, 128)
                        .default(20i64)
                        .label("Length"),
                )
                .option(
                    Field::select("charset", CHARSETS)
                        .default("alphanumeric+symbols")
                        .label("Character set"),
                )
                .output(Field::text("result").mono().label("Password"))
                .mode(RunMode::Generate),
        }
    }
}

fn alphabet(charset: &str) -> String {
    match charset {
        "letters" => LETTERS.to_string(),
        "digits" => DIGITS.to_string(),
        "hex" => HEX.to_string(),
        "alphanumeric" => format!("{LETTERS}{DIGITS}"),
        _ => format!("{LETTERS}{DIGITS}{SYMBOLS}"),
    }
}

impl Tool for PasswordTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let length = i.num("length").clamp(4, 128) as usize;
        let alphabet: Vec<char> = alphabet(i.choice("charset")).chars().collect();

        // Uniform over the alphabet, with no "must contain a digit" rule: such rules
        // lower entropy and surprise users who asked for a specific character set.
        let mut rng = rand::rng();
        let result: String = (0..length)
            .filter_map(|_| alphabet.choose(&mut rng).copied())
            .collect();

        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(length: i64, charset: &str) -> String {
        PasswordTool::default()
            .run(
                &Inputs::new()
                    .with("length", length)
                    .with("charset", charset),
            )
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn honors_the_requested_length() {
        for length in [4i64, 20, 64, 128] {
            assert_eq!(ok(length, "alphanumeric").chars().count(), length as usize);
        }
    }

    #[test]
    fn every_character_comes_from_the_chosen_set() {
        for charset in CHARSETS {
            let allowed = alphabet(charset);
            let password = ok(64, charset);
            for c in password.chars() {
                assert!(
                    allowed.contains(c),
                    "{c:?} is not in the {charset} alphabet"
                );
            }
        }
    }

    /// Two 32-char draws colliding has probability ~0 — if this ever fails, the RNG
    /// is not actually random.
    #[test]
    fn consecutive_calls_differ() {
        assert_ne!(ok(32, "alphanumeric"), ok(32, "alphanumeric"));
    }

    /// `spec_invariants` requires generators to succeed on their declared defaults.
    #[test]
    fn default_inputs_produce_a_password() {
        let tool = PasswordTool::default();
        let out = tool.run(&Inputs::new().with("length", 20i64)).unwrap();
        assert_eq!(out.get("result").unwrap().as_display().chars().count(), 20);
    }
}
