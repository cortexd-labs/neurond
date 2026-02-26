use rmcp::model::Tool;

/// Apply namespace prefix to a tool name.
///
/// Given namespace "linux" and tool name "system.cpu", returns "linux.system.cpu".
pub fn prefix_tool_name(namespace: &str, tool_name: &str) -> String {
    format!("{}.{}", namespace, tool_name)
}

/// Strip namespace prefix from a tool name.
///
/// Given namespace "linux" and prefixed name "linux.system.cpu", returns Some("system.cpu").
/// Returns None if the tool name doesn't match the namespace.
pub fn strip_namespace(namespace: &str, prefixed_name: &str) -> Option<String> {
    let prefix = format!("{}.", namespace);
    if prefixed_name.starts_with(&prefix) {
        Some(prefixed_name[prefix.len()..].to_string())
    } else {
        None
    }
}

/// Apply namespace prefix to all tools from a downstream.
/// Returns a new vec of tools with prefixed names.
pub fn namespace_tools(namespace: &str, tools: &[Tool]) -> Vec<Tool> {
    tools
        .iter()
        .map(|tool| {
            let mut namespaced = tool.clone();
            namespaced.name = prefix_tool_name(namespace, &tool.name).into();
            namespaced
        })
        .collect()
}

/// Resolve which downstream namespace a tool call belongs to.
///
/// Given a list of known namespaces and a tool name like "linux.system.cpu",
/// returns the matching namespace and the original tool name ("linux", "system.cpu").
pub fn resolve_namespace<'a>(
    namespaces: &'a [String],
    tool_name: &str,
) -> Option<(&'a str, String)> {
    // Sort by longest namespace first to handle nested namespaces correctly
    // (e.g., "linux.docker" before "linux")
    let mut sorted: Vec<&str> = namespaces.iter().map(|s| s.as_str()).collect();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.len()));

    for ns in sorted {
        if let Some(original) = strip_namespace(ns, tool_name) {
            return Some((ns, original));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_tool_name() {
        assert_eq!(prefix_tool_name("linux", "system.cpu"), "linux.system.cpu");
        assert_eq!(prefix_tool_name("redis", "get"), "redis.get");
    }

    #[test]
    fn test_strip_namespace() {
        assert_eq!(
            strip_namespace("linux", "linux.system.cpu"),
            Some("system.cpu".to_string())
        );
        assert_eq!(strip_namespace("linux", "redis.get"), None);
        assert_eq!(strip_namespace("linux", "linux"), None); // no trailing dot
    }

    #[test]
    fn test_namespace_tools() {
        let tools = vec![Tool {
            name: "system.cpu".into(),
            title: None,
            description: Some("Get CPU usage".into()),
            input_schema: serde_json::json!({"type": "object"}).as_object().unwrap().clone().into(),
            output_schema: None,
            annotations: None,
            icons: None,
            meta: None,
            execution: None,
        }];
        let namespaced = namespace_tools("linux", &tools);
        assert_eq!(namespaced[0].name.as_ref(), "linux.system.cpu");
    }

    #[test]
    fn test_resolve_namespace() {
        let namespaces = vec!["linux".to_string(), "redis".to_string()];
        let (ns, original) = resolve_namespace(&namespaces, "linux.system.cpu").unwrap();
        assert_eq!(ns, "linux");
        assert_eq!(original, "system.cpu");

        let (ns, original) = resolve_namespace(&namespaces, "redis.get").unwrap();
        assert_eq!(ns, "redis");
        assert_eq!(original, "get");

        assert!(resolve_namespace(&namespaces, "unknown.tool").is_none());
    }
}
