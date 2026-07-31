//! Lưới an toàn cho toàn bộ abstraction: mọi tool trong registry đều phải thỏa
//! các bất biến dưới đây. Viết từ P0 với 1 tool để sẵn sàng bắt lỗi khi catalog lớn dần.

use std::collections::HashMap;

use lazytools_core::error::ToolError;
use lazytools_core::registry::Registry;
use lazytools_core::spec::FieldKind;
use lazytools_core::value::{Inputs, Value};

/// Inputs dựng từ `default` của spec — thứ TUI nạp vào lúc mở tool.
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
        assert!(seen.insert(id, ()).is_none(), "id trùng: {id}");
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
                "{}: field key trùng: {}",
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
                        "{where_}: default {n} ngoài khoảng [{min}, {max}]"
                    );
                }
                (FieldKind::Select { options }, Value::Choice(c)) => {
                    assert!(
                        options.contains(&c.as_str()),
                        "{where_}: default {c:?} không nằm trong {options:?}"
                    );
                }
                (kind, value) => panic!("{where_}: default {value:?} không khớp kiểu {kind:?}"),
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

        assert!(!name.is_empty(), "{}: cli_name rỗng", spec.id);
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{}: cli_name {name:?} chứa ký tự không hợp lệ",
            spec.id
        );
        assert!(
            !name.starts_with('-') && !name.ends_with('-'),
            "{}: cli_name {name:?} không được bắt đầu/kết thúc bằng `-`",
            spec.id
        );

        if let Some(other) = seen.insert(name, spec.id) {
            panic!(
                "cli_name {name:?} bị dùng bởi cả `{other}` và `{}`",
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
                        "{}: khai output `{}` nhưng run() không trả về",
                        spec.id,
                        f.key
                    );
                }
            }
            // Tool có quyền từ chối input mặc định (ví dụ cần text không rỗng),
            // nhưng phải từ chối bằng InvalidInput chứ không phải panic hay Failed.
            Err(ToolError::InvalidInput { .. }) => {}
            Err(e) => panic!("{}: run() với input mặc định lỗi bất thường: {e}", spec.id),
        }
    }
}
