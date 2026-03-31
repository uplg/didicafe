mod nftables;

#[cfg(any(test, debug_assertions))]
mod mock;

pub use nftables::NftablesController;

#[cfg(any(test, debug_assertions))]
pub use mock::MockFirewall;

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

/// Trait abstracting firewall operations for testability.
///
/// `NftablesController` is the production implementation (calls `nft` CLI).
/// `MockFirewall` is used in tests (records calls, returns Ok).
///
/// Uses boxed futures for object safety (`dyn Firewall`).
pub trait Firewall: Send + Sync {
    /// Add a MAC address to the authenticated set with a timeout.
    fn authorize_mac(
        &self,
        mac: &str,
        timeout_secs: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Remove a MAC address from the authenticated set.
    fn deauthorize_mac(
        &self,
        mac: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Initialize the firewall table and set (idempotent, called on startup).
    fn init_ruleset(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}
