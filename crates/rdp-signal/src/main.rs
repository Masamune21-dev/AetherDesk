//! Signal Server AetherDesk.
//!
//! Meneruskan SDP dan kandidat ICE antara viewer dan agent, serta memelihara
//! kehadiran perangkat. Server **tidak pernah** membaca isi SDP: ia hanya
//! merutekan. Itu properti yang disyaratkan ADR-008 — begitu server ikut
//! menyentuh isinya, tanda tangan device key tidak lagi bermakna.

mod auth;
mod presence;
mod protocol;
mod sesi;
mod registry;

use anyhow::{Context, Result};
use auth::Verifier;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use protocol::{Keluar, Masuk};
use registry::{kirim, Koneksi, Registri};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Interval ping. Cloudflare memutus WebSocket idle sekitar 100 detik, jadi
/// 25 detik memberi tiga kesempatan sebelum ambang itu tercapai.
const INTERVAL_PING: Duration = Duration::from_secs(25);

/// Batas waktu pesan AUTH. Koneksi yang tidak memperkenalkan diri hanya
/// menghabiskan memori.
const BATAS_AUTH: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct AppState {
    db: PgPool,
    redis: redis::aio::ConnectionManager,
    verifier: Verifier,
    registri: Registri,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let bind = std::env::var("AETHERDESK_SIGNAL_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8081".to_string());
    let db_url = std::env::var("AETHERDESK_DB_URL").context("AETHERDESK_DB_URL belum diset")?;
    let redis_url =
        std::env::var("AETHERDESK_REDIS_URL").context("AETHERDESK_REDIS_URL belum diset")?;
    let pub_key = std::env::var("AETHERDESK_JWT_PUBLIC_KEY_PATH")
        .context("AETHERDESK_JWT_PUBLIC_KEY_PATH belum diset")?;
    let issuer =
        std::env::var("AETHERDESK_JWT_ISSUER").unwrap_or_else(|_| "aetherdesk".to_string());

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .context("gagal terhubung ke PostgreSQL")?;

    let redis = redis::aio::ConnectionManager::new(
        redis::Client::open(redis_url.as_str()).context("URL Redis tidak valid")?,
    )
    .await
    .context("gagal terhubung ke Redis")?;

    let verifier = Verifier::dari_pem(&pub_key, &issuer)?;
    tracing::info!("kunci publik JWT dimuat — server hanya memverifikasi, tidak menerbitkan");

    let state = AppState {
        db,
        redis,
        verifier,
        registri: Registri::baru(),
    };

    let app = Router::new()
        .route("/ws", get(upgrade))
        .route("/health", get(|| async { "signal-ok\n" }))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("gagal bind ke {bind}"))?;
    tracing::info!(addr = %bind, "rdp-signal siap melayani");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server berhenti tidak normal")?;
    Ok(())
}

async fn upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| tangani(socket, state))
}

