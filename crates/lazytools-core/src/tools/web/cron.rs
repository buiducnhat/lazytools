use std::str::FromStr;

use chrono::Utc;
use cron::Schedule;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct CronTool {
    spec: ToolSpec,
}

impl Default for CronTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.cron", "Cron Explainer", Category::Web)
                .describe("Explain a cron expression and list its next runs")
                .keywords(&["cron", "crontab", "schedule", "job", "expression"])
                .input(Field::text("expression").mono().label("Expression"))
                .option(
                    Field::number("count", 1, 20)
                        .default(5i64)
                        .label("Next runs"),
                )
                .output(Field::text("description").multiline().label("Description"))
                .output(
                    Field::text("next_runs")
                        .multiline()
                        .mono()
                        .label("Next runs"),
                ),
        }
    }
}

/// The `cron` crate speaks **7 fields** (seconds first, year last) while crontab —
/// what users actually type — is **5**. Pad a 5-field expression on both ends so both
/// forms work. A 6-field expression is treated as crontab plus seconds.
fn normalize(expr: &str) -> String {
    match expr.split_whitespace().count() {
        5 => format!("0 {expr} *"),
        6 => format!("{expr} *"),
        _ => expr.to_string(),
    }
}

/// Renders one field in plain English. Returns `None` when the pattern isn't one of
/// the shapes handled here, which is the caller's signal to fall back.
fn describe_field(field: &str, name: &str, plural: &str) -> Option<String> {
    if field == "*" {
        return Some(format!("every {name}"));
    }
    if let Some(step) = field.strip_prefix("*/") {
        let n: u32 = step.parse().ok()?;
        return Some(format!("every {n} {plural}"));
    }
    if let Some((lo, hi)) = field.split_once('-')
        && lo.parse::<u32>().is_ok()
        && hi.parse::<u32>().is_ok()
    {
        return Some(format!("{name}s {lo} through {hi}"));
    }
    if field.contains(',') && field.split(',').all(|p| p.parse::<u32>().is_ok()) {
        return Some(format!("{plural} {field}"));
    }
    if field.parse::<u32>().is_ok() {
        return Some(format!("{name} {field}"));
    }
    None
}

/// Deliberately narrow and honest: common shapes get a sentence, anything else gets
/// its fields listed verbatim. Saying `minute: */15, hour: *` is better than guessing
/// wrong about what a user's expression does.
fn describe(expr: &str) -> String {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    let (minute, hour, dom, month, dow) = match fields.len() {
        5 => (fields[0], fields[1], fields[2], fields[3], fields[4]),
        6 => (fields[1], fields[2], fields[3], fields[4], fields[5]),
        7 => (fields[1], fields[2], fields[3], fields[4], fields[5]),
        _ => return format!("unrecognized shape: {} fields", fields.len()),
    };

    let named = [
        ("minute", "minutes", minute),
        ("hour", "hours", hour),
        ("day of month", "days of month", dom),
        ("month", "months", month),
        ("day of week", "days of week", dow),
    ];

    let parts: Vec<String> = named
        .iter()
        .filter_map(|(name, plural, value)| {
            describe_field(value, name, plural).map(|d| (*value != "*").then_some(d))?
        })
        .collect();

    // Every field understood and every field a wildcard.
    if named
        .iter()
        .all(|(name, plural, value)| describe_field(value, name, plural).is_some())
    {
        if parts.is_empty() {
            return "Every minute".to_string();
        }
        let mut s = parts.join(", ");
        if let Some(first) = s.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        return s;
    }

    named
        .iter()
        .map(|(name, _, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Tool for CronTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let expr = i.text("expression").trim();
        if expr.is_empty() {
            return Err(ToolError::invalid(
                "expression",
                "expression must not be empty",
            ));
        }

        let schedule = Schedule::from_str(&normalize(expr))
            .map_err(|e| ToolError::invalid("expression", e.to_string()))?;

        // Clock-dependent branch, like `web.timestamp` with an empty value.
        let count = i.num("count").clamp(1, 20) as usize;
        let next_runs = schedule
            .upcoming(Utc)
            .take(count)
            .map(|dt| dt.to_rfc3339())
            .collect::<Vec<_>>()
            .join("\n");

        let mut out = Outputs::new();
        out.set("description", describe(expr));
        out.set("next_runs", next_runs);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(expression: &str, count: i64) -> Result<Outputs, ToolError> {
        CronTool::default().run(
            &Inputs::new()
                .with("expression", expression)
                .with("count", count),
        )
    }

    fn field(expression: &str, key: &str) -> String {
        run(expression, 5).unwrap().get(key).unwrap().as_display()
    }

    /// The 5-field form is what people actually type into a crontab.
    #[test]
    fn accepts_five_field_expressions() {
        assert_eq!(field("*/15 * * * *", "description"), "Every 15 minutes");
        assert_eq!(field("0 0 * * *", "description"), "Minute 0, hour 0");
        assert_eq!(field("*/15 * * * *", "next_runs").lines().count(), 5);
    }

    /// ...and the crate's native 7-field form must keep working too.
    #[test]
    fn accepts_seven_field_expressions() {
        let out = field("0 0 12 * * * *", "next_runs");
        assert_eq!(out.lines().count(), 5, "{out}");
    }

    #[test]
    fn accepts_six_field_expressions() {
        let out = field("0 */5 * * * *", "next_runs");
        assert_eq!(out.lines().count(), 5, "{out}");
    }

    #[test]
    fn count_controls_the_number_of_runs() {
        let out = run("*/15 * * * *", 3).unwrap();
        assert_eq!(
            out.get("next_runs").unwrap().as_display().lines().count(),
            3
        );
    }

    /// Property, not fixed values: the list depends on the clock.
    #[test]
    fn next_runs_are_strictly_increasing() {
        let out = field("*/15 * * * *", "next_runs");
        let lines: Vec<&str> = out.lines().collect();
        for pair in lines.windows(2) {
            assert!(pair[0] < pair[1], "not increasing: {pair:?}");
        }
    }

    #[test]
    fn invalid_expression_names_the_field() {
        let err = run("not a cron", 5).unwrap_err();
        assert!(
            matches!(
                err,
                ToolError::InvalidInput {
                    field: "expression",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn empty_expression_names_the_field() {
        let err = run("", 5).unwrap_err();
        assert!(
            matches!(
                err,
                ToolError::InvalidInput {
                    field: "expression",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn all_wildcards_reads_as_every_minute() {
        assert_eq!(field("* * * * *", "description"), "Every minute");
    }

    /// Honesty over guessing: an unsupported pattern lists its fields instead of
    /// inventing a description.
    #[test]
    fn unparseable_patterns_fall_back_to_listing_fields() {
        let desc = describe("*/15 * * * MON#2");
        assert!(desc.contains("day of week: MON#2"), "{desc}");
        assert!(desc.contains("minute: */15"), "{desc}");
    }

    #[test]
    fn describes_lists_and_ranges() {
        assert!(field("0 9-17 * * *", "description").contains("through"));
        assert!(field("0 1,2,3 * * *", "description").contains("1,2,3"));
    }
}
