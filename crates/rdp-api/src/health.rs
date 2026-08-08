//! Endpoint kesehatan.
//!
//! Dua endpoint yang berbeda tujuan, dan membedakannya penting:
//!
//! - `/health` — **liveness**. Proses hidup. Tidak menyentuh dependensi, jadi
//!   database yang sedang lambat tidak akan memicu restart beruntun.
//! - `/health/ready` — **readiness**. Dependensi terjangkau dan siap melayani.

use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct Liveness {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
pub struct Readiness {
    status: &'static str,
    checks: Vec<Check>,
}

#[derive(Serialize)]
pub struct Check {
    name: &'static str,
    ok: bool,
    latency_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// `GET /health` — selalu 200 selama proses menjawab.
pub async fn liveness() -> Json<Liveness> {
    Json(Liveness {
        status: "ok",
        service: "rdp-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// `GET /health/ready` — 200 hanya bila seluruh dependensi sehat.
pub async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    let checks = vec![check_postgres(&state).await, check_redis(&state).await];
    let semua_sehat = checks.iter().all(|c| c.ok);

    let code = if semua_sehat {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        code,
        Json(Readiness {
            status: if semua_sehat { "ready" } else { "degraded" },
            checks,
        }),
    )
}

async fn check_postgres(state: &AppState) -> Check {
    let mulai = Instant::now();
    let hasil = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await;
    Check {
        name: "postgres",
        ok: hasil.is_ok(),
        latency_ms: mulai.elapsed().as_millis(),
        // Pesan error koneksi bisa memuat host dan nama database, jadi tidak
        // dikirim apa adanya ke pemanggil — cukup dicatat di log server.
        error: hasil.err().map(|e| {
            tracing::warn!(error = %e, "readiness: postgres gagal");
            "tidak terjangkau".to_string()
        }),
    }
}

async fn check_redis(state: &AppState) -> Check {
    let mulai = Instant::now();
    let mut conn = state.redis.clone();
    let hasil: Result<String, _> = redis::cmd("PING").query_async(&mut conn).await;
    Check {
        name: "redis",
        ok: hasil.as_deref() == Ok("PONG"),
        latency_ms: mulai.elapsed().as_millis(),
        error: hasil.err().map(|e| {
            tracing::warn!(error = %e, "readiness: redis gagal");
            "tidak terjangkau".to_string()
        }),
    }
}
