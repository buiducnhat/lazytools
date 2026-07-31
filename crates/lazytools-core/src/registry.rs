use std::collections::HashMap;
use std::panic::AssertUnwindSafe;

use crate::error::ToolError;
use crate::spec::{Category, ToolSpec};
use crate::tools;
use crate::value::{Inputs, Outputs};

pub trait Tool: Send + Sync {
    fn spec(&self) -> &ToolSpec;
    fn run(&self, input: &Inputs) -> Result<Outputs, ToolError>;
}

pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
    index: HashMap<&'static str, usize>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self::from_tools(tools::register_all())
    }

    pub fn from_tools(tools: Vec<Box<dyn Tool>>) -> Self {
        let index = tools
            .iter()
            .enumerate()
            .map(|(i, t)| (t.spec().id, i))
            .collect();
        Self { tools, index }
    }

    pub fn get(&self, id: &str) -> Option<&dyn Tool> {
        self.index.get(id).map(|&i| self.tools[i].as_ref())
    }

    pub fn all(&self) -> impl Iterator<Item = &dyn Tool> {
        self.tools.iter().map(AsRef::as_ref)
    }

    pub fn by_category(&self, c: Category) -> impl Iterator<Item = &dyn Tool> {
        self.all().filter(move |t| t.spec().category == c)
    }

    /// Bọc `catch_unwind` quanh `tool.run()`: crate bên thứ ba panic không được
    /// phép làm hỏng terminal đang ở raw mode.
    pub fn run(&self, id: &str, i: &Inputs) -> Result<Outputs, ToolError> {
        let tool = self
            .get(id)
            .ok_or_else(|| ToolError::Failed(format!("không có tool nào tên `{id}`")))?;

        std::panic::catch_unwind(AssertUnwindSafe(|| tool.run(i))).unwrap_or_else(|payload| {
            let detail = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic không rõ nguyên nhân".to_string());
            Err(ToolError::Failed(format!("tool `{id}` panic: {detail}")))
        })
    }
}