async fn tangani(socket: WebSocket, state: AppState) {
    let conn_id = Uuid::new_v4();
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Satu task khusus menulis ke socket, supaya pengirim dari mana pun cukup
    // menaruh pesan di channel tanpa memperebutkan kunci socket.
    let penulis = tokio::spawn(async move {
        while let Some(pesan) = rx.recv().await {
            if ws_tx.send(Message::Text(pesan.into())).await.is_err() {
                break;
            }
        }
    });

    // ── Autentikasi wajib menjadi pesan pertama ─────────────────────────────
    let claims = match tokio::time::timeout(BATAS_AUTH, ws_rx.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<Masuk>(&t) {
            Ok(Masuk::Auth { token, device_uuid }) => match state.verifier.verifikasi(&token) {
                Some(c) => Some((c, device_uuid)),
                None => None,
            },
            _ => None,
        },
        _ => None,
    };

    let Some((claims, device_uuid)) = claims else {
        let _ = tx.send(Keluar::error("UNAUTHENTICATED", "autentikasi gagal").ke_json());
        tutup_setelah_terkuras(tx, penulis).await;
        return;
    };

    let koneksi = Koneksi {
        pengirim: tx.clone(),
        user_id: claims.sub,
        org_id: claims.org,
    };

    // Peran ditentukan oleh ada tidaknya device_uuid pada pesan AUTH.
    let peran_agent = match device_uuid {
        Some(dev) => {
            if !device_milik_org(&state.db, dev, claims.org).await {
                let _ = tx.send(
                    Keluar::error("PERMISSION_DENIED", "perangkat bukan milik organisasi ini")
                        .ke_json(),
                );
                drop(koneksi); // memegang klon `tx`; harus dilepas agar channel tertutup
                tutup_setelah_terkuras(tx, penulis).await;
                return;
            }
            // Agent lama untuk perangkat yang sama harus diputus: membiarkan
            // keduanya hidup berarti pesan sesi bisa mendarat di koneksi basi.
            if let Some(lama) = state.registri.daftar_agent(dev, koneksi.clone()).await {
                let _ = lama.pengirim.send(
                    Keluar::error("REPLACED", "koneksi digantikan sesi baru").ke_json(),
                );
            }
            let mut redis = state.redis.clone();
            if let Err(e) =
                presence::tandai_online(&state.db, &mut redis, claims.org, dev).await
            {
                tracing::error!(error = %e, "gagal menandai perangkat online");
            }
            true
        }
        None => {
            state.registri.daftar_viewer(conn_id, koneksi.clone()).await;
            false
        }
    };

    let _ = tx.send(
        Keluar::AuthOk {
            role: if peran_agent { "agent" } else { "viewer" },
            user_id: claims.sub,
            org_id: claims.org,
        }
        .ke_json(),
    );

    let (a, v) = state.registri.jumlah().await;
    tracing::info!(%conn_id, peran_agent, agent = a, viewer = v, "koneksi terautentikasi");

    // ── Ping berkala ────────────────────────────────────────────────────────
    let ping_tx = tx.clone();
    let ping_redis = state.redis.clone();
    let ping = tokio::spawn(async move {
        let mut redis = ping_redis;
        let mut tick = tokio::time::interval(INTERVAL_PING);
        tick.tick().await;
        loop {
            tick.tick().await;
            if ping_tx.send(Keluar::Ping.ke_json()).is_err() {
                break;
            }
            if let Some(dev) = device_uuid {
                let _ = presence::perbarui(&mut redis, dev).await;
            }
        }
    });

    // ── Loop pesan ──────────────────────────────────────────────────────────
    while let Some(Ok(pesan)) = ws_rx.next().await {
        let teks = match pesan {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        let masuk = match serde_json::from_str::<Masuk>(&teks) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(error = %e, "pesan tidak dikenali");
                let _ = tx.send(Keluar::error("BAD_MESSAGE", "pesan tidak dikenali").ke_json());
                continue;
            }
        };

        rutekan(&state, &masuk, conn_id, device_uuid, &claims, &tx).await;
    }

    // ── Pembersihan ─────────────────────────────────────────────────────────
    ping.abort();
    penulis.abort();

    if let Some(dev) = device_uuid {
        state.registri.lepas_agent(dev).await;
        let mut redis = state.redis.clone();
        // Inti perbaikan S-09: offline seketika, tidak menunggu TTL 90 detik.
        if let Err(e) = presence::tandai_offline(&state.db, &mut redis, claims.org, dev).await {
            tracing::error!(error = %e, "gagal menandai perangkat offline");
        }
        // Menutup tab tanpa menekan tombol akhiri adalah cara paling umum
        // orang mengakhiri sesuatu. Tanpa ini, sesinya menggantung selamanya.
        if let Err(e) = sesi::akhiri_semua_perangkat(&state.db, claims.org, dev).await {
            tracing::error!(error = %e, "gagal menutup sesi menggantung");
        }
    } else {
        state.registri.lepas_viewer(conn_id).await;
    }

    tracing::info!(%conn_id, "koneksi ditutup");
}

