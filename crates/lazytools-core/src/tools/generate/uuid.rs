use uuid::Uuid;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

const VERSIONS: &[&str] = &["v4", "v7"];
const FORMATS: &[&str] = &["lowercase", "uppercase", "no-hyphens", "urn"];

pub struct UuidTool {
    spec: ToolSpec,
}

impl Default for UuidTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("generate.uuid", "UUID", Category::Generate)
                .describe("Generate random UUIDs (v4 or time-ordered v7)")
                .keywords(&["uuid", "guid", "id", "identifier", "v4", "v7", "random"])
                .option(
                    Field::select("version", VERSIONS)
                        .default("v4")
                        .label("Version"),
                )
                .option(Field::number("count", 1, 100).default(1i64).label("Count"))
                .option(
                    Field::select("format", FORMATS)
                        .default("lowercase")
                        .label("Format"),
                )
                .output(Field::text("result").multiline().mono().label("UUIDs"))
                .mode(RunMode::Generate),
        }
    }
}

fn format_uuid(id: Uuid, format: &str) -> String {
    match format {
        "uppercase" => id.hyphenated().to_string().to_uppercase(),
        "no-hyphens" => id.simple().to_string(),
        "urn" => id.urn().to_string(),
        _ => id.hyphenated().to_string(),
    }
}

impl Tool for UuidTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let count = i.num("count").clamp(1, 100) as usize;
        let format = i.choice("format");
        let v7 = i.choice("version") == "v7";

        // Joined with `\n` and no trailing newline: with a single output the CLI prints
        // it raw, so a trailing blank line would end up in the caller's pipeline.
        let result = (0..count)
            .map(|_| {
                let id = if v7 { Uuid::now_v7() } else { Uuid::new_v4() };
                format_uuid(id, format)
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(version: &str, count: i64, format: &str) -> String {
        UuidTool::default()
            .run(
                &Inputs::new()
                    .with("version", version)
                    .with("count", count)
                    .with("format", format),
            )
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn single_uuid_parses() {
        let out = ok("v4", 1, "lowercase");
        assert!(!out.contains('\n'), "one UUID must be one line: {out}");
        assert!(Uuid::parse_str(&out).is_ok(), "{out}");
    }

    #[test]
    fn count_controls_the_number_of_lines() {
        let out = ok("v4", 7, "lowercase");
        assert_eq!(out.lines().count(), 7);
        assert!(!out.ends_with('\n'), "no trailing newline: {out:?}");
        for line in out.lines() {
            assert!(Uuid::parse_str(line).is_ok(), "{line}");
        }
    }

    #[test]
    fn version_is_reflected_in_the_value() {
        assert_eq!(
            Uuid::parse_str(&ok("v4", 1, "lowercase"))
                .unwrap()
                .get_version_num(),
            4
        );
        assert_eq!(
            Uuid::parse_str(&ok("v7", 1, "lowercase"))
                .unwrap()
                .get_version_num(),
            7
        );
    }

    #[test]
    fn formats_render_as_declared() {
        assert_eq!(ok("v4", 1, "no-hyphens").len(), 32);
        let upper = ok("v4", 1, "uppercase");
        assert_eq!(upper, upper.to_uppercase());
        assert!(ok("v4", 1, "urn").starts_with("urn:uuid:"));
    }

    #[test]
    fn consecutive_calls_differ() {
        assert_ne!(ok("v4", 1, "lowercase"), ok("v4", 1, "lowercase"));
    }
}
