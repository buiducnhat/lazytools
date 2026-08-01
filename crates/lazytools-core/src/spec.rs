use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Crypto,
    Convert,
    Generate,
    Text,
    Web,
}

impl Category {
    pub const ALL: &'static [Category] = &[
        Category::Crypto,
        Category::Convert,
        Category::Generate,
        Category::Text,
        Category::Web,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Crypto => "Crypto",
            Self::Convert => "Convert",
            Self::Generate => "Generate",
            Self::Text => "Text",
            Self::Web => "Web",
        }
    }
}

/// `Live` re-runs the tool every time the input changes (with debounce); `OnDemand`
/// only runs when the user explicitly requests it. This lives in the spec so the
/// cost constraint is visible right at the point the tool is declared — bcrypt at
/// cost 12 takes ~250ms, and running it live would freeze the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunMode {
    #[default]
    Live,
    OnDemand,
    /// Like `Live` — runs on open and whenever an option changes — but the confirm
    /// key also re-runs it. Random generators need that: with `Live` alone there is
    /// no way to ask for a *different* password without editing a field, and with
    /// `OnDemand` the tool opens showing nothing at all.
    Generate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    Text { multiline: bool, mono: bool },
    Secret,
    Number { min: i64, max: i64 },
    Select { options: &'static [&'static str] },
    Toggle,
    FilePath { must_exist: bool },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub default: Option<Value>,
    pub help: Option<&'static str>,
}

impl Field {
    fn of(key: &'static str, kind: FieldKind) -> Self {
        Self {
            key,
            label: key,
            kind,
            default: None,
            help: None,
        }
    }

    pub fn text(key: &'static str) -> Self {
        Self::of(
            key,
            FieldKind::Text {
                multiline: false,
                mono: false,
            },
        )
    }

    pub fn secret(key: &'static str) -> Self {
        Self::of(key, FieldKind::Secret)
    }

    pub fn number(key: &'static str, min: i64, max: i64) -> Self {
        Self::of(key, FieldKind::Number { min, max })
    }

    pub fn select(key: &'static str, options: &'static [&'static str]) -> Self {
        Self::of(key, FieldKind::Select { options })
    }

    pub fn toggle(key: &'static str) -> Self {
        Self::of(key, FieldKind::Toggle)
    }

    pub fn filepath(key: &'static str, must_exist: bool) -> Self {
        Self::of(key, FieldKind::FilePath { must_exist })
    }

    pub fn multiline(mut self) -> Self {
        if let FieldKind::Text { multiline, .. } = &mut self.kind {
            *multiline = true;
        }
        self
    }

    pub fn mono(mut self) -> Self {
        if let FieldKind::Text { mono, .. } = &mut self.kind {
            *mono = true;
        }
        self
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.label = label;
        self
    }

    pub fn default(mut self, v: impl Into<Value>) -> Self {
        // `Select` carries its value as `Choice` to distinguish it from free-form text.
        let v = v.into();
        self.default = Some(match (&self.kind, v) {
            (FieldKind::Select { .. }, Value::Text(s)) => Value::Choice(s),
            (_, v) => v,
        });
        self
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub inputs: Vec<Field>,
    pub options: Vec<Field>,
    pub outputs: Vec<Field>,
    pub mode: RunMode,
}

impl ToolSpec {
    pub fn new(id: &'static str, name: &'static str, category: Category) -> Self {
        Self {
            id,
            name,
            category,
            description: "",
            keywords: &[],
            inputs: Vec::new(),
            options: Vec::new(),
            outputs: Vec::new(),
            mode: RunMode::Live,
        }
    }

    pub fn describe(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub fn keywords(mut self, keywords: &'static [&'static str]) -> Self {
        self.keywords = keywords;
        self
    }

    pub fn input(mut self, f: Field) -> Self {
        self.inputs.push(f);
        self
    }

    pub fn option(mut self, f: Field) -> Self {
        self.options.push(f);
        self
    }

    pub fn output(mut self, f: Field) -> Self {
        self.outputs.push(f);
        self
    }

    pub fn mode(mut self, mode: RunMode) -> Self {
        self.mode = mode;
        self
    }

    /// CLI subcommand name: strips the category prefix (`crypto.hash` → `hash`).
    /// This lives here rather than in the CLI layer so the CLI layer doesn't need
    /// to know anything about the id.
    pub fn cli_name(&self) -> &'static str {
        match self.id.split_once('.') {
            Some((_, rest)) => rest,
            None => self.id,
        }
    }

    /// Every field of the tool, in the order inputs → options → outputs.
    pub fn all_fields(&self) -> impl Iterator<Item = &Field> {
        self.inputs
            .iter()
            .chain(self.options.iter())
            .chain(self.outputs.iter())
    }

    /// The input that receives data from stdin in the CLI, and is the target of
    /// "open file" in the TUI.
    pub fn primary_input(&self) -> Option<&Field> {
        self.inputs.first()
    }
}
