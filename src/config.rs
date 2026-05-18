use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de;

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
    #[serde(default)]
    pub portal: PortalConfig,
    #[serde(default)]
    pub tls: TlsConfig,
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
    /// Optional list of allowed CIDR networks for admin access.
    /// If empty (default), all IPs are accepted.
    /// Example: ["10.10.0.0/24", "192.168.1.0/24"]
    #[serde(default)]
    pub allowed_networks: Vec<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct PortalConfig {
    /// Local domain name resolved by dnsmasq (e.g. "wifi.didicafe").
    /// Used in CPD redirections and displayed in the UI.
    /// Must NOT use the `.local` TLD: iOS resolves `.local` exclusively via
    /// mDNS, and without an mDNS responder on the box, the captive sheet
    /// can't follow the portal redirect on iPhones.
    #[serde(default = "default_portal_domain")]
    pub domain: String,
    /// Admin domain resolved by dnsmasq (e.g. "admin.wifi.didicafe").
    /// The manager types `https://admin.wifi.didicafe` in his browser.
    /// Must be included as a SAN in the TLS certificate. Same `.local`
    /// caveat as `domain` applies.
    #[serde(default = "default_admin_domain")]
    pub admin_domain: String,
    /// Display name for the café. Shown in the portal header and admin UI.
    #[serde(default = "default_cafe_name")]
    pub cafe_name: String,
    /// Optional welcome message shown on the portal page.
    #[serde(default)]
    pub welcome_message: String,
    /// Primary theme color (hex). Used to generate CSS custom properties.
    #[serde(default = "default_theme_color")]
    pub theme_color: String,
    /// Contact phone number displayed on public pages.
    #[serde(default)]
    pub contact_phone: String,
    /// Contact person name displayed on public pages.
    #[serde(default)]
    pub contact_name: String,
    /// Business hours displayed on public pages.
    #[serde(default)]
    pub contact_hours: String,
}

fn default_portal_domain() -> String {
    "wifi.didicafe".to_string()
}

fn default_admin_domain() -> String {
    "admin.wifi.didicafe".to_string()
}

fn default_cafe_name() -> String {
    "DidiCafe".to_string()
}

fn default_theme_color() -> String {
    "#b45309".to_string()
}

impl PortalConfig {
    /// Generate a CSS `<style>` block that overrides the default primary color
    /// custom properties based on `theme_color`.
    ///
    /// Produces `:root { --c-primary: ...; --c-primary-hover: ...; --c-primary-light: ...; --c-primary-50: ...; }`
    /// This is injected into `base.html` / `admin/base.html` so the entire
    /// palette adapts to the configured brand color.
    pub fn generate_theme_css(&self) -> String {
        let hex = &self.theme_color;
        let (r, g, b) = parse_hex_rgb(hex);

        // Primary = the configured color
        let primary = format!("#{r:02x}{g:02x}{b:02x}");
        // Hover = 15% darker
        let hover = darken(r, g, b, 0.15);
        // Light = 85% lighter (mixed towards white)
        let light = lighten(r, g, b, 0.85);
        // 50 = 93% lighter
        let p50 = lighten(r, g, b, 0.93);

        format!(
            ":root {{ --c-primary: {primary}; --c-primary-hover: {hover}; --c-primary-light: {light}; --c-primary-50: {p50}; }}"
        )
    }
}

/// Parse a hex color (#RGB or #RRGGBB) into (r, g, b) components.
fn parse_hex_rgb(hex: &str) -> (u8, u8, u8) {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0);
        (r * 17, g * 17, b * 17)
    } else if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r, g, b)
    } else {
        (180, 83, 9) // fallback to default amber
    }
}

