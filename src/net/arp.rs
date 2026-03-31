use std::net::IpAddr;

#[cfg(not(debug_assertions))]
use tokio::process::Command;

use tracing::debug;

/// Look up a client's MAC address from their IP via the system ARP table.
///
/// On Linux (production target): reads `/proc/net/arp` directly (zero-cost, no subprocess).
/// Fallback: runs `ip neigh show <ip>` or `arp -an` for macOS/other platforms.
///
/// In dev mode: returns a deterministic MAC based on the IP (for local testing without router).
///
/// Returns `None` if the IP is not found in the ARP table (e.g., the client
/// connected over loopback, or the ARP entry has not yet been created).
pub async fn lookup_mac(ip: IpAddr) -> Option<String> {
    #[cfg(debug_assertions)]
    {
        debug!(%ip, "dev mode: returning deterministic MAC");
        Some(dev_fake_mac(ip))
    }

    #[cfg(not(debug_assertions))]
    {
        let ip_str = ip.to_string();

        // Try /proc/net/arp first (Linux only, no subprocess)
        if let Some(mac) = lookup_proc_net_arp(&ip_str) {
            debug!(%ip, %mac, "MAC resolved via /proc/net/arp");
            return Some(mac);
        }

        // Fallback: `ip neigh show <ip>` (works on Linux if /proc is unavailable)
        if let Some(mac) = lookup_ip_neigh(&ip_str).await {
            debug!(%ip, %mac, "MAC resolved via ip neigh");
            return Some(mac);
        }

        // Fallback: `arp -an` (macOS, BSD)
        if let Some(mac) = lookup_arp_command(&ip_str).await {
            debug!(%ip, %mac, "MAC resolved via arp -an");
            return Some(mac);
        }

        debug!(%ip, "MAC lookup failed: IP not found in ARP table");
        return None;
    }
}

/// Parse `/proc/net/arp` to find the MAC for a given IP.
///
/// Format (Linux):
/// ```text
/// IP address       HW type     Flags       HW address            Mask     Device
/// 10.10.0.42       0x1         0x2         aa:bb:cc:dd:ee:ff     *        wlan0
/// ```
#[cfg(not(debug_assertions))]
fn lookup_proc_net_arp(ip: &str) -> Option<String> {
    let content = std::fs::read_to_string("/proc/net/arp").ok()?;

    for line in content.lines().skip(1) {
        // Skip the header line
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 4 && fields[0] == ip {
            let mac = fields[3].to_lowercase();
            // Filter out incomplete entries (00:00:00:00:00:00)
            if mac != "00:00:00:00:00:00" {
                return Some(mac);
            }
        }
    }

    None
}

/// Run `ip neigh show <ip>` and parse the MAC address.
///
/// Output format:
/// ```text
/// 10.10.0.42 dev wlan0 lladdr aa:bb:cc:dd:ee:ff REACHABLE
/// ```
#[cfg(not(debug_assertions))]
async fn lookup_ip_neigh(ip: &str) -> Option<String> {
    let output = Command::new("ip")
        .args(["neigh", "show", ip])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        // Find the "lladdr" field followed by the MAC
        let fields: Vec<&str> = line.split_whitespace().collect();
        if let Some(idx) = fields.iter().position(|&f| f == "lladdr")
            && let Some(mac) = fields.get(idx + 1)
        {
            return Some(mac.to_lowercase());
        }
    }

    None
}

/// Run `arp -an` and parse the MAC for a given IP (macOS/BSD fallback).
///
/// Output format (macOS):
/// ```text
/// ? (10.10.0.42) at aa:bb:cc:dd:ee:ff on en0 ifscope [ethernet]
/// ```
#[cfg(not(debug_assertions))]
async fn lookup_arp_command(ip: &str) -> Option<String> {
    let output = Command::new("arp")
        .args(["-an"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let needle = format!("({ip})");
    for line in stdout.lines() {
        if line.contains(&needle) {
            // Format: "? (IP) at MAC on IFACE ..."
            let fields: Vec<&str> = line.split_whitespace().collect();
            if let Some(idx) = fields.iter().position(|&f| f == "at")
                && let Some(mac) = fields.get(idx + 1)
            {
                let mac = mac.to_lowercase();
                // macOS sometimes returns "(incomplete)" instead of a MAC
                if mac.contains(':') && mac != "00:00:00:00:00:00" {
                    return Some(mac);
                }
            }
        }
    }

    None
}

#[cfg(debug_assertions)]
fn dev_fake_mac(ip: IpAddr) -> String {
    let hash = match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            (u32::from(octets[0]) << 24)
                | (u32::from(octets[1]) << 16)
                | (u32::from(octets[2]) << 8)
                | u32::from(octets[3])
        }
        IpAddr::V6(ipv6) => {
            let octets = ipv6.octets();
            let mut hash = 0u32;
            for (i, &octet) in octets.iter().take(4).enumerate() {
                hash |= u32::from(octet) << (i * 8);
            }
            hash
        }
    };

    let hash = hash % 0xFFFFFF;
    format!(
        "02:00:00:{:02x}:{:02x}:{:02x}",
        (hash >> 16) & 0xFF,
        (hash >> 8) & 0xFF,
        hash & 0xFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_proc_net_arp() {
        // Write a fake /proc/net/arp content and test parsing
        // Since we can't mock the filesystem easily, test the parsing logic directly
        let content = "\
IP address       HW type     Flags       HW address            Mask     Device
10.10.0.42       0x1         0x2         aa:bb:cc:dd:ee:ff     *        wlan0
10.10.0.1        0x1         0x2         11:22:33:44:55:66     *        wlan0
10.10.0.99       0x1         0x0         00:00:00:00:00:00     *        wlan0
";
        // Parse lines manually (mirrors lookup_proc_net_arp logic)
        let find = |ip: &str| -> Option<String> {
            for line in content.lines().skip(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() >= 4 && fields[0] == ip {
                    let mac = fields[3].to_lowercase();
                    if mac != "00:00:00:00:00:00" {
                        return Some(mac);
                    }
                }
            }
            None
        };

        assert_eq!(find("10.10.0.42"), Some("aa:bb:cc:dd:ee:ff".to_string()));
        assert_eq!(find("10.10.0.1"), Some("11:22:33:44:55:66".to_string()));
        assert_eq!(find("10.10.0.99"), None); // incomplete entry
        assert_eq!(find("10.10.0.200"), None); // not in table
    }

    #[test]
    fn test_parse_ip_neigh_output() {
        let output = "10.10.0.42 dev wlan0 lladdr aa:bb:cc:dd:ee:ff REACHABLE\n";
        let fields: Vec<&str> = output.split_whitespace().collect();
        let idx = fields.iter().position(|&f| f == "lladdr").unwrap();
        let mac = fields[idx + 1].to_lowercase();
        assert_eq!(mac, "aa:bb:cc:dd:ee:ff");
    }

    #[test]
    fn test_parse_arp_command_output() {
        let output = "? (10.10.0.42) at aa:bb:cc:dd:ee:ff on en0 ifscope [ethernet]\n\
                      ? (10.10.0.1) at 11:22:33:44:55:66 on en0 ifscope [ethernet]\n";

        let needle = "(10.10.0.42)";
        let line = output.lines().find(|l| l.contains(needle)).unwrap();
        let fields: Vec<&str> = line.split_whitespace().collect();
        let idx = fields.iter().position(|&f| f == "at").unwrap();
        let mac = fields[idx + 1].to_lowercase();
        assert_eq!(mac, "aa:bb:cc:dd:ee:ff");
    }
}
