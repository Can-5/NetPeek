// NetPeek — network diagnostics helpers: parsing, validation, safe exports.
// Everything here is pure and unit-tested.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingStats {
    pub transmitted: u32,
    pub received: u32,
    pub loss_percent: f64,
    pub min_ms: Option<f64>,
    pub avg_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub per_reply: Vec<f64>,
}

/// Validate a host argument for `ping`: no shell chars, sane charset/length.
/// Returns the trimmed host unchanged if valid.
pub fn validate_host(input: &str) -> Result<String, String> {
    let host = input.trim();
    if host.is_empty() || host.len() > 253 {
        return Err("host is empty or too long".into());
    }
    let ok = host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'));
    if !ok {
        return Err("host may only contain letters, digits, dots, dashes, colons and underscores".into());
    }
    if host.starts_with('-') {
        return Err("host may not start with a dash".into());
    }
    Ok(host.to_string())
}

/// Parse Linux/Windows/macOS ping output into stats. Tolerant of format drift.
pub fn parse_ping_output(output: &str) -> PingStats {
    let mut transmitted = 0u32;
    let mut received = 0u32;
    // Linux/macOS: "4 packets transmitted, 4 received, 0% packet loss"
    // Windows:     "Packets: Sent = 4, Received = 4, Lost = 0 (0% loss)"
    for line in output.lines() {
        let l = line.to_lowercase();
        if l.contains("packets transmitted") || l.contains("packets: sent") {
            let nums: Vec<u32> = extract_numbers(line);
            if line.to_lowercase().contains("packets: sent") {
                // Windows ordering: Sent, Received, Lost
                if nums.len() >= 2 {
                    transmitted = nums[0];
                    received = nums[1];
                }
            } else if nums.len() >= 2 {
                transmitted = nums[0];
                received = nums[1];
            }
        }
    }
    if transmitted == 0 && received == 0 {
        // last-resort: count reply lines
        let replies = output
            .lines()
            .filter(|l| {
                let l = l.to_lowercase();
                l.contains("bytes from") || l.contains("reply from")
            })
            .count() as u32;
        received = replies;
        transmitted = replies;
    }
    let loss_percent = if transmitted > 0 {
        ((transmitted - received) as f64 / transmitted as f64) * 100.0
    } else {
        100.0
    };

    let mut per_reply = Vec::new();
    for line in output.lines() {
        let l = line.to_lowercase();
        if l.contains("bytes from") || l.contains("reply from") {
            if let Some(ms) = extract_rtt_ms(line) {
                per_reply.push(ms);
            }
        }
    }

    let (min_ms, avg_ms, max_ms) = if !per_reply.is_empty() {
        let min = per_reply.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = per_reply.iter().cloned().fold(0.0f64, f64::max);
        let avg = per_reply.iter().sum::<f64>() / per_reply.len() as f64;
        (Some(min), Some(avg), Some(max))
    } else {
        // try "rtt min/avg/max" or "Minimum = " lines
        parse_rtt_summary(output)
    };

    PingStats { transmitted, received, loss_percent, min_ms, avg_ms, max_ms, per_reply }
}

fn extract_numbers(line: &str) -> Vec<u32> {
    line.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect()
}

/// Extract the time=.. ms portion of a reply line (handles time<1ms).
fn extract_rtt_ms(line: &str) -> Option<f64> {
    let lower = line.to_lowercase();
    let marker = ["time=", "time<"].iter().find_map(|m| lower.find(m).map(|i| (i, m.len())))?;
    let (idx, mlen) = marker;
    let rest = &line[idx + mlen..];
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let v: f64 = num.parse().ok()?;
    Some(if lower[idx..].starts_with("time<") { 0.5 } else { v })
}

/// Parse "rtt min/avg/max/mdev = 1.2/3.4/5.6/0.7 ms" (Linux/macOS) or
/// "Minimum = 1ms, Maximum = 2ms, Average = 1ms" (Windows).
fn parse_rtt_summary(output: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    if let Some(line) = output.lines().find(|l| l.contains("min/avg/max") || l.contains("minimum")) {
        if let Some(eq) = line.split('=').last() {
            let parts: Vec<f64> = eq
                .split(['/', ' ', 'm'])
                .filter_map(|p| p.trim().parse::<f64>().ok())
                .collect();
            if parts.len() >= 3 {
                return (Some(parts[0]), Some(parts[1]), Some(parts[2]));
            }
        }
    }
    let nums: Vec<f64> = extract_floats(output);
    if output.to_lowercase().contains("average =") && nums.len() >= 3 {
        return (Some(nums[0]), Some(nums[2]), Some(nums[1]));
    }
    (None, None, None)
}

