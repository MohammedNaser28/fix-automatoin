use std::process::Command;

/// Returns the first non-loopback IPv4 address found on the system.
/// Runs `ip -4 addr show scope global` and parses the output.
pub fn get_ip() -> Option<String> {
    let output = Command::new("ip")
        .args(["-4", "addr", "show", "scope", "global"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("inet ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(addr) = parts.get(1) {
                // Strip CIDR: "192.168.1.5/24" -> "192.168.1.5"
                let ip = addr.split('/').next().unwrap_or(addr);
                return Some(ip.to_string());
            }
        }
    }
    None
}
