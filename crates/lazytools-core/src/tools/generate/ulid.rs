use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct UlidTool {
    spec: ToolSpec,
}

impl Default for UlidTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("generate.ulid", "ULID", Category::Generate)
                .describe("Generate lexicographically sortable ULIDs")
                .keywords(&["ulid", "id", "identifier", "sortable", "time"])
                .option(Field::number("count", 1, 100).default(1i64).label("Count"))
                .output(Field::text("result").multiline().mono().label("ULIDs"))
                .mode(RunMode::Generate),
        }
    }
}

impl Tool for UlidTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let count = i.num("count").clamp(1, 100) as usize;

        // `Generator`, not the bare `Ulid::generate()`: a plain ULID only sorts by its
        // millisecond timestamp, so a batch produced inside one millisecond comes out in
        // random order — visibly unsorted for a tool whose whole selling point is that
        // the values sort. `Generator` keeps the random field strictly increasing within
        // a millisecond. Overflow needs 2^80 draws in one millisecond; commit the
        // increment rather than failing the run over it.
        let mut generator = ulid::Generator::new();
        let result = (0..count)
            .map(|_| {
                generator
                    .generate()
                    .unwrap_or_else(|overflow| overflow.commit_overflow_increment())
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Crockford base32: digits plus uppercase letters, minus I, L, O and U.
    const CROCKFORD: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

    fn ok(count: i64) -> String {
        UlidTool::default()
            .run(&Inputs::new().with("count", count))
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn a_ulid_is_26_crockford_characters() {
        let out = ok(1);
        assert_eq!(out.len(), 26, "{out}");
        for c in out.chars() {
            assert!(
                CROCKFORD.contains(c),
                "{c:?} is not Crockford base32: {out}"
            );
        }
    }

    #[test]
    fn count_controls_the_number_of_lines() {
        let out = ok(5);
        assert_eq!(out.lines().count(), 5);
        assert!(!out.ends_with('\n'));
    }

    /// The defining property of a ULID: string ordering matches generation order.
    /// This is the whole reason to pick one over a UUIDv4.
    #[test]
    fn consecutive_ulids_sort_ascending_as_strings() {
        let out = ok(20);
        let lines: Vec<&str> = out.lines().collect();
        let mut sorted = lines.clone();
        sorted.sort_unstable();
        assert_eq!(lines, sorted, "ULIDs must already be in ascending order");
    }

    #[test]
    fn consecutive_calls_differ() {
        assert_ne!(ok(1), ok(1));
    }
}
