//! Enrolment dan autentikasi perangkat.
//!
//! Menutup celah yang menghalangi sisa M1: sampai sekarang satu-satunya cara
//! mendaftarkan perangkat adalah `POST /api/v1/devices`, yang mewajibkan JWT
//! **pengguna**. Agent tanpa pengawasan tidak boleh menyimpan kredensial
//! manusia — bila mesinnya dibongkar, yang bocor harus satu perangkat, bukan
//! satu akun beserta seluruh armada organisasinya.
//!
//! Tiga langkah:
//!
//! 1. `POST /devices/enrolment-tokens` — pengguna menerbitkan token sekali
//!    pakai dari dashboard
//! 2. `POST /devices/enrol` — agent menukarnya sambil mendaftarkan kunci
//!    publik Ed25519 miliknya
//! 3. `POST /devices/token` — seterusnya agent menandatangani tantangan dan
//!    menerima JWT perangkat berumur pendek
//!
//! Langkah 2 dan 3 **tidak** memerlukan sesi pengguna, dan itulah intinya.

use crate::{
    audit::{self, aksi},
    auth::{hash, PerangkatTerautentikasi, Terautentikasi},
    db,
    error::{ApiError, ApiResult, Sukses},
    net::IpKlien,
    ratelimit::{self, Keputusan},
    state::AppState,
};
use axum::extract::State;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use rdp_core::{device_key, DeviceId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Masa berlaku baku token enrolment: satu jam.
///
/// Cukup untuk berjalan ke mesin lain dan memasang agent, cukup pendek supaya
/// token yang terlanjur tertinggal di riwayat chat atau tiket dukungan tidak
/// berguna keesokan harinya.
const ENROLMENT_TTL_BAKU: i64 = 3_600;
const ENROLMENT_TTL_MIN: i64 = 60;
const ENROLMENT_TTL_MAKS: i64 = 86_400;

/// Panjang token enrolment dalam byte acak.
///
/// 256 bit. Menebaknya mustahil, dan itu yang membuat SHA-256 — bukan
/// Argon2id — menjadi pilihan hash yang benar untuk menyimpannya.
const ENROLMENT_TOKEN_BYTES: usize = 32;

fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

// ═════════════════════════════════════════════════════════════════════════════
// 1. Menerbitkan token enrolment
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct TerbitkanReq {
    pub alias: Option<String>,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TerbitkanResp {
    /// **Satu-satunya kali** token ini terlihat. Hanya hash-nya yang tersimpan.
    pub enrolment_token: String,
    pub expires_at: DateTime<Utc>,
}

/// `POST /api/v1/devices/enrolment-tokens`
pub async fn terbitkan(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    Terautentikasi(claims): Terautentikasi,
    axum::Json(req): axum::Json<TerbitkanReq>,
) -> ApiResult<Sukses<TerbitkanResp>> {
    let ttl = req
        .expires_in_seconds
        .unwrap_or(ENROLMENT_TTL_BAKU)
        .clamp(ENROLMENT_TTL_MIN, ENROLMENT_TTL_MAKS);

    let mut byte = [0u8; ENROLMENT_TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut byte);
    let token = device_key::ke_base64(&byte);
    let expires_at = Utc::now() + Duration::seconds(ttl);

    let mut tx = db::mulai_transaksi_tenant(&state.db, claims.org_id()).await?;
    sqlx::query(
        "INSERT INTO device_enrolment_tokens
            (organization_id, token_hash, alias, created_by, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(claims.org_id())
    .bind(hash_token(&token))
    .bind(&req.alias)
    .bind(claims.user_id())
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    audit::catat(&state.db, audit::Entri {
        org_id: claims.org_id(),
        user_id: Some(claims.user_id()),
        ip,
        aksi: aksi::DEVICE_TOKEN_ENROLMENT,
        // Token itu sendiri tidak pernah masuk audit log; yang berguna untuk
        // penyelidikan adalah siapa menerbitkan, kapan, dan untuk apa.
        payload: Some(serde_json::json!({
            "alias": req.alias, "expires_in_seconds": ttl,
        })),
    })
    .await;

    tracing::info!(org = %claims.org_id(), ttl, "token enrolment diterbitkan");

    Ok(Sukses::baru(TerbitkanResp {
        enrolment_token: token,
        expires_at,
    }))
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Menukar token dengan pendaftaran perangkat
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct EnrolReq {
    pub enrolment_token: String,
    /// Kunci publik Ed25519, base64url tanpa padding.
    pub public_key: String,
    pub os_type: String,
    pub os_version: Option<String>,
    pub hostname: Option<String>,
    pub alias: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EnrolResp {
    pub device_uuid: Uuid,
    pub device_id: String,
    pub device_id_tampil: String,
    /// Sama seperti pendaftaran lewat dashboard: satu-satunya kali password
    /// sesi dikirim dalam bentuk asli.
    pub session_password: String,
}

/// `POST /api/v1/devices/enrol`
///
/// Tanpa autentikasi pengguna — token enrolment **adalah** kredensialnya.
pub async fn enrol(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    axum::Json(req): axum::Json<EnrolReq>,
) -> ApiResult<Sukses<EnrolResp>> {
    if !matches!(req.os_type.as_str(), "Windows" | "macOS" | "Linux" | "Web") {
        return Err(ApiError::Validasi(
            "os_type harus salah satu dari: Windows, macOS, Linux, Web".into(),
        ));
    }

    // Kunci divalidasi sebelum menyentuh database, dan disimpan sebagai teks
    // base64url apa adanya — bentuk yang sama dengan yang dikirim agent.
    // Menyimpannya sebagai teks membuat perbandingan, indeks unik, dan
    // pembacaan manual saat menyelidiki insiden menjadi sepele.
    let mentah = device_key::dari_base64(&req.public_key)
        .map_err(|_| ApiError::Validasi("public_key bukan base64url yang valid".into()))?;
    if mentah.len() != device_key::KEY_LEN {
        return Err(ApiError::Validasi(format!(
            "public_key harus {} byte", device_key::KEY_LEN
        )));
    }
    // Dinormalkan lewat encode ulang: dua penulisan base64 yang berbeda untuk
    // byte yang sama tidak boleh menghasilkan dua baris berbeda dan lolos dari
    // indeks unik `idx_device_keys_publik_unik`.
    let public_key = device_key::ke_base64(&mentah);

    let mut redis = state.redis.clone();
    let kunci_ip = format!("enrol:{ip}");
    if let Keputusan::Dijeda { .. } = ratelimit::periksa(&mut redis, &kunci_ip).await? {
        return Err(ApiError::TidakTerautentikasi);
    }

    // ── Klaim token ──────────────────────────────────────────────────────────
    // Sekali pakai ditegakkan oleh UPDATE ... WHERE used_at IS NULL RETURNING,
    // bukan oleh SELECT lalu UPDATE. Dua agent yang berlomba dengan token yang
    // sama hanya menghasilkan satu perangkat.
    let klaim: Option<(Uuid, Uuid, Option<String>)> =
        sqlx::query_as("SELECT token_id, org_id, alias FROM claim_enrolment_token($1)")
            .bind(hash_token(&req.enrolment_token))
            .fetch_optional(&state.db)
            .await?;

    let Some((token_id, org_id, alias_token)) = klaim else {
        ratelimit::catat_kegagalan(&mut redis, &kunci_ip, ratelimit::ENROLMENT_PER_IP).await?;
        tracing::warn!(%ip, "enrolment ditolak: token tidak sah, terpakai, atau kedaluwarsa");
        // Satu respons untuk ketiga sebab. Membedakannya memberi tahu pemanggil
        // apakah tokennya pernah ada — informasi yang tidak berhak ia miliki.
        return Err(ApiError::TidakTerautentikasi);
    };

    let alias = req.alias.or(alias_token);
    let sesi_password = rdp_core::password::generate();
    let sesi_hash = hash::hash(&sesi_password).map_err(ApiError::Internal)?;

    // ── Alokasi device ID ────────────────────────────────────────────────────
    let mut terdaftar = None;
    for percobaan in 1..=5 {
        let kandidat = DeviceId::generate();
        let hasil: Option<Uuid> = sqlx::query_scalar(
            "SELECT enrol_device($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(org_id)
        .bind(kandidat.as_str())
        .bind(&public_key)
        .bind(&alias)
        .bind(&req.os_type)
        .bind(&req.os_version)
        .bind(&req.hostname)
        .bind(&sesi_hash)
        .fetch_one(&state.db)
        .await?;

        if let Some(uuid) = hasil {
            terdaftar = Some((uuid, kandidat));
            break;
        }
        tracing::warn!(percobaan, "device ID bertabrakan saat enrolment, mencoba ulang");
    }

    let Some((device_uuid, device_id)) = terdaftar else {
        tracing::error!("gagal mengalokasikan device ID setelah 5 percobaan — ruang ID padat");
        return Err(ApiError::Internal(anyhow::anyhow!("alokasi device ID gagal")));
    };

    // Menautkan token ke perangkat yang dihasilkannya. Constraint
    // `enrolment_terpakai_punya_perangkat` menjaga keduanya tetap konsisten.
    sqlx::query("SELECT link_enrolment_token($1, $2)")
        .bind(token_id)
        .bind(device_uuid)
        .execute(&state.db)
        .await?;

    ratelimit::bersihkan(&mut redis, &kunci_ip).await?;

    audit::catat(&state.db, audit::Entri {
        org_id,
        // Tidak ada pengguna di balik peristiwa ini. Menuliskan penerbit token
        // di sini akan membuat jejak audit mengklaim seseorang melakukan
        // sesuatu yang sebenarnya dilakukan mesin.
        user_id: None,
        ip,
        aksi: aksi::DEVICE_ENROL,
        payload: Some(serde_json::json!({
            "device_id": device_id.as_str(),
            "os_type": req.os_type,
            "enrolment_token_id": token_id,
        })),
    })
    .await;

    tracing::info!(%org_id, device = %device_id, "perangkat ter-enrol dengan kunci Ed25519");

    Ok(Sukses::baru(EnrolResp {
        device_uuid,
        device_id: device_id.to_string(),
        device_id_tampil: device_id.grouped(),
        session_password: sesi_password,
    }))
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. Menukar tanda tangan dengan token perangkat
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct TokenReq {
    pub device_uuid: Uuid,
    /// Unix epoch detik, ikut ditandatangani.
    pub timestamp: i64,
    pub nonce: String,
    /// Tanda tangan Ed25519 atas tantangan, base64url tanpa padding.
    pub signature: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResp {
    pub access_token: String,
    pub expires_in: i64,
    pub org_id: Uuid,
}

/// `POST /api/v1/devices/token`
pub async fn token_perangkat(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    axum::Json(req): axum::Json<TokenReq>,
) -> ApiResult<Sukses<TokenResp>> {
    let mut redis = state.redis.clone();

    // ── Kesegaran stempel waktu ──────────────────────────────────────────────
    if !device_key::stempel_waktu_segar(req.timestamp, Utc::now().timestamp()) {
        catat_upaya(&state, Some(req.device_uuid), ip, "stale_timestamp").await;
        return Err(ApiError::KredensialSalah);
    }

    // ── Nonce sekali pakai ───────────────────────────────────────────────────
    //
    // Diklaim **sebelum** tanda tangan diverifikasi, dan urutan itu disengaja.
    // Yang dilindungi di sini adalah pemutaran ulang request yang tanda
    // tangannya memang sah — bila nonce baru dicatat setelah verifikasi
    // berhasil, request sah yang tersadap dapat dikirim ulang berkali-kali
    // selama jendela stempel waktu masih terbuka.
    //
    // Membalik urutannya juga tidak membuka penolakan layanan: nonce dipilih
    // pemanggil sendiri, jadi penyerang tidak dapat menebak nonce mana yang
    // akan dipakai agent yang sah.
    let kunci_nonce = format!("dev-nonce:{}:{}", req.device_uuid, req.nonce);
    let baru: bool = redis::cmd("SET")
        .arg(&kunci_nonce)
        .arg(1u8)
        .arg("NX")
        .arg("EX")
        .arg(device_key::SKEW_MAX_SECONDS * 2)
        .query_async(&mut redis)
        .await
        .map(|r: Option<String>| r.is_some())?;

    if !baru {
        catat_upaya(&state, Some(req.device_uuid), ip, "replayed_nonce").await;
        tracing::warn!(%ip, device = %req.device_uuid, "nonce diputar ulang");
        return Err(ApiError::KredensialSalah);
    }

    // ── Kunci publik perangkat ───────────────────────────────────────────────
    let baris: Option<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT org_id, public_key FROM resolve_device_key($1)")
            .bind(req.device_uuid)
            .fetch_optional(&state.db)
            .await?;

    let (org_id, public_key) = match baris {
        // Kunci yang dicabut atau kedaluwarsa sudah tersaring di dalam
        // `resolve_device_key`, jadi mendarat di cabang ini berarti benar-benar
        // tidak ada kunci aktif.
        Some((org, Some(pk))) => (org, pk),
        // Perangkat ada tetapi belum pernah enrol — misalnya agent browser yang
        // didaftarkan lewat dashboard. Dibedakan di log, tidak di respons.
        Some((_, None)) => {
            catat_upaya(&state, Some(req.device_uuid), ip, "not_enrolled").await;
            return Err(ApiError::KredensialSalah);
        }
        None => {
            catat_upaya(&state, None, ip, "unknown_device").await;
            return Err(ApiError::KredensialSalah);
        }
    };

    // ── Verifikasi tanda tangan ──────────────────────────────────────────────
    let Ok(public_key) = device_key::dari_base64(&public_key) else {
        // Kunci tersimpan tidak dapat didekode. Bukan kesalahan pemanggil —
        // baris di database yang rusak — tetapi responsnya tetap harus sama.
        tracing::error!(device = %req.device_uuid, "kunci publik tersimpan rusak");
        catat_upaya(&state, Some(req.device_uuid), ip, "not_enrolled").await;
        return Err(ApiError::KredensialSalah);
    };

    let tanda_tangan = device_key::dari_base64(&req.signature)
        .map_err(|_| ApiError::KredensialSalah)?;
    let tantangan = device_key::tantangan(&req.device_uuid, req.timestamp, &req.nonce);

    if !device_key::verifikasi(&public_key, &tantangan, &tanda_tangan) {
        catat_upaya(&state, Some(req.device_uuid), ip, "bad_signature").await;
        tracing::warn!(%ip, device = %req.device_uuid, "tanda tangan perangkat tidak sah");
        return Err(ApiError::KredensialSalah);
    }

    catat_upaya(&state, Some(req.device_uuid), ip, "ok").await;
    tracing::info!(device = %req.device_uuid, "token perangkat diterbitkan");

    Ok(Sukses::baru(TokenResp {
        access_token: state.jwt.terbitkan_perangkat(req.device_uuid, org_id)?,
        expires_in: crate::auth::jwt::DEVICE_TOKEN_TTL_SECONDS,
        org_id,
    }))
}

async fn catat_upaya(state: &AppState, device: Option<Uuid>, ip: std::net::IpAddr, hasil: &str) {
    let r = sqlx::query("SELECT log_device_auth_attempt($1, $2::inet, $3)")
        .bind(device)
        .bind(ip.to_string())
        .bind(hasil)
        .execute(&state.db)
        .await;
    if let Err(e) = r {
        tracing::error!(error = %e, "gagal mencatat upaya autentikasi perangkat");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. Heartbeat
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct HeartbeatReq {
    pub os_version: Option<String>,
    pub hostname: Option<String>,
    pub client_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HeartbeatResp {
    /// Waktu server, supaya agent dapat mengoreksi hanyutan jamnya sendiri
    /// sebelum stempel waktu tanda tangannya keluar dari jendela yang
    /// diterima.
    pub server_time: DateTime<Utc>,
}

/// `POST /api/v1/devices/heartbeat`
///
/// Memperbarui keterjangkauan dan metadata. **Tidak** menyentuh `status` —
/// kehadiran dimiliki Signal Server, dan menulisnya dari dua tempat akan
/// menghidupkan kembali perangkat yang WebSocket-nya baru saja putus.
pub async fn heartbeat(
    State(state): State<AppState>,
    perangkat: PerangkatTerautentikasi,
    axum::Json(req): axum::Json<HeartbeatReq>,
) -> ApiResult<Sukses<HeartbeatResp>> {
    let ada: bool = sqlx::query_scalar("SELECT device_heartbeat($1, $2, $3, $4, $5)")
        .bind(perangkat.device_uuid)
        .bind(perangkat.org_id)
        .bind(&req.os_version)
        .bind(&req.hostname)
        .bind(&req.client_version)
        .fetch_one(&state.db)
        .await?;

    if !ada {
        // Token masih berlaku tetapi perangkatnya sudah tidak ada — dihapus
        // dari dashboard saat agent sedang berjalan. Agent perlu tahu supaya
        // berhenti mencoba, bukan mengulang selamanya.
        return Err(ApiError::TidakDitemukan("perangkat"));
    }

    Ok(Sukses::baru(HeartbeatResp {
        server_time: Utc::now(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_dijepit_ke_rentang_wajar() {
        let jepit = |v: i64| v.clamp(ENROLMENT_TTL_MIN, ENROLMENT_TTL_MAKS);
        assert_eq!(jepit(0), ENROLMENT_TTL_MIN);
        assert_eq!(jepit(-999), ENROLMENT_TTL_MIN);
        assert_eq!(jepit(999_999), ENROLMENT_TTL_MAKS);
        assert_eq!(jepit(3_600), 3_600);
    }

    #[test]
    fn token_enrolment_beruang_256_bit() {
        assert_eq!(ENROLMENT_TOKEN_BYTES * 8, 256);
    }

    #[test]
    fn hash_token_menghasilkan_sha256() {
        let h = hash_token("apa pun");
        assert_eq!(h.len(), 32, "constraint enrolment_token_hash_sha256 akan menolak");
    }

    #[test]
    fn hash_token_deterministik_dan_berbeda_per_masukan() {
        assert_eq!(hash_token("a"), hash_token("a"));
        assert_ne!(hash_token("a"), hash_token("b"));
    }

    #[test]
    fn token_tidak_pernah_disimpan_apa_adanya() {
        // Penjaga regresi: bila seseorang mengganti hash_token dengan fungsi
        // identitas, baris ini gagal.
        let t = "token-rahasia";
        let h = hash_token(t);
        assert_ne!(h, t.as_bytes().to_vec());
    }

    #[test]
    fn ttl_baku_di_dalam_rentang() {
        assert!(ENROLMENT_TTL_BAKU >= ENROLMENT_TTL_MIN);
        assert!(ENROLMENT_TTL_BAKU <= ENROLMENT_TTL_MAKS);
    }
}
