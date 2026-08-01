use rand::RngExt;
use rand::seq::IndexedRandom;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

const UNITS: &[&str] = &["words", "sentences", "paragraphs"];

/// The classic filler vocabulary, written out here rather than pulling in a crate —
/// a dependency is a poor trade for a hundred string literals.
const WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "in",
    "reprehenderit",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "eu",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
    "at",
    "vero",
    "eos",
    "accusamus",
    "iusto",
    "odio",
    "dignissimos",
    "ducimus",
    "blanditiis",
    "praesentium",
    "voluptatum",
    "deleniti",
    "atque",
    "corrupti",
    "quos",
    "dolores",
    "quas",
    "molestias",
    "excepturi",
    "similique",
    "mollitia",
    "animi",
    "dolorum",
    "fuga",
    "harum",
    "quidem",
    "rerum",
    "facilis",
    "expedita",
    "distinctio",
    "nam",
    "libero",
    "tempore",
    "cum",
    "soluta",
    "nobis",
    "eligendi",
    "optio",
    "cumque",
    "impedit",
    "quo",
    "minus",
    "maxime",
    "placeat",
    "facere",
    "possimus",
    "omnis",
    "assumenda",
    "repellendus",
];

pub struct LoremTool {
    spec: ToolSpec,
}

impl Default for LoremTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("generate.lorem", "Lorem Ipsum", Category::Generate)
                .describe("Generate placeholder lorem ipsum text")
                .keywords(&["lorem", "ipsum", "placeholder", "dummy", "filler", "text"])
                .option(
                    Field::select("unit", UNITS)
                        .default("paragraphs")
                        .label("Unit"),
                )
                .option(Field::number("count", 1, 50).default(3i64).label("Count"))
                .output(Field::text("result").multiline().label("Result"))
                .mode(RunMode::Generate),
        }
    }
}

fn word(rng: &mut impl RngExt) -> String {
    (*WORDS.choose(rng).expect("WORDS is never empty")).to_string()
}

/// 6–14 words, capitalized, ending in a period.
fn sentence(rng: &mut impl RngExt) -> String {
    let len = rng.random_range(6..=14);
    let mut words: Vec<String> = (0..len).map(|_| word(rng)).collect();
    if let Some(first) = words.first_mut() {
        let mut chars = first.chars();
        if let Some(c) = chars.next() {
            *first = c.to_uppercase().collect::<String>() + chars.as_str();
        }
    }
    format!("{}.", words.join(" "))
}

/// 3–6 sentences joined by spaces.
fn paragraph(rng: &mut impl RngExt) -> String {
    let len = rng.random_range(3..=6);
    (0..len)
        .map(|_| sentence(rng))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Tool for LoremTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let count = i.num("count").clamp(1, 50) as usize;
        let mut rng = rand::rng();

        let result = match i.choice("unit") {
            "words" => (0..count)
                .map(|_| word(&mut rng))
                .collect::<Vec<_>>()
                .join(" "),
            "sentences" => (0..count)
                .map(|_| sentence(&mut rng))
                .collect::<Vec<_>>()
                .join(" "),
            _ => (0..count)
                .map(|_| paragraph(&mut rng))
                .collect::<Vec<_>>()
                .join("\n\n"),
        };

        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(unit: &str, count: i64) -> String {
        LoremTool::default()
            .run(&Inputs::new().with("unit", unit).with("count", count))
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn word_count_is_exact() {
        let out = ok("words", 10);
        assert_eq!(out.split_whitespace().count(), 10, "{out}");
    }

    #[test]
    fn every_generated_word_comes_from_the_table() {
        let out = ok("words", 50);
        for w in out.split_whitespace() {
            assert!(WORDS.contains(&w), "{w:?} is not in WORDS");
        }
    }

    #[test]
    fn sentences_are_capitalized_and_terminated() {
        let out = ok("sentences", 1);
        assert!(out.ends_with('.'), "{out}");
        assert!(
            out.chars().next().unwrap().is_uppercase(),
            "must start with a capital: {out}"
        );
        let words = out.trim_end_matches('.').split_whitespace().count();
        assert!((6..=14).contains(&words), "{words} words: {out}");
    }

    #[test]
    fn paragraphs_are_separated_by_a_blank_line() {
        let out = ok("paragraphs", 2);
        assert_eq!(out.matches("\n\n").count(), 1, "{out}");
        assert_eq!(out.split("\n\n").count(), 2);
    }

    #[test]
    fn consecutive_calls_differ() {
        assert_ne!(ok("paragraphs", 3), ok("paragraphs", 3));
    }
}
