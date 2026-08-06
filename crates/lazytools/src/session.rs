//! `~/.local/state/lazytools/session.toml` — what the app remembers between runs.
//!
//! Written on quit, read on startup. Three rules shape everything here:
//!
//! - **A `Secret` field never reaches the disk.** Not in any restore mode, not
//!   truncated, not hashed. `crypto.hmac`'s key and `crypto.bcrypt`'s password
//!   are the reason the rule exists, and it is enforced by field *kind*, so a
//!   tool declaring a new secret inherits it without touching this file.
//! - **A restored value must still be legal.** The file outlives the version
//!   that wrote it: a `Select` option can disappear, a `Number` range can
//!   narrow. Every value is re-validated against the spec it is going into, and
//!   anything that no longer fits is dropped rather than forced in.
//! - **State is not config.** A corrupt or stale file costs the user their last
//!   open tool, so it is discarded quietly instead of raising a popup about a
//!   file they never wrote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lazytools_core::spec::{FieldKind, ToolSpec};
use lazytools_core::value::{Inputs, Value};
use serde::{Deserialize, Serialize};

use crate::settings::Restore;

pub const FILE_NAME: &str = "session.toml";

/// Bumped when the shape below changes incompatibly. A file from another
/// version is ignored, not migrated — the cost of getting it wrong is a user's
/// form filled with someone else's idea of what the fields meant.
const VERSION: u32 = 1;

/// Values longer than this are left out.
///
/// `Ctrl+O` reads files up to 10MB into the primary input, and a session file
/// is not a document store — writing one on every quit would turn "I checked a
/// diff once" into a permanent copy on disk. Options, which is what the default
/// mode saves, are never anywhere near this.
const MAX_VALUE_BYTES: usize = 8 * 1024;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Id of the tool that was open.
    pub tool: Option<String>,
    /// Its field values, keyed by field key.
    #[serde(default)]
    pub values: BTreeMap<String, toml::Value>,
}

/// The on-disk document. `version` lives here rather than on `Session` so the
/// rest of the program never has to carry it around.
#[derive(Debug, Serialize, Deserialize)]
struct Document {
    version: u32,
    #[serde(flatten)]
    session: Session,
}

impl Session {
    pub fn path() -> Option<PathBuf> {
        crate::paths::state_file(FILE_NAME)
    }

