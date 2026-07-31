#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("{field}: {msg}")]
    InvalidInput { field: &'static str, msg: String },
    #[error("{0}")]
    Failed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl ToolError {
    pub fn invalid(field: &'static str, msg: impl Into<String>) -> Self {
        Self::InvalidInput {
            field,
            msg: msg.into(),
        }
    }
}
