use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const DIRECTIONS: &[&str] = &["encode", "decode"];

/// The named entities that actually appear in the wild, plus every name HTML *requires*
/// for round-tripping (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`). The full WHATWG
/// list has ~2200 entries; shipping it would add a large table to serve names almost
/// nobody types, and anything missing still decodes through the numeric form.
const NAMED: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("hellip", '…'),
    ("mdash", '—'),
    ("ndash", '–'),
    ("lsquo", '\u{2018}'),
    ("rsquo", '\u{2019}'),
    ("ldquo", '\u{201c}'),
    ("rdquo", '\u{201d}'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("deg", '°'),
    ("plusmn", '±'),
    ("times", '×'),
    ("divide", '÷'),
    ("micro", 'µ'),
    ("para", '¶'),
    ("sect", '§'),
    ("bull", '•'),
    ("dagger", '†'),
    ("euro", '€'),
    ("pound", '£'),
    ("yen", '¥'),
    ("cent", '¢'),
    ("middot", '·'),
    ("larr", '←'),
    ("rarr", '→'),
    ("uarr", '↑'),
    ("darr", '↓'),
];

pub struct HtmlEntityTool {
    spec: ToolSpec,
}

impl Default for HtmlEntityTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.html-entity", "HTML Entities", Category::Convert)
                .describe("Escape text for HTML, or decode entities back to text")
                .keywords(&[
                    "html", "entity", "entities", "escape", "unescape", "encode", "decode", "xml",
                ])
                .input(Field::text("text").multiline().label("Input"))
                .option(
                    Field::select("direction", DIRECTIONS)
                        .default("encode")
                        .label("Direction"),
                )
                .option(
                    Field::toggle("all_non_ascii")
                        .default(false)
                        .label("Encode all non-ASCII")
                        .help("Also escape every character above U+007F as `&#NNN;`"),
                )
                .output(Field::text("result").multiline().mono().label("Result")),
        }
    }
}

fn encode(text: &str, all_non_ascii: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // `&#39;` rather than `&apos;`: the named form is XML/HTML5-only and is not
            // understood by HTML 4 parsers, while the numeric form always is.
            '\'' => out.push_str("&#39;"),
            c if all_non_ascii && !c.is_ascii() => {
                out.push_str(&format!("&#{};", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Resolves the body of one `&…;` — the part between the ampersand and the semicolon.
fn resolve(body: &str) -> Option<char> {
    if let Some(digits) = body.strip_prefix('#') {
        let code = match digits.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => digits.parse::<u32>().ok()?,
        };
        return char::from_u32(code);
    }
    // Entity names are case-sensitive in HTML (`&Amp;` is not `&amp;`), so no folding.
    NAMED
        .iter()
        .find(|(name, _)| *name == body)
        .map(|(_, c)| *c)
}

/// Anything that doesn't resolve is passed through **verbatim**. A decoder that dropped
/// or mangled `AT&T` — a literal ampersand that is not an entity — would corrupt exactly
/// the text people paste in to check.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];

        // A bare `&` far from any `;` is common prose; capping the scan keeps a stray
        // ampersand from swallowing the rest of the document looking for a terminator.
        let end = after
            .char_indices()
            .take_while(|(i, c)| *i < 32 && (c.is_ascii_alphanumeric() || *c == '#'))
            .count();
        match after[end..].strip_prefix(';').and(resolve(&after[..end])) {
            Some(c) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

impl Tool for HtmlEntityTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let text = i.text("text");
        let result = match i.choice("direction") {
            "decode" => decode(text),
            _ => encode(text, i.bool("all_non_ascii")),
        };
        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, direction: &str, all_non_ascii: bool) -> String {
        HtmlEntityTool::default()
            .run(
                &Inputs::new()
                    .with("text", text)
                    .with("direction", direction)
                    .with("all_non_ascii", all_non_ascii),
            )
            .unwrap()
            .get("result")
            .unwrap()
            .as_display()
    }

    fn enc(text: &str) -> String {
        run(text, "encode", false)
    }

    fn dec(text: &str) -> String {
        run(text, "decode", false)
    }

    #[test]
    fn the_five_dangerous_characters_are_escaped() {
        assert_eq!(
            enc(r#"<a href="x">'&'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn round_trip() {
        for text in ["<script>", "a & b", "\"quoted\"", "plain text", ""] {
            assert_eq!(dec(&enc(text)), text, "{text:?}");
        }
    }

    #[test]
    fn non_ascii_is_left_alone_unless_asked_for() {
        assert_eq!(enc("café ☕"), "café ☕");
        assert_eq!(run("café ☕", "encode", true), "caf&#233; &#9749;");
        assert_eq!(dec("caf&#233; &#9749;"), "café ☕");
    }

    #[test]
    fn numeric_entities_decode_in_both_bases() {
        assert_eq!(dec("&#65;&#x42;&#X43;"), "ABC");
    }

    #[test]
    fn named_entities_decode() {
        assert_eq!(
            dec("&copy; 2026 &mdash; caf&eacute;"),
            "© 2026 — caf&eacute;"
        );
    }

    /// The failure mode that matters: a literal ampersand in ordinary prose.
    #[test]
    fn unknown_and_unterminated_entities_pass_through_verbatim() {
        assert_eq!(dec("AT&T"), "AT&T");
        assert_eq!(dec("a & b"), "a & b");
        assert_eq!(dec("&notanentity;"), "&notanentity;");
        assert_eq!(dec("&#99999999999;"), "&#99999999999;");
        assert_eq!(dec("&"), "&");
    }

    /// A stray `&` must not consume the rest of the input hunting for a `;`.
    #[test]
    fn a_stray_ampersand_does_not_swallow_the_document() {
        let text = "& a very long stretch of prose that goes on well past thirty-two bytes;";
        assert_eq!(dec(text), text);
    }

    #[test]
    fn entity_names_are_case_sensitive() {
        assert_eq!(dec("&AMP;"), "&AMP;");
        assert_eq!(dec("&amp;"), "&");
    }
}
