use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct ColorTool {
    spec: ToolSpec,
}

impl Default for ColorTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.color", "Color Converter", Category::Convert)
                .describe("Convert a color between hex, RGB, HSL, HSV, and CMYK")
                .keywords(&[
                    "color", "colour", "hex", "rgb", "rgba", "hsl", "hsv", "hsb", "cmyk", "css",
                ])
                .input(
                    Field::text("color")
                        .mono()
                        .label("Color")
                        .help("`#a1b2c3`, `a1b2c3`, `#abc`, `rgb(…)`, `rgba(…)`, or `hsl(…)`"),
                )
                .output(Field::text("hex").mono().label("Hex"))
                .output(Field::text("rgb").mono().label("RGB"))
                .output(Field::text("hsl").mono().label("HSL"))
                .output(Field::text("hsv").mono().label("HSV"))
                .output(Field::text("cmyk").mono().label("CMYK")),
        }
    }
}

/// A parsed color. Channels are 0..=255 and alpha is 0.0..=1.0 — the same shape CSS
/// uses, so no output format needs a second representation to render from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: f64,
}

/// Splits the argument list of a `fn(...)` form. CSS accepts both the legacy comma
/// syntax (`rgb(1, 2, 3)`) and the modern space syntax with a slash before alpha
/// (`rgb(1 2 3 / 50%)`), and pasted colors come in both — so all three separators
/// are treated the same rather than picking a side.
fn split_args(body: &str) -> Vec<&str> {
    body.split([',', '/', ' ', '\t'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// A single channel of an `rgb()` form: either 0..=255 or a percentage.
fn parse_rgb_channel(s: &str) -> Result<u8, String> {
    let value = match s.strip_suffix('%') {
        Some(pct) => parse_number(pct)? * 255.0 / 100.0,
        None => parse_number(s)?,
    };
    Ok(value.round().clamp(0.0, 255.0) as u8)
}

/// Alpha is `0..=1` bare, or a percentage. Out-of-range values clamp rather than
/// erroring: CSS itself clamps, and rejecting `alpha=1.2` helps nobody.
fn parse_alpha(s: &str) -> Result<f64, String> {
    let value = match s.strip_suffix('%') {
        Some(pct) => parse_number(pct)? / 100.0,
        None => parse_number(s)?,
    };
    Ok(value.clamp(0.0, 1.0))
}

fn parse_number(s: &str) -> Result<f64, String> {
    s.trim()
        .parse::<f64>()
        .map_err(|_| format!("`{s}` is not a number"))
}

/// Percentages in `hsl()` may be written with or without the `%`; both are accepted
/// because the sign carries no information here — saturation is always a percentage.
fn parse_percent(s: &str) -> Result<f64, String> {
    let value = parse_number(s.strip_suffix('%').unwrap_or(s))?;
    Ok(value.clamp(0.0, 100.0) / 100.0)
}

fn parse_hex(digits: &str) -> Result<Rgba, String> {
    // The shorthand forms double each digit: `#abc` is `#aabbcc`, not `#0a0b0c`.
    let expand = |c: char| -> Result<u8, String> {
        let d = c
            .to_digit(16)
            .ok_or_else(|| format!("`{c}` is not a hex digit"))? as u8;
        Ok(d * 17)
    };
    let pair = |s: &str| -> Result<u8, String> {
        u8::from_str_radix(s, 16).map_err(|_| format!("`{s}` is not a hex pair"))
    };

    let chars: Vec<char> = digits.chars().collect();
    let (r, g, b, a) = match chars.len() {
        3 | 4 => (
            expand(chars[0])?,
            expand(chars[1])?,
            expand(chars[2])?,
            if chars.len() == 4 {
                expand(chars[3])?
            } else {
                255
            },
        ),
        6 | 8 => (
            pair(&digits[0..2])?,
            pair(&digits[2..4])?,
            pair(&digits[4..6])?,
            if digits.len() == 8 {
                pair(&digits[6..8])?
            } else {
                255
            },
        ),
        n => {
            return Err(format!(
                "hex color must have 3, 4, 6, or 8 digits (got {n})"
            ));
        }
    };

    Ok(Rgba {
        r,
        g,
        b,
        a: f64::from(a) / 255.0,
    })
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_byte = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to_byte(r), to_byte(g), to_byte(b))
}

fn parse(input: &str) -> Result<Rgba, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("color must not be empty".to_string());
    }

    // Function forms first: a bare `hsl(...)` has no `#`, and matching on the name
    // avoids the hex branch ever seeing letters it can't handle.
    if let Some((name, rest)) = input.split_once('(') {
        let body = rest
            .strip_suffix(')')
            .ok_or_else(|| format!("`{input}` is missing its closing `)`"))?;
        let args = split_args(body);
        let name = name.trim().to_ascii_lowercase();

        return match name.as_str() {
            "rgb" | "rgba" => {
                if args.len() < 3 {
                    return Err(format!("{name}() needs 3 channels (got {})", args.len()));
                }
                Ok(Rgba {
                    r: parse_rgb_channel(args[0])?,
                    g: parse_rgb_channel(args[1])?,
                    b: parse_rgb_channel(args[2])?,
                    a: args.get(3).map_or(Ok(1.0), |v| parse_alpha(v))?,
                })
            }
            "hsl" | "hsla" => {
                if args.len() < 3 {
                    return Err(format!(
                        "{name}() needs hue, saturation, and lightness (got {})",
                        args.len()
                    ));
                }
                // `deg` is the only angle unit accepted: turns/rads are rare enough in
                // pasted colors that supporting them would be guesswork, not utility.
                let hue = parse_number(args[0].trim_end_matches("deg"))?;
                let (r, g, b) = hsl_to_rgb(hue, parse_percent(args[1])?, parse_percent(args[2])?);
                Ok(Rgba {
                    r,
                    g,
                    b,
                    a: args.get(3).map_or(Ok(1.0), |v| parse_alpha(v))?,
                })
            }
            other => Err(format!("unknown color function `{other}()`")),
        };
    }

    // Hex, with the `#` optional — copying a color out of a config file often loses it.
    parse_hex(input.strip_prefix('#').unwrap_or(input))
}

