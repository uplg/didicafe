use anyhow::{Context, Result};
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::config::FirewallConfig;
use crate::net::mac::validate_mac;
use super::Firewall;

/// Controls nftables rules via the `nft` CLI.
///
/// Manages the `auth_clients` set: adding (MAC, IP) pairs with timeouts on auth,
/// and removing them on disconnect/expiry. Uses a concatenated set type
/// (`ether_addr . ipv4_addr`) so a spoofed MAC from a different IP won't match.
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
    fn authorize_client(
        &self,
        mac: &str,
        ip: &str,
        timeout_secs: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let mac = mac.to_owned();
        let ip = ip.to_owned();
        Box::pin(async move {
            validate_mac(&mac)
                .with_context(|| format!("refusing to authorize invalid MAC: {mac}"))?;

            let element = format!("{} . {} timeout {}s", mac, ip, timeout_secs);
            let cmd = format!(
                "add element inet {} {} {{ {} }}",
                self.table_name, self.set_name, element
            );

            debug!(%mac, %ip, timeout_secs, "authorizing client in nftables");
            self.run_nft(&cmd)
                .await
                .with_context(|| format!("failed to authorize client {mac}/{ip}"))
        })
    }

    fn deauthorize_client(
        &self,
        mac: &str,
        ip: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let mac = mac.to_owned();
        let ip = ip.to_owned();
        Box::pin(async move {
            validate_mac(&mac)
                .with_context(|| format!("refusing to deauthorize invalid MAC: {mac}"))?;

            let cmd = format!(
                "delete element inet {} {} {{ {} . {} }}",
                self.table_name, self.set_name, mac, ip
            );

            debug!(%mac, %ip, "deauthorizing client from nftables");
            // Ignore errors if the element doesn't exist (already expired)
            if let Err(e) = self.run_nft(&cmd).await {
                warn!(%mac, %ip, "deauthorize failed (may already be expired): {e}");
            }
            Ok(())
        })
    }

    fn init_ruleset(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Create table — ignore "already exists" via exit code
            let table_cmd = format!("add table inet {}", self.table_name);
            if let Err(e) = self.run_nft(&table_cmd).await {
                let err_msg = e.to_string().to_lowercase();
                if !err_msg.contains("exist") {
                    return Err(e);
                }
            }

            // Create authenticated client set (MAC+IP concatenation) with timeout support
            let set_cmd = format!(
                "add set inet {} {} {{ type ether_addr . ipv4_addr; flags timeout; }}",
                self.table_name, self.set_name
            );
            if let Err(e) = self.run_nft(&set_cmd).await {
                let err_msg = e.to_string().to_lowercase();
                if !err_msg.contains("exist") {
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
    fn test_nftables_controller_creation() {
        let config = FirewallConfig {
            nft_path: "/usr/sbin/nft".to_string(),
            table_name: "didicafe".to_string(),
            set_name: "auth_clients".to_string(),
        };
        let controller = NftablesController::new(&config);
        assert_eq!(controller.nft_path, "/usr/sbin/nft");
        assert_eq!(controller.table_name, "didicafe");
        assert_eq!(controller.set_name, "auth_clients");
    }
}
