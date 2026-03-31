use std::sync::Arc;
use anyhow::Result;
use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::AppState;

/// Create a new session: authorize MAC in firewall first, then redeem token and persist.
///
/// Order matters for fault tolerance:
/// 1. Authorize MAC in nftables (if this fails, token stays unused — customer can retry)
/// 2. Create session record in DB
/// 3. Redeem token (marks it as consumed)
///
/// This way, a firewall failure doesn't consume a paid token.
pub async fn create_session(
    state: &Arc<AppState>,
    token_id: i64,
    duration_minutes: i64,
    mac: &str,
    ip: &str,
) -> Result<i64> {
    let expires_at = Utc::now()
        + chrono::Duration::minutes(duration_minutes);
    let expires_str = expires_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let timeout_secs = (duration_minutes * 60) as u64;

    // 1. Add MAC to nftables with timeout (fail here = token not consumed)
    state.firewall.authorize_mac(mac, timeout_secs).await?;

    // 2. Create session record
    let session_id = state.db.create_session(token_id, mac, ip, &expires_str).await?;

    // 3. Mark token as active (consumed — no going back)
    state.db.redeem_token(token_id, &expires_str).await?;

    info!(
        session_id,
        mac,
        duration_minutes,
        %expires_at,
        "session created"
    );

    Ok(session_id)
}

/// Force-disconnect a session: remove MAC from nftables, update DB.
pub async fn disconnect(state: &Arc<AppState>, session_id: i64) -> Result<()> {
    let session = state.db.get_session_by_id(session_id).await?
        .ok_or_else(|| anyhow::anyhow!("session {session_id} not found"))?;

    state.firewall.deauthorize_mac(&session.mac_address).await?;
    state.db.disconnect_session(session_id).await?;
    info!(session_id, mac = %session.mac_address, "session disconnected");

    Ok(())
}

/// Periodic cleanup: expire sessions whose time has elapsed.
/// Called by a tokio interval task. Cancels when `shutdown` is triggered.
pub async fn cleanup_ticker(state: Arc<AppState>, shutdown: CancellationToken) {
    let interval = state.config.session.cleanup_interval_seconds;
    let grace = state.config.session.grace_period_seconds;

    let mut ticker = tokio::time::interval(
        tokio::time::Duration::from_secs(interval)
    );

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("cleanup ticker shutting down");
                return;
            }
            _ = ticker.tick() => {}
        }

        match state.db.get_active_sessions().await {
            Ok(sessions) => {
                for session in sessions {
                    let remaining = session.remaining_seconds();
                    if remaining <= -grace {
                        // Session has expired past the grace period — remove from firewall
                        state.firewall.deauthorize_mac(&session.mac_address).await.ok();
                        if let Err(e) = state.db.expire_session(session.id).await {
                            warn!(session_id = session.id, "failed to expire session: {e}");
                            continue;
                        }
                        // Also expire the associated token
                        if let Err(e) = state.db.expire_token(session.token_id).await {
                            warn!(token_id = session.token_id, "failed to expire token: {e}");
                        }
                        info!(
                            session_id = session.id,
                            mac = %session.mac_address,
                            "session expired (cleanup)"
                        );
                    }
                }
            }
            Err(e) => {
                warn!("cleanup: failed to fetch sessions: {e}");
            }
        }
    }
}
