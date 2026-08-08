//! Endpoint autentikasi.

use crate::{
    audit::{self, aksi},
    auth::{
        hash,
        jwt::ACCESS_TOKEN_TTL_SECONDS,
        refresh::{self, SesiRefresh},
        Terautentikasi,
    },
    db,
    error::{ApiError, ApiResult, Sukses},
    state::AppState,
};
use crate::net::IpKlien;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Bootstrap ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BootstrapReq {
    pub org_name: String,
    pub org_slug: String,
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct BootstrapResp {
    pub org_id: Uuid,
    pub user_id: Uuid,
}

/// `POST /api/v1/auth/bootstrap` — membuat organisasi pertama beserta pemiliknya.
///
/// Endpoint ini **hanya berfungsi selama belum ada organisasi apa pun**. Setelah
/// itu ia mengembalikan 409 selamanya. Pola ini menghindari kebutuhan akan
/// setup token yang harus disimpan dan dirotasi, sementara jendela paparannya
/// tertutup sendiri begitu instalasi dipakai.
pub async fn bootstrap(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    axum::Json(req): axum::Json<BootstrapReq>,
) -> ApiResult<Sukses<BootstrapResp>> {
    validasi_slug(&req.org_slug)?;
    validasi_password(&req.password)?;
    if !req.email.contains('@') {
        return Err(ApiError::Validasi("email tidak valid".into()));
    }

    let sudah_ada: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations")
        .fetch_one(&state.db)
        .await?;
    if sudah_ada > 0 {
        return Err(ApiError::Konflik(
            "instalasi ini sudah memiliki organisasi".into(),
        ));
    }

    let org_id = Uuid::new_v4();
    let password_hash = hash::hash(&req.password).map_err(ApiError::Internal)?;

    // RLS berlaku pada organizations juga, jadi id dibangkitkan lebih dulu
    // supaya transaksi dapat menetapkan tenant sebelum menyisipkan barisnya.
    let mut tx = db::mulai_transaksi_tenant(&state.db, org_id).await?;

    sqlx::query("INSERT INTO organizations (id, name, slug) VALUES ($1, $2, $3)")
        .bind(org_id)
        .bind(&req.org_name)
        .bind(&req.org_slug)
        .execute(&mut *tx)
        .await?;

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (organization_id, email, password_hash, name)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(org_id)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.name)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    audit::catat(&state.db, audit::Entri {
        org_id, user_id: Some(user_id), ip, aksi: aksi::ORG_DIBUAT,
        payload: Some(serde_json::json!({ "slug": req.org_slug })),
    }).await;

    tracing::info!(%org_id, %user_id, slug = %req.org_slug, "organisasi pertama dibuat");
    Ok(Sukses::baru(BootstrapResp { org_id, user_id }))
}

// ── Login ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    /// Wajib. Konsekuensi langsung dari T-05: email hanya unik **per
    /// organisasi**, jadi `email + password` saja tidak lagi menunjuk ke satu
    /// orang. Dua organisasi boleh punya `erik@msp.id` yang berbeda.
    pub org_slug: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResp {
    pub access_token: String,
    /// Ditukar lewat `/auth/refresh` saat access token kedaluwarsa.
    /// Tanpa ini, sesi mati total setiap 15 menit — dan agent yang seharusnya
    /// berbagi layar berjam-jam ikut terputus.
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

/// `POST /api/v1/auth/login`
pub async fn login(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    axum::Json(req): axum::Json<LoginReq>,
) -> ApiResult<Sukses<LoginResp>> {
    // Lookup lintas-tenant lewat SECURITY DEFINER — pada titik ini tenant
    // belum diketahui, sehingga RLS belum dapat diterapkan (migrasi 0002).
    let baris: Option<(Uuid, Uuid, String, String, bool)> = sqlx::query_as(
        "SELECT user_id, org_id, password_hash, status, mfa_enabled
         FROM resolve_login($1, $2)",
    )
    .bind(&req.org_slug)
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await?;

    let Some((user_id, org_id, password_hash, status, _mfa)) = baris else {
        // Tetap jalankan Argon2id meski akun tidak ada, supaya lama respons
        // tidak memberi tahu penyerang akun mana yang hidup.
        hash::verify_dummy(&req.password);
        return Err(ApiError::KredensialSalah);
    };

    if !hash::verify(&req.password, &password_hash) {
        audit::catat(&state.db, audit::Entri {
            org_id, user_id: Some(user_id), ip, aksi: aksi::LOGIN_GAGAL,
            payload: Some(serde_json::json!({ "sebab": "password_salah" })),
        }).await;
        tracing::info!(%org_id, %user_id, "login gagal: password salah");
        return Err(ApiError::KredensialSalah);
    }

    if status != "active" {
        // Dibedakan dari password salah secara sengaja: pemiliknya berhak tahu
        // akunnya ditangguhkan, dan penyerang sudah membuktikan tahu password.
        return Err(ApiError::IzinDitolak);
    }

    let token = state.jwt.terbitkan(user_id, org_id, &req.email)?;

    let refresh_token = refresh::buat();
    let mut redis = state.redis.clone();
    refresh::simpan(
        &mut redis,
        &refresh_token,
        &SesiRefresh { user_id, org_id, email: req.email.clone() },
    )
    .await?;

    audit::catat(&state.db, audit::Entri {
        org_id, user_id: Some(user_id), ip, aksi: aksi::LOGIN, payload: None,
    }).await;

    tracing::info!(%org_id, %user_id, "login berhasil");

    Ok(Sukses::baru(LoginResp {
        access_token: token,
        refresh_token,
        token_type: "Bearer",
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
    }))
}

// ── Refresh ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RefreshReq {
    pub refresh_token: String,
}

