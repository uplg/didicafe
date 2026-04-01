use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::request::Parts,
};
use tracing::warn;

use crate::AppState;
use crate::net::arp;
use crate::net::mac::is_valid_mac;
use super::error::AppError;

/// Cookie name for admin sessions.
pub const ADMIN_COOKIE_NAME: &str = "didicafe_admin";

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
        let api_path = parts.uri.path().starts_with("/api/");
        let session_id = extract_cookie(&parts.headers, ADMIN_COOKIE_NAME)
            .ok_or(AppError::Unauthorized { api_path })?;

        if !state.admin_sessions.validate(&session_id) {
            return Err(AppError::Unauthorized { api_path });
        }

        Ok(AdminSession)
    }
}

/// CSRF token extractor for admin forms.
///
/// Synchronizer Token Pattern:
/// 1. Token is generated and stored server-side in the session store
/// 2. Embedded as hidden field in forms by templates
/// 3. On POST, handler validates the submitted token against the server-side stored token
pub struct CsrfToken(pub String);

impl FromRequestParts<Arc<AppState>> for CsrfToken {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let api_path = parts.uri.path().starts_with("/api/");
        let session_id = extract_cookie(&parts.headers, ADMIN_COOKIE_NAME)
            .ok_or(AppError::Unauthorized { api_path })?;

        if !state.admin_sessions.validate(&session_id) {
            return Err(AppError::Unauthorized { api_path });
        }

        let csrf_token = state.admin_sessions.csrf_token(&session_id);
        Ok(CsrfToken(csrf_token))
    }
}

/// Client network identity extracted from the HTTP connection.
///
/// - `ip`: The client's IP address from the TCP socket (`ConnectInfo<SocketAddr>`).
/// - `mac`: The client's MAC address resolved from the ARP table.
pub struct ClientInfo {
    pub ip: IpAddr,
    pub mac: String,
}

impl<S: Send + Sync> FromRequestParts<S> for ClientInfo {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ConnectInfo(addr) = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_err(|e| {
                warn!("failed to extract ConnectInfo: {e}");
                AppError::Internal(anyhow::anyhow!("failed to determine client address"))
            })?;

        let ip = addr.ip();

        #[cfg(test)]
        let mac = mac_lookup_result(ip, if ip.is_loopback() {
            Some("02:00:00:00:00:01".to_string())
        } else {
            arp::lookup_mac(ip).await
        })?;

        #[cfg(not(test))]
        let mac = mac_lookup_result(ip, arp::lookup_mac(ip).await)?;

        Ok(ClientInfo { ip, mac })
    }
}

/// Extract a cookie value by name from request headers.
///
/// Matches exact cookie names only — `strip_prefix("{name}=")` prevents
/// prefix confusion attacks (e.g. `didicafe` matching `didicafe_admin`).
pub fn extract_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    let prefix = format!("{name}=");

    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }

    None
}

// -- Internal helpers --

#[cfg(test)]
fn mac_lookup_result(ip: IpAddr, mac: Option<String>) -> Result<String, AppError> {
    if ip.is_loopback() {
        return Ok("02:00:00:00:00:01".to_string());
    }
    validate_mac_or_err(ip, mac)
}

#[cfg(not(test))]
fn mac_lookup_result(ip: IpAddr, mac: Option<String>) -> Result<String, AppError> {
    validate_mac_or_err(ip, mac)
}

fn validate_mac_or_err(ip: IpAddr, mac: Option<String>) -> Result<String, AppError> {
    let mac = mac.ok_or_else(|| {
        warn!(%ip, "MAC lookup failed: client IP not found in ARP table");
        AppError::BadRequest("portal.error.device".to_string())
    })?;

    if !is_valid_mac(&mac) {
        warn!(%ip, %mac, "MAC address has invalid format");
        return Err(AppError::BadRequest("portal.error.device".to_string()));
    }

    Ok(mac)
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

    #[test]
    fn test_extract_cookie_prefix_attack() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "didicafe=evil; didicafe_admin=legit".parse().unwrap(),
        );
        assert_eq!(
            extract_cookie(&headers, "didicafe_admin"),
            Some("legit".to_string())
        );
        assert_eq!(
            extract_cookie(&headers, "didicafe"),
            Some("evil".to_string())
        );
    }
}
