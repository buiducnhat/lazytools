use serde_json::Value as Json;
use similar::{ChangeTag, TextDiff};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct JsonDiffTool {
    spec: ToolSpec,
}

impl Default for JsonDiffTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.json-diff", "JSON Diff", Category::Web)
                .describe("Compare two JSON documents structurally")
                .keywords(&["json", "diff", "compare", "difference", "changes"])
                .input(Field::text("left").multiline().mono().label("Left"))
                .input(
                    Field::text("right").multiline().mono().label("Right").help(
                        "Pass as the second positional argument — only `left` can read stdin",
                    ),
                )
                .output(Field::text("diff").multiline().mono().label("Diff")),
        }
    }
}

/// Sorts object keys recursively.
///
/// **This runs against the grain of `convert::json_fmt` on purpose.** `serde_json` is
/// built with `preserve_order` precisely so the *format* tool never reorders a user's
/// keys. A *diff* tool needs the opposite: without sorting, swapping two keys would
/// show up as a change when the documents are semantically identical — which is the
/// entire reason to use this instead of `diff a.json b.json`. Don't "unify" the two
/// behaviors; they are both correct for their own tool.
fn sort_keys(value: Json) -> Json {
    match value {
        Json::Object(map) => {
            let mut entries: Vec<(String, Json)> =
                map.into_iter().map(|(k, v)| (k, sort_keys(v))).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Json::Object(entries.into_iter().collect())
        }
        // Arrays keep their order: in JSON, array order is meaningful even though
        // object key order is not.
        Json::Array(items) => Json::Array(items.into_iter().map(sort_keys).collect()),
        other => other,
    }
}

fn normalize(text: &str, field: &'static str) -> Result<String, ToolError> {
    let value: Json = serde_json::from_str(text.trim())
        .map_err(|e| ToolError::invalid(field, format!("invalid JSON: {e}")))?;
    serde_json::to_string_pretty(&sort_keys(value))
        .map_err(|e| ToolError::invalid(field, format!("could not re-serialize: {e}")))
}

impl Tool for JsonDiffTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let (left, right) = (i.text("left").trim(), i.text("right").trim());
        if left.is_empty() {
            return Err(ToolError::invalid("left", "left must not be empty"));
        }
        if right.is_empty() {
            return Err(ToolError::invalid("right", "right must not be empty"));
        }

        let left = normalize(left, "left")?;
        let right = normalize(right, "right")?;

        let diff = TextDiff::from_lines(&left, &right);
        let body: String = diff
            .iter_all_changes()
            .map(|change| {
                let sign = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal => ' ',
                };
                format!("{sign}{}", change.value().trim_end())
            })
            .collect::<Vec<_>>()
            .join("\n");

        let identical = left == right;
        Ok(Outputs::one(
            "diff",
            if identical {
                "(identical)".to_string()
            } else {
                body
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(left: &str, right: &str) -> Result<Outputs, ToolError> {
        JsonDiffTool::default().run(&Inputs::new().with("left", left).with("right", right))
    }

    fn diff(left: &str, right: &str) -> String {
        run(left, right).unwrap().get("diff").unwrap().as_display()
    }

    /// The whole point of the tool: key order is not a difference.
    #[test]
    fn key_order_is_not_a_difference() {
        assert_eq!(
            diff(r#"{"zebra":1,"apple":2}"#, r#"{"apple":2,"zebra":1}"#),
            "(identical)"
        );
    }

    #[test]
    fn nested_key_order_is_also_ignored() {
        assert_eq!(
            diff(r#"{"a":{"y":1,"x":2}}"#, r#"{"a":{"x":2,"y":1}}"#),
            "(identical)"
        );
    }

    /// Array order, by contrast, *is* meaningful in JSON.
    #[test]
    fn array_order_is_a_difference() {
        let out = diff("[1,2]", "[2,1]");
        assert_ne!(out, "(identical)");
        assert!(out.contains('-') && out.contains('+'), "{out}");
    }

    #[test]
    fn changed_value_shows_both_sides() {
        let out = diff(r#"{"a":1}"#, r#"{"a":2}"#);
        assert!(out.contains("-  \"a\": 1"), "{out}");
        assert!(out.contains("+  \"a\": 2"), "{out}");
    }

    #[test]
    fn added_field_shows_as_an_insertion() {
        let out = diff(r#"{"a":1}"#, r#"{"a":1,"b":2}"#);
        assert!(out.contains("+  \"b\": 2"), "{out}");
    }

    #[test]
    fn removed_field_shows_as_a_deletion() {
        let out = diff(r#"{"a":1,"b":2}"#, r#"{"a":1}"#);
        assert!(out.contains("-  \"b\": 2"), "{out}");
    }

    #[test]
    fn broken_json_names_the_side_it_came_from() {
        assert!(
            matches!(
                run("{", r#"{"a":1}"#).unwrap_err(),
                ToolError::InvalidInput { field: "left", .. }
            ),
            "left must be named"
        );
        assert!(
            matches!(
                run(r#"{"a":1}"#, "{").unwrap_err(),
                ToolError::InvalidInput { field: "right", .. }
            ),
            "right must be named"
        );
    }

    #[test]
    fn empty_sides_name_the_field() {
        assert!(matches!(
            run("", r#"{"a":1}"#).unwrap_err(),
            ToolError::InvalidInput { field: "left", .. }
        ));
        assert!(matches!(
            run(r#"{"a":1}"#, "").unwrap_err(),
            ToolError::InvalidInput { field: "right", .. }
        ));
    }
}