    pub fn load() -> Self {
        Self::path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    /// Anything unreadable, unparseable, or written by another version reads as
    /// "no session" — see the module docs on why this one doesn't report.
    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        match toml::from_str::<Document>(&text) {
            Ok(doc) if doc.version == VERSION => doc.session,
            _ => Self::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        match Self::path() {
            Some(path) => self.save_to(&path),
            // No HOME: nowhere to put it, and inventing a location is worse.
            None => Ok(()),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let doc = Document {
            version: VERSION,
            session: self.clone(),
        };
        let text = toml::to_string_pretty(&doc)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::paths::ensure_parent(path)?;
        std::fs::write(path, text)
    }

    /// Removes the file. Used when persistence is switched off: a session left
    /// behind by an earlier setting would otherwise sit on disk forever.
    pub fn clear() -> std::io::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

/// The values of `spec`'s fields worth writing down, given the restore mode.
///
/// Outputs are never included: they are derived from the inputs, so saving them
/// would risk restoring a result that no longer matches the form above it.
pub fn capture(
    spec: &ToolSpec,
    inputs: &Inputs,
    restore: Restore,
) -> BTreeMap<String, toml::Value> {
    let mut out = BTreeMap::new();
    if restore.is_off() {
        return out;
    }
    let fields = spec
        .inputs
        .iter()
        .filter(|_| restore.includes_inputs())
        .chain(spec.options.iter());

    for field in fields {
        if field.kind == FieldKind::Secret {
            continue;
        }
        let Some(value) = inputs.get(field.key) else {
            continue;
        };
        if let Some(v) = to_toml(value) {
            out.insert(field.key.to_string(), v);
        }
    }
    out
}

/// The subset of `values` that can legally be put back into `spec`, in field
/// order. Unknown keys, wrong types, and values a field would now reject are
/// dropped silently — this is a stale-cache path, not user input.
pub fn restorable(
    spec: &ToolSpec,
    values: &BTreeMap<String, toml::Value>,
    restore: Restore,
) -> Vec<(&'static str, Value)> {
    if restore.is_off() {
        return Vec::new();
    }
    spec.inputs
        .iter()
        .filter(|_| restore.includes_inputs())
        .chain(spec.options.iter())
        .filter(|f| f.kind != FieldKind::Secret)
        .filter_map(|f| {
            let raw = values.get(f.key)?;
            Some((f.key, from_toml(raw, &f.kind)?))
        })
        .collect()
}

fn to_toml(value: &Value) -> Option<toml::Value> {
    let v = match value {
        Value::Text(s) | Value::Choice(s) => {
            if s.len() > MAX_VALUE_BYTES {
                return None;
            }
            toml::Value::String(s.clone())
        }
        Value::Num(n) => toml::Value::Integer(*n),
        Value::Bool(b) => toml::Value::Boolean(*b),
    };
    Some(v)
}

/// Converts a stored value back, or `None` if it no longer fits the field.
fn from_toml(raw: &toml::Value, kind: &FieldKind) -> Option<Value> {
    match kind {
        FieldKind::Text { .. } => {
            let s = raw.as_str()?;
            (s.len() <= MAX_VALUE_BYTES).then(|| Value::Text(s.to_string()))
        }
        FieldKind::FilePath { .. } => Some(Value::Text(raw.as_str()?.to_string())),
        // A `Select` whose stored choice was removed from the tool must not come
        // back: the widget would show a value its own list doesn't contain.
        FieldKind::Select { options } => {
            let s = raw.as_str()?;
            options.contains(&s).then(|| Value::Choice(s.to_string()))
        }
        // Same idea for a narrowed range — restore only what the field would accept.
        FieldKind::Number { min, max } => {
            let n = raw.as_integer()?;
            (*min..=*max).contains(&n).then_some(Value::Num(n))
        }
        FieldKind::Toggle => Some(Value::Bool(raw.as_bool()?)),
        // Unreachable: both callers filter secrets out first. Belt and braces —
        // this is the rule that must not be broken by a future caller either.
        FieldKind::Secret => None,
    }
}

#[cfg(test)]
mod tests {
    use lazytools_core::spec::{Category, Field};

    use super::*;

    const CHOICES: &[&str] = &["encode", "decode"];

    fn spec() -> ToolSpec {
        ToolSpec::new("test.tool", "Test", Category::Convert)
            .input(Field::text("text").multiline())
            .input(Field::secret("key"))
            .option(Field::select("direction", CHOICES).default("encode"))
            .option(Field::toggle("url_safe").default(false))
            .option(Field::number("cost", 4, 12).default(10i64))
            .output(Field::text("result"))
    }

    fn filled() -> Inputs {
        Inputs::new()
            .with("text", "hello")
            .with("key", "hunter2")
            .with("direction", Value::Choice("decode".into()))
            .with("url_safe", true)
            .with("cost", 12i64)
            .with("result", "derived")
    }

    #[test]
    fn the_default_mode_saves_options_but_no_input_data() {
        let saved = capture(&spec(), &filled(), Restore::Options);
        assert_eq!(
            saved.keys().collect::<Vec<_>>(),
            ["cost", "direction", "url_safe"]
        );
    }

    #[test]
    fn restore_all_adds_the_inputs() {
        let saved = capture(&spec(), &filled(), Restore::All);
        assert_eq!(
            saved.get("text").and_then(toml::Value::as_str),
            Some("hello")
        );
    }

    /// The rule the whole module exists to keep. `Restore::All` is the mode most
    /// likely to break it, so that is the one asserted.
    #[test]
    fn a_secret_is_never_written_in_any_mode() {
        for mode in [Restore::Off, Restore::Options, Restore::All] {
            let saved = capture(&spec(), &filled(), mode);
            assert!(!saved.contains_key("key"), "secret leaked in {mode:?}");
        }
    }

    #[test]
    fn outputs_are_not_saved() {
        let saved = capture(&spec(), &filled(), Restore::All);
        assert!(!saved.contains_key("result"));
    }

    #[test]
    fn off_saves_nothing_and_restores_nothing() {
        assert!(capture(&spec(), &filled(), Restore::Off).is_empty());
        let stored = capture(&spec(), &filled(), Restore::All);
        assert!(restorable(&spec(), &stored, Restore::Off).is_empty());
    }

    #[test]
    fn a_value_over_the_cap_is_left_out_rather_than_truncated() {
        let big = "x".repeat(MAX_VALUE_BYTES + 1);
        let inputs = filled().with("text", big);
        let saved = capture(&spec(), &inputs, Restore::All);
        assert!(!saved.contains_key("text"));
    }

    #[test]
    fn a_round_trip_comes_back_as_the_same_values() {
        let stored = capture(&spec(), &filled(), Restore::All);
        let back = restorable(&spec(), &stored, Restore::All);
        assert_eq!(
            back,
            vec![
                ("text", Value::Text("hello".into())),
                ("direction", Value::Choice("decode".into())),
                ("url_safe", Value::Bool(true)),
                ("cost", Value::Num(12)),
            ]
        );
    }

    /// A session file outlives the catalog that wrote it.
    #[test]
    fn values_the_field_would_now_reject_are_dropped() {
        let stored = BTreeMap::from([
            // Option removed from the tool since this was written.
            ("direction".to_string(), toml::Value::String("rot13".into())),
            // Range narrowed since this was written.
            ("cost".to_string(), toml::Value::Integer(31)),
            // Type changed since this was written.
            ("url_safe".to_string(), toml::Value::String("yes".into())),
        ]);
        assert!(restorable(&spec(), &stored, Restore::All).is_empty());
    }

    #[test]
    fn a_file_from_another_version_is_ignored() {
        let dir = std::env::temp_dir().join("lazytools-test-session-version");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);

        let session = Session {
            tool: Some("convert.base64".into()),
            values: BTreeMap::new(),
        };
        session.save_to(&path).unwrap();
        assert_eq!(
            Session::load_from(&path).tool.as_deref(),
            Some("convert.base64")
        );

        let bumped = std::fs::read_to_string(&path)
            .unwrap()
            .replace("version = 1", "version = 99");
        std::fs::write(&path, bumped).unwrap();
        assert!(Session::load_from(&path).tool.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_file_reads_as_no_session() {
        let dir = std::env::temp_dir().join("lazytools-test-session-corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);
        std::fs::write(&path, "not { toml ===").unwrap();
        assert!(Session::load_from(&path).tool.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn saving_creates_the_state_directory() {
        let dir = std::env::temp_dir().join("lazytools-test-session-mkdir");
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("nested").join(FILE_NAME);
        Session::default().save_to(&path).unwrap();
        assert!(path.is_file());
        std::fs::remove_dir_all(&dir).ok();
    }
}
