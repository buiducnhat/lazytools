use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

const UNITS: &[&str] = &["auto", "s", "ms", "us", "ns"];
const ZONES: &[&str] = &["utc", "local"];

pub struct TimestampTool {
    spec: ToolSpec,
}

impl Default for TimestampTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.timestamp", "Timestamp", Category::Web)
                .describe("Convert between Unix timestamps and human-readable dates")
                .keywords(&[
                    "timestamp",
                    "unix",
                    "epoch",
                    "date",
                    "time",
                    "iso",
                    "rfc3339",
                ])
                .input(
                    Field::text("value")
                        .label("Value")
                        .help("Unix timestamp or date string — leave empty for now"),
                )
                .option(Field::select("unit", UNITS).default("auto").label("Unit"))
                .option(
                    Field::select("timezone", ZONES)
                        .default("utc")
                        .label("Timezone"),
                )
                .output(Field::text("rfc3339").mono().label("RFC 3339"))
                .output(Field::text("unix_seconds").mono().label("Unix (s)"))
                .output(Field::text("unix_millis").mono().label("Unix (ms)"))
                .output(Field::text("utc").label("UTC"))
                .output(Field::text("local").label("Local"))
                .output(Field::text("relative").label("Relative")),
        }
    }
}

/// Splits a count in `unit` into whole seconds plus a nanosecond remainder.
/// `div_euclid`/`rem_euclid` rather than `/` and `%` so pre-epoch (negative) values
/// floor correctly instead of truncating toward zero.
fn split(n: i64, unit: &str) -> (i64, u32) {
    let (per_sec, to_nanos) = match unit {
        "ms" => (1_000i64, 1_000_000i64),
        "us" => (1_000_000, 1_000),
        "ns" => (1_000_000_000, 1),
        _ => return (n, 0),
    };
    (
        n.div_euclid(per_sec),
        (n.rem_euclid(per_sec) * to_nanos) as u32,
    )
}

/// Guesses the unit from magnitude. A plausible second-precision timestamp sits
/// around 1.7e9 and milliseconds around 1.7e12, so each threshold sits a couple of
/// orders of magnitude above the range it claims — far from any real value.
fn guess_unit(n: i64) -> &'static str {
    let abs = n.abs();
    if abs <= 100_000_000_000 {
        "s"
    } else if abs <= 100_000_000_000_000 {
        "ms"
    } else if abs <= 100_000_000_000_000_000 {
        "us"
    } else {
        "ns"
    }
}

/// Attaches a timezone to a wall-clock time that carried no offset.
fn localize(naive: NaiveDateTime, local: bool) -> DateTime<Utc> {
    if local {
        // A DST spring-forward makes some wall times nonexistent and a fall-back makes
        // others ambiguous; take the earliest valid reading rather than failing.
        Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| Utc.from_utc_datetime(&naive))
    } else {
        Utc.from_utc_datetime(&naive)
    }
}

fn parse_date(value: &str, local: bool) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        return Some(localize(naive, local));
    }
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Some(localize(date.and_hms_opt(0, 0, 0)?, local));
    }
    None
}

/// Hand-written rather than pulling in a humanization crate for six branches.
/// Months and years use average lengths — this is a rough readout, not arithmetic.
fn relative(delta_secs: i64) -> String {
    let abs = delta_secs.abs();
    if abs == 0 {
        return "now".to_string();
    }
    let (n, unit) = match abs {
        0..60 => (abs, "second"),
        60..3_600 => (abs / 60, "minute"),
        3_600..86_400 => (abs / 3_600, "hour"),
        86_400..2_592_000 => (abs / 86_400, "day"),
        2_592_000..31_536_000 => (abs / 2_592_000, "month"),
        _ => (abs / 31_536_000, "year"),
    };
    let plural = if n == 1 { "" } else { "s" };
    if delta_secs < 0 {
        format!("in {n} {unit}{plural}")
    } else {
        format!("{n} {unit}{plural} ago")
    }
}

