use regex::RegexBuilder;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

/// How many matches get spelled out in the `matches` field. `count` still reports the
/// real total — the cap is on the rendering, not on the search, because a pattern like
/// `.` against a 10MB file opened through `Ctrl+O` would otherwise build a listing far
/// larger than the input it describes.
const MAX_LISTED: usize = 200;

pub struct RegexTool {
    spec: ToolSpec,
}

impl Default for RegexTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.regex", "Regex Tester", Category::Text)
                .describe("Test a regular expression against text and see every match")
                .keywords(&[
                    "regex", "regexp", "pattern", "match", "search", "replace", "capture",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .input(
                    Field::text("pattern")
                        .mono()
                        .label("Pattern")
                        .help("Rust `regex` syntax — pass as the second positional argument"),
                )
                .option(
                    Field::toggle("ignore_case")
                        .default(false)
                        .label("Ignore case"),
                )
                .option(
                    Field::toggle("multiline")
                        .default(false)
                        .label("Multiline")
                        .help("`^` and `$` match at every line boundary"),
                )
                .option(
                    Field::toggle("dot_matches_newline")
                        .default(false)
                        .label("Dot matches newline"),
                )
                .option(
                    Field::text("replace")
                        .mono()
                        .label("Replacement")
                        .help("`$1` / `${name}` refer to capture groups; empty deletes each match"),
                )
                .output(Field::text("count").label("Matches"))
                .output(
                    Field::text("matches")
                        .multiline()
                        .mono()
                        .label("Match list"),
                )
                .output(Field::text("replaced").multiline().mono().label("Replaced")),
        }
    }
}

impl Tool for RegexTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let pattern = i.text("pattern");
        if pattern.is_empty() {
            return Err(ToolError::invalid("pattern", "pattern must not be empty"));
        }

        let re = RegexBuilder::new(pattern)
            .case_insensitive(i.bool("ignore_case"))
            .multi_line(i.bool("multiline"))
            .dot_matches_new_line(i.bool("dot_matches_newline"))
            .build()
            // The `regex` crate's own error already points at the offending construct
            // and is multi-line; passing it through beats paraphrasing it.
            .map_err(|e| ToolError::invalid("pattern", e.to_string()))?;

        let text = i.text("text");
        let mut listing = Vec::new();
        let mut count = 0usize;

        for caps in re.captures_iter(text) {
            count += 1;
            if count > MAX_LISTED {
                continue;
            }
            let whole = caps.get(0).expect("group 0 always exists");
            listing.push(format!(
                "{count}: [{}..{}] {}",
                whole.start(),
                whole.end(),
                whole.as_str()
            ));
            // Named groups are reported by name, unnamed ones by index — which is how
            // the replacement string refers to them, so the listing doubles as a key.
            let names: Vec<Option<&str>> = re.capture_names().collect();
            for (idx, group) in caps.iter().enumerate().skip(1) {
                let label = names
                    .get(idx)
                    .copied()
                    .flatten()
                    .map_or_else(|| format!("${idx}"), |n| format!("${n}"));
                match group {
                    Some(m) => listing.push(format!("    {label} = {}", m.as_str())),
                    // An optional group that didn't participate is not the same as one
                    // that matched empty, and the listing has to keep them apart.
                    None => listing.push(format!("    {label} = (no match)")),
                }
            }
        }

        if count > MAX_LISTED {
            listing.push(format!("… {} more matches not listed", count - MAX_LISTED));
        }

        let mut out = Outputs::new();
        out.set("count", count.to_string());
        out.set(
            "matches",
            if count == 0 {
                "(no matches)".to_string()
            } else {
                listing.join("\n")
            },
        );
        out.set(
            "replaced",
            re.replace_all(text, i.text("replace")).into_owned(),
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Inputs {
        Inputs::new()
            .with("ignore_case", false)
            .with("multiline", false)
            .with("dot_matches_newline", false)
            .with("replace", "")
    }

    fn run(text: &str, pattern: &str, inputs: Inputs) -> Result<Outputs, ToolError> {
        RegexTool::default().run(&inputs.with("text", text).with("pattern", pattern))
    }

    fn field(text: &str, pattern: &str, inputs: Inputs, key: &str) -> String {
        run(text, pattern, inputs)
            .unwrap()
            .get(key)
            .unwrap()
            .as_display()
    }

    #[test]
    fn matches_are_listed_with_their_byte_offsets() {
        let out = field("a1 b22 c333", r"\d+", opts(), "matches");
        assert_eq!(out, "1: [1..2] 1\n2: [4..6] 22\n3: [8..11] 333");
        assert_eq!(field("a1 b22 c333", r"\d+", opts(), "count"), "3");
    }

    #[test]
    fn no_match_says_so_rather_than_being_blank() {
        assert_eq!(field("abc", r"\d", opts(), "matches"), "(no matches)");
        assert_eq!(field("abc", r"\d", opts(), "count"), "0");
    }

    #[test]
    fn capture_groups_are_labelled_by_name_or_index() {
        let out = field("2026-08-04", r"(?<y>\d{4})-(\d{2})", opts(), "matches");
        assert!(out.contains("$y = 2026"), "{out}");
        assert!(out.contains("$2 = 08"), "{out}");
    }

    /// A group that didn't participate must not read like one that matched empty.
    #[test]
    fn a_non_participating_group_is_marked() {
        let out = field("ab", r"a(x)?b", opts(), "matches");
        assert!(out.contains("$1 = (no match)"), "{out}");
    }

    #[test]
    fn flags_change_the_result() {
        assert_eq!(field("ABC", "abc", opts(), "count"), "0");
        assert_eq!(
            field("ABC", "abc", opts().with("ignore_case", true), "count"),
            "1"
        );
        // `^` anchors to the whole input until multiline is on.
        assert_eq!(field("a\nb", "^b", opts(), "count"), "0");
        assert_eq!(
            field("a\nb", "^b", opts().with("multiline", true), "count"),
            "1"
        );
        assert_eq!(field("a\nb", "a.b", opts(), "count"), "0");
        assert_eq!(
            field(
                "a\nb",
                "a.b",
                opts().with("dot_matches_newline", true),
                "count"
            ),
            "1"
        );
    }

    #[test]
    fn replacement_supports_group_references() {
        let i = opts().with("replace", "$2/$1");
        assert_eq!(field("04-08", r"(\d+)-(\d+)", i, "replaced"), "08/04");
    }

    /// An empty replacement is a deletion, not a no-op — worth pinning so it isn't
    /// "fixed" into leaving the text alone.
    #[test]
    fn an_empty_replacement_deletes_each_match() {
        assert_eq!(field("a1b2c3", r"\d", opts(), "replaced"), "abc");
    }

    #[test]
    fn the_listing_is_capped_but_the_count_is_not() {
        let text = "x".repeat(MAX_LISTED + 50);
        let out = run(&text, "x", opts()).unwrap();
        assert_eq!(
            out.get("count").unwrap().as_display(),
            (MAX_LISTED + 50).to_string()
        );
        let listing = out.get("matches").unwrap().as_display();
        assert!(
            listing.ends_with("… 50 more matches not listed"),
            "{listing}"
        );
    }

    #[test]
    fn a_broken_pattern_names_the_field() {
        for bad in ["", "(unclosed", "a{2,1}", "[z-a]"] {
            let err = run("text", bad, opts()).unwrap_err();
            assert!(
                matches!(
                    err,
                    ToolError::InvalidInput {
                        field: "pattern",
                        ..
                    }
                ),
                "{bad:?}: {err:?}"
            );
        }
    }
}
