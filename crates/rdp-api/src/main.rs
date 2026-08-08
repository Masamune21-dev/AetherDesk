//! API Server AetherDesk.
//!
//! Fase 0: konfigurasi, kolam koneksi, endpoint kesehatan, dan shutdown yang
//! rapi. Modul domain (auth, devices, sessions) menyusul di atas fondasi ini.

mod config;
mod health;
mod state;

use anyhow::{Context, Result};
use axum::{routing::get, Router};
use config::Config;
use sqlx::postgres::PgPoolOptions;
use state::AppState;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cfg = Config::from_env().context("gagal memuat konfigurasi")?;
    tracing::info!(bind = %cfg.bind, "rdp-api mulai");

    let db = PgPoolOptions::new()
        .max_connections(cfg.db_max_connections)
        .acquire_timeout(cfg.db_acquire_timeout)
        .connect(&cfg.database_url)
        .await
        .context("gagal terhubung ke PostgreSQL")?;
    tracing::info!("postgres terhubung");

    let redis_client =
        redis::Client::open(cfg.redis_url.as_str()).context("URL Redis tidak valid")?;
    let redis = redis::aio::ConnectionManager::new(redis_client)
        .await
        .context("gagal terhubung ke Redis")?;
    tracing::info!("redis terhubung");

    let app_state = AppState {
        db,
        redis,
        events: Arc::new(rdp_core::InProcessBus),
    };

    // Seluruh route hidup di bawah `/api` karena nginx meneruskan URI apa
    // adanya, bukan memotong prefiksnya. Ini sekaligus menetapkan satu bentuk
    // path untuk seluruh sistem — dokumen sebelumnya memakai `/v1` dan
    // `/api/v1` bergantian. (Temuan R-05)
    //
    // Endpoint kesehatan sengaja tidak diversikan: sifatnya operasional dan
    // harus tetap stabil melewati pergantian versi API.
    let operasional = Router::new()
        .route("/health", get(health::liveness))
        .route("/health/ready", get(health::readiness));

    let v1 = Router::new();

    let app = Router::new()
        .nest("/api", operasional)
        .nest("/api/v1", v1)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind)
        .await
        .with_context(|| format!("gagal bind ke {}", cfg.bind))?;

    tracing::info!(addr = %cfg.bind, "siap melayani");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server berhenti tidak normal")?;

    tracing::info!("rdp-api berhenti dengan rapi");
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_env("AETHERDESK_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug"));

    // SYSTEM_DESIGN.md §5 mensyaratkan log terstruktur JSON supaya dapat
    // diparsing mesin analitik.
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_current_span(true))
        .init();
}

/// Menunggu SIGTERM atau SIGINT supaya systemd dapat menghentikan layanan
/// tanpa memutus request yang sedang berjalan.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("gagal memasang handler Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("gagal memasang handler SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("SIGINT diterima"),
        _ = terminate => tracing::info!("SIGTERM diterima"),
    }
}
