use anyhow::{Context, Result};
use serde::de;
use serde::Deserialize;

/// Deserializes a value that can be either a single string or a list of strings.
/// This allows TOML config to use either:
///   interfaces = "br-lan"           # single interface (bridge)
///   interfaces = ["wlan0", "wlan1"] # multiple interfaces (dual-band)
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Vec<String>, E> {
            Ok(vec![value.to_owned()])
        }

        fn visit_seq<A: de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Vec<String>, A::Error> {
            let mut v = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                v.push(s);
            }
            Ok(v)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub admin: AdminConfig,
    pub firewall: FirewallConfig,
    pub token: TokenConfig,
    pub rate_limit: RateLimitConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen: String,
    pub port: u16,
    /// Network interface(s) for the captive portal WiFi.
    /// Accepts a single string (e.g. "br-lan" for a bridge) or a list
    /// (e.g. ["wlan0", "wlan1"] for dual-band without bridge).
    #[serde(deserialize_with = "deserialize_string_or_vec")]
    pub interfaces: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    pub username: String,
    pub password_hash: String,
    /// Admin session timeout in seconds. Defaults to 3600 (1 hour).
    #[serde(default = "default_admin_session_timeout")]
    pub session_timeout_seconds: u64,
}

fn default_admin_session_timeout() -> u64 {
    3600
}

