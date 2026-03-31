use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use super::Firewall;

/// Recorded firewall operation for test assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirewallCall {
    Authorize { mac: String, timeout_secs: u64 },
    Deauthorize { mac: String },
    InitRuleset,
}

/// Mock firewall implementation for tests.
///
/// Records all calls for later assertion. Always returns `Ok(())`.
pub struct MockFirewall {
    calls: Mutex<Vec<FirewallCall>>,
}

impl MockFirewall {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Return a snapshot of all recorded calls.
    #[cfg(test)]
    pub fn calls(&self) -> Vec<FirewallCall> {
        self.calls.lock().expect("mock lock poisoned").clone()
    }
}

impl Firewall for MockFirewall {
    fn authorize_mac(
        &self,
        mac: &str,
        timeout_secs: u64,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        self.calls
            .lock()
            .expect("mock lock poisoned")
            .push(FirewallCall::Authorize {
                mac: mac.to_owned(),
                timeout_secs,
            });
        Box::pin(async { Ok(()) })
    }

    fn deauthorize_mac(
        &self,
        mac: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        self.calls
            .lock()
            .expect("mock lock poisoned")
            .push(FirewallCall::Deauthorize {
                mac: mac.to_owned(),
            });
        Box::pin(async { Ok(()) })
    }

    fn init_ruleset(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        self.calls
            .lock()
            .expect("mock lock poisoned")
            .push(FirewallCall::InitRuleset);
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_records_calls() {
        let fw = MockFirewall::new();

        fw.init_ruleset().await.unwrap();
        fw.authorize_mac("AA:BB:CC:DD:EE:FF", 3600).await.unwrap();
        fw.deauthorize_mac("AA:BB:CC:DD:EE:FF").await.unwrap();

        let calls = fw.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], FirewallCall::InitRuleset);
        assert_eq!(
            calls[1],
            FirewallCall::Authorize {
                mac: "AA:BB:CC:DD:EE:FF".to_string(),
                timeout_secs: 3600,
            }
        );
        assert_eq!(
            calls[2],
            FirewallCall::Deauthorize {
                mac: "AA:BB:CC:DD:EE:FF".to_string(),
            }
        );
    }
}