fn to_hsl(c: Rgba) -> (f64, f64, f64) {
    let (r, g, b) = (
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let l = (max + min) / 2.0;

    if delta == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = delta / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (h.rem_euclid(360.0), s, l)
}

fn to_hsv(c: Rgba) -> (f64, f64, f64) {
    let (h, _, _) = to_hsl(c);
    let (r, g, b) = (
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let s = if max == 0.0 { 0.0 } else { (max - min) / max };
    (h, s, max)
}

fn to_cmyk(c: Rgba) -> (f64, f64, f64, f64) {
    let (r, g, b) = (
        f64::from(c.r) / 255.0,
        f64::from(c.g) / 255.0,
        f64::from(c.b) / 255.0,
    );
    let k = 1.0 - r.max(g).max(b);
    // Pure black has no chromatic component; the general formula divides by zero there.
    if (k - 1.0).abs() < f64::EPSILON {
        return (0.0, 0.0, 0.0, 1.0);
    }
    (
        (1.0 - r - k) / (1.0 - k),
        (1.0 - g - k) / (1.0 - k),
        (1.0 - b - k) / (1.0 - k),
        k,
    )
}

fn pct(v: f64) -> String {
    format!("{}%", (v * 100.0).round())
}

impl Tool for ColorTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let color = parse(i.text("color")).map_err(|e| ToolError::invalid("color", e))?;
        // Alpha is only shown when it carries information. Rendering `#ff0000ff` for a
        // color the user typed as `#f00` would be noise in every ordinary case.
        let opaque = (color.a - 1.0).abs() < 1e-9;

        let hex = if opaque {
            format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
        } else {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                color.r,
                color.g,
                color.b,
                (color.a * 255.0).round() as u8
            )
        };
        let rgb = if opaque {
            format!("rgb({}, {}, {})", color.r, color.g, color.b)
        } else {
            format!(
                "rgba({}, {}, {}, {})",
                color.r,
                color.g,
                color.b,
                // Two decimals: enough to round-trip an 8-bit alpha, short enough to read.
                format!("{:.2}", color.a)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
            )
        };

        let (h, s, l) = to_hsl(color);
        let (_, sv, v) = to_hsv(color);
        let (c, m, y, k) = to_cmyk(color);

        let mut out = Outputs::new();
        out.set("hex", hex);
        out.set("rgb", rgb);
        out.set("hsl", format!("hsl({}, {}, {})", h.round(), pct(s), pct(l)));
        out.set(
            "hsv",
            format!("hsv({}, {}, {})", h.round(), pct(sv), pct(v)),
        );
        out.set(
            "cmyk",
            format!("cmyk({}, {}, {}, {})", pct(c), pct(m), pct(y), pct(k)),
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(color: &str) -> Result<Outputs, ToolError> {
        ColorTool::default().run(&Inputs::new().with("color", color))
    }

    fn field(color: &str, key: &str) -> String {
        run(color).unwrap().get(key).unwrap().as_display()
    }

    #[test]
    fn hex_expands_to_every_format() {
        assert_eq!(field("#ff0000", "hex"), "#ff0000");
        assert_eq!(field("#ff0000", "rgb"), "rgb(255, 0, 0)");
        assert_eq!(field("#ff0000", "hsl"), "hsl(0, 100%, 50%)");
        assert_eq!(field("#ff0000", "hsv"), "hsv(0, 100%, 100%)");
        assert_eq!(field("#ff0000", "cmyk"), "cmyk(0%, 100%, 100%, 0%)");
    }

    #[test]
    fn shorthand_hex_doubles_each_digit() {
        assert_eq!(field("#abc", "hex"), "#aabbcc");
        assert_eq!(field("#f00", "rgb"), "rgb(255, 0, 0)");
    }

    /// A color pasted out of a config file often arrives without its `#`.
    #[test]
    fn leading_hash_is_optional() {
        assert_eq!(field("00ff00", "hex"), "#00ff00");
    }

    #[test]
    fn rgb_and_hsl_functions_round_trip_to_hex() {
        assert_eq!(field("rgb(255, 0, 0)", "hex"), "#ff0000");
        assert_eq!(field("hsl(120, 100%, 50%)", "hex"), "#00ff00");
        assert_eq!(field("hsl(240deg 100% 50%)", "hex"), "#0000ff");
    }

    /// The modern space + slash syntax is as common as the legacy comma one.
    #[test]
    fn space_separated_syntax_is_accepted() {
        assert_eq!(field("rgb(255 128 0)", "hex"), "#ff8000");
        assert_eq!(field("rgb(255 0 0 / 50%)", "rgb"), "rgba(255, 0, 0, 0.5)");
    }

    #[test]
    fn alpha_is_only_shown_when_it_is_not_opaque() {
        assert_eq!(field("#ff000080", "hex"), "#ff000080");
        assert!(field("#ff000080", "rgb").starts_with("rgba("));
        // Fully opaque alpha collapses back to the short forms.
        assert_eq!(field("#ff0000ff", "hex"), "#ff0000");
        assert_eq!(field("rgba(255, 0, 0, 1)", "rgb"), "rgb(255, 0, 0)");
    }

    #[test]
    fn greyscale_has_zero_saturation_and_black_is_pure_k() {
        assert_eq!(field("#808080", "hsl"), "hsl(0, 0%, 50%)");
        assert_eq!(field("#000000", "cmyk"), "cmyk(0%, 0%, 0%, 100%)");
        assert_eq!(field("#ffffff", "cmyk"), "cmyk(0%, 0%, 0%, 0%)");
    }

    /// The conversion itself must be exact: every one of the 8-bit greys and the corner
    /// colors, plus a spread of arbitrary ones, must survive RGB → HSL → RGB unchanged
    /// at full precision. This is the test that would catch a real algebra bug.
    #[test]
    fn hsl_conversion_is_lossless_at_full_precision() {
        let mut cases: Vec<Rgba> = (0..=255u8)
            .step_by(17)
            .map(|v| Rgba {
                r: v,
                g: v,
                b: v,
                a: 1.0,
            })
            .collect();
        for (r, g, b) in [
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (52, 152, 219),
            (230, 126, 34),
            (18, 52, 86),
            (1, 2, 3),
            (254, 253, 252),
        ] {
            cases.push(Rgba { r, g, b, a: 1.0 });
        }

        for c in cases {
            let (h, s, l) = to_hsl(c);
            assert_eq!(hsl_to_rgb(h, s, l), (c.r, c.g, c.b), "{c:?}");
        }
    }

    /// The *rendered* HSL is rounded to whole degrees and whole percents, because that
    /// is the form CSS is written in — so its round trip is lossy by construction. Half
    /// a percent of saturation or lightness is already ±1.3 of an 8-bit channel, and
    /// half a degree of hue adds ~2 more at full chroma, which bounds the drift at 3.
    #[test]
    fn rendered_hsl_round_trips_within_the_rounding_error() {
        for hex in ["#3498db", "#e67e22", "#2ecc71", "#9b59b6", "#123456"] {
            let back = field(&field(hex, "hsl"), "hex");
            for (a, b) in [(1, 3), (3, 5), (5, 7)] {
                let (want, got) = (&hex[a..b], &back[a..b]);
                let delta =
                    i32::from_str_radix(want, 16).unwrap() - i32::from_str_radix(got, 16).unwrap();
                assert!(delta.abs() <= 3, "{hex} -> {back}: channel {want}/{got}");
            }
        }
    }

    #[test]
    fn bad_input_names_the_field() {
        for bad in [
            "",
            "#12345",
            "zzz",
            "rgb(1, 2)",
            "cmyk(1,2,3,4)",
            "rgb(1,2,3",
        ] {
            let err = run(bad).unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidInput { field: "color", .. }),
                "{bad:?}: {err:?}"
            );
        }
    }
}