#[derive(Debug, Clone, Deserialize)]
pub struct FirewallConfig {
    pub nft_path: String,
    pub table_name: String,
    pub set_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenConfig {
    pub prefix: String,
    pub charset: String,
    pub length: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    pub max_auth_attempts: u32,
    pub auth_window_seconds: u64,
    pub ban_after_attempts: u32,
    pub ban_duration_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionConfig {
    pub cleanup_interval_seconds: u64,
    pub grace_period_seconds: i64,
    /// Days to retain expired/disconnected sessions before hard deletion.
    #[serde(default = "default_session_retention_days")]
    pub retention_days: i64,
    /// Days to retain audit log entries before hard deletion.
    #[serde(default = "default_audit_retention_days")]
    pub audit_retention_days: i64,
}

fn default_session_retention_days() -> i64 {
    90
}

fn default_audit_retention_days() -> i64 {
    365
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {path}"))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("failed to parse config file: {path}"))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate all config values after parsing.
    ///
    /// Fails fast with a clear message for each invalid value.
    fn validate(&self) -> Result<()> {
        // Server
        anyhow::ensure!(
            (1..=65535).contains(&self.server.port),
            "server.port must be 1–65535, got {}",
            self.server.port
        );
        anyhow::ensure!(
            !self.server.listen.is_empty(),
            "server.listen must not be empty"
        );
        anyhow::ensure!(
            self.server.listen.parse::<std::net::IpAddr>().is_ok(),
            "server.listen must be a valid IP address, got '{}'",
            self.server.listen
        );
        anyhow::ensure!(
            !self.server.interfaces.is_empty(),
            "server.interfaces must not be empty"
        );
        for iface in &self.server.interfaces {
            anyhow::ensure!(
                !iface.is_empty() && iface.len() <= 15
                    && iface.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "server.interfaces: '{}' is not a valid Linux interface name (max 15 chars, alphanumeric/-/_)",
                iface
            );
        }

        // Database
        anyhow::ensure!(
            !self.database.path.is_empty(),
            "database.path must not be empty"
        );

        // Admin
        anyhow::ensure!(
            !self.admin.username.is_empty(),
            "admin.username must not be empty"
        );
        anyhow::ensure!(
            self.admin.password_hash.starts_with("$argon2id$"),
            "admin.password_hash must be an Argon2id PHC string (starts with $argon2id$)"
        );
        // Reject known default/weak password hashes
        const KNOWN_WEAK_HASHES: &[&str] = &[
            "$argon2id$v=19$m=19456,t=2,p=1$UmHhUjxRtY3c7xV5M6xDaA$Vwia8SfxfxVLTom4LovEtY8PbJitJYDissghdMfTcSM",
        ];
        anyhow::ensure!(
            !KNOWN_WEAK_HASHES.contains(&self.admin.password_hash.as_str()),
            "admin.password_hash is a known default/weak password. Generate a strong password hash before deploying."
        );
        anyhow::ensure!(
            self.admin.session_timeout_seconds > 0,
            "admin.session_timeout_seconds must be > 0"
        );

        // Firewall
        anyhow::ensure!(
            !self.firewall.nft_path.is_empty(),
            "firewall.nft_path must not be empty"
        );
        anyhow::ensure!(
            !self.firewall.table_name.is_empty()
                && self
                    .firewall
                    .table_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "firewall.table_name must contain only alphanumeric characters and underscores"
        );
        anyhow::ensure!(
            !self.firewall.set_name.is_empty()
                && self
                    .firewall
                    .set_name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "firewall.set_name must contain only alphanumeric characters and underscores"
        );

        // Token
        anyhow::ensure!(
            !self.token.charset.is_empty(),
            "token.charset must not be empty"
        );
        anyhow::ensure!(
            self.token.length >= 6 && self.token.length.is_multiple_of(2),
            "token.length must be >= 6 and even, got {}",
            self.token.length
        );
        anyhow::ensure!(
            !self.token.prefix.is_empty(),
            "token.prefix must not be empty"
        );

        // Rate limit
        anyhow::ensure!(
            self.rate_limit.max_auth_attempts > 0,
            "rate_limit.max_auth_attempts must be > 0"
        );
        anyhow::ensure!(
            self.rate_limit.auth_window_seconds > 0,
            "rate_limit.auth_window_seconds must be > 0"
        );
        anyhow::ensure!(
            self.rate_limit.ban_after_attempts >= self.rate_limit.max_auth_attempts,
            "rate_limit.ban_after_attempts must be >= max_auth_attempts"
        );
        anyhow::ensure!(
            self.rate_limit.ban_duration_seconds > 0,
            "rate_limit.ban_duration_seconds must be > 0"
        );

        // Session
        anyhow::ensure!(
            self.session.cleanup_interval_seconds > 0,
            "session.cleanup_interval_seconds must be > 0"
        );
        anyhow::ensure!(
            self.session.retention_days > 0,
            "session.retention_days must be > 0, got {}",
            self.session.retention_days
        );
        anyhow::ensure!(
            self.session.audit_retention_days > 0,
            "session.audit_retention_days must be > 0, got {}",
            self.session.audit_retention_days
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::test_config;

    #[test]
    fn test_valid_config_passes() {
        assert!(test_config().validate().is_ok());
    }

    #[test]
    fn test_port_zero_rejected() {
        let mut cfg = test_config();
        cfg.server.port = 0;
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("port"), "expected port error, got: {err}");
    }

    #[test]
    fn test_empty_charset_rejected() {
        let mut cfg = test_config();
        cfg.token.charset = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_odd_token_length_rejected() {
        let mut cfg = test_config();
        cfg.token.length = 7;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_bad_password_hash_rejected() {
        let mut cfg = test_config();
        cfg.admin.password_hash = "bcrypt$plaintext".to_string();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("argon2id"),
            "expected argon2id error, got: {err}"
        );
    }

    #[test]
    fn test_ban_below_max_rejected() {
        let mut cfg = test_config();
        cfg.rate_limit.ban_after_attempts = 3; // below max_auth_attempts=5
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_firewall_table_name_injection_rejected() {
        let mut cfg = test_config();
        cfg.firewall.table_name = "didicafe; DROP TABLE".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_firewall_set_name_injection_rejected() {
        let mut cfg = test_config();
        cfg.firewall.set_name = "auth_macs; rm -rf".to_string();
        assert!(cfg.validate().is_err());
    }
}
