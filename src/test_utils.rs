use std::net::SocketAddr;
use std::sync::Arc;

use crate::AppState;
use crate::config::{
    AdminConfig, Config, DatabaseConfig, FirewallConfig, PortalConfig,
    RateLimitConfig, ServerConfig, SessionConfig, TlsConfig, TokenConfig,
};
use crate::db::Database;
use crate::firewall::MockFirewall;
use crate::services::admin_session::AdminSessionStore;
use crate::services::csrf::PortalCsrfStore;
use crate::services::rate_limit::RateLimiter;

/// Default valid config for tests.
pub fn test_config() -> Config {
    Config {
        server: ServerConfig {
            listen: "0.0.0.0".to_string(),
            port: 8080,
            interfaces: vec!["wlan0".to_string()],
        },
        database: DatabaseConfig {
            path: "/tmp/test.db".to_string(),
        },
        admin: AdminConfig {
            username: "admin".to_string(),
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$salt$hash".to_string(),
            session_timeout_seconds: 3600,
            allowed_networks: vec![],
        },
        firewall: FirewallConfig {
            nft_path: "/usr/sbin/nft".to_string(),
            table_name: "didicafe".to_string(),
            set_name: "auth_macs".to_string(),
        },
        token: TokenConfig {
            prefix: "DIDI".to_string(),
            charset: "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".to_string(),
            length: 8,
        },
        rate_limit: RateLimitConfig {
            max_auth_attempts: 5,
            auth_window_seconds: 60,
            ban_after_attempts: 10,
            ban_duration_seconds: 900,
        },
        session: SessionConfig {
            cleanup_interval_seconds: 30,
            grace_period_seconds: 10,
            retention_days: 90,
            audit_retention_days: 365,
        },
        portal: PortalConfig::default(),
        tls: TlsConfig::default(),
    }
}

/// Create a fresh in-memory AppState for integration tests.
pub async fn test_state() -> Arc<AppState> {
    let db = Database::open(":memory:").await.unwrap();
    db.migrate().await.unwrap();

    let mut config = test_config();
    config.server.listen = "127.0.0.1".to_string();
    config.server.interfaces = vec!["lo".to_string()];
    config.database.path = ":memory:".to_string();
    config.firewall.table_name = "test".to_string();
    config.firewall.set_name = "test".to_string();

    let rate_limiter = RateLimiter::new(&config.rate_limit);
    let admin_rate_limiter = RateLimiter::new(&config.rate_limit);

    Arc::new(AppState {
        db,
        config,
        firewall: Arc::new(MockFirewall::new()),
        admin_sessions: AdminSessionStore::new(3600),
        rate_limiter,
        admin_rate_limiter,
        portal_csrf_store: PortalCsrfStore::new(),
        login_csrf_store: PortalCsrfStore::new(),
    })
}

/// Build a test GET request with fake ConnectInfo extension.
pub fn test_get(uri: &str) -> axum::http::Request<axum::body::Body> {
    use axum::extract::ConnectInfo;

    let mut req = axum::http::Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));
    req
}

/// Build a form POST request with fake ConnectInfo extension.
pub fn test_post_form(uri: &str, body: &str) -> axum::http::Request<axum::body::Body> {
    use axum::extract::ConnectInfo;

    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));
    req
}
