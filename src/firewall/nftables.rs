use anyhow::{Context, Result};
use std::future::Future;
use std::pin::Pin;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::config::FirewallConfig;
use super::Firewall;

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
            // Create table if not exists
            self.run_nft(&format!("add table inet {}", self.table_name))
                .await
                .ok(); // ignore "already exists"

            // Create authenticated MAC set with timeout support
            self.run_nft(&format!(
                "add set inet {} {} {{ type ether_addr; flags timeout; }}",
                self.table_name, self.set_name
            ))
            .await
            .ok(); // ignore "already exists"

            Ok(())
        })
    }
}
