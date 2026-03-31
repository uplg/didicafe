mod models;

pub use models::*;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

// -- Shared SQL query constants --
// All token queries use the same column list and JOIN. All session queries use
// the same column list and JOIN. If the schema changes, update the constants
// below in one place.

const TOKEN_BY_CODE: &str =
    "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes \
     FROM token t JOIN plan p ON t.plan_id = p.id WHERE t.code = ?1";

const TOKEN_BY_ID: &str =
    "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes \
     FROM token t JOIN plan p ON t.plan_id = p.id WHERE t.id = ?1";

const TOKEN_LIST: &str =
    "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes \
     FROM token t JOIN plan p ON t.plan_id = p.id ORDER BY t.created_at DESC";

const TOKEN_LIST_BY_STATUS: &str =
    "SELECT t.id, t.code, t.name, t.plan_id, t.status, t.created_at, \
            t.redeemed_at, t.expires_at, p.duration_minutes \
     FROM token t JOIN plan p ON t.plan_id = p.id WHERE t.status = ?1 ORDER BY t.created_at DESC";

const SESSION_ACTIVE: &str =
    "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name \
     FROM session s JOIN token t ON s.token_id = t.id WHERE s.status = 'active'";

const SESSION_BY_MAC: &str =
    "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name \
     FROM session s JOIN token t ON s.token_id = t.id WHERE s.mac_address = ?1 AND s.status = 'active'";

const SESSION_BY_ID: &str =
    "SELECT s.id, s.token_id, s.mac_address, s.ip_address, s.started_at, s.expires_at, s.status, \
            t.code as token_code, t.name as token_name \
     FROM session s JOIN token t ON s.token_id = t.id WHERE s.id = ?1";

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
        sqlx::query("PRAGMA foreign_keys=ON")
            .execute(&pool)
            .await?;

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

    pub async fn revoke_token(&self, token_id: i64) -> Result<()> {
        sqlx::query("UPDATE token SET status = 'revoked' WHERE id = ?1 AND status = 'unused'")
            .bind(token_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Sessions --

    pub async fn create_session(
        &self,
        token_id: i64,
        mac: &str,
        ip: &str,
        expires_at: &str,
    ) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO session (token_id, mac_address, ip_address, started_at, expires_at, status) \
             VALUES (?1, ?2, ?3, datetime('now'), ?4, 'active')",
        )
        .bind(token_id)
        .bind(mac)
        .bind(ip)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get_active_sessions(&self) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as::<_, Session>(SESSION_ACTIVE)
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

    // -- Stats --

    pub async fn get_daily_stats(&self) -> Result<DailyStats> {
        let (tokens_sold,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM token \
             WHERE redeemed_at >= date('now') AND status IN ('active', 'expired')",
        )
        .fetch_one(&self.pool)
        .await?;

        let (active_sessions,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM session WHERE status = 'active'")
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

        let updated = db.update_plan(id, "New Name", 120, 2000, false).await.unwrap();
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

        let token_id = db.create_token("DIDI-ABCD-EF23", None, plan_id).await.unwrap();
        assert!(token_id > 0);

        let token = db.get_token_by_code("DIDI-ABCD-EF23").await.unwrap().unwrap();
        assert_eq!(token.status, "unused");
        assert_eq!(token.duration_minutes, 60);

        let token2 = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token2.code, "DIDI-ABCD-EF23");
    }

    #[tokio::test]
    async fn test_redeem_and_expire_token() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let token_id = db.create_token("DIDI-TEST-CODE", None, plan_id).await.unwrap();

        db.redeem_token(token_id, "2026-12-31 23:59:59").await.unwrap();
        let token = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token.status, "active");
        assert!(token.redeemed_at.is_some());

        db.expire_token(token_id).await.unwrap();
        let token = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token.status, "expired");
    }

    #[tokio::test]
    async fn test_revoke_token() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let token_id = db.create_token("DIDI-REVO-KEXX", None, plan_id).await.unwrap();

        db.revoke_token(token_id).await.unwrap();
        let token = db.get_token_by_id(token_id).await.unwrap().unwrap();
        assert_eq!(token.status, "revoked");
    }

    #[tokio::test]
    async fn test_list_tokens_with_filter() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();

        let id1 = db.create_token("DIDI-AAAA-BBBB", None, plan_id).await.unwrap();
        db.create_token("DIDI-CCCC-DDDD", None, plan_id).await.unwrap();
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
        let token_id = db.create_token("DIDI-SESS-TEST", None, plan_id).await.unwrap();

        let session_id = db
            .create_session(token_id, "AA:BB:CC:DD:EE:FF", "10.10.0.5", "2099-12-31 23:59:59")
            .await
            .unwrap();
        assert!(session_id > 0);

        let session = db.get_session_by_mac("AA:BB:CC:DD:EE:FF").await.unwrap().unwrap();
        assert_eq!(session.ip_address, "10.10.0.5");
        assert_eq!(session.status, "active");
    }

    #[tokio::test]
    async fn test_active_sessions() {
        let db = test_db().await;
        let plan_id = db.create_plan("1h", 60, 1000).await.unwrap();
        let t1 = db.create_token("DIDI-ACT1-TEST", None, plan_id).await.unwrap();
        let t2 = db.create_token("DIDI-ACT2-TEST", None, plan_id).await.unwrap();

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
        let token_id = db.create_token("DIDI-EXPR-TEST", None, plan_id).await.unwrap();
        let session_id = db
            .create_session(token_id, "CC:DD:EE:FF:00:11", "10.10.0.3", "2099-12-31 23:59:59")
            .await
            .unwrap();

        db.expire_session(session_id).await.unwrap();
        let session = db.get_session_by_mac("CC:DD:EE:FF:00:11").await.unwrap();
        assert!(session.is_none()); // get_session_by_mac only returns active
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
            status: "active".to_string(),
            token_code: None,
            token_name: None,
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
            ip_address: "10.0.0.1".to_string(),
            started_at: "2020-01-01 00:00:00".to_string(),
            expires_at: "2020-01-01 01:00:00".to_string(),
            status: "active".to_string(),
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
            status: "active".to_string(),
            token_code: None,
            token_name: None,
        };
        assert_eq!(session.remaining_seconds(), 0);
    }

    // -- Type-safe enums --

    #[test]
    fn test_token_status_roundtrip() {
        for s in TokenStatus::ALL {
            let status = TokenStatus::from_str(s).unwrap();
            assert_eq!(&status.to_string(), *s);
        }
        assert!(TokenStatus::from_str("bogus").is_none());
    }

    #[test]
    fn test_session_status_roundtrip() {
        for s in &["active", "expired", "disconnected"] {
            let status = SessionStatus::from_str(s).unwrap();
            assert_eq!(&status.to_string(), *s);
        }
        assert!(SessionStatus::from_str("bogus").is_none());
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
}
