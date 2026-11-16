// NetPeek — honest network diagnostics. Local facts need no network.
// Anything that contacts the outside world happens ONLY when the user clicks
// the corresponding button, and the UI/README name the endpoint used.

mod netutil;

use netutil::{parse_ping_output, parse_resolv_conf, safe_export_path, validate_host, PingStats};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};
use tauri::Manager;

#[derive(Serialize)]
struct LocalInfo {
    hostname: String,
    ipv4: Option<String>,
    ipv6: Option<String>,
    interfaces: Vec<InterfaceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterfaceInfo {
    name: String,
    addr: String,
    kind: String, // "ipv4" | "ipv6"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LatencyEntry {
    ts: u64,
    host: String,
    avg_ms: Option<f64>,
    loss_percent: f64,
}

fn interfaces() -> Vec<InterfaceInfo> {
    match if_addrs::get_if_addrs() {
        Ok(list) => list
            .into_iter()
            .filter_map(|i| {
                if i.is_loopback() {
                    return None;
                }
                let (addr, kind) = match i.ip() {
                    std::net::IpAddr::V4(a) => (a.to_string(), "ipv4"),
                    std::net::IpAddr::V6(a) => (a.to_string(), "ipv6"),
                };
                Some(InterfaceInfo { name: i.name, addr, kind: kind.into() })
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn primary_ip(v6: bool) -> Option<String> {
    use std::net::UdpSocket;
    let target = if v6 { "[2606:4700:4700::1111]:80" } else { "8.8.8.8:80" };
    let socket = if v6 {
        UdpSocket::bind("[::]:0").ok()?
    } else {
        UdpSocket::bind("0.0.0.0:0").ok()?
    };
    socket.connect(target).ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}

#[tauri::command]
fn local_info() -> LocalInfo {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| {
            std::process::Command::new("hostname")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "unknown".into())
        });
    LocalInfo {
        hostname,
        ipv4: primary_ip(false),
        ipv6: primary_ip(true),
        interfaces: interfaces(),
    }
}

#[tauri::command]
fn ping_host(host: String, count: u32) -> Result<PingStats, String> {
    let host = validate_host(&host)?;
    let count = count.clamp(1, 20);

    #[cfg(target_os = "linux")]
    let args: Vec<String> = vec!["-c".into(), count.to_string(), "-W".into(), "2".into(), host.clone()];
    #[cfg(target_os = "macos")]
    let args: Vec<String> = vec!["-c".into(), count.to_string(), "-W".into(), "2000".into(), host.clone()];
    #[cfg(target_os = "windows")]
    let args: Vec<String> = vec!["-n".into(), count.to_string(), "-w".into(), "2000".into(), host.clone()];

    let output = std::process::Command::new("ping")
        .args(&args)
        .output()
        .map_err(|e| format!("ping not available on this system: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if text.trim().is_empty() { stderr } else { text };
    if combined.trim().is_empty() {
        return Err("ping produced no output".into());
    }
    let stats = parse_ping_output(&combined);
    append_latency_history(&host, stats.avg_ms, stats.loss_percent);
    Ok(stats)
}

fn append_latency_history(host: &str, avg_ms: Option<f64>, loss_percent: f64) {
    // Resolve the data dir without a handle by using the standard path.
    let base = dirs_fallback();
    if let Some(dir) = base {
        let path = dir.join("latency-history.jsonl");
        let entry = LatencyEntry {
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            host: host.to_string(),
            avg_ms,
            loss_percent,
        };
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            use std::io::Write;
            let _ = writeln!(f, "{}", serde_json::to_string(&entry).unwrap_or_default());
        }
    }
}

fn dirs_fallback() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "linux") {
        std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share/com.can5.netpeek")))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support/com.can5.netpeek"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("com.can5.netpeek"))
    } else {
        None
    }
}

#[tauri::command]
fn latency_history(limit: usize) -> Vec<LatencyEntry> {
    let Some(dir) = dirs_fallback() else { return Vec::new() };
    let path = dir.join("latency-history.jsonl");
    let Ok(content) = std::fs::read_to_string(&path) else { return Vec::new() };
    let mut entries: Vec<LatencyEntry> = content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let limit = limit.clamp(1, 100);
    if entries.len() > limit {
        entries = entries.split_off(entries.len() - limit);
    }
    entries
}

#[tauri::command]
fn clear_latency_history() -> Result<(), String> {
    if let Some(dir) = dirs_fallback() {
        let path = dir.join("latency-history.jsonl");
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn dns_lookup(host: String) -> Result<Vec<String>, String> {
    let host = validate_host(&host)?;
    let mut addrs: Vec<String> = (host.as_str(), 0u16)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?
        .map(|a| a.ip().to_string())
        .collect();
    addrs.sort();
    addrs.dedup();
    addrs.truncate(16);
    if addrs.is_empty() {
        return Err(format!("no addresses returned for {host}"));
    }
    Ok(addrs)
}

#[tauri::command]
fn dns_servers() -> Result<Vec<String>, String> {
    #[cfg(target_os = "windows")]
    {
        windows_dns_servers()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let content = std::fs::read_to_string("/etc/resolv.conf")
            .map_err(|e| format!("could not read /etc/resolv.conf: {e}"))?;
        let servers = parse_resolv_conf(&content);
        if servers.is_empty() {
            return Err("no nameservers found in /etc/resolv.conf".into());
        }
        Ok(servers)
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[cfg(target_os = "windows")]
fn windows_dns_servers() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("ipconfig")
        .arg("/all")
        .output()
        .map_err(|e| format!("ipconfig unavailable: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut servers = Vec::new();
    let mut in_dns_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("dns servers") {
            in_dns_block = true;
            if let Some(ip) = trimmed.split(':').nth(1).map(str::trim) {
                if !ip.is_empty() {
                    servers.push(ip.to_string());
                }
            }
            continue;
        }
        if in_dns_block {
            // follow-on IPs are listed on their own lines without a label
            if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':') && !trimmed.is_empty() {
                servers.push(trimmed.to_string());
            } else if trimmed.contains(':') {
                in_dns_block = false;
            } else {
                in_dns_block = false;
            }
        }
    }
    servers.sort();
    servers.dedup();
    if servers.is_empty() {
        return Err("no DNS servers found in ipconfig /all output".into());
    }
    Ok(servers)
}



/// User-initiated connectivity probe against 1.1.1.1:443 (Cloudflare), 2 s timeout.
#[tauri::command]
fn connection_check() -> Result<serde_json::Value, String> {
    use std::net::TcpStream;
    let start = Instant::now();
    match TcpStream::connect_timeout(&"1.1.1.1:443".parse().map_err(|_| "bad addr")?, Duration::from_secs(2)) {
        Ok(_) => Ok(serde_json::json!({
            "online": true,
            "latency_ms": start.elapsed().as_millis() as u64,
            "target": "1.1.1.1:443",
        })),
        Err(e) => Ok(serde_json::json!({
            "online": false,
            "latency_ms": null,
            "target": "1.1.1.1:443",
            "error": e.to_string(),
        })),
    }
}

/// User-initiated public IP lookup via ifconfig.me (explicitly labelled in the UI).
#[tauri::command]
fn public_ip() -> Result<String, String> {
    let resp = ureq::get("https://ifconfig.me/ip")
        .timeout(Duration::from_secs(8))
        .call()
        .map_err(|e| format!("public IP lookup failed: {e}"))?;
    let body = resp
        .into_string()
        .map_err(|e| format!("could not read response: {e}"))?;
    let ip = body.trim().to_string();
    let looks_ip = ip.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':' || c.is_ascii_hexdigit()) && ip.len() <= 45;
    if !looks_ip {
        return Err("unexpected response from ifconfig.me".into());
    }
    Ok(ip)
}

/// User-initiated download speed test against Cloudflare's open endpoint (~8 MB).
#[tauri::command]
fn speed_test() -> Result<serde_json::Value, String> {
    let url = "https://speed.cloudflare.com/__down?bytes=8000000";
    let start = Instant::now();
    let resp = ureq::get(url)
        .timeout(Duration::from_secs(60))
        .call()
        .map_err(|e| format!("speed test download failed: {e}"))?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0usize;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(e) => return Err(format!("speed test read failed: {e}")),
        }
    }
    let secs = start.elapsed().as_secs_f64().max(0.001);
    let mbps = (total as f64 * 8.0) / secs / 1e6;
    Ok(serde_json::json!({
        "mbps": (mbps * 10.0).round() / 10.0,
        "bytes": total,
        "seconds": (secs * 100.0).round() / 100.0,
        "endpoint": "speed.cloudflare.com/__down?bytes=8000000",
    }))
}

#[tauri::command]
fn write_export(app: tauri::AppHandle, filename: String, content: String) -> Result<String, String> {
    let download_dir = app
        .path()
        .download_dir()
        .map_err(|e| format!("could not locate your Downloads folder: {e}"))?;
    let path = safe_export_path(&download_dir, &filename)?;
    std::fs::write(&path, content).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            local_info, ping_host, latency_history, clear_latency_history,
            dns_lookup, dns_servers, connection_check, public_ip, speed_test, write_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NetPeek");
}
