use crate::engine::provider::{Provider, ProviderError, Result, Tool, ToolType};
use serde_json::Value;

pub struct ProcessProvider;

impl Provider for ProcessProvider {
    fn namespace(&self) -> &str {
        "process"
    }

    fn tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "process.list".into(),
                description: "All processes: PID, name, user, state, CPU%, mem MB, cmd".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                tool_type: ToolType::Observable,
            },
            Tool {
                name: "process.top".into(),
                description: "Top N processes sorted by CPU or memory".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "sort_by": {
                            "type": "string",
                            "enum": ["cpu", "memory"],
                            "description": "Field to sort by (cpu or memory)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Number of processes to return (default 10)"
                        }
                    }
                }),
                tool_type: ToolType::Observable,
            },
        ]
    }

    fn call(&self, tool: &str, params: Value) -> Result<Value> {
        match tool {
            "process.list" => crate::linux::procfs::get_process_list(),
            "process.top" => {
                let sort_by = params.get("sort_by").and_then(|s| s.as_str()).unwrap_or("memory");
                let limit = params.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;
                
                let mut procs = crate::linux::procfs::get_process_list_vec()?;
                
                match sort_by {
                    "memory" => {
                        procs.sort_by(|a, b| {
                            let mem_a = a.get("mem_mb").and_then(|m| m.as_f64()).unwrap_or(0.0);
                            let mem_b = b.get("mem_mb").and_then(|m| m.as_f64()).unwrap_or(0.0);
                            mem_b.partial_cmp(&mem_a).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    _ => { // default: cpu
                        procs.sort_by(|a, b| {
                            let cpu_a = a.get("cpu_percent").and_then(|m| m.as_f64()).unwrap_or(0.0);
                            let cpu_b = b.get("cpu_percent").and_then(|m| m.as_f64()).unwrap_or(0.0);
                            cpu_b.partial_cmp(&cpu_a).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                }
                
                procs.truncate(limit);
                Ok(serde_json::json!(procs))
            }
            _ => Err(ProviderError::NotFound(tool.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_provider_namespace() {
        let provider = ProcessProvider;
        assert_eq!(provider.namespace(), "process");
    }

    #[test]
    fn test_process_provider_tools() {
        let provider = ProcessProvider;
        let tools = provider.tools();
        assert_eq!(tools.len(), 2);
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"process.list"));
        assert!(names.contains(&"process.top"));
    }
}
