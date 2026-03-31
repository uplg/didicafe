use serde::Serialize;
use std::fmt;

/// Type-safe token status values.
///
/// Stored in SQLite as lowercase strings. Parsed via `sqlx::FromRow` using
/// the `String` field, then converted via helper methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenStatus {
    Unused,
    Active,
    Expired,
    Revoked,
}

impl fmt::Display for TokenStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unused => write!(f, "unused"),
            Self::Active => write!(f, "active"),
            Self::Expired => write!(f, "expired"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}

impl TokenStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "unused" => Some(Self::Unused),
            "active" => Some(Self::Active),
            "expired" => Some(Self::Expired),
            "revoked" => Some(Self::Revoked),
            _ => None,
        }
    }

    /// All valid status values (for input validation).
    pub const ALL: &[&str] = &["unused", "active", "expired", "revoked"];
}

/// Type-safe session status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Expired,
    Disconnected,
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Expired => write!(f, "expired"),
            Self::Disconnected => write!(f, "disconnected"),
        }
    }
}

impl SessionStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "expired" => Some(Self::Expired),
            "disconnected" => Some(Self::Disconnected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Plan {
    pub id: i64,
    pub name: String,
    pub duration_minutes: i64,
    pub price_ariary: i64,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Token {
    pub id: i64,
    pub code: String,
    pub name: Option<String>,
    pub plan_id: i64,
    pub status: String,
    pub created_at: String,
    pub redeemed_at: Option<String>,
    pub expires_at: Option<String>,
    pub duration_minutes: i64,
}

impl Token {
    /// Parse the status string into a typed enum.
    pub fn token_status(&self) -> Option<TokenStatus> {
        TokenStatus::from_str(&self.status)
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Session {
    pub id: i64,
    pub token_id: i64,
    pub mac_address: String,
    pub ip_address: String,
    pub started_at: String,
    pub expires_at: String,
    pub status: String,
}

impl Session {
    /// Returns remaining seconds for this session, or 0 if expired.
    pub fn remaining_seconds(&self) -> i64 {
        let Ok(expires) =
            chrono::NaiveDateTime::parse_from_str(&self.expires_at, "%Y-%m-%d %H:%M:%S")
        else {
            return 0;
        };
        let now = chrono::Utc::now().naive_utc();
        let diff = expires.signed_duration_since(now).num_seconds();
        diff.max(0)
    }

    /// Parse the status string into a typed enum.
    pub fn session_status(&self) -> Option<SessionStatus> {
        SessionStatus::from_str(&self.status)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyStats {
    pub tokens_sold: i64,
    pub active_sessions: i64,
    pub revenue_ariary: i64,
}