async fn rutekan(
    state: &AppState,
    masuk: &Masuk,
    conn_id: Uuid,
    device_uuid: Option<Uuid>,
    claims: &auth::Claims,
    tx: &mpsc::UnboundedSender<String>,
) {
    let dari_viewer = device_uuid.is_none();

    // Setiap pesan bersesi diperiksa keanggotaannya. Tanpa ini, siapa pun yang
    // terautentikasi dapat menyuntikkan SDP ke sesi orang lain hanya dengan
    // menebak session_id.
    macro_rules! wajib_peserta {
        ($sid:expr) => {
            if !state
                .registri
                .peserta_sah($sid, conn_id, device_uuid)
                .await
            {
                tracing::warn!(session_id = %$sid, %conn_id, "bukan peserta sesi — ditolak");
                let _ = tx.send(
                    Keluar::error("NOT_A_PARTICIPANT", "bukan peserta sesi ini").ke_json(),
                );
                return;
            }
        };
    }

    macro_rules! teruskan {
        ($sid:expr, $pesan:expr) => {{
            wajib_peserta!($sid);
            match state.registri.lawan($sid, dari_viewer).await {
                Some(lawan) => {
                    if !kirim(&lawan, &$pesan) {
                        let _ = tx.send(
                            Keluar::error("PEER_GONE", "lawan sudah terputus").ke_json(),
                        );
                    }
                }
                None => {
                    let _ =
                        tx.send(Keluar::error("PEER_GONE", "lawan tidak terhubung").ke_json());
                }
            }
        }};
    }

    match masuk {
        Masuk::Auth { .. } => {
            let _ = tx.send(Keluar::error("ALREADY_AUTHED", "sudah terautentikasi").ke_json());
        }

        Masuk::SessionRequest {
            session_id,
            device_uuid: target,
        } => {
            if !dari_viewer {
                let _ = tx.send(Keluar::error("WRONG_ROLE", "hanya viewer").ke_json());
                return;
            }
            let Some(agent) = state.registri.agent(*target).await else {
                let _ = tx.send(Keluar::error("DEVICE_OFFLINE", "perangkat tidak terhubung").ke_json());
                return;
            };
            if agent.org_id != claims.org {
                // Tidak membocorkan bahwa perangkatnya ada — jawabannya sama
                // dengan perangkat yang tidak terhubung.
                let _ = tx.send(Keluar::error("DEVICE_OFFLINE", "perangkat tidak terhubung").ke_json());
                return;
            }

            state.registri.catat_sesi(*session_id, conn_id, *target).await;
            kirim(
                &agent,
                &Keluar::SessionOffer {
                    session_id: *session_id,
                    viewer_name: claims.email.clone(),
                    viewer_email: claims.email.clone(),
                    viewer_ip: String::new(),
                },
            );
            tracing::info!(%session_id, device = %target, "permintaan sesi diteruskan ke agent");
        }

        Masuk::SessionAccept { session_id } => {
            if let Err(e) = sesi::aktifkan(&state.db, claims.org, *session_id).await {
                tracing::error!(error = %e, "gagal menandai sesi aktif");
            }
            teruskan!(*session_id, Keluar::SessionAccepted { session_id: *session_id })
        }
        Masuk::SessionReject { session_id, reason } => {
            if let Err(e) = sesi::akhiri(&state.db, claims.org, *session_id, "ditolak").await {
                tracing::error!(error = %e, "gagal menutup sesi yang ditolak");
            }
            teruskan!(
                *session_id,
                Keluar::SessionRejected {
                    session_id: *session_id,
                    reason: reason.clone(),
                }
            )
        }
        Masuk::SdpOffer { session_id, sdp } => teruskan!(
            *session_id,
            Keluar::SdpOffer {
                session_id: *session_id,
                sdp: sdp.clone(),
            }
        ),
        Masuk::SdpAnswer { session_id, sdp } => teruskan!(
            *session_id,
            Keluar::SdpAnswer {
                session_id: *session_id,
                sdp: sdp.clone(),
            }
        ),
        Masuk::IceCandidate {
            session_id,
            candidate,
        } => teruskan!(
            *session_id,
            Keluar::IceCandidate {
                session_id: *session_id,
                candidate: candidate.clone(),
            }
        ),
        Masuk::SessionEnd { session_id } => {
            wajib_peserta!(*session_id);
            if let Some(lawan) = state.registri.lawan(*session_id, dari_viewer).await {
                kirim(&lawan, &Keluar::SessionEnd { session_id: *session_id });
            }
            if let Err(e) = sesi::akhiri(&state.db, claims.org, *session_id, "diakhiri pengguna").await {
                tracing::error!(error = %e, "gagal menutup sesi");
            }
            state.registri.hapus_sesi(*session_id).await;
        }
        Masuk::Pong => {}
    }
}

/// Menutup koneksi setelah pesan yang tertunda benar-benar terkirim.
///
/// Memanggil `abort()` pada task penulis akan membunuhnya sebelum pesan
/// terakhir sempat mengalir ke socket, sehingga klien tidak pernah tahu alasan
/// penolakannya. Melepas seluruh pengirim membuat channel tertutup, dan task
/// penulis menguras antrean lalu berhenti sendiri.
///
/// Batas waktu tetap dipasang: klien yang berhenti membaca tidak boleh menahan
/// task ini selamanya.
async fn tutup_setelah_terkuras(
    tx: mpsc::UnboundedSender<String>,
    penulis: tokio::task::JoinHandle<()>,
) {
    drop(tx);
    if tokio::time::timeout(Duration::from_secs(2), penulis)
        .await
        .is_err()
    {
        tracing::debug!("penulis tidak selesai dalam 2 detik saat menutup koneksi");
    }
}

async fn device_milik_org(db: &PgPool, device_uuid: Uuid, org_id: Uuid) -> bool {
    // Query lewat pemilik tabel dengan tenant ditetapkan — RLS ikut memeriksa,
    // jadi perangkat milik organisasi lain tidak akan pernah cocok.
    let hasil = async {
        let mut tx = db.begin().await.ok()?;
        sqlx::query("SELECT set_config('aetherdesk.current_org', $1, true)")
            .bind(org_id.to_string())
            .execute(&mut *tx)
            .await
            .ok()?;
        let ada: Option<Uuid> = sqlx::query_scalar("SELECT id FROM devices WHERE id = $1")
            .bind(device_uuid)
            .fetch_optional(&mut *tx)
            .await
            .ok()?;
        tx.commit().await.ok()?;
        ada
    }
    .await;
    hasil.is_some()
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter =
        EnvFilter::try_from_env("AETHERDESK_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_current_span(true))
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("SIGINT diterima"),
        _ = terminate => tracing::info!("SIGTERM diterima"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_ping_di_bawah_ambang_cloudflare() {
        // Cloudflare memutus WebSocket idle sekitar 100 detik.
        assert!(INTERVAL_PING.as_secs() * 3 < 100);
    }

    #[test]
    fn batas_auth_wajar() {
        assert!(BATAS_AUTH.as_secs() >= 5 && BATAS_AUTH.as_secs() <= 30);
    }
}
