mod models;

pub use models::*;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

// -- Shared SQL query constants --
// All token queries use the same column list and JOIN. All session queries use
// the same column list and JOIN. If the schema changes, update the constants
// below in one place.
//
// Effective-status reconciliation: a token or session marked `active` whose
// `expires_at` is past is *effectively* expired even though the DB row hasn't
// been transitioned yet (the cleanup ticker runs on a delay). Admin-facing
// list queries reflect that reconciliation in two places:
//   1. The WHERE clause filters rows by effective status (so an expired-by-
//      clock row falls out of "active"/"current" and into "expired").
//   2. The SELECT rewrites `status` to 'expired' for those rows so the
//      template's status badge matches what the user sees.
// Single-row lookups (TOKEN_BY_CODE / TOKEN_BY_ID) keep the raw status, since
// they feed enforcement logic (redemption, revocation) which acts on the
// stored state, not on the displayed state.

const TOKEN_BY_CODE: &str = "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes, p.name as plan_name \
     FROM token t JOIN plan p ON t.plan_id = p.id WHERE t.code = ?1";

const TOKEN_BY_ID: &str = "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes, p.name as plan_name \
     FROM token t JOIN plan p ON t.plan_id = p.id WHERE t.id = ?1";

const TOKEN_LIST: &str = "SELECT t.id, t.code, t.name, t.plan_id, \
            CASE WHEN t.status = 'active' AND t.expires_at <= datetime('now') \
                 THEN 'expired' ELSE t.status END as status, \
            t.created_at, t.redeemed_at, t.expires_at, p.duration_minutes, p.name as plan_name \
     FROM token t JOIN plan p ON t.plan_id = p.id ORDER BY t.created_at DESC";

/// Filters by raw status (used for 'unused' / 'revoked' — independent of expires_at).
const TOKEN_LIST_BY_STATUS: &str = "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes, p.name as plan_name \
     FROM token t JOIN plan p ON t.plan_id = p.id WHERE t.status = ?1 ORDER BY t.created_at DESC";

/// Effectively-active tokens: redeemed and not yet past expiration.
const TOKEN_LIST_ACTIVE: &str = "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes, p.name as plan_name \
     FROM token t JOIN plan p ON t.plan_id = p.id \
     WHERE t.status = 'active' AND t.expires_at > datetime('now') ORDER BY t.created_at DESC";

/// Effectively-expired tokens: explicitly expired, OR active-but-past-expiration.
const TOKEN_LIST_EXPIRED: &str = "SELECT t.id, t.code, t.name, t.plan_id, 'expired' as status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes, p.name as plan_name \
     FROM token t JOIN plan p ON t.plan_id = p.id \
     WHERE t.status = 'expired' \
        OR (t.status = 'active' AND t.expires_at <= datetime('now')) \
     ORDER BY t.created_at DESC";

/// Tokens that are unused or effectively active (the default admin view — what matters day-to-day).
const TOKEN_LIST_CURRENT: &str = "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes, p.name as plan_name \
     FROM token t JOIN plan p ON t.plan_id = p.id \
     WHERE t.status = 'unused' \
        OR (t.status = 'active' AND t.expires_at > datetime('now')) \
     ORDER BY t.created_at DESC";

const TOKEN_COUNT_ALL: &str = "SELECT COUNT(*) FROM token";
const TOKEN_COUNT_BY_STATUS: &str = "SELECT COUNT(*) FROM token WHERE status = ?1";
const TOKEN_COUNT_ACTIVE: &str =
    "SELECT COUNT(*) FROM token WHERE status = 'active' AND expires_at > datetime('now')";
const TOKEN_COUNT_EXPIRED: &str = "SELECT COUNT(*) FROM token \
     WHERE status = 'expired' OR (status = 'active' AND expires_at <= datetime('now'))";
const TOKEN_COUNT_CURRENT: &str = "SELECT COUNT(*) FROM token \
     WHERE status = 'unused' OR (status = 'active' AND expires_at > datetime('now'))";

const SESSION_ACTIVE: &str = "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name, p.name as plan_name \
     FROM session s JOIN token t ON s.token_id = t.id JOIN plan p ON t.plan_id = p.id WHERE s.status = 'active'";

/// Sessions that are admin-visible "live": still active and not yet past expiration.
/// The cleanup ticker uses `SESSION_ACTIVE` (raw) — it needs to *find* expired-
/// but-uncleaned rows in order to clean them up. The admin uses this — it must
/// not show users who are already disconnected at the firewall.
const SESSION_LIVE: &str = "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name, p.name as plan_name \
     FROM session s JOIN token t ON s.token_id = t.id JOIN plan p ON t.plan_id = p.id \
     WHERE s.status = 'active' AND s.expires_at > datetime('now')";

/// Live session lookup by MAC. We filter on `expires_at > now` AND order by id
/// DESC LIMIT 1 because there is a transient window after a session expires
/// where the row is still `status='active'` (the cleanup ticker hasn't run yet)
/// and the user may have already redeemed a fresh token. Without these guards,
/// SQLite would return the older expired-by-clock row in rowid order, and the
/// portal would treat the user as having no live session.
const SESSION_BY_MAC: &str = "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name, p.name as plan_name \
     FROM session s JOIN token t ON s.token_id = t.id JOIN plan p ON t.plan_id = p.id \
     WHERE s.mac_address = ?1 AND s.status = 'active' AND s.expires_at > datetime('now') \
     ORDER BY s.id DESC LIMIT 1";

const SESSION_BY_ID: &str = "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name, p.name as plan_name \
     FROM session s JOIN token t ON s.token_id = t.id JOIN plan p ON t.plan_id = p.id WHERE s.id = ?1";

