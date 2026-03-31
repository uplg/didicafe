use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use rand::RngExt;

const SESSION_ID_LEN: usize = 32;
const SESSION_CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// In-memory admin session store.
///
/// Stores session IDs mapped to their expiry time. Thread-safe via `RwLock`.
/// Sessions are invalidated on logout or when they expire.
///
/// For a single-admin cybercafe, this is more than sufficient. Sessions do not
/// survive daemon restarts (by design — OWASP recommends short-lived sessions).
pub struct AdminSessionStore {
    sessions: RwLock<HashMap<String, Instant>>,
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
        let mut rng = rand::rng();
        let session_id: String = (0..SESSION_ID_LEN)
            .map(|_| SESSION_CHARSET[rng.random_range(0..SESSION_CHARSET.len())] as char)
            .collect();

        let expires_at = Instant::now() + self.timeout;
        self.sessions
            .write()
            .expect("session store lock poisoned")
            .insert(session_id.clone(), expires_at);

        session_id
    }

    /// Validate a session ID. Returns `true` if the session exists and has not expired.
    ///
    /// Expired sessions are removed lazily on validation.
    pub fn validate(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().expect("session store lock poisoned");
        if let Some(&expires_at) = sessions.get(session_id) {
            if Instant::now() < expires_at {
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
}
