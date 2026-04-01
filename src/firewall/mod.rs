/// Production firewall implementation (nftables CLI).
/// Compiled in release builds and tests (for unit testing the controller itself).
#[cfg(any(test, not(debug_assertions)))]
mod nftables;

#[cfg(any(test, debug_assertions))]
mod mock;

#[cfg(not(debug_assertions))]
pub use nftables::NftablesController;

#[cfg(any(test, debug_assertions))]
pub use mock::MockFirewall;

use anyhow::Result;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;

/// Trait abstracting firewall operations for testability.
///
/// `NftablesController` is the production implementation (calls `nft` CLI).
/// `MockFirewall` is used in tests (records calls, returns Ok).
///
/// Uses boxed futures for object safety (`dyn Firewall`).
///
/// MAC+IP binding: both `authorize` and `deauthorize` take a (MAC, IP) pair.
/// nftables uses a concatenated set `{ type ether_addr . ipv4_addr; flags timeout; }`
/// so a spoofed MAC from a different IP will NOT match.
pub trait Firewall: Send + Sync {
    /// Add a (MAC, IP) pair to the authenticated set with a timeout.
    fn authorize_client(
        &self,
        mac: &str,
        ip: &str,
        timeout_secs: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Remove a (MAC, IP) pair from the authenticated set.
    fn deauthorize_client(
        &self,
        mac: &str,
        ip: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Initialize the firewall table and set (idempotent, called on startup).
    fn init_ruleset(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Downcast support for tests.
    fn as_any(&self) -> &dyn Any;
}
