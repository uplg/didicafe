use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use rand::RngExt;
use subtle::ConstantTimeEq;

const SESSION_ID_LEN: usize = 32;
const SESSION_CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Session data stored per admin session.
struct SessionData {
    expires_at: Instant,
    csrf_token: String,
}

/// Generate a random alphanumeric token of the given length.
pub fn generate_session_token(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| SESSION_CHARSET[rng.random_range(0..SESSION_CHARSET.len())] as char)
        .collect()
}

/// In-memory admin session store.
///
/// Stores session IDs mapped to their expiry time and CSRF token. Thread-safe via `RwLock`.
/// Sessions are invalidated on logout or when they expire.
///
/// For a single-admin cybercafe, this is more than sufficient. Sessions do not
/// survive daemon restarts (by design — OWASP recommends short-lived sessions).
pub struct AdminSessionStore {
    sessions: RwLock<HashMap<String, SessionData>>,
    timeout: Duration,
}

impl AdminSessionStore {
    pub fn new(timeout_seconds: u64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            timeout: Duration::from_secs(timeout_seconds),
        }
    }

    /// Create a new session and return its cryptographically random ID.
    ///
    /// The session ID has 192 bits of entropy (32 chars from a 62-char alphabet),
    /// exceeding the OWASP minimum of 128 bits.
    pub fn create(&self) -> String {
        let session_id = generate_session_token(SESSION_ID_LEN);
        let csrf_token = generate_session_token(SESSION_ID_LEN);
        let expires_at = Instant::now() + self.timeout;

        self.sessions
            .write()
            .expect("session store lock poisoned")
            .insert(
                session_id.clone(),
                SessionData {
                    expires_at,
                    csrf_token,
                },
            );

        session_id
    }

    /// Get the CSRF token for a session.
    pub fn csrf_token(&self, session_id: &str) -> String {
        self.sessions
            .read()
            .expect("session store lock poisoned")
            .get(session_id)
            .map(|d| d.csrf_token.clone())
            .unwrap_or_default()
    }

    /// Validate a CSRF token against the session's stored token.
    /// Uses constant-time comparison to prevent timing side-channels.
    pub fn validate_csrf(&self, session_id: &str, submitted: &str) -> bool {
        self.sessions
            .read()
            .expect("session store lock poisoned")
            .get(session_id)
            .is_some_and(|d| d.csrf_token.as_bytes().ct_eq(submitted.as_bytes()).into())
    }

    /// Validate a session ID. Returns `true` if the session exists and has not expired.
    ///
    /// Expired sessions are removed lazily on validation.
    pub fn validate(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().expect("session store lock poisoned");
        if let Some(data) = sessions.get(session_id) {
            if Instant::now() < data.expires_at {
                return true;
            }
            // Expired — remove lazily
            sessions.remove(session_id);
        }
        false
    }

    /// Invalidate a session (logout).
    pub fn remove(&self, session_id: &str) {
        self.sessions
            .write()
            .expect("session store lock poisoned")
            .remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_validate_session() {
        let store = AdminSessionStore::new(3600);
        let id = store.create();
        assert_eq!(id.len(), SESSION_ID_LEN);
        assert!(store.validate(&id));
    }

    #[test]
    fn test_invalid_session_rejected() {
        let store = AdminSessionStore::new(3600);
        assert!(!store.validate("nonexistent-session-id"));
    }

    #[test]
    fn test_remove_session() {
        let store = AdminSessionStore::new(3600);
        let id = store.create();
        assert!(store.validate(&id));
        store.remove(&id);
        assert!(!store.validate(&id));
    }

    #[test]
    fn test_expired_session_rejected() {
        // Create store with 0-second timeout (immediate expiry)
        let store = AdminSessionStore::new(0);
        let id = store.create();
        // Session should already be expired
        std::thread::sleep(Duration::from_millis(1));
        assert!(!store.validate(&id));
    }

    #[test]
    fn test_session_id_uniqueness() {
        let store = AdminSessionStore::new(3600);
        let id1 = store.create();
        let id2 = store.create();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_csrf_token_roundtrip() {
        let store = AdminSessionStore::new(3600);
        let id = store.create();
        let csrf = store.csrf_token(&id);
        assert_eq!(csrf.len(), SESSION_ID_LEN);
        assert!(store.validate_csrf(&id, &csrf));
        assert!(!store.validate_csrf(&id, "wrong-token"));
    }

    #[test]
    fn test_csrf_token_unique_per_session() {
        let store = AdminSessionStore::new(3600);
        let id1 = store.create();
        let id2 = store.create();
        let csrf1 = store.csrf_token(&id1);
        let csrf2 = store.csrf_token(&id2);
        assert_ne!(csrf1, csrf2);
    }
}
