use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const STYLES: &[&str] = &[
    "camel", "pascal", "snake", "kebab", "constant", "title", "lower", "upper",
];

pub struct CaseTool {
    spec: ToolSpec,
}

impl Default for CaseTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.case", "Change Case", Category::Text)
                .describe("Convert text between camel, snake, kebab, and other cases")
                .keywords(&[
                    "case", "camel", "snake", "kebab", "pascal", "slug", "constant", "title",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("style", STYLES)
                        .default("snake")
                        .label("Style"),
                )
                .output(Field::text("result").multiline().label("Result")),
        }
    }
}

/// Splits text into words at three boundaries: non-alphanumeric characters,
/// lower→upper transitions (`fooBar`), and a run of uppercase followed by a
/// lowercase (`HTTPServer` → `HTTP` + `Server`).
fn words(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();

    for (idx, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }

        let prev = idx.checked_sub(1).map(|i| chars[i]);
        let next = chars.get(idx + 1).copied();
        let starts_word = match prev {
            None => false,
            Some(p) if !p.is_alphanumeric() => false,
            // `fooBar` → break before `B`.
            Some(p) if p.is_lowercase() && c.is_uppercase() => true,
            // `HTTPServer` → break before the `S` that starts a lowercase run.
            Some(p) if p.is_uppercase() && c.is_uppercase() => next.is_some_and(char::is_lowercase),
            Some(_) => false,
        };
        if starts_word && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current.push(c);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn capitalize(w: &str) -> String {
    let mut chars = w.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

impl Tool for CaseTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        let style = i.choice("style");

        // `lower`/`upper` apply to the text as written — splitting into words would
        // throw away the spacing and punctuation the user wants to keep.
        if style == "lower" {
            return Ok(Outputs::one("result", text.to_lowercase()));
        }
        if style == "upper" {
            return Ok(Outputs::one("result", text.to_uppercase()));
        }

        let words = words(text);
        let result = match style {
            "camel" => words
                .iter()
                .enumerate()
                .map(|(idx, w)| {
                    if idx == 0 {
                        w.to_lowercase()
                    } else {
                        capitalize(w)
                    }
                })
                .collect::<String>(),
            "pascal" => words.iter().map(|w| capitalize(w)).collect::<String>(),
            "kebab" => words
                .iter()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>()
                .join("-"),
            "constant" => words
                .iter()
                .map(|w| w.to_uppercase())
                .collect::<Vec<_>>()
                .join("_"),
            "title" => words
                .iter()
                .map(|w| capitalize(w))
                .collect::<Vec<_>>()
                .join(" "),
            // `snake` is the default; an unknown style can only come from a caller
            // bypassing the `Select`, and falling back beats erroring here.
            _ => words
                .iter()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>()
                .join("_"),
        };

        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str, style: &str) -> String {
        CaseTool::default()
            .run(&Inputs::new().with("text", text).with("style", style))
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    #[test]
    fn spaced_words() {
        assert_eq!(ok("hello world", "snake"), "hello_world");
        assert_eq!(ok("hello world", "kebab"), "hello-world");
        assert_eq!(ok("hello world", "camel"), "helloWorld");
        assert_eq!(ok("hello world", "pascal"), "HelloWorld");
        assert_eq!(ok("hello world", "constant"), "HELLO_WORLD");
        assert_eq!(ok("hello world", "title"), "Hello World");
    }

    #[test]
    fn splits_on_lower_to_upper() {
        assert_eq!(ok("fooBar", "snake"), "foo_bar");
    }

    #[test]
    fn splits_acronym_before_the_next_word() {
        assert_eq!(ok("HTTPServerError", "snake"), "http_server_error");
        assert_eq!(ok("HTTPServerError", "pascal"), "HttpServerError");
    }

    #[test]
    fn mixed_separators_collapse() {
        assert_eq!(ok("foo_bar-baz qux", "kebab"), "foo-bar-baz-qux");
    }

    #[test]
    fn empty_and_punctuation_only_yield_empty() {
        assert_eq!(ok("", "snake"), "");
        assert_eq!(ok("--__--", "snake"), "");
    }

    #[test]
    fn unicode_words_survive() {
        assert_eq!(ok("xin chào", "snake"), "xin_chào");
        assert_eq!(ok("xin chào", "title"), "Xin Chào");
    }

    /// `lower`/`upper` must not go through word splitting: the spacing stays.
    #[test]
    fn lower_and_upper_keep_the_original_shape() {
        assert_eq!(ok("Hello, World!", "lower"), "hello, world!");
        assert_eq!(ok("Hello, World!", "upper"), "HELLO, WORLD!");
    }

    #[test]
    fn digits_stay_attached() {
        assert_eq!(ok("foo2bar", "snake"), "foo2bar");
        assert_eq!(ok("version 2 beta", "kebab"), "version-2-beta");
    }
}