fn extract_floats(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                if let Ok(v) = cur.parse::<f64>() {
                    out.push(v);
                }
                cur.clear();
            }
        }
    }
    if !cur.is_empty() {
        if let Ok(v) = cur.parse::<f64>() {
            out.push(v);
        }
    }
    out
}

/// Parse /etc/resolv.conf (or similar) nameserver lines.
pub fn parse_resolv_conf(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with(';'))
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            if parts.next()? == "nameserver" {
                parts.next().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Validates a user-chosen export filename inside the Downloads folder.
pub fn safe_export_path(download_dir: &std::path::Path, filename: &str) -> Result<std::path::PathBuf, String> {
    let name = filename.trim();
    if name.is_empty() {
        return Err("filename is empty".into());
    }
    if name.contains(['/', '\\', '\0']) {
        return Err("filename must not contain path separators".into());
    }
    if name.starts_with('.') || name.len() > 120 {
        return Err("invalid filename".into());
    }
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !matches!(ext.as_str(), "csv" | "json" | "md" | "txt") {
        return Err("allowed export extensions: csv, json, md, txt".into());
    }
    Ok(download_dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_validation() {
        assert_eq!(validate_host("8.8.8.8").unwrap(), "8.8.8.8");
        assert_eq!(validate_host("example.com").unwrap(), "example.com");
        assert!(validate_host("-n8.8.8.8").is_err());
        assert!(validate_host("a;id").is_err());
        assert!(validate_host("a b").is_err());
        assert!(validate_host("").is_err());
        assert!(validate_host("$(reboot)").is_err());
    }

    #[test]
    fn parses_linux_ping() {
        let out = "PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.\n64 bytes from 8.8.8.8: icmp_seq=1 ttl=117 time=23.4 ms\n64 bytes from 8.8.8.8: icmp_seq=2 ttl=117 time=21.1 ms\n64 bytes from 8.8.8.8: icmp_seq=3 ttl=117 time=99.2 ms\n\n--- 8.8.8.8 ping statistics ---\n3 packets transmitted, 3 received, 0% packet loss, time 2002ms\nrtt min/avg/max/mdev = 21.1/47.9/99.2/36.4 ms\n";
        let stats = parse_ping_output(out);
        assert_eq!(stats.transmitted, 3);
        assert_eq!(stats.received, 3);
        assert_eq!(stats.loss_percent, 0.0);
        assert_eq!(stats.min_ms, Some(21.1));
        assert!((stats.avg_ms.unwrap() - 47.9).abs() < 0.01);
        assert_eq!(stats.max_ms, Some(99.2));
        assert_eq!(stats.per_reply.len(), 3);
    }

    #[test]
    fn parses_windows_ping() {
        let out = "Pinging 8.8.8.8 with 32 bytes of data:\nReply from 8.8.8.8: bytes=32 time=12ms TTL=117\nReply from 8.8.8.8: bytes=32 time=15ms TTL=117\n\nPing statistics for 8.8.8.8:\n    Packets: Sent = 4, Received = 4, Lost = 0 (0% loss),\nApproximate round trip times in milli-seconds:\n    Minimum = 12ms, Maximum = 18ms, Average = 14ms\n";
        let stats = parse_ping_output(out);
        assert_eq!(stats.transmitted, 4);
        assert_eq!(stats.received, 4);
        assert_eq!(stats.loss_percent, 0.0);
        assert_eq!(stats.per_reply.len(), 2);
    }

    #[test]
    fn parses_resolv_conf() {
        let conf = "# comment\nnameserver 1.1.1.1\nnameserver 8.8.8.8\nsearch example.com\n";
        assert_eq!(parse_resolv_conf(conf), vec!["1.1.1.1", "8.8.8.8"]);
    }

    #[test]
    fn export_path_rules() {
        let dir = std::path::Path::new("/tmp/downloads");
        assert!(safe_export_path(dir, "diag.md").is_ok());
        assert!(safe_export_path(dir, "a/b.md").is_err());
        assert!(safe_export_path(dir, "../x.md").is_err());
        assert!(safe_export_path(dir, "x.exe").is_err());
    }
}
