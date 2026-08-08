//! Pendaftaran dan pengelolaan perangkat.

use crate::{
    auth::{hash, Terautentikasi},
    db,
    error::{ApiError, ApiResult, Sukses},
    state::AppState,
};
use axum::extract::State;
use chrono::{DateTime, Utc};
use rdp_core::{password, DeviceId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct DaftarReq {
    pub alias: Option<String>,
    pub os_type: String,
    pub os_version: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DaftarResp {
    pub device_uuid: Uuid,
    pub device_id: String,
    /// Bentuk berkelompok `942 716 382`, untuk ditampilkan dan dibacakan.
    pub device_id_tampil: String,
    /// **Satu-satunya kali** password ini dikirim dalam bentuk asli.
    /// Setelah ini hanya hash-nya yang tersimpan.
    pub session_password: String,
}

/// `POST /api/v1/devices`
pub async fn daftar(
    State(state): State<AppState>,
    Terautentikasi(claims): Terautentikasi,
    axum::Json(req): axum::Json<DaftarReq>,
) -> ApiResult<Sukses<DaftarResp>> {
    if !matches!(req.os_type.as_str(), "Windows" | "macOS" | "Linux" | "Web") {
        return Err(ApiError::Validasi(
            "os_type harus salah satu dari: Windows, macOS, Linux, Web".into(),
        ));
    }

    let sesi_password = password::generate();
    let sesi_hash = hash::hash(&sesi_password).map_err(ApiError::Internal)?;

    let mut tx = db::mulai_transaksi_tenant(&state.db, claims.org_id()).await?;

    // QUICK_CONNECT.md §2.3: ID dibangkitkan acak, bukan berurutan. Tabrakan
    // ditangani dengan mencoba ulang; lebih dari lima kegagalan berarti ruang
    // ID sudah terlalu padat dan layak dijadikan peringatan operasional.
    let mut device_id = None;
    for percobaan in 1..=5 {
        let kandidat = DeviceId::generate();
        let hasil = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO devices
                (organization_id, device_id, alias, os_type, os_version,
                 hostname, session_password_hash, session_password_set_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, now())
             ON CONFLICT (device_id) DO NOTHING
             RETURNING id",
        )
        .bind(claims.org_id())
        .bind(kandidat.as_str())
        .bind(&req.alias)
        .bind(&req.os_type)
        .bind(&req.os_version)
        .bind(&req.hostname)
        .bind(&sesi_hash)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(uuid) = hasil {
            device_id = Some((uuid, kandidat));
            break;
        }
        tracing::warn!(percobaan, "device ID bertabrakan, mencoba ulang");
    }

    let Some((device_uuid, id)) = device_id else {
        tracing::error!("gagal mengalokasikan device ID setelah 5 percobaan — ruang ID padat");
        return Err(ApiError::Internal(anyhow::anyhow!(
            "alokasi device ID gagal"
        )));
    };

    tx.commit().await?;
    tracing::info!(org = %claims.org_id(), device = %id, "perangkat terdaftar");

    Ok(Sukses::baru(DaftarResp {
        device_uuid,
        device_id: id.to_string(),
        device_id_tampil: id.grouped(),
        session_password: sesi_password,
    }))
}

// ── Daftar perangkat ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct Perangkat {
    pub device_uuid: Uuid,
    pub device_id: String,
    pub device_id_tampil: String,
    pub alias: Option<String>,
    pub os_type: String,
    pub hostname: Option<String>,
    pub status: String,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub quick_connect_enabled: bool,
}

/// `GET /api/v1/devices`
pub async fn daftar_semua(
    State(state): State<AppState>,
    Terautentikasi(claims): Terautentikasi,
) -> ApiResult<Sukses<Vec<Perangkat>>> {
    let mut tx = db::mulai_transaksi_tenant(&state.db, claims.org_id()).await?;

    // RLS sudah membatasi ke organisasi aktif; tidak perlu WHERE tambahan,
    // dan justru lebih aman begitu — filter tidak bisa lupa ditulis.
    let baris: Vec<(Uuid, String, Option<String>, String, Option<String>, String, Option<DateTime<Utc>>, bool)> =
        sqlx::query_as(
            "SELECT id, device_id, alias, os_type, hostname, status,
                    last_heartbeat, quick_connect_enabled
             FROM devices
             ORDER BY created_at DESC
             LIMIT 200",
        )
        .fetch_all(&mut *tx)
        .await?;

    tx.commit().await?;

    let perangkat = baris
        .into_iter()
        .map(|(id, did, alias, os, host, status, hb, qc)| {
            let tampil = DeviceId::parse(&did)
                .map(|d| d.grouped())
                .unwrap_or_else(|_| did.clone());
            Perangkat {
                device_uuid: id,
                device_id: did,
                device_id_tampil: tampil,
                alias,
                os_type: os,
                hostname: host,
                status,
                last_heartbeat: hb,
                quick_connect_enabled: qc,
            }
        })
        .collect();

    Ok(Sukses::baru(perangkat))
}

// ── Rotasi password sesi ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RotasiResp {
    pub session_password: String,
}

/// `POST /api/v1/devices/{uuid}/rotate-password`
///
/// QUICK_CONNECT.md §3.1 mewajibkan password berotasi setelah setiap sesi dan
/// saat pengguna memintanya.
pub async fn rotasi_password(
    State(state): State<AppState>,
    Terautentikasi(claims): Terautentikasi,
    axum::extract::Path(device_uuid): axum::extract::Path<Uuid>,
) -> ApiResult<Sukses<RotasiResp>> {
    let baru = password::generate();
    let hash_baru = hash::hash(&baru).map_err(ApiError::Internal)?;

    let mut tx = db::mulai_transaksi_tenant(&state.db, claims.org_id()).await?;

    let terpengaruh = sqlx::query(
        "UPDATE devices
         SET session_password_hash = $1, session_password_set_at = now()
         WHERE id = $2",
    )
    .bind(&hash_baru)
    .bind(device_uuid)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;

    if terpengaruh == 0 {
        return Err(ApiError::TidakDitemukan("perangkat"));
    }

    tracing::info!(%device_uuid, "password sesi dirotasi");
    Ok(Sukses::baru(RotasiResp {
        session_password: baru,
    }))
}