impl Tool for TimestampTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let value = i.text("value").trim();
        let local = i.choice("timezone") == "local";
        let now = Utc::now();

        // Empty input means "now" — one of the two deliberate impurities in the whole
        // catalog (the other being the random generators). See
        // docs/architecture/spec-driven-tools.md.
        let (dt, is_now) = if value.is_empty() {
            (now, true)
        } else if value
            .strip_prefix('-')
            .unwrap_or(value)
            .chars()
            .all(|c| c.is_ascii_digit())
            && !value.strip_prefix('-').unwrap_or(value).is_empty()
        {
            let n: i64 = value
                .parse()
                .map_err(|_| ToolError::invalid("value", "timestamp is out of range for i64"))?;
            let unit = match i.choice("unit") {
                "auto" | "" => guess_unit(n),
                explicit => explicit,
            };
            let (secs, nanos) = split(n, unit);
            let dt = DateTime::from_timestamp(secs, nanos).ok_or_else(|| {
                ToolError::invalid("value", "timestamp is outside the representable range")
            })?;
            (dt, false)
        } else {
            let dt = parse_date(value, local).ok_or_else(|| {
                ToolError::invalid("value", "not a Unix timestamp or a recognized date format")
            })?;
            (dt, false)
        };

        let mut out = Outputs::new();
        out.set("rfc3339", dt.to_rfc3339());
        out.set("unix_seconds", dt.timestamp().to_string());
        out.set("unix_millis", dt.timestamp_millis().to_string());
        out.set("utc", dt.format("%Y-%m-%d %H:%M:%S UTC").to_string());
        out.set(
            "local",
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S %Z")
                .to_string(),
        );
        out.set(
            "relative",
            if is_now {
                "now".to_string()
            } else {
                relative(now.timestamp() - dt.timestamp())
            },
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(value: &str, unit: &str, timezone: &str) -> Result<Outputs, ToolError> {
        TimestampTool::default().run(
            &Inputs::new()
                .with("value", value)
                .with("unit", unit)
                .with("timezone", timezone),
        )
    }

    fn field(value: &str, unit: &str, key: &str) -> String {
        run(value, unit, "utc")
            .unwrap()
            .get(key)
            .unwrap()
            .as_display()
    }

    #[test]
    fn epoch_zero() {
        assert_eq!(field("0", "auto", "rfc3339"), "1970-01-01T00:00:00+00:00");
        assert_eq!(field("0", "auto", "unix_seconds"), "0");
        assert_eq!(field("0", "auto", "unix_millis"), "0");
    }

    #[test]
    fn known_second_timestamp() {
        assert_eq!(
            field("1700000000", "auto", "rfc3339"),
            "2023-11-14T22:13:20+00:00"
        );
    }

    /// Every magnitude must land on the same instant — that's the point of `auto`.
    #[test]
    fn auto_detects_all_four_units() {
        let cases = [
            "1700000000",
            "1700000000000",
            "1700000000000000",
            "1700000000000000000",
        ];
        for value in cases {
            assert_eq!(
                field(value, "auto", "unix_seconds"),
                "1700000000",
                "value={value}"
            );
        }
    }

    #[test]
    fn explicit_unit_overrides_the_guess() {
        // Read as milliseconds, 1700000000 is a much earlier instant than as seconds.
        assert_eq!(field("1700000000", "ms", "unix_seconds"), "1700000");
    }

    #[test]
    fn date_strings_round_trip_back_to_the_same_timestamp() {
        let seconds = field("1700000000", "auto", "unix_seconds");
        let rfc = field("1700000000", "auto", "rfc3339");
        assert_eq!(field(&rfc, "auto", "unix_seconds"), seconds);
    }

    #[test]
    fn accepts_plain_date_formats() {
        assert_eq!(
            field("2023-11-14 22:13:20", "auto", "rfc3339"),
            "2023-11-14T22:13:20+00:00"
        );
        assert_eq!(
            field("2023-11-14", "auto", "rfc3339"),
            "2023-11-14T00:00:00+00:00"
        );
    }

    #[test]
    fn pre_epoch_timestamps_work() {
        assert_eq!(field("-1", "s", "rfc3339"), "1969-12-31T23:59:59+00:00");
    }

    #[test]
    fn garbage_names_the_field() {
        let err = run("not a date", "auto", "utc").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "value", .. }),
            "{err:?}"
        );
    }

    /// The clock-dependent branch: assert only that it succeeds and reports "now",
    /// never a concrete value.
    #[test]
    fn empty_value_means_now() {
        let out = run("", "auto", "utc").unwrap();
        assert_eq!(out.get("relative").unwrap().as_display(), "now");
        for key in ["rfc3339", "unix_seconds", "unix_millis", "utc", "local"] {
            assert!(!out.get(key).unwrap().as_display().is_empty(), "{key}");
        }
    }

    #[test]
    fn relative_reads_in_both_directions() {
        assert_eq!(relative(0), "now");
        assert_eq!(relative(1), "1 second ago");
        assert_eq!(relative(3 * 3_600), "3 hours ago");
        assert_eq!(relative(-2 * 86_400), "in 2 days");
    }
}