/// Darken an RGB color by a factor (0.0 = unchanged, 1.0 = black).
fn darken(r: u8, g: u8, b: u8, factor: f64) -> String {
    let f = 1.0 - factor;
    let r = (r as f64 * f).round() as u8;
    let g = (g as f64 * f).round() as u8;
    let b = (b as f64 * f).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Lighten an RGB color by mixing towards white (0.0 = unchanged, 1.0 = white).
fn lighten(r: u8, g: u8, b: u8, factor: f64) -> String {
    let r = (r as f64 + (255.0 - r as f64) * factor).round() as u8;
    let g = (g as f64 + (255.0 - g as f64) * factor).round() as u8;
    let b = (b as f64 + (255.0 - b as f64) * factor).round() as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

impl Default for PortalConfig {
    fn default() -> Self {
        Self {
            domain: default_portal_domain(),
            admin_domain: default_admin_domain(),
            cafe_name: default_cafe_name(),
            welcome_message: String::new(),
            theme_color: default_theme_color(),
            contact_phone: String::new(),
            contact_name: String::new(),
            contact_hours: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    /// Enable HTTPS admin listener. Defaults to false (dev mode).
    #[serde(default)]
    pub enabled: bool,
    /// Path to the PEM-encoded server certificate.
    #[serde(default)]
    pub cert_path: String,
    /// Path to the PEM-encoded server private key.
    #[serde(default)]
    pub key_path: String,
    /// Port for the HTTPS admin listener.
    #[serde(default = "default_admin_port")]
    pub admin_port: u16,
}

fn default_admin_port() -> u16 {
    443
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_path: String::new(),
            key_path: String::new(),
            admin_port: default_admin_port(),
        }
    }
}

/// Validate a hex color string (#RGB or #RRGGBB).
pub fn is_valid_hex_color(s: &str) -> bool {
    let s = s.strip_prefix('#').unwrap_or("");
    (s.len() == 3 || s.len() == 6) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse a CIDR notation string (e.g. "10.10.0.0/24") into (network, prefix_len).
/// Returns `None` if the format is invalid.
pub fn parse_cidr(cidr: &str) -> Option<(std::net::IpAddr, u8)> {
    let (addr_str, prefix_str) = cidr.split_once('/')?;
    let addr: std::net::IpAddr = addr_str.parse().ok()?;
    let prefix_len: u8 = prefix_str.parse().ok()?;
    let max_prefix = if addr.is_ipv4() { 32 } else { 128 };
    if prefix_len > max_prefix {
        return None;
    }
    Some((addr, prefix_len))
}

/// Check whether an IP address falls within a CIDR network.
pub fn ip_in_cidr(ip: std::net::IpAddr, cidr: &str) -> bool {
    let Some((network, prefix_len)) = parse_cidr(cidr) else {
        return false;
    };

    match (ip, network) {
        (std::net::IpAddr::V4(ip), std::net::IpAddr::V4(net)) => {
            let ip_bits = u32::from(ip);
            let net_bits = u32::from(net);
            if prefix_len == 0 {
                return true;
            }
            let mask = u32::MAX << (32 - prefix_len);
            (ip_bits & mask) == (net_bits & mask)
        }
        (std::net::IpAddr::V6(ip), std::net::IpAddr::V6(net)) => {
            let ip_bits = u128::from(ip);
            let net_bits = u128::from(net);
            if prefix_len == 0 {
                return true;
            }
            let mask = u128::MAX << (128 - prefix_len);
            (ip_bits & mask) == (net_bits & mask)
        }
        _ => false, // IPv4/IPv6 mismatch
    }
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
                !iface.is_empty()
                    && iface.len() <= 15
                    && iface
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
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

        // Portal
        anyhow::ensure!(
            !self.portal.domain.is_empty(),
            "portal.domain must not be empty"
        );
        // Domain: alphanumeric, hyphens, dots only (no path, no scheme)
        anyhow::ensure!(
            self.portal
                .domain
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
            "portal.domain must contain only alphanumeric characters, dots, and hyphens, got '{}'",
            self.portal.domain
        );
        anyhow::ensure!(
            !self.portal.admin_domain.is_empty(),
            "portal.admin_domain must not be empty"
        );
        anyhow::ensure!(
            self.portal
                .admin_domain
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
            "portal.admin_domain must contain only alphanumeric characters, dots, and hyphens, got '{}'",
            self.portal.admin_domain
        );
        anyhow::ensure!(
            self.portal.domain != self.portal.admin_domain,
            "portal.admin_domain ('{}') must differ from portal.domain ('{}')",
            self.portal.admin_domain,
            self.portal.domain
        );
        anyhow::ensure!(
            !self.portal.cafe_name.is_empty(),
            "portal.cafe_name must not be empty"
        );
        anyhow::ensure!(
            self.portal.cafe_name.len() <= 100,
            "portal.cafe_name must be 100 characters or less"
        );
        // Theme color: valid hex color (#RGB or #RRGGBB)
        anyhow::ensure!(
            is_valid_hex_color(&self.portal.theme_color),
            "portal.theme_color must be a valid hex color (#RGB or #RRGGBB), got '{}'",
            self.portal.theme_color
        );

        // TLS
        if self.tls.enabled {
            anyhow::ensure!(
                !self.tls.cert_path.is_empty(),
                "tls.cert_path must not be empty when tls.enabled = true"
            );
            anyhow::ensure!(
                !self.tls.key_path.is_empty(),
                "tls.key_path must not be empty when tls.enabled = true"
            );
        }
        anyhow::ensure!(
            (1..=65535).contains(&self.tls.admin_port),
            "tls.admin_port must be 1–65535, got {}",
            self.tls.admin_port
        );
        anyhow::ensure!(
            self.tls.admin_port != self.server.port,
            "tls.admin_port ({}) must differ from server.port ({})",
            self.tls.admin_port,
            self.server.port
        );

        // Admin allowed_networks CIDR validation
        for cidr in &self.admin.allowed_networks {
            anyhow::ensure!(
                parse_cidr(cidr).is_some(),
                "admin.allowed_networks: '{}' is not a valid CIDR notation (e.g. 10.10.0.0/24)",
                cidr
            );
        }

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

    #[test]
    fn test_portal_domain_empty_rejected() {
        let mut cfg = test_config();
        cfg.portal.domain = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_portal_domain_with_scheme_rejected() {
        let mut cfg = test_config();
        cfg.portal.domain = "http://didicafe.local".to_string();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("portal.domain"),
            "expected domain error, got: {err}"
        );
    }

    #[test]
    fn test_portal_domain_valid() {
        let mut cfg = test_config();
        cfg.portal.domain = "cafe.local".to_string();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_portal_cafe_name_empty_rejected() {
        let mut cfg = test_config();
        cfg.portal.cafe_name = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_portal_theme_color_invalid_rejected() {
        let mut cfg = test_config();
        cfg.portal.theme_color = "red".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_portal_theme_color_valid_short() {
        let mut cfg = test_config();
        cfg.portal.theme_color = "#f80".to_string();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_tls_enabled_requires_cert_path() {
        let mut cfg = test_config();
        cfg.tls.enabled = true;
        cfg.tls.cert_path = String::new();
        cfg.tls.key_path = "/some/key.pem".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_tls_enabled_requires_key_path() {
        let mut cfg = test_config();
        cfg.tls.enabled = true;
        cfg.tls.cert_path = "/some/cert.pem".to_string();
        cfg.tls.key_path = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_tls_disabled_no_paths_required() {
        let cfg = test_config();
        // tls.enabled defaults to false, no paths needed
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_tls_admin_port_same_as_server_rejected() {
        let mut cfg = test_config();
        cfg.tls.admin_port = cfg.server.port;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_admin_allowed_networks_valid_cidr() {
        let mut cfg = test_config();
        cfg.admin.allowed_networks = vec!["10.10.0.0/24".to_string()];
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_admin_allowed_networks_invalid_cidr() {
        let mut cfg = test_config();
        cfg.admin.allowed_networks = vec!["not-a-cidr".to_string()];
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_hex_color_valid() {
        assert!(super::is_valid_hex_color("#b45309"));
        assert!(super::is_valid_hex_color("#fff"));
        assert!(super::is_valid_hex_color("#AABBCC"));
    }

    #[test]
    fn test_hex_color_invalid() {
        assert!(!super::is_valid_hex_color("red"));
        assert!(!super::is_valid_hex_color("#gg0000"));
        assert!(!super::is_valid_hex_color("#12345"));
        assert!(!super::is_valid_hex_color(""));
    }

    #[test]
    fn test_ip_in_cidr() {
        use std::net::IpAddr;
        assert!(super::ip_in_cidr(
            "10.10.0.5".parse::<IpAddr>().unwrap(),
            "10.10.0.0/24"
        ));
        assert!(!super::ip_in_cidr(
            "10.10.1.5".parse::<IpAddr>().unwrap(),
            "10.10.0.0/24"
        ));
        assert!(super::ip_in_cidr(
            "192.168.1.1".parse::<IpAddr>().unwrap(),
            "192.168.0.0/16"
        ));
    }

    #[test]
    fn test_parse_cidr_invalid() {
        assert!(super::parse_cidr("garbage").is_none());
        assert!(super::parse_cidr("10.10.0.0/33").is_none());
        assert!(super::parse_cidr("10.10.0.0/abc").is_none());
    }

    #[test]
    fn test_admin_domain_empty_rejected() {
        let mut cfg = test_config();
        cfg.portal.admin_domain = String::new();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_admin_domain_with_scheme_rejected() {
        let mut cfg = test_config();
        cfg.portal.admin_domain = "https://admin.didicafe.local".to_string();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("admin_domain"),
            "expected admin_domain error, got: {err}"
        );
    }

    #[test]
    fn test_admin_domain_same_as_domain_rejected() {
        let mut cfg = test_config();
        cfg.portal.admin_domain = cfg.portal.domain.clone();
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("must differ"),
            "expected 'must differ' error, got: {err}"
        );
    }

    #[test]
    fn test_admin_domain_valid() {
        let mut cfg = test_config();
        cfg.portal.admin_domain = "admin.cafe.local".to_string();
        cfg.portal.domain = "cafe.local".to_string();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_parse_hex_rgb_6digit() {
        let (r, g, b) = super::parse_hex_rgb("#b45309");
        assert_eq!((r, g, b), (180, 83, 9));
    }

    #[test]
    fn test_parse_hex_rgb_3digit() {
        let (r, g, b) = super::parse_hex_rgb("#f80");
        assert_eq!((r, g, b), (0xff, 0x88, 0x00));
    }

    #[test]
    fn test_generate_theme_css_contains_properties() {
        let cfg = test_config();
        let css = cfg.portal.generate_theme_css();
        assert!(
            css.starts_with(":root {"),
            "expected :root block, got: {css}"
        );
        assert!(css.contains("--c-primary:"));
        assert!(css.contains("--c-primary-hover:"));
        assert!(css.contains("--c-primary-light:"));
        assert!(css.contains("--c-primary-50:"));
    }

    #[test]
    fn test_darken_produces_darker_color() {
        let dark = super::darken(180, 83, 9, 0.15);
        // 180 * 0.85 = 153, 83 * 0.85 ≈ 71, 9 * 0.85 ≈ 8
        assert_eq!(dark, "#994708");
    }

    #[test]
    fn test_lighten_produces_lighter_color() {
        let light = super::lighten(180, 83, 9, 0.85);
        // 180 + (255-180)*0.85 ≈ 244, 83 + (255-83)*0.85 ≈ 229, 9 + (255-9)*0.85 ≈ 218
        assert_eq!(light, "#f4e5da");
    }
}
