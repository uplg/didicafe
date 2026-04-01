use serde::Serialize;
use std::fmt;
use std::str::FromStr;

/// Type-safe token status values.
///
/// Stored in SQLite as lowercase strings. Decoded directly by sqlx via
/// the `Type` derive with `rename_all = "lowercase"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
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

impl FromStr for TokenStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "unused" => Ok(Self::Unused),
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "revoked" => Ok(Self::Revoked),
            _ => Err(format!("invalid token status: {s}")),
        }
    }
}

impl TokenStatus {
    /// All valid status values (for input validation).
    pub const ALL: &[&str] = &["unused", "active", "expired", "revoked"];

    /// Parse from string, returning `None` on invalid input.
    pub fn try_from_str(s: &str) -> Option<Self> {
        Self::from_str(s).ok()
    }
}

/// Allow comparison with string slices (used by askama templates).
impl PartialEq<&str> for TokenStatus {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Self::Unused => *other == "unused",
            Self::Active => *other == "active",
            Self::Expired => *other == "expired",
            Self::Revoked => *other == "revoked",
        }
    }
}

/// Type-safe session status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
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

impl FromStr for SessionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "disconnected" => Ok(Self::Disconnected),
            _ => Err(format!("invalid session status: {s}")),
        }
    }
}

impl SessionStatus {
    /// Parse from string, returning `None` on invalid input.
    pub fn try_from_str(s: &str) -> Option<Self> {
        Self::from_str(s).ok()
    }
}

/// Allow comparison with string slices (used by askama templates).
impl PartialEq<&str> for SessionStatus {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Self::Active => *other == "active",
            Self::Expired => *other == "expired",
            Self::Disconnected => *other == "disconnected",
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
    pub status: TokenStatus,
    pub created_at: String,
    pub redeemed_at: Option<String>,
    pub expires_at: Option<String>,
    pub duration_minutes: i64,
    pub plan_name: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Session {
    pub id: i64,
    pub token_id: i64,
    pub mac_address: String,
    pub ip_address: String,
    pub started_at: String,
    pub expires_at: String,
    pub status: SessionStatus,
    pub token_code: Option<String>,
    pub token_name: Option<String>,
}

impl Session {
    pub fn token_code(&self) -> String {
        self.token_code.clone().unwrap_or_default()
    }

    pub fn token_name(&self) -> String {
        self.token_name.clone().unwrap_or_default()
    }

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
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyStats {
    pub tokens_sold: i64,
    pub active_sessions: i64,
    pub revenue_ariary: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub admin_user: String,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub detail: Option<String>,
}
