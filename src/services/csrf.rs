use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

use subtle::ConstantTimeEq;

use crate::services::admin_session::generate_session_token;

/// Maximum age for portal CSRF tokens before they are pruned (10 minutes).
const CSRF_TOKEN_TTL_SECS: u64 = 600;

/// CSRF token with creation timestamp for TTL-based cleanup.
struct CsrfEntry {
    token: String,
    created_at: Instant,
}

/// Server-side CSRF token store for portal forms.
///
/// Uses Synchronizer Token Pattern: tokens are stored server-side
/// keyed by client MAC address. No cookies, no client-side token storage.
///
/// Tokens are short-lived (replaced on each GET /portal, TTL 10 min)
/// and cleared after successful POST validation.
pub struct PortalCsrfStore {
    tokens: RwLock<HashMap<String, CsrfEntry>>,
}

impl PortalCsrfStore {
    pub fn new() -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
        }
    }

    /// Generate and store a new CSRF token for a MAC address.
    /// Returns the generated token. Also prunes expired entries.
    pub fn generate(&self, mac: &str) -> String {
        let token = generate_session_token(32);
        let now = Instant::now();
        let mut tokens = self.tokens.write().expect("csrf store lock poisoned");

        // Prune expired entries on each generate to bound memory growth
        let ttl = std::time::Duration::from_secs(CSRF_TOKEN_TTL_SECS);
        tokens.retain(|_, entry| now.duration_since(entry.created_at) < ttl);

        tokens.insert(
            mac.to_lowercase(),
            CsrfEntry {
                token: token.clone(),
                created_at: now,
            },
        );
        token
    }

    /// Return the existing non-expired token for a MAC, or generate a new one.
    ///
    /// Why: captive portal browsers (Android, iOS) may issue multiple GET /portal
    /// in quick succession (initial render, CPD background poll, refresh). If we
    /// regenerated on every GET, the form rendered first would carry a token that
    /// gets invalidated by the 2nd GET — submitting it then fails with CSRF
    /// mismatch even though the user did everything right.
    pub fn get_or_generate(&self, mac: &str) -> String {
        let mac_lower = mac.to_lowercase();
        let ttl = std::time::Duration::from_secs(CSRF_TOKEN_TTL_SECS);

        // Fast path: return existing valid token
        {
            let tokens = self.tokens.read().expect("csrf store lock poisoned");
            if let Some(entry) = tokens.get(&mac_lower)
                && entry.created_at.elapsed() < ttl
            {
                return entry.token.clone();
            }
        }

        // Slow path: generate fresh token
        self.generate(mac)
    }

    /// Validate a submitted CSRF token against the stored token for a MAC.
    /// Returns true if valid, then removes the token (one-time use).
    /// Uses constant-time comparison to prevent timing side-channels.
    pub fn validate(&self, mac: &str, submitted: &str) -> bool {
        let mut tokens = self.tokens.write().expect("csrf store lock poisoned");
        let mac_lower = mac.to_lowercase();
        if let Some(entry) = tokens.remove(&mac_lower) {
            // Reject expired tokens
            if entry.created_at.elapsed() > std::time::Duration::from_secs(CSRF_TOKEN_TTL_SECS) {
                return false;
            }
            return entry.token.as_bytes().ct_eq(submitted.as_bytes()).into();
        }
        false
    }

    /// Get the current token for a MAC without removing it (for test helpers).
    pub fn get(&self, mac: &str) -> Option<String> {
        self.tokens
            .read()
            .expect("csrf store lock poisoned")
            .get(&mac.to_lowercase())
            .map(|e| e.token.clone())
    }

    /// Store a token directly (used by tests to seed known tokens).
    pub fn set(&self, mac: &str, token: &str) {
        self.tokens
            .write()
            .expect("csrf store lock poisoned")
            .insert(
                mac.to_lowercase(),
                CsrfEntry {
                    token: token.to_string(),
                    created_at: Instant::now(),
                },
            );
    }
}
