use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const UNITS: &[&str] = &["ms", "s", "m", "h", "d"];

pub struct DurationTool {
    spec: ToolSpec,
}

impl Default for DurationTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("convert.duration", "Duration", Category::Convert)
                .describe("Convert a duration between seconds, a clock, and ISO 8601")
                .keywords(&[
                    "duration", "time", "seconds", "ms", "humanize", "elapsed", "timeout",
                    "iso8601", "clock",
                ])
                .input(
                    Field::text("value")
                        .label("Duration")
                        .help("`90`, `1h30m`, `1:30:00`, or `1d 2h 3m`"),
                )
                .option(
                    Field::select("unit", UNITS)
                        .default("s")
                        .label("Unit of a bare number")
                        .help("Only used when the input carries no unit of its own"),
                )
                .output(Field::text("seconds").mono().label("Seconds"))
                .output(Field::text("milliseconds").mono().label("Milliseconds"))
                .output(Field::text("human").label("Human"))
                .output(Field::text("clock").mono().label("Clock (h:mm:ss)"))
                .output(Field::text("iso8601").mono().label("ISO 8601")),
        }
    }
}

const MS_PER_SECOND: i128 = 1_000;
const MS_PER_MINUTE: i128 = 60 * MS_PER_SECOND;
const MS_PER_HOUR: i128 = 60 * MS_PER_MINUTE;
const MS_PER_DAY: i128 = 24 * MS_PER_HOUR;
const MS_PER_WEEK: i128 = 7 * MS_PER_DAY;

/// Milliseconds in one unit suffix, or `None` if it isn't one.
fn unit_ms(suffix: &str) -> Option<i128> {
    let ms = match suffix {
        "ms" | "milli" | "millis" | "millisecond" | "milliseconds" => 1,
        "s" | "sec" | "secs" | "second" | "seconds" => MS_PER_SECOND,
        "m" | "min" | "mins" | "minute" | "minutes" => MS_PER_MINUTE,
        "h" | "hr" | "hrs" | "hour" | "hours" => MS_PER_HOUR,
        "d" | "day" | "days" => MS_PER_DAY,
        "w" | "wk" | "week" | "weeks" => MS_PER_WEEK,
        _ => return None,
    };
    Some(ms)
}

/// `1h30m` → 5_400_000. Every token must carry a unit; a trailing bare number
/// is rejected rather than guessed at, since `1h30` reads as both 30 minutes
/// and 30 seconds depending on who wrote it.
fn parse_units(input: &str) -> Result<i128, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut total: i128 = 0;
    let mut at = 0;
    let mut matched = false;

    while at < chars.len() {
        if chars[at].is_whitespace() || chars[at] == ',' || chars[at] == '+' {
            at += 1;
            continue;
        }
        let start = at;
        while at < chars.len() && (chars[at].is_ascii_digit() || chars[at] == '.') {
            at += 1;
        }
        if at == start {
            return Err(format!("`{}` is not a number or a unit", chars[at]));
        }
        let amount: f64 = chars[start..at]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| {
                format!(
                    "`{}` is not a number",
                    chars[start..at].iter().collect::<String>()
                )
            })?;

        let unit_start = at;
        while at < chars.len() && chars[at].is_ascii_alphabetic() {
            at += 1;
        }
        let suffix: String = chars[unit_start..at]
            .iter()
            .collect::<String>()
            .to_lowercase();
        if suffix.is_empty() {
            return Err("every part needs a unit, e.g. `1h30m`".to_string());
        }
        let unit = unit_ms(&suffix).ok_or_else(|| format!("`{suffix}` is not a unit of time"))?;
        total += (amount * unit as f64).round() as i128;
        matched = true;
    }

    if !matched {
        return Err("duration must not be empty".to_string());
    }
    Ok(total)
}

/// `1:30:00` → 5_400_000. Two parts are `m:ss`, three are `h:mm:ss`; the
/// seconds field may carry a fraction.
fn parse_clock(input: &str) -> Result<i128, String> {
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() > 3 {
        return Err("a clock has at most three parts: h:mm:ss".to_string());
    }
    // Right-aligned against seconds, so `1:30` is a minute and a half.
    let scales = [MS_PER_SECOND, MS_PER_MINUTE, MS_PER_HOUR];
    let mut total = 0;
    for (i, part) in parts.iter().rev().enumerate() {
        let part = part.trim();
        let value: f64 = part
            .parse()
            .map_err(|_| format!("`{part}` is not a number"))?;
        if value < 0.0 {
            return Err("a duration cannot be negative".to_string());
        }
        total += (value * scales[i] as f64).round() as i128;
    }
    Ok(total)
}

/// `1h 30m`, largest unit first, zero parts omitted.
fn humanize(mut ms: i128) -> String {
    if ms == 0 {
        return "0s".to_string();
    }
    let mut parts = Vec::new();
    for (unit, label) in [
        (MS_PER_WEEK, "w"),
        (MS_PER_DAY, "d"),
        (MS_PER_HOUR, "h"),
        (MS_PER_MINUTE, "m"),
        (MS_PER_SECOND, "s"),
    ] {
        let count = ms / unit;
        if count > 0 {
            parts.push(format!("{count}{label}"));
            ms -= count * unit;
        }
    }
    if ms > 0 {
        parts.push(format!("{ms}ms"));
    }
    parts.join(" ")
}

