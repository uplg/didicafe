use anyhow::Result;
use clap::Parser;
use tracing::info;
use std::net::SocketAddr;
use std::sync::Arc;

mod config;
mod db;
mod firewall;
mod net;
mod services;
mod web;

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

    let fw: Arc<dyn firewall::Firewall> = if cfg!(debug_assertions) {
        info!("dev mode: using mock firewall");
        Arc::new(firewall::MockFirewall::new())
    } else {
        Arc::new(firewall::NftablesController::new(&config.firewall))
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
                database.expire_token(session.token_id).await.ok();
                expired_count += 1;
                tracing::info!(session_id = session.id, "session expired during downtime");
            }
        }
    }
    info!("restored {} active sessions, expired {} during downtime", restored_count, expired_count);

    let admin_sessions = services::admin_session::AdminSessionStore::new(
        config.admin.session_timeout_seconds,
    );

    let rate_limiter = services::rate_limit::RateLimiter::new(&config.rate_limit);

    let state = Arc::new(AppState {
        db: database,
        config: config.clone(),
        firewall: fw,
        admin_sessions,
        rate_limiter,
    });

    // Spawn session cleanup ticker
    let cleanup_state = Arc::clone(&state);
    tokio::spawn(async move {
        services::session::cleanup_ticker(cleanup_state).await;
    });

    // Build and serve HTTP
    let app = web::router(Arc::clone(&state));
    let addr = format!("{}:{}", config.server.listen, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("listening on {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
