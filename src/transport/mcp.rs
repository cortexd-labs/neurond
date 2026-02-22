use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

use crate::engine::registry::ProviderRegistry;

/// Standard JSON-RPC 2.0 Error Codes
#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
}

impl ErrorCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Strongly typed params for tools/call
#[derive(Debug, Deserialize)]
pub struct CallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

/// A standard JSON-RPC 2.0 Request
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// A standard JSON-RPC 2.0 Response
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: code.as_i32(),
                message: message.into(),
                data: None,
            }),
        }
    }
}

pub struct McpTransport<'a> {
    registry: &'a ProviderRegistry,
}

impl<'a> McpTransport<'a> {
    pub fn new(registry: &'a ProviderRegistry) -> Self {
        Self { registry }
    }

    pub fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        // Handle notifications (JSON-RPC without an ID)
        let id = req.id.clone()?;

        match req.method.as_str() {
            "initialize" => {
                let result = serde_json::json!({
                    "protocolVersion": "2024-11-05", // Standard MCP version
                    "capabilities": {
                        "tools": {
                            "listChanged": true
                        }
                    },
                    "serverInfo": {
                        "name": "cortexd",
                        "version": "0.1.0"
                    }
                });
                Some(JsonRpcResponse::success(id, result))
            }
            "tools/list" => {
                let tools = self.registry.list_tools();
                
                // Convert to MCP Tool format
                let mcp_tools: Vec<Value> = tools.into_iter().map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })
                }).collect();

                let result = serde_json::json!({
                    "tools": mcp_tools
                });
                Some(JsonRpcResponse::success(id, result))
            }
            "tools/call" => {
                // Parse tool parameters into explicitly typed struct
                match serde_json::from_value::<CallParams>(req.params) {
                    Ok(call_params) => {
                        match self.registry.call_tool(&call_params.name, call_params.arguments) {
                            Ok(data) => {
                                // MCP result requires wrapping the tools output inside `content` array
                                let result = serde_json::json!({
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": serde_json::to_string(&data).unwrap_or_default()
                                        }
                                    ],
                                    "isError": false
                                });
                                Some(JsonRpcResponse::success(id, result))
                            }
                            Err(e) => {
                                let result = serde_json::json!({
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": format!("{:?}", e)
                                        }
                                    ],
                                    "isError": true
                                });
                                Some(JsonRpcResponse::success(id, result))
                            }
                        }
                    }
                    Err(e) => {
                        Some(JsonRpcResponse::error(id, ErrorCode::InvalidParams, format!("Invalid params: {}", e)))
                    }
                }
            }
            _ => {
                // Method not found
                Some(JsonRpcResponse::error(id, ErrorCode::MethodNotFound, "Method not found"))
            }
        }
    }

    /// Primary run loop reading from stdin and writing to stdout for MCP stdio layer
    pub fn run_stdio_loop(&self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<JsonRpcRequest>(&line) {
                Ok(req) => {
                    if let Some(res) = self.handle_request(req) {
                        let response_json = serde_json::to_string(&res)?;
                        writeln!(stdout, "{}", response_json)?;
                        stdout.flush()?;
                    }
                }
                Err(e) => {
                    let err_res = JsonRpcResponse::error(Value::Null, ErrorCode::ParseError, format!("Parse error: {}", e));
                    let response_json = serde_json::to_string(&err_res)?;
                    writeln!(stdout, "{}", response_json)?;
                    stdout.flush()?;
                }
            }
        }

        Ok(())
    }
}

// ========================================================================= //
// TDD Tests                                                                 //
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::provider::{Provider, Tool, ToolType, ProviderError};

    struct TestProvider;
    impl Provider for TestProvider {
        fn namespace(&self) -> &str { "test" }
        fn tools(&self) -> Vec<Tool> {
            vec![
                Tool {
                    name: "test.echo".into(),
                    description: "Echo parameters".into(),
                    input_schema: serde_json::json!({}),
                    tool_type: ToolType::Observable,
                }
            ]
        }
        fn call(&self, tool: &str, params: Value) -> crate::engine::provider::Result<Value> {
            if tool == "test.echo" {
                Ok(params)
            } else {
                Err(ProviderError::NotFound(tool.into()))
            }
        }
    }

    #[test]
    fn test_mcp_initialize() {
        let registry = ProviderRegistry::new();
        let mcp = McpTransport::new(&registry);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "initialize".into(),
            params: Value::Null,
        };
        let res = mcp.handle_request(req).unwrap();
        assert_eq!(res.id, serde_json::json!(1));
        assert!(res.error.is_none());
        let result = res.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "cortexd");
    }

    #[test]
    fn test_mcp_tools_list() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(TestProvider));
        let mcp = McpTransport::new(&registry);
        
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".into(),
            params: Value::Null,
        };
        let res = mcp.handle_request(req).unwrap();
        let tools = res.result.unwrap()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "test.echo");
    }

    #[test]
    fn test_mcp_tools_call() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(TestProvider));
        let mcp = McpTransport::new(&registry);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "tools/call".into(),
            params: serde_json::json!({
                "name": "test.echo",
                "arguments": {
                    "hello": "world"
                }
            }),
        };
        
        let res = mcp.handle_request(req).unwrap();
        let content = res.result.unwrap()["content"].as_array().unwrap().clone();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "{\"hello\":\"world\"}");
    }
}
