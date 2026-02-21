use crate::core::provider::{ProviderError, Result};
use serde_json::Value;
use std::fs;

// Helper function to robustly read and map /proc files
fn read_proc_file(path: &str) -> Result<String> {
    fs::read_to_string(path)
        .map_err(|e| ProviderError::Execution(format!("Failed to read {}: {}", path, e)))
}

pub fn get_system_info() -> Result<Value> {
    // Read hostname
    let hostname = read_proc_file("/proc/sys/kernel/hostname")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();

    // Read kernel version
    let version_str = read_proc_file("/proc/version")
        .unwrap_or_else(|_| "unknown".to_string());
    
    // Parse out architecture (roughly)
    let arch = if version_str.contains("x86_64") {
        "x86_64"
    } else if version_str.contains("aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    Ok(serde_json::json!({
        "hostname": hostname,
        "os": "linux",
        "kernel": version_str.split(' ').nth(2).unwrap_or("unknown"),
        "arch": arch,
    }))
}

pub fn get_system_cpu() -> Result<Value> {
    // Basic CPU parsing from /proc/stat
    let stat = read_proc_file("/proc/stat")?;
    
    // Simplistic line counting for cores
    let cores = stat.lines().filter(|l| l.starts_with("cpu") && l.len() > 3).count();
    
    Ok(serde_json::json!({
        "cores": cores,
    }))
}

pub fn get_system_memory() -> Result<Value> {
    let meminfo = read_proc_file("/proc/meminfo")?;

    let mut total = 0;
    let mut available = 0;
    let mut swap_total = 0;
    let mut swap_free = 0;

    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            total = parse_kb(line);
        } else if line.starts_with("MemAvailable:") {
            available = parse_kb(line);
        } else if line.starts_with("SwapTotal:") {
            swap_total = parse_kb(line);
        } else if line.starts_with("SwapFree:") {
            swap_free = parse_kb(line);
        }
    }

    Ok(serde_json::json!({
        "total_mb": total / 1024,
        "used_mb": (total.saturating_sub(available)) / 1024,
        "available_mb": available / 1024,
        "swap_total_mb": swap_total / 1024,
        "swap_used_mb": (swap_total.saturating_sub(swap_free)) / 1024,
    }))
}

pub fn get_system_disk() -> Result<Value> {
    // Stubbed. Complete implementation requires iterating /proc/mounts and statvfs
    Ok(serde_json::json!([]))
}

pub fn get_system_uptime() -> Result<Value> {
    let uptime_str = read_proc_file("/proc/uptime")?;
    
    let parts: Vec<&str> = uptime_str.split_whitespace().collect();
    let uptime_sec = parts.first().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    let idle_sec = parts.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);

    let loadavg_str = fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let loads: Vec<&str> = loadavg_str.split_whitespace().collect();

    Ok(serde_json::json!({
        "uptime_seconds": uptime_sec,
        "idle_seconds": idle_sec,
        "load_1m": loads.first().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
        "load_5m": loads.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
        "load_15m": loads.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
    }))
}

// -----------------------------------------------------
// Process Tools
// -----------------------------------------------------

pub fn get_process_list_vec() -> Result<Vec<serde_json::Map<String, Value>>> {
    let mut procs = Vec::new();
    
    let entries = fs::read_dir("/proc")
        .map_err(|e| ProviderError::Execution(format!("Failed to read /proc: {}", e)))?;

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        if let Ok(pid) = name_str.parse::<u32>() {
            let mut proc_obj = serde_json::Map::new();
            proc_obj.insert("pid".into(), serde_json::json!(pid));

            // Basic parsing of /proc/[pid]/status
            let status_path = format!("/proc/{}/status", pid);
            if let Ok(status) = fs::read_to_string(&status_path) {
                for line in status.lines() {
                    if line.starts_with("Name:\t") {
                        let name = line.replace("Name:\t", "").trim().to_string();
                        proc_obj.insert("name".into(), serde_json::json!(name));
                    } else if line.starts_with("State:\t") {
                        let state = line.replace("State:\t", "").trim().to_string();
                        proc_obj.insert("state".into(), serde_json::json!(state));
                    } else if line.starts_with("VmRSS:\t") {
                        let kb = parse_kb(line);
                        proc_obj.insert("mem_mb".into(), serde_json::json!((kb as f64) / 1024.0));
                    }
                }
            }

            // Command line parsing (null delimited)
            let cmdline_path = format!("/proc/{}/cmdline", pid);
            if let Ok(cmd_bytes) = fs::read(&cmdline_path) {
                let cmd: String = cmd_bytes.split(|&b| b == 0)
                    .filter_map(|b| std::str::from_utf8(b).ok())
                    .collect::<Vec<_>>()
                    .join(" ");
                if !cmd.is_empty() {
                    proc_obj.insert("command".into(), serde_json::json!(cmd));
                }
            }
            
            // Just defaults for CPU until full parsing implemented
            proc_obj.insert("cpu_percent".into(), serde_json::json!(0.0));
            proc_obj.insert("user".into(), serde_json::json!("unknown"));

            procs.push(proc_obj);
        }
    }
    
    Ok(procs)
}

pub fn get_process_list() -> Result<Value> {
    let procs = get_process_list_vec()?;
    Ok(serde_json::json!(procs))
}

// Helper: parse "MemTotal: 1234 kB" into 1234
fn parse_kb(line: &str) -> u64 {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().unwrap_or(0)
    } else {
        0
    }
}

// ========================================================================= //
// TDD Tests                                                                 //
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kb() {
        assert_eq!(parse_kb("MemTotal:       16275820 kB"), 16275820);
        assert_eq!(parse_kb("SwapFree:              0 kB"), 0);
    }

    #[test]
    fn test_memory_returns_json() {
        let res = get_system_memory().unwrap();
        assert!(res.get("total_mb").is_some());
        assert!(res.get("used_mb").is_some());
    }

    #[test]
    fn test_uptime_returns_json() {
        let res = get_system_uptime().unwrap();
        assert!(res.get("uptime_seconds").is_some());
        assert!(res.get("load_1m").is_some());
    }

    #[test]
    fn test_process_list() {
        let res = get_process_list().unwrap();
        assert!(res.is_array());
        let array = res.as_array().unwrap();
        assert!(!array.is_empty()); // At least current process should exist
        assert!(array[0].get("pid").is_some());
    }
}
