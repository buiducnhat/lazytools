use unicode_segmentation::UnicodeSegmentation;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct StatsTool {
    spec: ToolSpec,
}

impl Default for StatsTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.stats", "Text Stats", Category::Text)
                .describe("Count characters, words, lines, and bytes in text")
                .keywords(&[
                    "stats",
                    "count",
                    "words",
                    "characters",
                    "lines",
                    "bytes",
                    "wc",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .output(Field::text("characters").label("Characters"))
                .output(Field::text("words").label("Words"))
                .output(Field::text("lines").label("Lines"))
                .output(Field::text("bytes").label("Bytes"))
                .output(Field::text("codepoints").label("Code points")),
        }
    }
}

impl Tool for StatsTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    // Counting definitions, spelled out so they aren't re-litigated later:
    //   characters — grapheme clusters, i.e. what a reader calls "one character".
    //   codepoints — `char`s. Differs from `characters` for combining marks and
    //                composed emoji; having both is the point.
    //   words      — Unicode word boundaries, not whitespace splitting.
    //   lines      — 0 for empty input, otherwise `str::lines()` (a trailing
    //                newline does not add an extra line).
    //   bytes      — UTF-8 length.
    // Values are written as `Field::text` rather than `Value::Num` because both
    // frontends render outputs as strings anyway, and a numeric value would not
    // match the declared `FieldKind::Text`.
    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");

        let characters = text.graphemes(true).count();
        let codepoints = text.chars().count();
        let words = text.unicode_words().count();
        let lines = if text.is_empty() {
            0
        } else {
            text.lines().count()
        };
        let bytes = text.len();

        let mut out = Outputs::new();
        out.set("characters", characters.to_string());
        out.set("words", words.to_string());
        out.set("lines", lines.to_string());
        out.set("bytes", bytes.to_string());
        out.set("codepoints", codepoints.to_string());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stat(text: &str, key: &str) -> usize {
        StatsTool::default()
            .run(&Inputs::new().with("text", text))
            .unwrap()
            .get(key)
            .unwrap()
            .as_display()
            .parse()
            .unwrap()
    }

    #[test]
    fn empty_text_is_all_zeros() {
        for key in ["characters", "words", "lines", "bytes", "codepoints"] {
            assert_eq!(stat("", key), 0, "{key}");
        }
    }

    #[test]
    fn ascii_sentence() {
        assert_eq!(stat("hello world", "characters"), 11);
        assert_eq!(stat("hello world", "words"), 2);
        assert_eq!(stat("hello world", "lines"), 1);
        assert_eq!(stat("hello world", "bytes"), 11);
        assert_eq!(stat("hello world", "codepoints"), 11);
    }

    /// The whole reason both counts exist: a composed emoji is one character but
    /// several code points.
    #[test]
    fn composed_emoji_is_one_character_but_many_codepoints() {
        let family = "👨‍👩‍👧";
        assert_eq!(stat(family, "characters"), 1);
        assert!(
            stat(family, "codepoints") > stat(family, "characters"),
            "codepoints={} characters={}",
            stat(family, "codepoints"),
            stat(family, "characters")
        );
    }

    #[test]
    fn trailing_newline_does_not_add_a_line() {
        assert_eq!(stat("a\nb", "lines"), 2);
        assert_eq!(stat("a\nb\n", "lines"), 2);
    }

    #[test]
    fn multibyte_text_counts_bytes_separately() {
        assert_eq!(stat("xin chào", "characters"), 8);
        assert_eq!(stat("xin chào", "words"), 2);
        assert!(stat("xin chào", "bytes") > 8);
    }
}
