use anyhow::{Context, Result};
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::config::FirewallConfig;
use super::Firewall;

/// Validate MAC address format before interpolation into nft commands.
///
/// Defense-in-depth: even though the web extractor validates MACs on the request
/// path, the firewall is also called from the session restore path at startup
/// (reading MACs directly from the database). A corrupted DB entry must not
/// enable nft command injection.
///
/// Accepts only lowercase hex-colon format: `xx:xx:xx:xx:xx:xx` (exactly 17 chars).
/// Rejects null (00:00:00:00:00:00) and broadcast (ff:ff:ff:ff:ff:ff) MACs.
fn validate_mac(mac: &str) -> Result<()> {
    let bytes = mac.as_bytes();
    if bytes.len() != 17 {
        anyhow::bail!("invalid MAC length: {}", mac.len());
    }
    for (i, &b) in bytes.iter().enumerate() {
        if i % 3 == 2 {
            if b != b':' {
                anyhow::bail!("invalid MAC separator at position {i}");
            }
        } else if !b.is_ascii_hexdigit() || (b.is_ascii_alphabetic() && !b.is_ascii_lowercase()) {
            anyhow::bail!("invalid MAC character at position {i}");
        }
    }
    if mac == "00:00:00:00:00:00" {
        anyhow::bail!("null MAC address rejected");
    }
    if mac == "ff:ff:ff:ff:ff:ff" {
        anyhow::bail!("broadcast MAC address rejected");
    }
    Ok(())
}

/// Controls nftables rules via the `nft` CLI.
///
/// Manages the `auth_macs` set: adding MACs with timeouts on auth,
/// and removing them on disconnect/expiry.
pub struct NftablesController {
    nft_path: String,
    table_name: String,
    set_name: String,
}

impl NftablesController {
    pub fn new(config: &FirewallConfig) -> Self {
        Self {
            nft_path: config.nft_path.clone(),
            table_name: config.table_name.clone(),
            set_name: config.set_name.clone(),
        }
    }

    /// Execute an nft command and return the result.
    async fn run_nft(&self, nft_cmd: &str) -> Result<()> {
        let output = Command::new(&self.nft_path)
            .arg(nft_cmd)
            .output()
            .await
            .with_context(|| format!("failed to execute: {} {}", self.nft_path, nft_cmd))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("nft command failed: {stderr}");
        }

        Ok(())
    }
}

impl Firewall for NftablesController {
    fn authorize_mac(
        &self,
        mac: &str,
        timeout_secs: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let mac = mac.to_owned();
        Box::pin(async move {
            validate_mac(&mac)
                .with_context(|| format!("refusing to authorize invalid MAC: {mac}"))?;

            let element = format!("{} timeout {}s", mac, timeout_secs);
            let cmd = format!(
                "add element inet {} {} {{ {} }}",
                self.table_name, self.set_name, element
            );

            debug!(%mac, timeout_secs, "authorizing MAC in nftables");
            self.run_nft(&cmd)
                .await
                .with_context(|| format!("failed to authorize MAC {mac}"))
        })
    }

    fn deauthorize_mac(
        &self,
        mac: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let mac = mac.to_owned();
        Box::pin(async move {
            validate_mac(&mac)
                .with_context(|| format!("refusing to deauthorize invalid MAC: {mac}"))?;

            let cmd = format!(
                "delete element inet {} {} {{ {} }}",
                self.table_name, self.set_name, mac
            );

            debug!(%mac, "deauthorizing MAC from nftables");
            // Ignore errors if the element doesn't exist (already expired)
            if let Err(e) = self.run_nft(&cmd).await {
                warn!(%mac, "deauthorize failed (may already be expired): {e}");
            }
            Ok(())
        })
    }

    fn init_ruleset(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Create table if not exists — only ignore "already exists" errors
            if let Err(e) = self.run_nft(&format!("add table inet {}", self.table_name)).await {
                let err_msg = e.to_string();
                if !err_msg.contains("already") {
                    return Err(e);
                }
            }

            // Create authenticated MAC set with timeout support
            if let Err(e) = self.run_nft(&format!(
                "add set inet {} {} {{ type ether_addr; flags timeout; }}",
                self.table_name, self.set_name
            )).await {
                let err_msg = e.to_string();
                if !err_msg.contains("already") {
                    return Err(e);
                }
            }

            Ok(())
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_mac_valid() {
        assert!(validate_mac("aa:bb:cc:dd:ee:ff").is_ok());
        assert!(validate_mac("02:00:00:00:00:01").is_ok());
        assert!(validate_mac("12:34:56:78:9a:bc").is_ok());
    }

    #[test]
    fn test_validate_mac_rejects_uppercase() {
        assert!(validate_mac("AA:BB:CC:DD:EE:FF").is_err());
    }

    #[test]
    fn test_validate_mac_rejects_null() {
        assert!(validate_mac("00:00:00:00:00:00").is_err());
    }

    #[test]
    fn test_validate_mac_rejects_broadcast() {
        assert!(validate_mac("ff:ff:ff:ff:ff:ff").is_err());
    }

    #[test]
    fn test_validate_mac_rejects_injection() {
        assert!(validate_mac("aa:bb:cc:dd:ee:ff } ; add rule inet didicafe input accept ; #").is_err());
        assert!(validate_mac("aa:bb:cc:dd:ee:f").is_err());
        assert!(validate_mac("not-a-mac").is_err());
        assert!(validate_mac("").is_err());
    }
}
