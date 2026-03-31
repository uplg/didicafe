//! Captive Portal Detection (CPD) probe handlers.
//!
//! Modern OSes probe specific HTTP URLs to detect captive portals:
//! - Apple (iOS/macOS): `GET /hotspot-detect.html`
//! - Android: `GET /generate_204`
//! - Windows: `GET /connecttest.txt`
//! - Linux (NetworkManager/GNOME): `GET /check_network_status.txt`
//!
//! nftables DNATs all HTTP (port 80) traffic from unauthenticated clients
//! to our server. We respond with a redirect to the portal page, which
//! triggers the OS to open its built-in captive portal browser.

use std::sync::Arc;
use axum::{Router, response::Redirect, routing::get};

use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Apple CNA (Captive Network Assistant)
        .route("/hotspot-detect.html", get(redirect_to_portal))
        // Android connectivity check
        .route("/generate_204", get(redirect_to_portal))
        // Windows NCSI (Network Connectivity Status Indicator)
        .route("/connecttest.txt", get(redirect_to_portal))
        // Linux NetworkManager / GNOME
        .route("/check_network_status.txt", get(redirect_to_portal))
}

/// All CPD probes redirect to the portal page via 302.
///
/// 302 (not 301) because we don't want the OS to cache the redirect
/// permanently — after authentication, the probes should succeed normally
/// (though they'll go through the real internet, not our server).
async fn redirect_to_portal() -> Redirect {
    Redirect::to("/portal")
}
