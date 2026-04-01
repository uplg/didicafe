use anyhow::{Context, Result};
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
    /// CSRF store for the login form (Synchronizer Token Pattern, keyed by client IP).
    pub login_csrf_store: services::csrf::PortalCsrfStore,
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
            if let Err(e) = fw.authorize_client(&session.mac_address, &session.ip_address, remaining as u64).await {
                tracing::warn!(mac = %session.mac_address, ip = %session.ip_address, "failed to restore session: {e}");
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
    let login_csrf_store = services::csrf::PortalCsrfStore::new();

    let tls_enabled = config.tls.enabled;
    let admin_port = config.tls.admin_port;

    let state = Arc::new(AppState {
        db: database,
        config: config.clone(),
        firewall: fw,
        admin_sessions,
        rate_limiter,
        admin_rate_limiter,
        portal_csrf_store,
        login_csrf_store,
    });

    // Cancellation token for graceful shutdown
    let shutdown = CancellationToken::new();

    // Spawn session cleanup ticker with shutdown signal
    let cleanup_state = Arc::clone(&state);
    let cleanup_shutdown = shutdown.clone();
    tokio::spawn(async move {
        services::session::cleanup_ticker(cleanup_state, cleanup_shutdown).await;
    });

    let http_addr = format!("{}:{}", config.server.listen, config.server.port);

    if tls_enabled {
        // Dual-listener mode: HTTP (portal) + HTTPS (admin)
        let portal_app = web::portal_router(Arc::clone(&state));
        let admin_app = web::admin_router(Arc::clone(&state));

        let portal_listener = tokio::net::TcpListener::bind(&http_addr).await?;
        info!("portal (HTTP) listening on {http_addr}");

        let admin_addr = format!("{}:{}", config.server.listen, admin_port);
        let tls_acceptor = build_tls_acceptor(&config.tls)?;
        let admin_listener = tokio::net::TcpListener::bind(&admin_addr).await?;
        info!("admin (HTTPS) listening on {admin_addr}");

        // Spawn the portal HTTP server
        let portal_shutdown = shutdown.clone();
        let portal_handle = tokio::spawn(async move {
            let server = axum::serve(
                portal_listener,
                portal_app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(portal_shutdown.cancelled_owned());
            if let Err(e) = server.await {
                tracing::error!("portal server error: {e}");
            }
        });

        // Spawn the admin HTTPS server (TLS connection loop)
        let admin_shutdown = shutdown.clone();
        let admin_handle = tokio::spawn(async move {
            serve_tls(admin_listener, tls_acceptor, admin_app, admin_shutdown).await;
        });

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        info!("shutdown signal received, draining...");
        shutdown.cancel();

        // Wait for both servers to finish
        let _ = tokio::join!(portal_handle, admin_handle);
    } else {
        // Single-listener mode: all routes on one HTTP listener (dev mode)
        let app = web::combined_router(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind(&http_addr).await?;
        info!("listening on {http_addr} (combined mode, TLS disabled)");

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
    }

    Ok(())
}

/// Build a `tokio_rustls::TlsAcceptor` from the TLS config.
///
/// Loads PEM certificate chain and private key from disk.
/// Uses rustls with safe defaults (TLS 1.2+, strong cipher suites).
fn build_tls_acceptor(tls_config: &config::TlsConfig) -> Result<tokio_rustls::TlsAcceptor> {
    use rustls_pemfile::{certs, private_key};
    use std::fs::File;
    use std::io::BufReader;
    use tokio_rustls::rustls::ServerConfig;

    // Load certificate chain
    let cert_file = File::open(&tls_config.cert_path)
        .with_context(|| format!("failed to open TLS cert: {}", tls_config.cert_path))?;
    let cert_chain: Vec<_> = certs(&mut BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to parse TLS cert: {}", tls_config.cert_path))?;
    anyhow::ensure!(!cert_chain.is_empty(), "TLS cert file contains no certificates");

    // Load private key
    let key_file = File::open(&tls_config.key_path)
        .with_context(|| format!("failed to open TLS key: {}", tls_config.key_path))?;
    let key = private_key(&mut BufReader::new(key_file))
        .with_context(|| format!("failed to parse TLS key: {}", tls_config.key_path))?
        .ok_or_else(|| anyhow::anyhow!("TLS key file contains no private key: {}", tls_config.key_path))?;

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .context("failed to build TLS server config")?;

    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(server_config)))
}

/// Accept TLS connections and serve the admin router.
///
/// Runs a loop accepting TCP connections, performing TLS handshake,
/// and dispatching to the axum router via `hyper`. Each connection
/// is handled in its own tokio task.
async fn serve_tls(
    listener: tokio::net::TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
    app: axum::Router,
    shutdown: CancellationToken,
) {
    use hyper_util::rt::TokioIo;
    use tower::Service;

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (tcp_stream, remote_addr) = match result {
                    Ok(conn) => conn,
                    Err(e) => {
                        tracing::warn!("admin listener accept error: {e}");
                        continue;
                    }
                };

                let acceptor = acceptor.clone();
                let app = app.clone();
                let conn_shutdown = shutdown.clone();

                tokio::spawn(async move {
                    // TLS handshake with timeout (prevent slowloris)
                    let tls_stream = match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        acceptor.accept(tcp_stream),
                    ).await {
                        Ok(Ok(stream)) => stream,
                        Ok(Err(e)) => {
                            tracing::debug!(%remote_addr, "TLS handshake failed: {e}");
                            return;
                        }
                        Err(_) => {
                            tracing::debug!(%remote_addr, "TLS handshake timed out");
                            return;
                        }
                    };

                    // Serve HTTP over the TLS stream
                    let io = TokioIo::new(tls_stream);
                    let hyper_service = hyper::service::service_fn(move |mut req: hyper::Request<hyper::body::Incoming>| {
                        // Inject ConnectInfo so extractors can access the client address
                        req.extensions_mut().insert(axum::extract::ConnectInfo(remote_addr));
                        let mut svc = app.clone();
                        async move {
                            svc.call(req).await
                        }
                    });

                    let conn = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(io, hyper_service)
                    .into_owned();

                    tokio::select! {
                        result = conn => {
                            if let Err(e) = result {
                                tracing::debug!(%remote_addr, "admin connection error: {e}");
                            }
                        }
                        _ = conn_shutdown.cancelled() => {
                            tracing::debug!(%remote_addr, "admin connection shutdown");
                        }
                    }
                });
            }
            _ = shutdown.cancelled() => {
                info!("admin HTTPS listener shutting down");
                break;
            }
        }
    }
}
