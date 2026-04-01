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

    // 1. Add MAC+IP to nftables with timeout (fail here = token not consumed)
    state.firewall.authorize_client(mac, ip, timeout_secs).await?;

    // 2. Create session record
    let session_id = state.db.create_session(token_id, mac, ip, &expires_str).await?;

    // 3. Mark token as active (consumed — no going back)
    state.db.redeem_token(token_id, &expires_str).await?;

    info!(
        session_id,
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

    state.firewall.deauthorize_client(&session.mac_address, &session.ip_address).await?;
    state.db.disconnect_session(session_id).await?;
    info!(session_id, "session disconnected");

    Ok(())
}

/// Migrate an active session to a new MAC address.
///
/// This handles the case where a client reconnects with a different MAC
/// (Android 10+ / iOS 14+ MAC randomization, or WiFi outage causing
/// the device to generate a new random MAC).
///
/// Flow:
/// 1. Look up the active session for this token
/// 2. Verify it still has remaining time
/// 3. Deauthorize the old MAC from nftables
/// 4. Close the old session (mark as disconnected)
/// 5. Authorize the new MAC with the remaining time
/// 6. Create a new session record
///
/// If the new MAC is the same as the old one (client reconnected without
/// MAC change), this still works — it refreshes the nftables timeout.
pub async fn migrate_session(
    state: &Arc<AppState>,
    token_id: i64,
    new_mac: &str,
    new_ip: &str,
) -> Result<i64> {
    let old_session = state.db.get_active_session_by_token(token_id).await?
        .ok_or_else(|| anyhow::anyhow!("no active session for token {token_id}"))?;

    let remaining = old_session.remaining_seconds();
    if remaining <= 0 {
        anyhow::bail!("session for token {token_id} has already expired");
    }

    // 1. Deauthorize old MAC+IP (ignore errors — may already be gone after outage)
    if let Err(e) = state.firewall.deauthorize_client(&old_session.mac_address, &old_session.ip_address).await {
        warn!(
            session_id = old_session.id,
            old_mac = %old_session.mac_address,
            "deauthorize old client failed (may already be expired): {e}"
        );
    }

    // 2. Close old session
    state.db.disconnect_session(old_session.id).await?;

    // 3. Authorize new MAC+IP with remaining time
    let timeout_secs = remaining as u64;
    state.firewall.authorize_client(new_mac, new_ip, timeout_secs).await?;

    // 4. Create new session with the original expiry time
    let session_id = state.db.create_session(
        token_id,
        new_mac,
        new_ip,
        &old_session.expires_at,
    ).await?;

    info!(
        old_session_id = old_session.id,
        new_session_id = session_id,
        old_mac = %old_session.mac_address,
        new_mac = %new_mac,
        remaining_seconds = remaining,
        "session migrated to new MAC"
    );

    Ok(session_id)
}

/// Periodic cleanup: expire sessions whose time has elapsed, purge old data.
/// Called by a tokio interval task. Cancels when `shutdown` is triggered.
pub async fn cleanup_ticker(state: Arc<AppState>, shutdown: CancellationToken) {
    let interval = state.config.session.cleanup_interval_seconds;
    let grace = state.config.session.grace_period_seconds;

    let mut ticker = tokio::time::interval(
        tokio::time::Duration::from_secs(interval)
    );

    // Track when we last did a full purge (once per hour is enough)
    let mut last_purge = tokio::time::Instant::now();
    let purge_interval = tokio::time::Duration::from_secs(3600); // 1 hour

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("cleanup ticker shutting down");
                return;
            }
            _ = ticker.tick() => {}
        }

        // 1. Expire sessions whose time has elapsed
        match state.db.get_active_sessions().await {
            Ok(sessions) => {
                for session in sessions {
                    let remaining = session.remaining_seconds();
                    if remaining <= -grace {
                        // Session has expired past the grace period — remove from firewall
                        if let Err(e) = state.firewall.deauthorize_client(&session.mac_address, &session.ip_address).await {
                            warn!(
                                session_id = session.id,
                                mac = %session.mac_address,
                                "deauthorize failed, will retry next cycle: {e}"
                            );
                            continue; // Do NOT mark as expired — client stays authorized
                        }
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
                            "session expired (cleanup)"
                        );
                    }
                }
            }
            Err(e) => {
                warn!("cleanup: failed to fetch sessions: {e}");
            }
        }

        // 2. Periodic purge of old data (once per hour)
        if last_purge.elapsed() >= purge_interval {
            let retention = state.config.session.retention_days;
            match state.db.purge_expired_sessions(retention).await {
                Ok(count) if count > 0 => {
                    info!(count, retention, "purged expired sessions");
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("failed to purge expired sessions: {e}");
                }
            }

            // Also purge old audit log entries
            let audit_retention = state.config.session.audit_retention_days;
            match state.db.purge_audit_log(audit_retention).await {
                Ok(count) if count > 0 => {
                    info!(count, audit_retention, "purged old audit log entries");
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("failed to purge audit log: {e}");
                }
            }

            last_purge = tokio::time::Instant::now();
        }
    }
}