const SESSION_BY_TOKEN: &str = "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name, p.name as plan_name \
     FROM session s JOIN token t ON s.token_id = t.id JOIN plan p ON t.plan_id = p.id WHERE s.token_id = ?1 AND s.status = 'active'";

/// Async SQLite database wrapper using sqlx.
///
/// Uses a connection pool internally. SQLite in WAL mode supports
/// concurrent readers and serialized writers. For a cafe workload
/// (~50 clients), this is more than sufficient.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open (or create) the database at the given path and configure it.
    pub async fn open(path: &str) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create database directory: {}", parent.display())
            })?;
        }

        let url = format!("sqlite:{path}?mode=rwc");
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .with_context(|| format!("failed to open database: {path}"))?;

        // Enable WAL mode and foreign keys
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;

        Ok(Self { pool })
    }

    /// Run schema migrations embedded in the binary.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(include_str!("../../migrations/001_initial_schema.sql"))
            .execute(&self.pool)
            .await
            .context("failed to run migration 001")?;
        sqlx::query(include_str!("../../migrations/002_audit_log.sql"))
            .execute(&self.pool)
            .await
            .context("failed to run migration 002")?;
        sqlx::query(include_str!("../../migrations/003_session_token_index.sql"))
            .execute(&self.pool)
            .await
            .context("failed to run migration 003")?;
        sqlx::query(include_str!("../../migrations/004_settings.sql"))
            .execute(&self.pool)
            .await
            .context("failed to run migration 004")?;
        Ok(())
    }

    // -- Plans --

    pub async fn list_plans(&self) -> Result<Vec<Plan>> {
        let plans = sqlx::query_as::<_, Plan>(
            "SELECT id, name, duration_minutes, price_ariary, active, created_at \
             FROM plan ORDER BY duration_minutes",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(plans)
    }

    /// List only active plans, ordered by duration. Used by the public plans page.
    pub async fn list_active_plans(&self) -> Result<Vec<Plan>> {
        let plans = sqlx::query_as::<_, Plan>(
            "SELECT id, name, duration_minutes, price_ariary, active, created_at \
             FROM plan WHERE active = 1 ORDER BY duration_minutes",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(plans)
    }

    pub async fn create_plan(
        &self,
        name: &str,
        duration_minutes: i64,
        price_ariary: i64,
    ) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO plan (name, duration_minutes, price_ariary) VALUES (?1, ?2, ?3)",
        )
        .bind(name)
        .bind(duration_minutes)
        .bind(price_ariary)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_plan(&self, id: i64) -> Result<Option<Plan>> {
        let plan = sqlx::query_as::<_, Plan>(
            "SELECT id, name, duration_minutes, price_ariary, active, created_at \
             FROM plan WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(plan)
    }

    pub async fn update_plan(
        &self,
        id: i64,
        name: &str,
        duration_minutes: i64,
        price_ariary: i64,
        active: bool,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE plan SET name = ?1, duration_minutes = ?2, price_ariary = ?3, active = ?4 \
             WHERE id = ?5",
        )
        .bind(name)
        .bind(duration_minutes)
        .bind(price_ariary)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -- Tokens --

    pub async fn create_token(&self, code: &str, name: Option<&str>, plan_id: i64) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO token (code, name, plan_id, status) VALUES (?1, ?2, ?3, 'unused')",
        )
        .bind(code)
        .bind(name)
        .bind(plan_id)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_token_by_code(&self, code: &str) -> Result<Option<Token>> {
        let token = sqlx::query_as::<_, Token>(TOKEN_BY_CODE)
            .bind(code)
            .fetch_optional(&self.pool)
            .await?;
        Ok(token)
    }

    pub async fn get_token_by_id(&self, id: i64) -> Result<Option<Token>> {
        let token = sqlx::query_as::<_, Token>(TOKEN_BY_ID)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(token)
    }

    pub async fn redeem_token(&self, token_id: i64, expires_at: &str) -> Result<()> {
        sqlx::query(
            "UPDATE token SET status = 'active', redeemed_at = datetime('now'), \
             expires_at = ?1 WHERE id = ?2",
        )
        .bind(expires_at)
        .bind(token_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn expire_token(&self, token_id: i64) -> Result<()> {
        sqlx::query("UPDATE token SET status = 'expired' WHERE id = ?1")
            .bind(token_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_tokens(&self, status_filter: Option<&str>) -> Result<Vec<Token>> {
        let tokens = match status_filter {
            Some(status) => {
                sqlx::query_as::<_, Token>(TOKEN_LIST_BY_STATUS)
                    .bind(status)
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                sqlx::query_as::<_, Token>(TOKEN_LIST)
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(tokens)
    }

    /// List tokens with server-side pagination.
    ///
    /// `status_filter`:
    ///   - `None` → all tokens
    ///   - `Some("current")` → unused + active (the default admin view)
    ///   - `Some("unused"|"active"|"expired"|"revoked")` → single status
    ///
    /// Returns `(tokens, total_count)` where `total_count` is the count
    /// **before** applying LIMIT/OFFSET (for computing page count).
    pub async fn list_tokens_paged(
        &self,
        status_filter: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<Token>, i64)> {
        let offset = (page - 1) * per_page;

        let (count_query, list_query, bind_status) = match status_filter {
            Some("current") => (TOKEN_COUNT_CURRENT, TOKEN_LIST_CURRENT, None),
            Some("active") => (TOKEN_COUNT_ACTIVE, TOKEN_LIST_ACTIVE, None),
            Some("expired") => (TOKEN_COUNT_EXPIRED, TOKEN_LIST_EXPIRED, None),
            Some(status) => (TOKEN_COUNT_BY_STATUS, TOKEN_LIST_BY_STATUS, Some(status)),
            None => (TOKEN_COUNT_ALL, TOKEN_LIST, None),
        };

        // 1. Count total matching rows
        let total: i64 = if let Some(status) = bind_status {
            let (count,): (i64,) = sqlx::query_as(count_query)
                .bind(status)
                .fetch_one(&self.pool)
                .await?;
            count
        } else {
            let (count,): (i64,) = sqlx::query_as(count_query).fetch_one(&self.pool).await?;
            count
        };

        // 2. Fetch the page
        let tokens = if let Some(status) = bind_status {
            sqlx::query_as::<_, Token>(&format!("{list_query} LIMIT ?2 OFFSET ?3"))
                .bind(status)
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query_as::<_, Token>(&format!("{list_query} LIMIT ? OFFSET ?"))
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
        };

        Ok((tokens, total))
    }

    pub async fn revoke_token(&self, token_id: i64) -> Result<()> {
        sqlx::query("UPDATE token SET status = 'revoked' WHERE id = ?1 AND status = 'unused'")
            .bind(token_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Sessions --

    /// Insert a new session row, transactionally disconnecting any prior
    /// `active` session for the same MAC.
    ///
    /// Without the disconnect step we get duplicates: when a session expires
    /// at the firewall (nftables timeout fires immediately) but the DB row is
    /// still `status='active'` waiting for the cleanup ticker (~30–40 s window),
    /// a fresh token redeem would INSERT a second `active` row for the same
    /// MAC. `get_session_by_mac` would then ramble between them and the user's
    /// success/status pages would return the stale row instead of the new one.
    pub async fn create_session(
        &self,
        token_id: i64,
        mac: &str,
        ip: &str,
        expires_at: &str,
    ) -> Result<i64> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE session SET status = 'disconnected' \
             WHERE mac_address = ?1 AND status = 'active'",
        )
        .bind(mac)
        .execute(&mut *tx)
        .await?;

        let result = sqlx::query(
            "INSERT INTO session (token_id, mac_address, ip_address, started_at, expires_at, status) \
             VALUES (?1, ?2, ?3, datetime('now'), ?4, 'active')",
        )
        .bind(token_id)
        .bind(mac)
        .bind(ip)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.last_insert_rowid())
    }

    /// Sessions whose row status is `active` — used by the cleanup ticker to find
    /// rows that need transitioning to `expired`. Includes rows whose `expires_at`
    /// is already in the past.
    pub async fn get_active_sessions(&self) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as::<_, Session>(SESSION_ACTIVE)
            .fetch_all(&self.pool)
            .await?;
        Ok(sessions)
    }

    /// Sessions that are still live from the user's perspective: `active` AND not
    /// yet past `expires_at`. Used by the admin UI so disconnected-but-uncleaned
    /// sessions don't appear as "active".
    pub async fn get_live_sessions(&self) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as::<_, Session>(SESSION_LIVE)
            .fetch_all(&self.pool)
            .await?;
        Ok(sessions)
    }

    pub async fn expire_session(&self, session_id: i64) -> Result<()> {
        sqlx::query("UPDATE session SET status = 'expired' WHERE id = ?1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn disconnect_session(&self, session_id: i64) -> Result<()> {
        sqlx::query("UPDATE session SET status = 'disconnected' WHERE id = ?1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Hard-delete expired and disconnected sessions older than `retention_days`.
    /// Also deletes associated tokens that have no active sessions.
    pub async fn purge_expired_sessions(&self, retention_days: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM session \
             WHERE status IN ('expired', 'disconnected') \
             AND started_at < datetime('now', ?1 || ' days')",
        )
        .bind(format!("-{retention_days}"))
        .execute(&self.pool)
        .await?;

        // Also purge orphan tokens (expired/revoked with no referencing session at all).
        // We must check ALL sessions (not just active) because expired/disconnected
        // sessions within the retention window still hold FK references.
        sqlx::query(
            "DELETE FROM token \
             WHERE status IN ('expired', 'revoked') \
             AND id NOT IN (SELECT token_id FROM session)",
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn get_session_by_mac(&self, mac: &str) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>(SESSION_BY_MAC)
            .bind(mac)
            .fetch_optional(&self.pool)
            .await?;
        Ok(session)
    }

    pub async fn get_session_by_id(&self, id: i64) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>(SESSION_BY_ID)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(session)
    }

    /// Get the active session for a given token, if any.
    /// Used by the session migration flow (MAC changed after WiFi reconnect).
    pub async fn get_active_session_by_token(&self, token_id: i64) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>(SESSION_BY_TOKEN)
            .bind(token_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(session)
    }

    // -- Audit Log --

    pub async fn audit_log(
        &self,
        admin_user: &str,
        action: &str,
        target_type: Option<&str>,
        target_id: Option<i64>,
        detail: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO audit_log (admin_user, action, target_type, target_id, detail) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(admin_user)
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_audit_log(&self, limit: i64) -> Result<Vec<AuditLogEntry>> {
        let entries = sqlx::query_as::<_, AuditLogEntry>(
            "SELECT id, timestamp, admin_user, action, target_type, target_id, detail \
             FROM audit_log ORDER BY timestamp DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    /// List audit log entries with server-side pagination.
    ///
    /// Returns `(entries, total_count)` where `total_count` is the full count
    /// before LIMIT/OFFSET (for computing page count).
    pub async fn get_audit_log_paged(
        &self,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<AuditLogEntry>, i64)> {
        let offset = (page - 1) * per_page;

        let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
            .fetch_one(&self.pool)
            .await?;

        let entries = sqlx::query_as::<_, AuditLogEntry>(
            "SELECT id, timestamp, admin_user, action, target_type, target_id, detail \
             FROM audit_log ORDER BY timestamp DESC LIMIT ? OFFSET ?",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok((entries, total))
    }

    /// Purge audit log entries older than `retention_days`.
    pub async fn purge_audit_log(&self, retention_days: i64) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM audit_log WHERE timestamp < datetime('now', ?1 || ' days')")
                .bind(format!("-{retention_days}"))
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    // -- Settings (key-value store) --

    /// Get a single setting by key. Returns `None` if not found.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM setting WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(v,)| v))
    }

    /// Set a setting (insert or update). Empty values are still stored;
    /// use `delete_setting` to remove a key entirely.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO setting (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all settings as key-value pairs.
    pub async fn get_all_settings(&self) -> Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM setting ORDER BY key")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    // -- Stats --

    pub async fn get_daily_stats(&self) -> Result<DailyStats> {
        let (tokens_sold,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM token \
             WHERE redeemed_at >= date('now') AND status IN ('active', 'expired')",
        )
        .fetch_one(&self.pool)
        .await?;

        let (active_sessions,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM session \
             WHERE status = 'active' AND expires_at > datetime('now')",
        )
        .fetch_one(&self.pool)
        .await?;

        let (revenue,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(p.price_ariary), 0) \
             FROM token t JOIN plan p ON t.plan_id = p.id \
             WHERE t.redeemed_at >= date('now') AND t.status IN ('active', 'expired')",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(DailyStats {
            tokens_sold,
            active_sessions,
            revenue_ariary: revenue,
        })
    }

    /// Returns per-day stats for the last 7 days (rolling window).
    ///
    /// Each entry contains the date, tokens sold, and revenue for that day.
    /// Days with no activity are included with zeroes.
    pub async fn get_weekly_stats(&self) -> Result<Vec<DayStats>> {
        // Generate the 7 date strings in application code, then query each.
        // SQLite doesn't have generate_series natively, so we use a CTE with
        // a recursive range. This runs as a single query.
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "WITH RECURSIVE dates(d) AS ( \
                 SELECT date('now', '-6 days') \
                 UNION ALL \
                 SELECT date(d, '+1 day') FROM dates WHERE d < date('now') \
             ) \
             SELECT \
                 dates.d, \
                 COALESCE(( \
                     SELECT COUNT(*) FROM token \
                     WHERE date(redeemed_at) = dates.d \
                       AND status IN ('active', 'expired') \
                 ), 0), \
                 COALESCE(( \
                     SELECT SUM(p.price_ariary) FROM token t \
                     JOIN plan p ON t.plan_id = p.id \
                     WHERE date(t.redeemed_at) = dates.d \
                       AND t.status IN ('active', 'expired') \
                 ), 0) \
             FROM dates ORDER BY dates.d",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(date, tokens_sold, revenue_ariary)| DayStats {
                date,
                tokens_sold,
                revenue_ariary,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a fresh in-memory database for testing.
    async fn test_db() -> Database {
        let db = Database::open(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        db
    }

    // -- Plan CRUD --

    #[tokio::test]
    async fn test_create_and_list_plans() {
        let db = test_db().await;

        let id = db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        assert!(id > 0);

        let plans = db.list_plans().await.unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].name, "1h WiFi");
        assert_eq!(plans[0].duration_minutes, 60);
        assert_eq!(plans[0].price_ariary, 1000);
        assert!(plans[0].active);
    }

    #[tokio::test]
    async fn test_get_plan() {
        let db = test_db().await;
        let id = db.create_plan("2h WiFi", 120, 2000).await.unwrap();

        let plan = db.get_plan(id).await.unwrap().unwrap();
        assert_eq!(plan.name, "2h WiFi");

        let none = db.get_plan(9999).await.unwrap();
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn test_update_plan() {
        let db = test_db().await;
        let id = db.create_plan("Old Name", 60, 1000).await.unwrap();

        let updated = db
            .update_plan(id, "New Name", 120, 2000, false)
            .await
            .unwrap();
        assert!(updated);

        let plan = db.get_plan(id).await.unwrap().unwrap();
        assert_eq!(plan.name, "New Name");
        assert_eq!(plan.duration_minutes, 120);
        assert!(!plan.active);

        let not_updated = db.update_plan(9999, "x", 1, 1, true).await.unwrap();
        assert!(!not_updated);
    }

    // -- Token CRUD --

    #[tokio::test]
    async fn test_create_and_get_token() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();

        let token_id = db
            .create_token("DIDI-ABCD-EF23", None, plan_id)
            .await
            .unwrap();
        assert!(token_id > 0);

        let token = db
            .get_token_by_code("DIDI-ABCD-EF23")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(token.status, TokenStatus::Unused);
        assert_eq!(token.duration_minutes, 60);

        let token2 = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token2.code, "DIDI-ABCD-EF23");
    }

    #[tokio::test]
    async fn test_redeem_and_expire_token() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let token_id = db
            .create_token("DIDI-TEST-CODE", None, plan_id)
            .await
            .unwrap();

        db.redeem_token(token_id, "2026-12-31 23:59:59")
            .await
            .unwrap();
        let token = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token.status, TokenStatus::Active);
        assert!(token.redeemed_at.is_some());

        db.expire_token(token_id).await.unwrap();
        let token = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token.status, TokenStatus::Expired);
    }

    #[tokio::test]
    async fn test_revoke_token() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let token_id = db
            .create_token("DIDI-REVO-KEXX", None, plan_id)
            .await
            .unwrap();

        db.revoke_token(token_id).await.unwrap();
        let token = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token.status, TokenStatus::Revoked);
    }

    #[tokio::test]
    async fn test_list_tokens_with_filter() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();

        let id1 = db
            .create_token("DIDI-AAAA-BBBB", None, plan_id)
            .await
            .unwrap();
        db.create_token("DIDI-CCCC-DDDD", None, plan_id)
            .await
            .unwrap();
        db.revoke_token(id1).await.unwrap();

        let all = db.list_tokens(None).await.unwrap();
        assert_eq!(all.len(), 2);

        let unused = db.list_tokens(Some("unused")).await.unwrap();
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].code, "DIDI-CCCC-DDDD");

        let revoked = db.list_tokens(Some("revoked")).await.unwrap();
        assert_eq!(revoked.len(), 1);
        assert_eq!(revoked[0].code, "DIDI-AAAA-BBBB");
    }

    // -- Session CRUD --

    #[tokio::test]
    async fn test_create_and_get_session() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let token_id = db
            .create_token("DIDI-SESS-TEST", None, plan_id)
            .await
            .unwrap();

        let session_id = db
            .create_session(
                token_id,
                "AA:BB:CC:DD:EE:FF",
                "10.10.0.5",
                "2099-12-31 23:59:59",
            )
            .await
            .unwrap();
        assert!(session_id > 0);

        let session = db
            .get_session_by_mac("AA:BB:CC:DD:EE:FF")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session.ip_address, "10.10.0.5");
        assert_eq!(session.status, SessionStatus::Active);
    }

    #[tokio::test]
    async fn test_active_sessions() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let t1 = db
            .create_token("DIDI-ACT1-TEST", None, plan_id)
            .await
            .unwrap();
        let t2 = db
            .create_token("DIDI-ACT2-TEST", None, plan_id)
            .await
            .unwrap();

        db.create_session(t1, "AA:11:22:33:44:55", "10.10.0.1", "2099-12-31 23:59:59")
            .await
            .unwrap();
        let s2 = db
            .create_session(t2, "BB:11:22:33:44:55", "10.10.0.2", "2099-12-31 23:59:59")
            .await
            .unwrap();

        let sessions = db.get_active_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);

        db.disconnect_session(s2).await.unwrap();
        let sessions = db.get_active_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_expire_session() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let token_id = db
            .create_token("DIDI-EXPR-TEST", None, plan_id)
            .await
            .unwrap();
        let session_id = db
            .create_session(
                token_id,
                "CC:DD:EE:FF:00:11",
                "10.10.0.3",
                "2099-12-31 23:59:59",
            )
            .await
            .unwrap();

        db.expire_session(session_id).await.unwrap();
        let session = db.get_session_by_mac("CC:DD:EE:FF:00:11").await.unwrap();
        assert!(session.is_none()); // get_session_by_mac only returns active
    }

    /// Re-redeeming a token after expiry must not leave two `active` rows for
    /// the same MAC. Reproduces the bug where `get_session_by_mac` would return
    /// the stale row (older rowid, `expires_at` in the past) instead of the
    /// freshly-created live one.
    #[tokio::test]
    async fn test_create_session_disconnects_prior_active_row_same_mac() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let mac = "AA:BB:CC:DD:EE:FF";

        // Old session: still `active` in DB, but expired by clock (mimics the
        // 30–40 s window between nftables timeout and the DB cleanup ticker).
        let stale_token = db
            .create_token("DIDI-STAL-EOLD", None, plan_id)
            .await
            .unwrap();
        let stale_id = db
            .create_session(stale_token, mac, "10.10.0.5", "2020-01-01 00:00:00")
            .await
            .unwrap();

        // User redeems a fresh token while the stale row is still 'active'.
        let fresh_token = db
            .create_token("DIDI-FRSH-NEWX", None, plan_id)
            .await
            .unwrap();
        let fresh_id = db
            .create_session(fresh_token, mac, "10.10.0.5", "2099-12-31 23:59:59")
            .await
            .unwrap();
        assert_ne!(stale_id, fresh_id);

        // Only the fresh row should be `active` after the second create_session.
        let active = db.get_active_sessions().await.unwrap();
        let active_for_mac: Vec<_> = active.iter().filter(|s| s.mac_address == mac).collect();
        assert_eq!(
            active_for_mac.len(),
            1,
            "expected exactly one active row for MAC"
        );
        assert_eq!(active_for_mac[0].id, fresh_id);

        // get_session_by_mac must surface the fresh row.
        let by_mac = db.get_session_by_mac(mac).await.unwrap().unwrap();
        assert_eq!(by_mac.id, fresh_id);
        assert!(by_mac.remaining_seconds() > 0);
    }

    /// Defense-in-depth: even if two `active` rows somehow coexist (e.g. a
    /// pre-fix data state restored from backup), `get_session_by_mac` must
    /// return only the live one — not the stale one with `expires_at` in
    /// the past.
    #[tokio::test]
    async fn test_get_session_by_mac_filters_past_expires_at() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let mac = "BB:CC:DD:EE:FF:00";

        // Bypass create_session's transactional cleanup by INSERTing rows
        // directly — simulates a corrupted state where two `active` rows
        // exist for the same MAC.
        let t1 = db
            .create_token("DIDI-TWOA-CTV1", None, plan_id)
            .await
            .unwrap();
        let t2 = db
            .create_token("DIDI-TWOA-CTV2", None, plan_id)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO session (token_id, mac_address, ip_address, started_at, expires_at, status) \
             VALUES (?1, ?2, '10.0.0.1', datetime('now'), '2020-01-01 00:00:00', 'active')",
        )
        .bind(t1)
        .bind(mac)
        .execute(&db.pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO session (token_id, mac_address, ip_address, started_at, expires_at, status) \
             VALUES (?1, ?2, '10.0.0.1', datetime('now'), '2099-12-31 23:59:59', 'active')",
        )
        .bind(t2)
        .bind(mac)
        .execute(&db.pool)
        .await
        .unwrap();

        let by_mac = db.get_session_by_mac(mac).await.unwrap().unwrap();
        assert_eq!(
            by_mac.token_id, t2,
            "expected the row whose expires_at is in the future"
        );
    }

    // -- Session remaining_seconds --

    #[test]
    fn test_remaining_seconds_future() {
        let session = Session {
            id: 1,
            token_id: 1,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            ip_address: "10.0.0.1".to_string(),
            started_at: "2026-01-01 00:00:00".to_string(),
            expires_at: "2099-12-31 23:59:59".to_string(),
            status: SessionStatus::Active,
            token_code: None,
            token_name: None,
            plan_name: None,
        };
        assert!(session.remaining_seconds() > 0);
    }

    #[test]
    fn test_remaining_seconds_past() {
        let session = Session {
            id: 1,
            token_id: 1,
            mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
            token_code: None,
            token_name: None,
            plan_name: None,
            ip_address: "10.0.0.1".to_string(),
            started_at: "2020-01-01 00:00:00".to_string(),
            expires_at: "2020-01-01 01:00:00".to_string(),
            status: SessionStatus::Active,
        };
        assert_eq!(session.remaining_seconds(), 0);
    }

    #[test]
    fn test_remaining_seconds_bad_format() {
        let session = Session {
            id: 1,
            token_id: 1,
            mac_address: String::new(),
            ip_address: String::new(),
            started_at: String::new(),
            expires_at: "not-a-date".to_string(),
            status: SessionStatus::Active,
            token_code: None,
            token_name: None,
            plan_name: None,
        };
        assert_eq!(session.remaining_seconds(), 0);
    }

    // -- Type-safe enums --

    #[test]
    fn test_token_status_roundtrip() {
        for s in TokenStatus::ALL {
            let status = TokenStatus::try_from_str(s).unwrap();
            assert_eq!(&status.to_string(), *s);
        }
        assert!(TokenStatus::try_from_str("bogus").is_none());
    }

    #[test]
    fn test_session_status_roundtrip() {
        for s in &["active", "expired", "disconnected"] {
            let status = SessionStatus::try_from_str(s).unwrap();
            assert_eq!(&status.to_string(), *s);
        }
        assert!(SessionStatus::try_from_str("bogus").is_none());
    }

    // -- Daily stats --

    #[tokio::test]
    async fn test_daily_stats_empty() {
        let db = test_db().await;
        let stats = db.get_daily_stats().await.unwrap();
        assert_eq!(stats.tokens_sold, 0);
        assert_eq!(stats.active_sessions, 0);
        assert_eq!(stats.revenue_ariary, 0);
    }

    // -- list_active_plans --

    #[tokio::test]
    async fn test_list_active_plans() {
        let db = test_db().await;

        // Create plans: two active, one inactive
        db.create_plan("30min WiFi", 30, 500).await.unwrap();
        db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        let id3 = db.create_plan("2h WiFi", 120, 2000).await.unwrap();
        db.update_plan(id3, "2h WiFi", 120, 2000, false)
            .await
            .unwrap();

        let active = db.list_active_plans().await.unwrap();
        assert_eq!(active.len(), 2);
        // Ordered by duration_minutes ASC
        assert_eq!(active[0].name, "30min WiFi");
        assert_eq!(active[1].name, "1h WiFi");
        // All returned plans are active
        assert!(active.iter().all(|p| p.active));
    }

    #[tokio::test]
    async fn test_list_active_plans_empty() {
        let db = test_db().await;
        let active = db.list_active_plans().await.unwrap();
        assert!(active.is_empty());
    }

    // -- Settings --

    #[tokio::test]
    async fn test_get_setting_missing() {
        let db = test_db().await;
        let val = db.get_setting("nonexistent").await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn test_set_and_get_setting() {
        let db = test_db().await;
        db.set_setting("theme_color", "#ff0000").await.unwrap();
        let val = db.get_setting("theme_color").await.unwrap();
        assert_eq!(val.as_deref(), Some("#ff0000"));
    }

    #[tokio::test]
    async fn test_set_setting_upsert() {
        let db = test_db().await;
        db.set_setting("cafe_name", "OldName").await.unwrap();
        db.set_setting("cafe_name", "NewName").await.unwrap();
        let val = db.get_setting("cafe_name").await.unwrap();
        assert_eq!(val.as_deref(), Some("NewName"));
    }

    #[tokio::test]
    async fn test_get_all_settings() {
        let db = test_db().await;
        db.set_setting("b_key", "beta").await.unwrap();
        db.set_setting("a_key", "alpha").await.unwrap();
        let all = db.get_all_settings().await.unwrap();
        assert_eq!(all.len(), 2);
        // Sorted by key
        assert_eq!(all[0].0, "a_key");
        assert_eq!(all[1].0, "b_key");
    }

    // -- Weekly stats --

    #[tokio::test]
    async fn test_weekly_stats_empty() {
        let db = test_db().await;
        let days = db.get_weekly_stats().await.unwrap();
        assert_eq!(days.len(), 7);
        // All days should have zero tokens and revenue
        for day in &days {
            assert_eq!(day.tokens_sold, 0);
            assert_eq!(day.revenue_ariary, 0);
        }
        // Dates should be sorted ascending
        for i in 1..days.len() {
            assert!(days[i].date > days[i - 1].date);
        }
    }

    #[tokio::test]
    async fn test_weekly_stats_with_data() {
        let db = test_db().await;

        // Create a plan and redeem a token today
        let plan_id = db.create_plan("1h WiFi", 60, 1000).await.unwrap();
        let token_id = db
            .create_token("TEST-CODE-0001", Some("test"), plan_id)
            .await
            .unwrap();
        db.redeem_token(token_id, "2099-12-31 23:59:59")
            .await
            .unwrap();

        let days = db.get_weekly_stats().await.unwrap();
        assert_eq!(days.len(), 7);

        // Last day (today) should have 1 token sold and 1000 Ar revenue
        let today = &days[6];
        assert_eq!(today.tokens_sold, 1);
        assert_eq!(today.revenue_ariary, 1000);

        // All other days should be zero
        for day in &days[..6] {
            assert_eq!(day.tokens_sold, 0);
            assert_eq!(day.revenue_ariary, 0);
        }
    }

    // -- Paginated token listing --

    #[tokio::test]
    async fn test_list_tokens_paged_empty() {
        let db = test_db().await;
        let (tokens, total) = db.list_tokens_paged(None, 1, 50).await.unwrap();
        assert!(tokens.is_empty());
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_list_tokens_paged_all() {
        let db = test_db().await;
        let plan_id = db.create_plan("WiFi 1h", 60, 1000).await.unwrap();
        for i in 0..5 {
            db.create_token(&format!("CODE-{i:04}"), Some("test"), plan_id)
                .await
                .unwrap();
        }
        let (tokens, total) = db.list_tokens_paged(None, 1, 50).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(tokens.len(), 5);
    }

    #[tokio::test]
    async fn test_list_tokens_paged_limit_offset() {
        let db = test_db().await;
        let plan_id = db.create_plan("WiFi 1h", 60, 1000).await.unwrap();
        for i in 0..5 {
            db.create_token(&format!("CODE-{i:04}"), Some("test"), plan_id)
                .await
                .unwrap();
        }

        // Page 1 of 2 (per_page=3)
        let (page1, total) = db.list_tokens_paged(None, 1, 3).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page1.len(), 3);

        // Page 2 of 2
        let (page2, total) = db.list_tokens_paged(None, 2, 3).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page2.len(), 2);

        // Page 3 — beyond data
        let (page3, total) = db.list_tokens_paged(None, 3, 3).await.unwrap();
        assert_eq!(total, 5);
        assert!(page3.is_empty());
    }

    #[tokio::test]
    async fn test_list_tokens_paged_current_filter() {
        let db = test_db().await;
        let plan_id = db.create_plan("WiFi 1h", 60, 1000).await.unwrap();

        // Create 3 unused tokens
        for i in 0..3 {
            db.create_token(&format!("UNUSED-{i:04}"), Some("test"), plan_id)
                .await
                .unwrap();
        }
        // Create 1 active token (redeem it)
        let active_id = db
            .create_token("ACTIVE-0001", Some("test"), plan_id)
            .await
            .unwrap();
        db.redeem_token(active_id, "2099-12-31 23:59:59")
            .await
            .unwrap();

        // Create 1 expired token
        let expired_id = db
            .create_token("EXPIRED-001", Some("test"), plan_id)
            .await
            .unwrap();
        db.redeem_token(expired_id, "2020-01-01 00:00:00")
            .await
            .unwrap();
        db.expire_token(expired_id).await.unwrap();

        // "current" = unused + active → 4 tokens
        let (tokens, total) = db.list_tokens_paged(Some("current"), 1, 50).await.unwrap();
        assert_eq!(total, 4);
        assert_eq!(tokens.len(), 4);
        for t in &tokens {
            assert!(t.status == "unused" || t.status == "active");
        }
    }

    #[tokio::test]
    async fn test_list_tokens_paged_single_status_filter() {
        let db = test_db().await;
        let plan_id = db.create_plan("WiFi 1h", 60, 1000).await.unwrap();

        db.create_token("UNUSED-0001", Some("u"), plan_id)
            .await
            .unwrap();
        let active_id = db
            .create_token("ACTIVE-0001", Some("a"), plan_id)
            .await
            .unwrap();
        db.redeem_token(active_id, "2099-12-31 23:59:59")
            .await
            .unwrap();

        // Filter: unused only
        let (unused, total) = db.list_tokens_paged(Some("unused"), 1, 50).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].status, "unused");

        // Filter: active only
        let (active, total) = db.list_tokens_paged(Some("active"), 1, 50).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].status, "active");

        // Filter: expired → 0
        let (expired, total) = db.list_tokens_paged(Some("expired"), 1, 50).await.unwrap();
        assert_eq!(total, 0);
        assert!(expired.is_empty());
    }

    // -- Effective expiration (admin reconciliation) --

    /// A session whose row is still `status='active'` but whose `expires_at` is
    /// in the past must be hidden from the admin's "active" list (the user is
    /// already disconnected at the firewall) — but the cleanup ticker must
    /// still see it via `get_active_sessions()` so it can transition the row.
    #[tokio::test]
    async fn test_live_sessions_excludes_past_expires_at() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let live_tok = db
            .create_token("LIVE-AAAA-AAAA", None, plan_id)
            .await
            .unwrap();
        let dead_tok = db
            .create_token("DEAD-BBBB-BBBB", None, plan_id)
            .await
            .unwrap();

        db.create_session(
            live_tok,
            "AA:AA:AA:AA:AA:AA",
            "10.0.0.1",
            "2099-12-31 23:59:59",
        )
        .await
        .unwrap();
        db.create_session(
            dead_tok,
            "BB:BB:BB:BB:BB:BB",
            "10.0.0.2",
            "2020-01-01 00:00:00",
        )
        .await
        .unwrap();

        // Cleanup ticker sees both rows (it needs to transition the dead one).
        let active = db.get_active_sessions().await.unwrap();
        assert_eq!(active.len(), 2);

        // Admin sees only the live one.
        let live = db.get_live_sessions().await.unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].mac_address, "AA:AA:AA:AA:AA:AA");
    }

    /// Daily stats' `active_sessions` count must match what the admin sees as
    /// live, not the raw DB row count.
    #[tokio::test]
    async fn test_daily_stats_excludes_past_expires_at() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let live_tok = db
            .create_token("LIVE-CCCC-CCCC", None, plan_id)
            .await
            .unwrap();
        let dead_tok = db
            .create_token("DEAD-DDDD-DDDD", None, plan_id)
            .await
            .unwrap();

        db.create_session(
            live_tok,
            "AA:AA:AA:AA:AA:AA",
            "10.0.0.1",
            "2099-12-31 23:59:59",
        )
        .await
        .unwrap();
        db.create_session(
            dead_tok,
            "BB:BB:BB:BB:BB:BB",
            "10.0.0.2",
            "2020-01-01 00:00:00",
        )
        .await
        .unwrap();

        let stats = db.get_daily_stats().await.unwrap();
        assert_eq!(stats.active_sessions, 1);
    }

    /// A token with `status='active'` but past `expires_at` is *effectively*
    /// expired: it must drop out of "current" and "active" filters, appear in
    /// "expired", and display as 'expired' in the unfiltered "all" view.
    #[tokio::test]
    async fn test_token_filters_use_effective_expiration() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();

        db.create_token("UNUSED-0001", None, plan_id).await.unwrap();

        let live_id = db
            .create_token("LIVE-AAAA-AAAA", None, plan_id)
            .await
            .unwrap();
        db.redeem_token(live_id, "2099-12-31 23:59:59")
            .await
            .unwrap();

        // Effectively expired: redeemed in the past, ticker hasn't run yet.
        let stale_id = db
            .create_token("STALE-BBBB-BB", None, plan_id)
            .await
            .unwrap();
        db.redeem_token(stale_id, "2020-01-01 00:00:00")
            .await
            .unwrap();

        // Already-transitioned expired token (ticker did run).
        let cleaned_id = db
            .create_token("CLEAN-CCCC-CC", None, plan_id)
            .await
            .unwrap();
        db.redeem_token(cleaned_id, "2020-01-01 00:00:00")
            .await
            .unwrap();
        db.expire_token(cleaned_id).await.unwrap();

        // "current" = unused + effectively-active → 2 (UNUSED + LIVE)
        let (cur, total) = db.list_tokens_paged(Some("current"), 1, 50).await.unwrap();
        assert_eq!(total, 2);
        assert!(cur.iter().any(|t| t.code == "UNUSED-0001"));
        assert!(cur.iter().any(|t| t.code == "LIVE-AAAA-AAAA"));

        // "active" = effectively-active only → 1 (LIVE)
        let (act, total) = db.list_tokens_paged(Some("active"), 1, 50).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(act[0].code, "LIVE-AAAA-AAAA");

        // "expired" = explicitly expired OR active-but-past → 2 (STALE + CLEAN)
        let (exp, total) = db.list_tokens_paged(Some("expired"), 1, 50).await.unwrap();
        assert_eq!(total, 2);
        for t in &exp {
            assert_eq!(t.status, TokenStatus::Expired);
        }

        // "all" view: STALE row's displayed status is rewritten to 'expired'.
        let (all, total) = db.list_tokens_paged(None, 1, 50).await.unwrap();
        assert_eq!(total, 4);
        let stale = all.iter().find(|t| t.code == "STALE-BBBB-BB").unwrap();
        assert_eq!(stale.status, TokenStatus::Expired);
    }

    // -- Paginated audit log --

    #[tokio::test]
    async fn test_get_audit_log_paged_empty() {
        let db = test_db().await;
        let (entries, total) = db.get_audit_log_paged(1, 50).await.unwrap();
        assert!(entries.is_empty());
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn test_get_audit_log_paged_with_data() {
        let db = test_db().await;
        for i in 0..5 {
            db.audit_log("admin", &format!("action_{i}"), None, None, None)
                .await
                .unwrap();
        }
        let (entries, total) = db.get_audit_log_paged(1, 50).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(entries.len(), 5);
    }

    #[tokio::test]
    async fn test_get_audit_log_paged_limit_offset() {
        let db = test_db().await;
        for i in 0..5 {
            db.audit_log("admin", &format!("action_{i}"), None, None, None)
                .await
                .unwrap();
        }

        // Page 1 of 2 (per_page=3)
        let (page1, total) = db.get_audit_log_paged(1, 3).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page1.len(), 3);

        // Page 2 of 2
        let (page2, total) = db.get_audit_log_paged(2, 3).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page2.len(), 2);

        // Page 3 — beyond data
        let (page3, total) = db.get_audit_log_paged(3, 3).await.unwrap();
        assert_eq!(total, 5);
        assert!(page3.is_empty());
    }
}
