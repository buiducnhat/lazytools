use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Num(i64),
    Bool(bool),
    Choice(String),
}

impl Value {
    /// Dạng chuỗi để hiển thị / ghi ra CLI. `Num`/`Bool` cũng chuyển được.
    pub fn as_display(&self) -> String {
        match self {
            Self::Text(s) | Self::Choice(s) => s.clone(),
            Self::Num(n) => n.to_string(),
            Self::Bool(b) => b.to_string(),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Text(s) | Self::Choice(s) => s,
            _ => "",
        }
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Num(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

/// Accessor đều panic-free: thiếu key hoặc sai biến thể thì trả giá trị rỗng/zero.
/// `run()` nhờ vậy không phải viết `unwrap()`, và tool khai thiếu field không làm sập app.
#[derive(Debug, Default, Clone)]
pub struct Inputs(HashMap<&'static str, Value>);

impl Inputs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: &'static str, v: impl Into<Value>) {
        self.0.insert(key, v.into());
    }

    pub fn with(mut self, key: &'static str, v: impl Into<Value>) -> Self {
        self.set(key, v);
        self
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn text(&self, key: &str) -> &str {
        self.0.get(key).map_or("", Value::as_str)
    }

    pub fn choice(&self, key: &str) -> &str {
        self.text(key)
    }

    pub fn num(&self, key: &str) -> i64 {
        match self.0.get(key) {
            Some(Value::Num(n)) => *n,
            _ => 0,
        }
    }

    pub fn bool(&self, key: &str) -> bool {
        match self.0.get(key) {
            Some(Value::Bool(b)) => *b,
            _ => false,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Outputs(HashMap<&'static str, Value>);

impl Outputs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shorthand cho trường hợp phổ biến nhất: đúng một output.
    pub fn one(key: &'static str, v: impl Into<Value>) -> Self {
        let mut o = Self::new();
        o.set(key, v);
        o
    }

    pub fn set(&mut self, key: &'static str, v: impl Into<Value>) {
        self.0.insert(key, v.into());
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
