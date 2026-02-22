use crate::engine::provider::{Provider, ProviderError, Result, Tool, ToolType};
use serde_json::Value;

pub struct LogProvider;

impl Provider for LogProvider {
    fn namespace(&self) -> &str {
        "log"
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "log.tail".into(),
                description: "Last N journal entries, optionally filtered by unit".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "unit": {
                            "type": "string",
                            "description": "Optional unit name to filter by (e.g. nginx.service)"
                        },
                        "lines": {
                            "type": "integer",
                            "description": "Number of lines to tail (default 50)"
                        }
                    }
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "log.search".into(),
                description: "Search journal by keyword, time range, priority".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "keyword": {
                            "type": "string",
                            "description": "Keyword to search for in message body"
                        },
                        "since": {
                            "type": "string",
                            "description": "Time range (e.g. '1 hour ago', 'yesterday')"
                        },
                        "priority": {
                            "type": "string",
                            "description": "Min priority (e.g. 'err', 'warning', 'info')"
                        }
                    },
                    "required": ["keyword"]
                }),
                tool_type: ToolType::Observable,
            },
        ]
    }

    fn call(&self, tool: &str, params: Value) -> Result<Value> {
        match tool {
            "log.tail" => {
                let unit = params.get("unit").and_then(|u| u.as_str());
                let lines = params.get("lines").and_then(|l| l.as_u64()).unwrap_or(50) as usize;
                
                crate::linux::systemd::journal_tail(unit, lines)
            }
            "log.search" => {
                let keyword = params.get("keyword").and_then(|k| k.as_str())
                    .ok_or_else(|| ProviderError::Execution("Missing required parameter: keyword".into()))?;
                    
                let since = params.get("since").and_then(|s| s.as_str());
                let priority = params.get("priority").and_then(|p| p.as_str());
                
                crate::linux::systemd::journal_search(keyword, since, priority)
            }
            _ => Err(ProviderError::NotFound(tool.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_provider_namespace() {
        let provider = LogProvider;
        assert_eq!(provider.namespace(), "log");
    }

    #[test]
    fn test_log_provider_tools() {
        let provider = LogProvider;
        let tools = provider.tools();
        assert_eq!(tools.len(), 2);
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"log.tail"));
        assert!(names.contains(&"log.search"));
    }
}
