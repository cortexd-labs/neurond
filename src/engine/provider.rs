use serde_json::Value;

pub type Result<T> = std::result::Result<T, ProviderError>;

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderError {
    Execution(String),
    NotFound(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Execution(msg) => write!(f, "Execution error: {}", msg),
            ProviderError::NotFound(msg) => write!(f, "Tool not found: {}", msg),
        }
    }
}

impl std::error::Error for ProviderError {}

/// Defines whether a tool is observable (read-only) or actionable (mutates state)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolType {
    Observable,
    Actionable,
}

/// The definition of an exposed tool.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub tool_type: ToolType,
}

/// The trait that must be implemented by all providers
pub trait Provider: Send + Sync {
    /// Unique namespace prefix: "system", "service", "process"
    fn namespace(&self) -> &str;

    /// All tools this provider offers
    fn tools(&self) -> Vec<Tool>;

    /// Execute a tool call, return structured JSON
    fn call(&self, tool: &str, params: Value) -> Result<Value>;
}
