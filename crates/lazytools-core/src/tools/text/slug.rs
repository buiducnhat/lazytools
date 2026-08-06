use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const SEPARATORS: &[&str] = &["-", "_", "."];

pub struct SlugTool {
    spec: ToolSpec,
}

impl Default for SlugTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.slug", "Slugify", Category::Text)
                .describe("Turn a title into a URL-safe slug")
                .keywords(&[
                    "slug",
                    "slugify",
                    "url",
                    "permalink",
                    "kebab",
                    "filename",
                    "ascii",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("separator", SEPARATORS)
                        .default("-")
                        .label("Separator"),
                )
                .option(Field::toggle("lowercase").default(true).label("Lowercase"))
                .option(
                    Field::number("max_length", 0, 200)
                        .default(0)
                        .label("Max length")
                        .help("0 leaves the slug at its natural length"),
                )
                .output(Field::text("slug").mono().label("Slug")),
        }
    }
}

/// The Latin-1 and Latin Extended-A letters people actually paste in — accents,
/// the Scandinavian and German vowels, and the ligatures that expand to two
/// letters. Anything outside the table is dropped rather than transliterated:
/// a wrong guess at Greek or Cyrillic is worse than an honest omission, and a
/// full transliteration table is a dependency, not a tool.
fn fold(c: char) -> Option<&'static str> {
    let s = match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è'..='ë' | 'ē' | 'ę' | 'ě' => "e",
        'ì'..='ï' | 'ī' | 'į' => "i",
        'ñ' | 'ń' | 'ň' => "n",
        'ò'..='ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ř' => "r",
        'ś' | 'š' | 'ş' => "s",
        'ß' => "ss",
        'ť' | 'ţ' => "t",
        'ù'..='ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'ð' => "d",
        'þ' => "th",
        '&' => "and",
        _ => return None,
    };
    Some(s)
}

impl Tool for SlugTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let separator = match i.choice("separator") {
            "_" => '_',
            "." => '.',
            _ => '-',
        };
        let lowercase = i.bool("lowercase");
        let max_length = i.num("max_length").max(0) as usize;

        let mut slug = String::new();
        // Runs of anything unusable collapse into a single separator, and a
        // leading one is never emitted — hence "pending" rather than pushing.
        let mut pending_separator = false;

        for c in i.text("text").chars() {
            let folded = if c.is_ascii_alphanumeric() {
                let mut buf = [0u8; 4];
                Some(c.encode_utf8(&mut buf).to_string())
            } else {
                // Fold on the lowercase form so `Ü` and `ü` both become `u`,
                // then restore the case if the user asked to keep it.
                c.to_lowercase().next().and_then(fold).map(|s| {
                    if lowercase {
                        s.to_string()
                    } else {
                        restore_case(s, c)
                    }
                })
            };

            match folded {
                Some(text) => {
                    if pending_separator && !slug.is_empty() {
                        slug.push(separator);
                    }
                    pending_separator = false;
                    slug.push_str(&text);
                }
                None => pending_separator = true,
            }
        }

        if lowercase {
            slug = slug.to_lowercase();
        }
        if max_length > 0 && slug.chars().count() > max_length {
            slug = slug.chars().take(max_length).collect();
            // Never end on the separator the truncation happened to land on.
            slug = slug.trim_end_matches(separator).to_string();
        }

        Ok(Outputs::one("slug", slug))
    }
}

/// `Ü` folded to `u` comes back as `U`; `ß` to `ss` comes back as `Ss`.
fn restore_case(folded: &str, original: char) -> String {
    if !original.is_uppercase() {
        return folded.to_string();
    }
    let mut chars = folded.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn run(text: &str, separator: &str, lowercase: bool, max_length: i64) -> String {
        SlugTool::default()
            .run(
                &Inputs::new()
                    .with("text", text)
                    .with("separator", separator)
                    .with("lowercase", Value::Bool(lowercase))
                    .with("max_length", max_length),
            )
            .unwrap()
            .get("slug")
            .unwrap()
            .as_display()
    }

    fn slug(text: &str) -> String {
        run(text, "-", true, 0)
    }

    #[test]
    fn a_title_becomes_a_url_safe_slug() {
        assert_eq!(slug("Hello, World!"), "hello-world");
        assert_eq!(slug("  Rust 1.97 released  "), "rust-1-97-released");
    }

    /// Runs of punctuation and whitespace collapse, and the slug never starts
    /// or ends on a separator — the two things that make a slug look broken.
    #[test]
    fn separators_never_double_up_or_sit_at_the_edges() {
        assert_eq!(slug("--a   ///  b--"), "a-b");
        assert_eq!(slug("!!!"), "");
    }

    #[test]
    fn accented_letters_fold_to_ascii() {
        assert_eq!(slug("Crème Brûlée"), "creme-brulee");
        assert_eq!(slug("Straße"), "strasse");
        assert_eq!(slug("Æther & Ørsted"), "aether-and-orsted");
    }

    /// A wrong transliteration is worse than a missing one, so scripts outside
    /// the table drop out instead of being guessed at.
    #[test]
    fn scripts_outside_the_table_are_dropped_not_guessed() {
        assert_eq!(slug("Привет мир"), "");
        assert_eq!(slug("日本 language"), "language");
    }

    #[test]
    fn the_separator_and_case_are_configurable() {
        assert_eq!(run("Hello World", "_", true, 0), "hello_world");
        assert_eq!(run("Hello World", ".", true, 0), "hello.world");
        assert_eq!(run("Hello Wörld", "-", false, 0), "Hello-World");
    }

    #[test]
    fn truncation_does_not_leave_a_trailing_separator() {
        assert_eq!(run("hello world again", "-", true, 11), "hello-world");
        // The cut lands exactly on the separator after `hello`.
        assert_eq!(run("hello world", "-", true, 6), "hello");
        assert_eq!(run("hello world", "-", true, 0), "hello-world");
    }

    #[test]
    fn empty_input_produces_an_empty_slug_rather_than_an_error() {
        assert_eq!(slug(""), "");
    }
}
