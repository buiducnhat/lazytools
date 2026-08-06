use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct HttpStatusTool {
    spec: ToolSpec,
}

impl Default for HttpStatusTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.http-status", "HTTP Status", Category::Web)
                .describe("Look up what an HTTP status code means")
                .keywords(&[
                    "http", "status", "code", "404", "500", "response", "rest", "api",
                ])
                .input(
                    Field::text("code")
                        .label("Status code")
                        .help("A number, `404`, or a name fragment, `gateway`"),
                )
                .output(Field::text("status").mono().label("Status"))
                .output(Field::text("class").label("Class"))
                .output(Field::text("meaning").multiline().label("Meaning"))
                .output(Field::text("matches").multiline().label("Other matches")),
        }
    }
}

/// Code, reason phrase, and what it is actually used for.
///
/// The registry is IANA's, but this is the working subset: the codes that turn
/// up in a real response, plus the ones people look up precisely because they
/// are rare. A complete table would be mostly `4xx` WebDAV extensions nobody is
/// debugging in a terminal.
#[rustfmt::skip]
const STATUSES: &[(u16, &str, &str)] = &[
    (100, "Continue", "The request headers are fine; send the body."),
    (101, "Switching Protocols", "Upgrading, typically to WebSocket."),
    (102, "Processing", "WebDAV: the request is still being worked on."),
    (103, "Early Hints", "Preload hints sent ahead of the real response."),
    (200, "OK", "The request succeeded."),
    (201, "Created", "A new resource exists; `Location` names it."),
    (202, "Accepted", "Queued for processing, not yet done."),
    (203, "Non-Authoritative Information", "A proxy modified the origin's response."),
    (204, "No Content", "Success, with deliberately no body."),
    (205, "Reset Content", "Success; the client should clear its form."),
    (206, "Partial Content", "The requested byte range only."),
    (207, "Multi-Status", "WebDAV: per-resource results in the body."),
    (208, "Already Reported", "WebDAV: this member was listed earlier."),
    (226, "IM Used", "The response is a delta encoding of the resource."),
    (300, "Multiple Choices", "Several representations; the client picks."),
    (301, "Moved Permanently", "Use the new URL from now on. Caches and search engines act on this."),
    (302, "Found", "A temporary redirect; keep using the old URL."),
    (303, "See Other", "Fetch the result with GET, whatever was sent."),
    (304, "Not Modified", "The cached copy is still current; no body follows."),
    (307, "Temporary Redirect", "Like 302, but the method is preserved."),
    (308, "Permanent Redirect", "Like 301, but the method is preserved."),
    (400, "Bad Request", "The server could not parse the request."),
    (401, "Unauthorized", "Authentication is missing or wrong — despite the name, this is about credentials, not permission."),
    (402, "Payment Required", "Reserved; used ad hoc for billing limits."),
    (403, "Forbidden", "Authenticated, but not allowed. The counterpart to 401."),
    (404, "Not Found", "No resource at this URL, and no reason given."),
    (405, "Method Not Allowed", "The URL exists but not for this method; `Allow` lists the ones it takes."),
    (406, "Not Acceptable", "Nothing matches the `Accept` headers."),
    (407, "Proxy Authentication Required", "401, but for the proxy."),
    (408, "Request Timeout", "The client took too long to send it."),
    (409, "Conflict", "The request collides with the resource's current state, e.g. an edit conflict."),
    (410, "Gone", "Deleted on purpose, and not coming back — unlike 404."),
    (411, "Length Required", "A `Content-Length` header is mandatory here."),
    (412, "Precondition Failed", "An `If-*` header did not hold."),
    (413, "Content Too Large", "The body exceeds what the server accepts."),
    (414, "URI Too Long", "The URL exceeds what the server accepts."),
    (415, "Unsupported Media Type", "The body's `Content-Type` is refused."),
    (416, "Range Not Satisfiable", "The requested range is outside the file."),
    (417, "Expectation Failed", "The `Expect` header cannot be met."),
    (418, "I'm a Teapot", "An April Fools' joke from RFC 2324, kept alive."),
    (421, "Misdirected Request", "This server cannot answer for that authority."),
    (422, "Unprocessable Content", "Well-formed, but semantically wrong — the usual choice for validation errors."),
    (423, "Locked", "WebDAV: the resource is locked."),
    (424, "Failed Dependency", "WebDAV: a prior request in the chain failed."),
    (425, "Too Early", "Replaying this request would be unsafe."),
    (426, "Upgrade Required", "Switch protocols and try again."),
    (428, "Precondition Required", "Send an `If-Match`, so a lost update can be detected."),
    (429, "Too Many Requests", "Rate limited; `Retry-After` says how long."),
    (431, "Request Header Fields Too Large", "The headers alone are over the limit."),
    (451, "Unavailable For Legal Reasons", "Blocked by a legal demand. The number is a Bradbury reference."),
    (500, "Internal Server Error", "An unhandled failure on the server."),
    (501, "Not Implemented", "The server does not support this method at all."),
    (502, "Bad Gateway", "A proxy got an invalid response from upstream."),
    (503, "Service Unavailable", "Down or overloaded, expected to return; `Retry-After` may say when."),
    (504, "Gateway Timeout", "A proxy gave up waiting for upstream."),
    (505, "HTTP Version Not Supported", "The request's HTTP version is refused."),
    (507, "Insufficient Storage", "WebDAV: no room to store the result."),
    (508, "Loop Detected", "WebDAV: the traversal is circular."),
    (511, "Network Authentication Required", "A captive portal wants a login."),
];

