use anyhow::Result;
use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing::info;
use std::net::SocketAddr;
use std::sync::Arc;

mod config;
mod db;
mod firewall;
mod net;
mod services;
mod web;

#[cfg(test)]
mod test_utils;

#[derive(Parser)]
#[command(name = "didicafe", about = "Captive portal daemon for time-limited WiFi access")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/didicafe/didicafe.toml")]
    config: String,

    /// Run database migrations and exit
    #[arg(long)]
    migrate: bool,
}

/// Shared application state passed to all handlers
pub struct AppState {
    pub db: db::Database,
    pub config: config::Config,
    pub firewall: Arc<dyn firewall::Firewall>,
    pub admin_sessions: services::admin_session::AdminSessionStore,
    pub rate_limiter: services::rate_limit::RateLimiter,
    pub admin_rate_limiter: services::rate_limit::RateLimiter,
    pub portal_csrf_store: services::csrf::PortalCsrfStore,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "didicafe=info".into()),
        )
        .init();

    let cli = Cli::parse();

    info!("loading configuration from {}", cli.config);
    let config = config::Config::load(&cli.config)?;

    info!("opening database at {}", config.database.path);
    let database = db::Database::open(&config.database.path).await?;
    database.migrate().await?;

    if cli.migrate {
        info!("migrations complete, exiting");
        return Ok(());
    }

    let fw: Arc<dyn firewall::Firewall> = {
        #[cfg(debug_assertions)]
        {
            info!("dev mode: using mock firewall");
            Arc::new(firewall::MockFirewall::new())
        }
        #[cfg(not(debug_assertions))]
        {
            Arc::new(firewall::NftablesController::new(&config.firewall))
        }
    };

    // Initialize nftables table and set (idempotent)
    fw.init_ruleset().await?;
    info!("firewall ruleset initialized");

    // Restore active sessions from database into nftables
    // Also expire any sessions that have already elapsed during downtime
    let active_sessions = database.get_active_sessions().await?;
    let grace = config.session.grace_period_seconds;
    let mut restored_count = 0;
    let mut expired_count = 0;

    for session in &active_sessions {
        let remaining = session.remaining_seconds();
        if remaining > grace {
            // Session still valid - restore to firewall
            if let Err(e) = fw.authorize_mac(&session.mac_address, remaining as u64).await {
                tracing::warn!(mac = %session.mac_address, "failed to restore session: {e}");
            } else {
                restored_count += 1;
            }
        } else {
            // Session has expired - mark as expired in DB
            if let Err(e) = database.expire_session(session.id).await {
                tracing::warn!(session_id = session.id, "failed to expire session: {e}");
            } else {
                if let Err(e) = database.expire_token(session.token_id).await {
                    tracing::warn!(session_id = session.id, token_id = session.token_id, "failed to expire token: {e}");
                }
                expired_count += 1;
                tracing::info!(session_id = session.id, "session expired during downtime");
            }
        }
    }
    info!("restored {} active sessions, expired {} during downtime", restored_count, expired_count);

    // Purge old expired/disconnected sessions (retention policy)
    let retention = config.session.retention_days;
    match database.purge_expired_sessions(retention).await {
        Ok(purged) if purged > 0 => {
            info!("purged {purged} expired sessions older than {retention} days");
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("failed to purge expired sessions: {e}");
        }
    }

    let admin_sessions = services::admin_session::AdminSessionStore::new(
        config.admin.session_timeout_seconds,
    );

    let rate_limiter = services::rate_limit::RateLimiter::new(&config.rate_limit);
    let admin_rate_limiter = services::rate_limit::RateLimiter::new(&config.rate_limit);
    let portal_csrf_store = services::csrf::PortalCsrfStore::new();

    let state = Arc::new(AppState {
        db: database,
        config: config.clone(),
        firewall: fw,
        admin_sessions,
        rate_limiter,
        admin_rate_limiter,
        portal_csrf_store,
    });

    // Cancellation token for graceful shutdown
    let shutdown = CancellationToken::new();

    // Spawn session cleanup ticker with shutdown signal
    let cleanup_state = Arc::clone(&state);
    let cleanup_shutdown = shutdown.clone();
    tokio::spawn(async move {
        services::session::cleanup_ticker(cleanup_state, cleanup_shutdown).await;
    });

    // Build and serve HTTP
    let app = web::router(Arc::clone(&state));
    let addr = format!("{}:{}", config.server.listen, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("listening on {addr}");

    // Wait for SIGTERM/SIGINT then gracefully shut down
    let shutdown_signal = tokio::signal::ctrl_c();
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );

    tokio::select! {
        result = server => {
            result?;
        }
        _ = shutdown_signal => {
            info!("shutdown signal received, draining...");
            shutdown.cancel();
        }
    }

    Ok(())
}