/// `POST /api/v1/auth/refresh`
///
/// Menukar refresh token dengan pasangan token baru. Token lama langsung
/// dihapus — rotasi sekali pakai, sesuai praktik OAuth untuk klien publik.
pub async fn refresh_token(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<RefreshReq>,
) -> ApiResult<Sukses<LoginResp>> {
    let mut redis = state.redis.clone();
    let sesi = refresh::tukar(&mut redis, &req.refresh_token).await?;

    let access = state.jwt.terbitkan(sesi.user_id, sesi.org_id, &sesi.email)?;
    let baru = refresh::buat();
    refresh::simpan(&mut redis, &baru, &sesi).await?;

    tracing::debug!(user_id = %sesi.user_id, "token diperbarui");

    Ok(Sukses::baru(LoginResp {
        access_token: access,
        refresh_token: baru,
        token_type: "Bearer",
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
    }))
}

// ── Logout ───────────────────────────────────────────────────────────────────

/// `POST /api/v1/auth/logout`
///
/// Mencabut refresh token. Access token yang sudah terbit tetap berlaku sampai
/// kedaluwarsa — itu sifat JWT tanpa daftar cabut, dan 15 menit adalah harga
/// yang sengaja dipilih untuk itu.
pub async fn logout(
    State(state): State<AppState>,
    axum::Json(req): axum::Json<RefreshReq>,
) -> ApiResult<Sukses<serde_json::Value>> {
    let mut redis = state.redis.clone();
    refresh::cabut(&mut redis, &req.refresh_token).await?;
    Ok(Sukses::baru(serde_json::json!({ "revoked": true })))
}

// ── Profil ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MeResp {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub name: String,
    pub org_name: String,
}

/// `GET /api/v1/auth/me`
pub async fn me(
    State(state): State<AppState>,
    Terautentikasi(claims): Terautentikasi,
) -> ApiResult<Sukses<MeResp>> {
    let mut tx = db::mulai_transaksi_tenant(&state.db, claims.org_id()).await?;

    let baris: Option<(String, String, String)> = sqlx::query_as(
        "SELECT u.email, u.name, o.name
         FROM users u JOIN organizations o ON o.id = u.organization_id
         WHERE u.id = $1",
    )
    .bind(claims.user_id())
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;

    let (email, name, org_name) = baris.ok_or(ApiError::TidakDitemukan("pengguna"))?;

    Ok(Sukses::baru(MeResp {
        user_id: claims.user_id(),
        org_id: claims.org_id(),
        email,
        name,
        org_name,
    }))
}

// ── Validasi ─────────────────────────────────────────────────────────────────

fn validasi_slug(slug: &str) -> ApiResult<()> {
    if slug.len() < 2 || slug.len() > 100 {
        return Err(ApiError::Validasi("slug harus 2-100 karakter".into()));
    }
    if !slug
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(ApiError::Validasi(
            "slug hanya boleh huruf kecil, angka, dan tanda hubung".into(),
        ));
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(ApiError::Validasi(
            "slug tidak boleh diawali atau diakhiri tanda hubung".into(),
        ));
    }
    Ok(())
}

fn validasi_password(p: &str) -> ApiResult<()> {
    // Panjang minimum 12 mengikuti panduan NIST SP 800-63B, yang menekankan
    // panjang dan menolak aturan komposisi wajib.
    if p.chars().count() < 12 {
        return Err(ApiError::Validasi("password minimal 12 karakter".into()));
    }
    if p.chars().count() > 256 {
        return Err(ApiError::Validasi("password maksimal 256 karakter".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_valid_diterima() {
        for s in ["alpha", "msp-alpha", "klien-01", "ab"] {
            assert!(validasi_slug(s).is_ok(), "seharusnya valid: {s}");
        }
    }

    #[test]
    fn slug_cacat_ditolak() {
        for s in ["A", "Alpha", "msp_alpha", "-alpha", "alpha-", "a", "spasi ada"] {
            assert!(validasi_slug(s).is_err(), "seharusnya ditolak: {s}");
        }
    }

    #[test]
    fn password_pendek_ditolak() {
        assert!(validasi_password("pendek").is_err());
        assert!(validasi_password("12345678901").is_err()); // 11
        assert!(validasi_password("123456789012").is_ok()); // 12
    }

    #[test]
    fn password_panjang_ditolak_agar_argon2_tidak_dibanjiri() {
        let panjang = "a".repeat(257);
        assert!(validasi_password(&panjang).is_err());
    }

    #[test]
    fn password_menghitung_karakter_bukan_byte() {
        // 12 karakter CJK = 36 byte. Harus diterima.
        assert!(validasi_password("あいうえおかきくけこさし").is_ok());
    }
}
