use url::Url;

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct UrlParseTool {
    spec: ToolSpec,
}

impl Default for UrlParseTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.url-parse", "URL Parser", Category::Web)
                .describe("Break a URL into its parts")
                .keywords(&[
                    "url",
                    "uri",
                    "parse",
                    "query",
                    "host",
                    "path",
                    "querystring",
                ])
                .input(Field::text("url").mono().label("URL"))
                .output(Field::text("scheme").label("Scheme"))
                .output(Field::text("username").label("Username"))
                .output(Field::text("host").label("Host"))
                .output(Field::text("port").label("Port"))
                .output(Field::text("path").mono().label("Path"))
                .output(Field::text("query").multiline().mono().label("Query"))
                .output(Field::text("fragment").label("Fragment")),
        }
    }
}

impl Tool for UrlParseTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let input = i.text("url").trim();
        if input.is_empty() {
            return Err(ToolError::invalid("url", "url must not be empty"));
        }

        let url = Url::parse(input).map_err(|e| ToolError::invalid("url", e.to_string()))?;

        // Percent-decoded, one pair per line. Decoding is the actual value here — a
        // user staring at the raw string can already see the encoded form.
        let query = url
            .query_pairs()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("\n");

        let mut out = Outputs::new();
        out.set("scheme", url.scheme().to_string());
        out.set("username", url.username().to_string());
        out.set("host", url.host_str().unwrap_or("").to_string());
        // `port_or_known_default` so `https://x` reports 443 instead of nothing.
        out.set(
            "port",
            url.port_or_known_default()
                .map(|p| p.to_string())
                .unwrap_or_default(),
        );
        out.set("path", url.path().to_string());
        out.set("query", query);
        out.set("fragment", url.fragment().unwrap_or("").to_string());
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(url: &str) -> Result<Outputs, ToolError> {
        UrlParseTool::default().run(&Inputs::new().with("url", url))
    }

    fn field(url: &str, key: &str) -> String {
        run(url).unwrap().get(key).unwrap().as_display()
    }

    #[test]
    fn full_url_splits_into_every_part() {
        let url = "https://user@example.com:8443/a/b?q=1#top";
        assert_eq!(field(url, "scheme"), "https");
        assert_eq!(field(url, "username"), "user");
        assert_eq!(field(url, "host"), "example.com");
        assert_eq!(field(url, "port"), "8443");
        assert_eq!(field(url, "path"), "/a/b");
        assert_eq!(field(url, "query"), "q=1");
        assert_eq!(field(url, "fragment"), "top");
    }

    /// Missing parts are empty strings, not errors.
    #[test]
    fn minimal_url_leaves_absent_parts_empty() {
        let url = "https://example.com";
        assert_eq!(field(url, "username"), "");
        assert_eq!(field(url, "query"), "");
        assert_eq!(field(url, "fragment"), "");
        assert_eq!(field(url, "path"), "/");
        // The known default for https, even though the URL doesn't say so.
        assert_eq!(field(url, "port"), "443");
    }

    #[test]
    fn query_values_are_percent_decoded() {
        assert_eq!(field("https://x.dev/?q=a%20b", "query"), "q=a b");
        assert_eq!(field("https://x.dev/?e=a%2Bb%26c", "query"), "e=a+b&c");
    }

    /// Repeated keys are both real; neither may be dropped.
    #[test]
    fn repeated_keys_each_get_a_line() {
        let out = field("https://x.dev/?a=1&a=2", "query");
        assert_eq!(out, "a=1\na=2");
    }

    #[test]
    fn garbage_names_the_field() {
        let err = run("not a url").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "url", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn empty_input_names_the_field() {
        let err = run("").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "url", .. }),
            "{err:?}"
        );
    }
}
