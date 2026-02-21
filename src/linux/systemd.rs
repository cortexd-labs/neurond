use crate::core::provider::{ProviderError, Result};
use serde_json::Value;
use std::process::Command;

// zbus D-Bus interaction isn't strictly synchronously trivial.
// We'll wrap a `block_on` or simply use a subprocess for the MVP, 
// since the spec mandates sync `call` functions and rust `zbus` is highly async.
// As this is a 10-day MVP, invoking `systemctl` is an acceptable stand-in 
// until zbus async boundaries are fully established across the `Provider` trait.

// Helper function to execute a command and return stdout as a String.
// This deduplicates process handling, error mapping, and utf8 conversion.
fn execute_command_stdout(cmd_name: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd_name)
        .args(args)
        .output()
        .map_err(|e| ProviderError::Execution(format!("Failed to execute {}: {}", cmd_name, e)))?;

    if !output.status.success() {
        return Err(ProviderError::Execution(format!(
            "Command {} failed with status: {}",
            cmd_name, output.status
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn list_units() -> Result<Value> {
    let stdout = execute_command_stdout(
        "systemctl",
        &["list-units", "--type=service", "--all", "--no-pager", "--no-legend"]
    )?;

    let mut units = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let name = parts[0];
            let load = parts[1];
            let active = parts[2];
            let sub = parts[3];
            let description = parts[4..].join(" ");
            
            units.push(serde_json::json!({
                "name": name,
                "load_state": load,
                "state": active,
                "sub_state": sub,
                "description": description
            }));
        }
    }

    Ok(serde_json::json!(units))
}

pub fn get_unit_status(name: &str) -> Result<Value> {
    let stdout = execute_command_stdout("systemctl", &["show", name, "--no-pager"])?;
    let mut props = serde_json::Map::new();

    for line in stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            props.insert(k.to_string(), serde_json::json!(v.trim()));
        }
    }

    Ok(serde_json::Value::Object(props))
}

pub fn journal_tail(unit: Option<&str>, lines: usize) -> Result<Value> {
    let lines_str = lines.to_string();
    let mut args = vec!["-o", "json", "-n", &lines_str, "--no-pager"];
    
    if let Some(u) = unit {
        args.push("-u");
        args.push(u);
    }

    let stdout = execute_command_stdout("journalctl", &args)?;
    let mut entries = Vec::new();

    for line in stdout.lines() {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            entries.push(json);
        }
    }

    Ok(serde_json::json!({
        "unit": unit,
        "entries": entries
    }))
}

pub fn journal_search(keyword: &str, since: Option<&str>, priority: Option<&str>) -> Result<Value> {
    let mut args = vec!["-o", "json", "--no-pager", "--grep", keyword];
    
    let since_arg;
    if let Some(s) = since {
        since_arg = format!("--since={}", s);
        args.push(&since_arg);
    }
    
    if let Some(p) = priority {
        // systemctl technically parses `-p n` or `-p=n`, split mapping may be needed depending on flags,
        // but passing as a single argument might fail if standard split isn't used internally by Command.
        // We push `-p` and `p` safely:
        args.push("-p");
        args.push(p);
    }

    let stdout = execute_command_stdout("journalctl", &args)?;
    let mut entries = Vec::new();

    for line in stdout.lines() {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            entries.push(json);
        }
    }

    Ok(serde_json::json!({
        "keyword": keyword,
        "entries": entries
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_units_executes() {
        // Just verify the command doesn't crash in CI/local
        let res = list_units();
        assert!(res.is_ok(), "systemctl list-units should execute");
        
        let array = res.unwrap();
        assert!(array.is_array());
    }
}
