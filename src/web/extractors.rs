use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::request::Parts,
};
use tracing::warn;

use crate::AppState;
use crate::net::arp;
use super::error::AppError;

/// Cookie name for admin sessions.
pub const ADMIN_COOKIE_NAME: &str = "didicafe_admin";

/// Client network identity extracted from the HTTP connection.
///
/// - `ip`: The client's IP address from the TCP socket (`ConnectInfo<SocketAddr>`).
/// - `mac`: The client's MAC address resolved from the ARP table.
///
/// If the MAC cannot be resolved (e.g., loopback connections during development),
/// the extractor returns `AppError::BadRequest`.
pub struct ClientInfo {
    pub ip: IpAddr,
    pub mac: String,
}

impl<S: Send + Sync> FromRequestParts<S> for ClientInfo {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract the client's socket address from the TCP connection
        let ConnectInfo(addr) = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_err(|e| {
                warn!("failed to extract ConnectInfo: {e}");
                AppError::Internal(anyhow::anyhow!("failed to determine client address"))
            })?;

        let ip = addr.ip();

        // Look up the MAC address from the ARP table
        #[cfg(test)]
        let mac = if ip.is_loopback() {
            "02:00:00:00:00:01".to_string()
        } else {
            arp::lookup_mac(ip).await.ok_or_else(|| {
                warn!(%ip, "MAC lookup failed: client IP not found in ARP table");
                AppError::BadRequest(
                    "Unable to identify your device. Please ensure you are connected via WiFi."
                        .to_string(),
                )
            })?
        };

        #[cfg(not(test))]
        let mac = arp::lookup_mac(ip).await.ok_or_else(|| {
            warn!(%ip, "MAC lookup failed: client IP not found in ARP table");
            AppError::BadRequest(
                "Unable to identify your device. Please ensure you are connected via WiFi."
                    .to_string(),
            )
        })?;

        Ok(ClientInfo { ip, mac })
    }
}

/// Admin session extractor.
///
/// Validates the `didicafe_admin` cookie against the in-memory session store.
/// If the cookie is missing or the session is expired/invalid, returns `AppError::Unauthorized`.
///
/// Usage: add `_admin: AdminSession` to any handler that requires admin authentication.
pub struct AdminSession;

impl FromRequestParts<Arc<AppState>> for AdminSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let session_id = extract_cookie(&parts.headers, ADMIN_COOKIE_NAME)
            .ok_or(AppError::Unauthorized)?;

        if !state.admin_sessions.validate(&session_id) {
            return Err(AppError::Unauthorized);
        }

        Ok(AdminSession)
    }
}

/// Extract a cookie value by name from request headers.
pub fn extract_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;

    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix(name) {
            let value = value.strip_prefix('=')?;
            return Some(value.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cookie_single() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "didicafe_admin=abc123".parse().unwrap(),
        );
        assert_eq!(
            extract_cookie(&headers, "didicafe_admin"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn test_extract_cookie_multiple() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "other=foo; didicafe_admin=xyz789; third=bar".parse().unwrap(),
        );
        assert_eq!(
            extract_cookie(&headers, "didicafe_admin"),
            Some("xyz789".to_string())
        );
    }

    #[test]
    fn test_extract_cookie_missing() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "other=foo".parse().unwrap(),
        );
        assert_eq!(extract_cookie(&headers, "didicafe_admin"), None);
    }

    #[test]
    fn test_extract_cookie_no_header() {
        let headers = axum::http::HeaderMap::new();
        assert_eq!(extract_cookie(&headers, "didicafe_admin"), None);
    }
}
