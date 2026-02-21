use crate::core::provider::{Provider, ProviderError, Result, Tool, ToolType};
use serde_json::Value;

pub struct ServiceProvider;

impl Provider for ServiceProvider {
    fn namespace(&self) -> &str {
        "service"
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "service.list".into(),
                description: "All systemd units with state, sub-state, description".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "service.status".into(),
                description: "Unit detail: state, PID, memory, CPU, started_at".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The unit name, e.g. nginx.service"
                        }
                    },
                    "required": ["name"]
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "service.logs".into(),
                description: "Recent journal entries for a unit (configurable lines)".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The unit name, e.g. nginx.service"
                        },
                        "lines": {
                            "type": "integer",
                            "description": "Number of lines to return (default 50)"
                        }
                    },
                    "required": ["name"]
                }),
                tool_type: ToolType::Observable,
            },
        ]
    }

    fn call(&self, tool: &str, params: Value) -> Result<Value> {
        match tool {
            "service.list" => crate::linux::systemd::list_units(),
            "service.status" => {
                let name = params.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| ProviderError::Execution("Missing required parameter: name".into()))?;
                crate::linux::systemd::get_unit_status(name)
            }
            "service.logs" => {
                let name = params.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| ProviderError::Execution("Missing required parameter: name".into()))?;
                let lines = params.get("lines").and_then(|n| n.as_u64()).unwrap_or(50) as usize;
                
                // Currently returning stub for journal as it requires sd-journal or process spanning
                Ok(serde_json::json!({
                    "unit": name,
                    "entries": [format!("Journal stub for {} (lines: {})", name, lines)]
                }))
            }
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
    fn test_service_provider_namespace() {
        let provider = ServiceProvider;
        assert_eq!(provider.namespace(), "service");
    }

    #[test]
    fn test_service_provider_tools() {
        let provider = ServiceProvider;
        let tools = provider.tools();
        assert_eq!(tools.len(), 3);
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"service.list"));
        assert!(names.contains(&"service.status"));
        assert!(names.contains(&"service.logs"));
    }
}
