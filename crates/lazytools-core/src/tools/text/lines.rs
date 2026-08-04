use std::collections::HashSet;

use rand::seq::SliceRandom;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

const ORDERS: &[&str] = &["keep", "asc", "desc", "reverse", "shuffle"];

pub struct LinesTool {
    spec: ToolSpec,
}

impl Default for LinesTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.lines", "Line Tools", Category::Text)
                .describe("Sort, deduplicate, trim, and number lines of text")
                .keywords(&[
                    "lines",
                    "sort",
                    "unique",
                    "dedupe",
                    "duplicate",
                    "shuffle",
                    "trim",
                    "number",
                    "uniq",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("order", ORDERS)
                        .default("keep")
                        .label("Order")
                        .help("`reverse` flips the current order; `asc`/`desc` sort"),
                )
                .option(
                    Field::toggle("trim")
                        .default(true)
                        .label("Trim each line")
                        .help("Strip leading and trailing whitespace"),
                )
                .option(
                    Field::toggle("unique")
                        .default(false)
                        .label("Remove duplicates"),
                )
                .option(
                    Field::toggle("drop_empty")
                        .default(false)
                        .label("Drop empty lines"),
                )
                .option(
                    Field::toggle("ignore_case")
                        .default(false)
                        .label("Ignore case")
                        .help("Applies to both sorting and duplicate detection"),
                )
                .option(Field::toggle("number").default(false).label("Number lines"))
                .output(Field::text("result").multiline().mono().label("Result"))
                .output(Field::text("count").label("Lines out"))
                .output(Field::text("removed").label("Lines removed"))
                // `shuffle` reads the RNG, so the confirm key has to be able to ask for a
                // different result — same reason the `Generate` category uses this mode.
                .mode(RunMode::Generate),
        }
    }
}

/// The stage order is fixed and deliberate: trimming first is what makes `unique` and
/// `drop_empty` see the values a reader would call equal, and numbering last is what
/// makes the numbers describe the output rather than the input.
fn transform(text: &str, i: &Inputs) -> (Vec<String>, usize) {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let original = lines.len();

    if i.bool("trim") {
        for line in &mut lines {
            *line = line.trim().to_string();
        }
    }
    if i.bool("drop_empty") {
        lines.retain(|l| !l.trim().is_empty());
    }

    let ignore_case = i.bool("ignore_case");
    let fold = |s: &str| {
        if ignore_case {
            s.to_lowercase()
        } else {
            s.to_string()
        }
    };

    if i.bool("unique") {
        // First occurrence wins, so deduplicating without sorting keeps the input order.
        let mut seen = HashSet::new();
        lines.retain(|l| seen.insert(fold(l)));
    }

    match i.choice("order") {
        "asc" => lines.sort_by_key(|l| fold(l)),
        "desc" => {
            lines.sort_by_key(|l| fold(l));
            lines.reverse();
        }
        "reverse" => lines.reverse(),
        "shuffle" => lines.shuffle(&mut rand::rng()),
        _ => {}
    }

    if i.bool("number") {
        // Right-aligned to the widest index so the text stays in one column.
        let width = lines.len().to_string().len();
        for (n, line) in lines.iter_mut().enumerate() {
            *line = format!("{:>width$}  {line}", n + 1, width = width);
        }
    }

    (lines, original)
}

impl Tool for LinesTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let (lines, original) = transform(i.text("text"), i);

        let mut out = Outputs::new();
        out.set("result", lines.join("\n"));
        out.set("count", lines.len().to_string());
        out.set("removed", original.saturating_sub(lines.len()).to_string());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Inputs {
        // Mirrors the declared defaults, so each test only states what it changes.
        Inputs::new()
            .with("order", "keep")
            .with("trim", true)
            .with("unique", false)
            .with("drop_empty", false)
            .with("ignore_case", false)
            .with("number", false)
    }

    fn run(text: &str, inputs: Inputs) -> String {
        LinesTool::default()
            .run(&inputs.with("text", text))
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    fn count(text: &str, inputs: Inputs, key: &str) -> usize {
        LinesTool::default()
            .run(&inputs.with("text", text))
            .unwrap()
            .get(key)
            .unwrap()
            .as_display()
            .parse()
            .unwrap()
    }

    #[test]
    fn defaults_only_trim() {
        assert_eq!(run("  b  \na\n\nb", opts()), "b\na\n\nb");
    }

    #[test]
    fn sorting_goes_both_ways() {
        assert_eq!(run("b\na\nc", opts().with("order", "asc")), "a\nb\nc");
        assert_eq!(run("b\na\nc", opts().with("order", "desc")), "c\nb\na");
        assert_eq!(run("b\na\nc", opts().with("order", "reverse")), "c\na\nb");
    }

    /// Deduplicating without sorting must not reorder — first occurrence wins.
    #[test]
    fn unique_preserves_input_order() {
        assert_eq!(run("c\na\nc\nb\na", opts().with("unique", true)), "c\na\nb");
    }

    #[test]
    fn ignore_case_folds_both_sorting_and_dedupe() {
        let i = opts().with("unique", true).with("ignore_case", true);
        assert_eq!(run("Apple\napple\nBanana", i), "Apple\nBanana");
        // Without folding, `Zebra` sorts before `apple` by codepoint; with it, after.
        assert_eq!(
            run("apple\nZebra", opts().with("order", "asc")),
            "Zebra\napple"
        );
        assert_eq!(
            run(
                "apple\nZebra",
                opts().with("order", "asc").with("ignore_case", true)
            ),
            "apple\nZebra"
        );
    }

    #[test]
    fn drop_empty_removes_whitespace_only_lines() {
        assert_eq!(run("a\n\n   \nb", opts().with("drop_empty", true)), "a\nb");
    }

    /// Numbering describes the output, so it runs after every line has been removed.
    #[test]
    fn numbering_runs_last_and_pads_to_the_widest_index() {
        let i = opts().with("unique", true).with("number", true);
        assert_eq!(run("b\nb\na", i), "1  b\n2  a");

        let ten = (1..=10)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let out = run(&ten, opts().with("number", true));
        assert!(out.starts_with(" 1  1\n"), "{out}");
        assert!(out.ends_with("10  10"), "{out}");
    }

    #[test]
    fn counts_report_what_was_removed() {
        let i = opts().with("unique", true).with("drop_empty", true);
        assert_eq!(count("a\na\n\nb", i.clone(), "count"), 2);
        assert_eq!(count("a\na\n\nb", i, "removed"), 2);
    }

    #[test]
    fn empty_input_is_not_an_error() {
        assert_eq!(run("", opts()), "");
        assert_eq!(count("", opts(), "count"), 0);
    }

    /// A shuffle is asserted by property, never by value: same multiset, same length.
    #[test]
    fn shuffle_keeps_every_line() {
        let input = "a\nb\nc\nd\ne\nf\ng\nh";
        let out = run(input, opts().with("order", "shuffle"));
        let mut got: Vec<&str> = out.lines().collect();
        let mut want: Vec<&str> = input.lines().collect();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
    }
}
