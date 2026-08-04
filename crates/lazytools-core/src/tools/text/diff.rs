use similar::{ChangeTag, TextDiff};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const GRANULARITY: &[&str] = &["line", "word", "char"];

pub struct DiffTool {
    spec: ToolSpec,
}

impl Default for DiffTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.diff", "Text Diff", Category::Text)
                .describe("Compare two blocks of text line by line, word by word, or character by character")
                .keywords(&["diff", "compare", "difference", "changes", "patch", "delta"])
                .input(Field::text("left").multiline().mono().label("Left"))
                .input(
                    Field::text("right")
                        .multiline()
                        .mono()
                        .label("Right")
                        .help("Pass as the second positional argument — only `left` can read stdin"),
                )
                .option(
                    Field::select("granularity", GRANULARITY)
                        .default("line")
                        .label("Granularity"),
                )
                .option(
                    Field::toggle("ignore_whitespace")
                        .default(false)
                        .label("Ignore whitespace"),
                )
                .output(Field::text("diff").multiline().mono().label("Diff"))
                .output(Field::text("summary").label("Summary")),
        }
    }
}

/// `line` mode renders a diff the way `diff`/`git` do — one prefixed line per change —
/// because that is the form people already read fluently. `word` and `char` modes
/// instead render **inline**: at that granularity a change is a fragment inside a line,
/// and splitting it across `-`/`+` rows would destroy the context that makes the finer
/// granularity worth choosing.
fn render_inline(changes: impl Iterator<Item = (ChangeTag, String)>) -> String {
    let mut out = String::new();
    for (tag, value) in changes {
        match tag {
            ChangeTag::Delete => {
                out.push_str("[-");
                out.push_str(&value);
                out.push_str("-]");
            }
            ChangeTag::Insert => {
                out.push_str("{+");
                out.push_str(&value);
                out.push_str("+}");
            }
            ChangeTag::Equal => out.push_str(&value),
        }
    }
    out
}

impl Tool for DiffTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let (mut left, mut right) = (i.text("left").to_string(), i.text("right").to_string());

        // Normalizing before diffing, rather than asking `similar` to ignore whitespace,
        // keeps the rendered output free of the whitespace the user asked to ignore.
        if i.bool("ignore_whitespace") {
            let squash = |s: &str| {
                s.lines()
                    .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            left = squash(&left);
            right = squash(&right);
        }

        if left == right {
            let mut out = Outputs::new();
            out.set("diff", "(identical)");
            out.set("summary", "no changes");
            return Ok(out);
        }

        let (body, added, removed) = match i.choice("granularity") {
            "word" | "char" => {
                let diff = if i.choice("granularity") == "word" {
                    TextDiff::from_words(left.as_str(), right.as_str())
                } else {
                    TextDiff::from_chars(left.as_str(), right.as_str())
                };
                let changes: Vec<(ChangeTag, String)> = diff
                    .iter_all_changes()
                    .map(|c| (c.tag(), c.value().to_string()))
                    .collect();
                let added = changes
                    .iter()
                    .filter(|(t, _)| *t == ChangeTag::Insert)
                    .count();
                let removed = changes
                    .iter()
                    .filter(|(t, _)| *t == ChangeTag::Delete)
                    .count();
                (render_inline(changes.into_iter()), added, removed)
            }
            _ => {
                let diff = TextDiff::from_lines(left.as_str(), right.as_str());
                let (mut added, mut removed) = (0, 0);
                let body = diff
                    .iter_all_changes()
                    .map(|change| {
                        let sign = match change.tag() {
                            ChangeTag::Delete => {
                                removed += 1;
                                '-'
                            }
                            ChangeTag::Insert => {
                                added += 1;
                                '+'
                            }
                            ChangeTag::Equal => ' ',
                        };
                        format!("{sign}{}", change.value().trim_end_matches(['\r', '\n']))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (body, added, removed)
            }
        };

        let unit = if i.choice("granularity") == "line" {
            "lines"
        } else {
            "fragments"
        };

        let mut out = Outputs::new();
        out.set("diff", body);
        out.set("summary", format!("+{added} / -{removed} {unit}"));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Inputs {
        Inputs::new()
            .with("granularity", "line")
            .with("ignore_whitespace", false)
    }

    fn run(left: &str, right: &str, inputs: Inputs) -> Outputs {
        DiffTool::default()
            .run(&inputs.with("left", left).with("right", right))
            .unwrap()
    }

    fn diff(left: &str, right: &str, inputs: Inputs) -> String {
        run(left, right, inputs).get("diff").unwrap().as_display()
    }

    fn summary(left: &str, right: &str, inputs: Inputs) -> String {
        run(left, right, inputs)
            .get("summary")
            .unwrap()
            .as_display()
    }

    #[test]
    fn identical_text_says_so_at_every_granularity() {
        for g in ["line", "word", "char"] {
            let i = opts().with("granularity", g);
            assert_eq!(diff("same", "same", i.clone()), "(identical)");
            assert_eq!(summary("same", "same", i), "no changes");
        }
    }

    #[test]
    fn line_mode_prefixes_each_line() {
        assert_eq!(diff("a\nb\nc", "a\nB\nc", opts()), " a\n-b\n+B\n c");
        assert_eq!(summary("a\nb\nc", "a\nB\nc", opts()), "+1 / -1 lines");
    }

    /// Empty on one side is a legitimate diff, not an error — unlike `web.json-diff`,
    /// there is nothing to parse and so nothing to reject.
    #[test]
    fn an_empty_side_is_a_pure_insert_or_delete() {
        assert_eq!(diff("", "a\nb", opts()), "+a\n+b");
        assert_eq!(summary("a\nb", "", opts()), "+0 / -2 lines");
    }

    #[test]
    fn word_mode_marks_changes_inline() {
        let out = diff(
            "the quick brown fox",
            "the slow brown fox",
            opts().with("granularity", "word"),
        );
        assert!(out.starts_with("the "), "{out}");
        assert!(out.contains("[-quick-]"), "{out}");
        assert!(out.contains("{+slow+}"), "{out}");
        assert!(out.ends_with(" brown fox"), "{out}");
    }

    #[test]
    fn char_mode_narrows_to_single_characters() {
        let out = diff("cat", "cot", opts().with("granularity", "char"));
        assert_eq!(out, "c[-a-]{+o+}t");
    }

    #[test]
    fn ignoring_whitespace_collapses_indentation_and_runs_of_spaces() {
        let i = opts().with("ignore_whitespace", true);
        assert_eq!(diff("  a  b", "a b", i.clone()), "(identical)");
        // Line breaks are still real differences: only intra-line whitespace collapses.
        assert_ne!(diff("a b", "a\nb", i), "(identical)");
    }

    /// Whitespace differences are real by default; the toggle exists precisely because
    /// they are sometimes not what you're looking for.
    #[test]
    fn whitespace_matters_unless_the_toggle_is_set() {
        assert_ne!(diff("  a", "a", opts()), "(identical)");
    }
}