/// The class a code belongs to, from its leading digit.
fn class_of(code: u16) -> Option<&'static str> {
    let label = match code / 100 {
        1 => "1xx Informational",
        2 => "2xx Success",
        3 => "3xx Redirection",
        4 => "4xx Client Error",
        5 => "5xx Server Error",
        _ => return None,
    };
    Some(label)
}

fn find(code: u16) -> Option<&'static (u16, &'static str, &'static str)> {
    STATUSES.iter().find(|(c, _, _)| *c == code)
}

/// Codes whose reason phrase contains `needle`, case-insensitively.
fn search(needle: &str) -> Vec<String> {
    let needle = needle.to_lowercase();
    STATUSES
        .iter()
        .filter(|(_, phrase, _)| phrase.to_lowercase().contains(&needle))
        .map(|(code, phrase, _)| format!("{code} {phrase}"))
        .collect()
}

impl Tool for HttpStatusTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let query = i.text("code").trim();
        if query.is_empty() {
            return Err(ToolError::invalid("code", "enter a status code or a name"));
        }

        // A number is looked up; anything else searches the reason phrases, so
        // `gateway` finds 502 and 504 without knowing either number.
        if let Ok(code) = query.parse::<u16>() {
            let class = class_of(code).ok_or_else(|| {
                ToolError::invalid("code", format!("{code} is outside the 100-599 range"))
            })?;
            let mut out = Outputs::new();
            match find(code) {
                Some((_, phrase, meaning)) => {
                    out.set("status", format!("{code} {phrase}"));
                    out.set("meaning", *meaning);
                }
                // In range but unassigned: report the class rather than
                // inventing a phrase for a code nobody registered.
                None => {
                    out.set("status", code.to_string());
                    out.set(
                        "meaning",
                        "Not a registered status code. A client must treat it as the generic \
                         status of its class.",
                    );
                }
            }
            out.set("class", class);
            out.set("matches", "");
            return Ok(out);
        }

        let hits = search(query);
        if hits.is_empty() {
            return Err(ToolError::invalid(
                "code",
                format!("no status code matches `{query}`"),
            ));
        }
        // The first hit fills the detail fields; the rest are listed so a broad
        // search doesn't hide what else it found.
        let first: u16 = hits[0]
            .split_whitespace()
            .next()
            .and_then(|c| c.parse().ok())
            .expect("every entry is formatted `<code> <phrase>`");
        let (_, phrase, meaning) = find(first).expect("the hit came from the table");

        let mut out = Outputs::new();
        out.set("status", format!("{first} {phrase}"));
        out.set("class", class_of(first).unwrap_or("unknown"));
        out.set("meaning", *meaning);
        out.set("matches", hits[1..].join("\n"));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &str) -> Result<Outputs, ToolError> {
        HttpStatusTool::default().run(&Inputs::new().with("code", code))
    }

    fn field(code: &str, key: &str) -> String {
        run(code).unwrap().get(key).unwrap().as_display()
    }

    #[test]
    fn a_number_is_looked_up() {
        assert_eq!(field("404", "status"), "404 Not Found");
        assert_eq!(field("404", "class"), "4xx Client Error");
        assert_eq!(field("503", "status"), "503 Service Unavailable");
    }

    #[test]
    fn a_name_fragment_searches_the_reason_phrases() {
        assert_eq!(field("gateway", "status"), "502 Bad Gateway");
        assert!(
            field("gateway", "matches").contains("504 Gateway Timeout"),
            "the other match must be listed"
        );
        // Case-insensitive, and a full phrase works too.
        assert_eq!(field("NOT FOUND", "status"), "404 Not Found");
    }

    /// In range but unregistered: report the class instead of inventing a
    /// reason phrase for a code that has none.
    #[test]
    fn an_unassigned_code_in_range_reports_its_class() {
        assert_eq!(field("499", "status"), "499");
        assert_eq!(field("499", "class"), "4xx Client Error");
        assert!(field("499", "meaning").contains("Not a registered"));
    }

    #[test]
    fn out_of_range_empty_and_unmatched_input_all_name_the_field() {
        for code in ["", "42", "600", "banana"] {
            let err = run(code).unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidInput { field: "code", .. }),
                "{code:?}: {err:?}"
            );
        }
    }

    #[test]
    fn the_table_has_no_duplicate_codes_and_is_sorted() {
        let codes: Vec<u16> = STATUSES.iter().map(|(c, _, _)| *c).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(codes, sorted, "the table must be sorted and unique");
    }
}