/// `h:mm:ss`, with hours running past 24 rather than rolling over — this is an
/// elapsed time, not a time of day.
fn clock(ms: i128) -> String {
    let hours = ms / MS_PER_HOUR;
    let minutes = (ms % MS_PER_HOUR) / MS_PER_MINUTE;
    let seconds = (ms % MS_PER_MINUTE) / MS_PER_SECOND;
    let millis = ms % MS_PER_SECOND;
    let base = format!("{hours}:{minutes:02}:{seconds:02}");
    if millis > 0 {
        format!("{base}.{millis:03}")
    } else {
        base
    }
}

/// ISO 8601, in days and below. Weeks are deliberately not used: `P1W` may not
/// be combined with other fields, so `1w 2h` has no ISO form with a `W` in it.
fn iso8601(ms: i128) -> String {
    if ms == 0 {
        return "PT0S".to_string();
    }
    let days = ms / MS_PER_DAY;
    let hours = (ms % MS_PER_DAY) / MS_PER_HOUR;
    let minutes = (ms % MS_PER_HOUR) / MS_PER_MINUTE;
    let seconds = (ms % MS_PER_MINUTE) as f64 / MS_PER_SECOND as f64;

    let mut out = String::from("P");
    if days > 0 {
        out.push_str(&format!("{days}D"));
    }
    if hours > 0 || minutes > 0 || seconds > 0.0 {
        out.push('T');
        if hours > 0 {
            out.push_str(&format!("{hours}H"));
        }
        if minutes > 0 {
            out.push_str(&format!("{minutes}M"));
        }
        if seconds > 0.0 {
            let rendered = format!("{seconds:.3}");
            out.push_str(rendered.trim_end_matches('0').trim_end_matches('.'));
            out.push('S');
        }
    }
    out
}

/// Seconds, with a fraction only when there is one.
fn seconds(ms: i128) -> String {
    let whole = ms / MS_PER_SECOND;
    let millis = ms % MS_PER_SECOND;
    if millis == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{millis:03}")
    }
}

impl Tool for DurationTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let raw = i.text("value").trim();
        if raw.is_empty() {
            return Err(ToolError::invalid("value", "duration must not be empty"));
        }
        if raw.starts_with('-') {
            return Err(ToolError::invalid("value", "a duration cannot be negative"));
        }

        let bare: String = raw.chars().filter(|c| *c != '_').collect();
        let ms = if raw.contains(':') {
            parse_clock(raw).map_err(|e| ToolError::invalid("value", e))?
        } else if bare.parse::<f64>().is_ok() {
            // No unit in the input, so the option decides what it meant.
            let unit = unit_ms(i.choice("unit")).unwrap_or(MS_PER_SECOND);
            let amount: f64 = bare.parse().expect("just checked");
            (amount * unit as f64).round() as i128
        } else {
            parse_units(raw).map_err(|e| ToolError::invalid("value", e))?
        };

        let mut out = Outputs::new();
        out.set("seconds", seconds(ms));
        out.set("milliseconds", ms.to_string());
        out.set("human", humanize(ms));
        out.set("clock", clock(ms));
        out.set("iso8601", iso8601(ms));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(value: &str, unit: &str) -> Result<Outputs, ToolError> {
        DurationTool::default().run(&Inputs::new().with("value", value).with("unit", unit))
    }

    fn field(value: &str, key: &str) -> String {
        run(value, "s").unwrap().get(key).unwrap().as_display()
    }

    #[test]
    fn a_bare_number_uses_the_declared_unit() {
        assert_eq!(field("5400", "human"), "1h 30m");
        let out = run("5400", "ms").unwrap();
        assert_eq!(out.get("human").unwrap().as_display(), "5s 400ms");
        let out = run("90", "m").unwrap();
        assert_eq!(out.get("human").unwrap().as_display(), "1h 30m");
    }

    #[test]
    fn unit_suffixes_add_up_in_any_order() {
        assert_eq!(field("1h30m", "seconds"), "5400");
        assert_eq!(field("1d 2h 3m 4s", "seconds"), "93784");
        assert_eq!(field("30m 1h", "seconds"), "5400");
        assert_eq!(field("500ms", "milliseconds"), "500");
    }

    #[test]
    fn the_clock_form_is_read_right_to_left() {
        assert_eq!(field("1:30", "seconds"), "90");
        assert_eq!(field("1:30:00", "seconds"), "5400");
        assert_eq!(field("100:00:00", "clock"), "100:00:00");
    }

    #[test]
    fn every_representation_agrees_on_the_same_duration() {
        let out = run("1d2h3m4.5s", "s").unwrap();
        let get = |k: &str| out.get(k).unwrap().as_display();
        assert_eq!(get("seconds"), "93784.500");
        assert_eq!(get("milliseconds"), "93784500");
        assert_eq!(get("human"), "1d 2h 3m 4s 500ms");
        assert_eq!(get("clock"), "26:03:04.500");
        assert_eq!(get("iso8601"), "P1DT2H3M4.5S");
    }

    #[test]
    fn zero_is_rendered_rather_than_left_blank() {
        assert_eq!(field("0", "human"), "0s");
        assert_eq!(field("0", "iso8601"), "PT0S");
        assert_eq!(field("0", "clock"), "0:00:00");
    }

    /// `1h30` means 30 minutes to some people and 30 seconds to others, so it
    /// is refused rather than silently picking one.
    #[test]
    fn a_trailing_number_without_a_unit_is_refused() {
        let err = run("1h30", "s").unwrap_err();
        match err {
            ToolError::InvalidInput {
                field: "value",
                msg,
            } => {
                assert!(msg.contains("unit"), "{msg}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn empty_negative_and_unknown_units_all_name_the_field() {
        for value in ["", "-5", "3 fortnights", "1:2:3:4"] {
            let err = run(value, "s").unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidInput { field: "value", .. }),
                "{value:?}: {err:?}"
            );
        }
    }
}
