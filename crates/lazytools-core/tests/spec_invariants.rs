//! Safety net for the whole abstraction: every tool in the registry must satisfy
//! the invariants below. Written back in P0 with a single tool, so it's ready to
//! catch errors as the catalog grows.

use std::collections::HashMap;

use lazytools_core::error::ToolError;
use lazytools_core::registry::Registry;
use lazytools_core::spec::FieldKind;
use lazytools_core::value::{Inputs, Value};

/// Inputs built from the spec's `default` — what the TUI loads when opening a tool.
fn default_inputs(spec: &lazytools_core::spec::ToolSpec) -> Inputs {
    let mut inputs = Inputs::new();
    for f in spec.inputs.iter().chain(spec.options.iter()) {
        if let Some(v) = &f.default {
            inputs.set(f.key, v.clone());
        }
    }
    inputs
}

#[test]
fn tool_ids_are_unique() {
    let registry = Registry::new();
    let mut seen: HashMap<&str, ()> = HashMap::new();
    for tool in registry.all() {
        let id = tool.spec().id;
        assert!(seen.insert(id, ()).is_none(), "duplicate id: {id}");
    }
}

#[test]
fn field_keys_are_unique_within_each_tool() {
    let registry = Registry::new();
    for tool in registry.all() {
        let spec = tool.spec();
        let mut seen: HashMap<&str, ()> = HashMap::new();
        for f in spec.all_fields() {
            assert!(
                seen.insert(f.key, ()).is_none(),
                "{}: duplicate field key: {}",
                spec.id,
                f.key
            );
        }
    }
}

#[test]
fn defaults_match_field_kind() {
    let registry = Registry::new();
    for tool in registry.all() {
        let spec = tool.spec();
        for f in spec.all_fields() {
            let Some(default) = &f.default else { continue };
            let where_ = format!("{}.{}", spec.id, f.key);
            match (&f.kind, default) {
                (
                    FieldKind::Text { .. } | FieldKind::Secret | FieldKind::FilePath { .. },
                    Value::Text(_),
                ) => {}
                (FieldKind::Toggle, Value::Bool(_)) => {}
                (FieldKind::Number { min, max }, Value::Num(n)) => {
                    assert!(
                        n >= min && n <= max,
                        "{where_}: default {n} is outside range [{min}, {max}]"
                    );
                }
                (FieldKind::Select { options }, Value::Choice(c)) => {
                    assert!(
                        options.contains(&c.as_str()),
                        "{where_}: default {c:?} is not among {options:?}"
                    );
                }
                (kind, value) => panic!("{where_}: default {value:?} doesn't match kind {kind:?}"),
            }
        }
    }
}

#[test]
fn ids_map_to_unique_valid_cli_names() {
    let registry = Registry::new();
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for tool in registry.all() {
        let spec = tool.spec();
        let name = spec.cli_name();

        assert!(!name.is_empty(), "{}: cli_name is empty", spec.id);
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{}: cli_name {name:?} contains invalid characters",
            spec.id
        );
        assert!(
            !name.starts_with('-') && !name.ends_with('-'),
            "{}: cli_name {name:?} must not start/end with `-`",
            spec.id
        );

        if let Some(other) = seen.insert(name, spec.id) {
            panic!(
                "cli_name {name:?} is used by both `{other}` and `{}`",
                spec.id
            );
        }
    }
}

#[test]
fn declared_outputs_are_actually_produced() {
    let registry = Registry::new();
    for tool in registry.all() {
        let spec = tool.spec();
        let inputs = default_inputs(spec);

        match registry.run(spec.id, &inputs) {
            Ok(outputs) => {
                for f in &spec.outputs {
                    assert!(
                        outputs.get(f.key).is_some(),
                        "{}: declares output `{}` but run() didn't return it",
                        spec.id,
                        f.key
                    );
                }
            }
            // A tool is allowed to reject the default input (e.g. it requires
            // non-empty text), but it must reject it with InvalidInput rather
            // than panicking or returning Failed.
            Err(ToolError::InvalidInput { .. }) => {}
            Err(e) => panic!(
                "{}: run() with default input failed unexpectedly: {e}",
                spec.id
            ),
        }
    }
}
