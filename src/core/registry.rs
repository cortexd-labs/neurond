use crate::core::provider::{Provider, Result, Tool};
use serde_json::Value;
use std::collections::HashMap;

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider with the registry
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        self.providers.insert(provider.namespace().to_string(), provider);
    }

    /// List all tools exposed by all registered providers
    pub fn list_tools(&self) -> Vec<Tool> {
        let mut all_tools = Vec::new();
        for provider in self.providers.values() {
            all_tools.extend(provider.tools());
        }
        all_tools
    }

    /// Call a specific tool by its namespaced name (e.g., "system.info")
    pub fn call_tool(&self, name: &str, params: Value) -> Result<Value> {
        // Find the provider that matches the namespace prefix
        let parts: Vec<&str> = name.splitn(2, '.').collect();
        if parts.len() != 2 {
            use crate::core::provider::ProviderError;
            return Err(ProviderError::NotFound(name.to_string()));
        }

        let namespace = parts[0];
        
        if let Some(provider) = self.providers.get(namespace) {
            provider.call(name, params)
        } else {
            use crate::core::provider::ProviderError;
            Err(ProviderError::NotFound(name.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::provider::{ToolType, ProviderError};

    struct MockSystemProvider;

    impl Provider for MockSystemProvider {
        fn namespace(&self) -> &str {
            "system"
        }

        fn tools(&self) -> Vec<Tool> {
            vec![
                Tool {
                    name: "system.info".to_string(),
                    description: "Get system info".to_string(),
                    input_schema: serde_json::json!({}),
                    tool_type: ToolType::Observable,
                }
            ]
        }

        fn call(&self, tool: &str, _params: Value) -> Result<Value> {
            match tool {
                "system.info" => Ok(serde_json::json!({"os": "linux"})),
                _ => Err(ProviderError::NotFound(tool.to_string())),
            }
        }
    }

    #[test]
    fn test_registry_list_tools() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockSystemProvider));

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1, "Should list exactly 1 tool");
        assert_eq!(tools[0].name, "system.info");
    }

    #[test]
    fn test_registry_call_tool() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockSystemProvider));

        let result = registry.call_tool("system.info", serde_json::json!({}));
        assert_eq!(result.unwrap(), serde_json::json!({"os": "linux"}));
    }

    #[test]
    fn test_registry_call_unknown_tool() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockSystemProvider));

        let result = registry.call_tool("system.unknown", serde_json::json!({}));
        assert!(matches!(result, Err(ProviderError::NotFound(_))));
        
        let result = registry.call_tool("unknown.tool", serde_json::json!({}));
        assert!(matches!(result, Err(ProviderError::NotFound(_))));
    }
}
