use crate::engine::provider::{Provider, ProviderError, Result, Tool, ToolType};
use serde_json::Value;

pub struct SystemProvider;

impl Provider for SystemProvider {
    fn namespace(&self) -> &str {
        "system"
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "system.info".into(),
                description: "Hostname, OS, kernel version, arch, uptime, boot time".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "system.cpu".into(),
                description: "Core count, model, usage % total and per-core".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "system.memory".into(),
                description: "Total, used, available, swap (MB)".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "system.disk".into(),
                description: "Per-mount: device, total, used, available, use %".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "system.uptime".into(),
                description: "Uptime in seconds, idle time, load averages".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                tool_type: ToolType::Observable,
            },
        ]
    }

    fn call(&self, tool: &str, _params: Value) -> Result<Value> {
        match tool {
            "system.info" => crate::linux::procfs::get_system_info(),
            "system.cpu" => crate::linux::procfs::get_system_cpu(),
            "system.memory" => crate::linux::procfs::get_system_memory(),
            "system.disk" => crate::linux::procfs::get_system_disk(),
            "system.uptime" => crate::linux::procfs::get_system_uptime(),
            _ => Err(ProviderError::NotFound(tool.into())),
        }
    }
}

// ========================================================================= //
// TDD Tests                                                                 //
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_provider_namespace() {
        let provider = SystemProvider;
        assert_eq!(provider.namespace(), "system");
    }

    #[test]
    fn test_system_provider_tools() {
        let provider = SystemProvider;
        let tools = provider.tools();
        assert_eq!(tools.len(), 5);
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"system.info"));
        assert!(names.contains(&"system.cpu"));
        assert!(names.contains(&"system.memory"));
        assert!(names.contains(&"system.disk"));
        assert!(names.contains(&"system.uptime"));
        assert!(tools.iter().all(|t| t.tool_type == ToolType::Observable));
    }

    #[test]
    fn test_system_provider_call_not_found() {
        let provider = SystemProvider;
        let res = provider.call("system.doesnotexist", serde_json::json!({}));
        assert!(matches!(res, Err(ProviderError::NotFound(_))));
    }
}
