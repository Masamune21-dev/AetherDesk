//! Swalayan perangkat.
//!
//! Endpoint yang dipanggil agent dengan **token perangkat**, bukan sesi
//! pengguna. Inilah yang membuat aplikasi Windows dapat menampilkan dan
//! mengubah identitasnya sendiri tanpa pemiliknya perlu membuka dashboard —
//! persis kebiasaan yang sudah dikenal orang dari UltraViewer dan AnyDesk.
//!
//! Perangkat hanya pernah dapat menyentuh dirinya sendiri: UUID tidak diambil
//! dari badan permintaan melainkan dari klaim token yang sudah diverifikasi,
//! dan tenant selalu ikut menjadi syarat di sisi SQL.

use crate::{
    audit::{self, aksi},
    auth::{hash, PerangkatTerautentikasi},
    error::{ApiError, ApiResult, Sukses},
    net::IpKlien,
    state::AppState,
};
use axum::extract::State;
use rdp_core::DeviceId;
use serde::{Deserialize, Serialize};

/// Panjang minimum kata sandi tetap.
///
/// Kata sandi sesi beruang 40 bit karena dibangkitkan mesin. Yang ini dipilih
/// manusia, dan manusia memilih buruk — panjang minimum adalah satu-satunya
/// pertahanan yang benar-benar dapat dipaksakan tanpa menebak-nebak kualitas
/// pilihan seseorang. Sepuluh karakter, bukan delapan: perbedaannya kecil bagi
/// yang mengetik, besar bagi yang menebak.
const SANDI_TETAP_MIN: usize = 10;

/// Beberapa kata sandi yang paling sering dipakai. Bukan daftar yang lengkap —
/// tidak ada daftar yang lengkap — tetapi menolak yang paling jelas lebih baik
/// daripada tidak menolak apa pun.
const SANDI_TERLARANG: &[&str] = &[
    "1234567890",
    "0123456789",
    "password12",
    "qwertyuiop",
    "aaaaaaaaaa",
    "1111111111",
];

// ── Ringkasan diri ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DiriResp {
    pub device_id: String,
    /// Bentuk berkelompok `942 716 382`, untuk ditampilkan dan dibacakan.
    pub device_id_tampil: String,
    pub handle: Option<String>,
    pub alias: Option<String>,
    pub org_slug: String,
    pub org_name: String,
    pub punya_sandi_tetap: bool,
    pub status: String,
}

/// `GET /api/v1/devices/self`
pub async fn diri(
    State(state): State<AppState>,
    perangkat: PerangkatTerautentikasi,
) -> ApiResult<Sukses<DiriResp>> {
    let baris: Option<(String, Option<String>, Option<String>, String, String, bool, String)> =
        sqlx::query_as(
            "SELECT device_id, handle, alias, org_slug, org_name,
                    punya_sandi_tetap, status
             FROM device_self($1, $2)",
        )
        .bind(perangkat.device_uuid)
        .bind(perangkat.org_id)
        .fetch_optional(&state.db)
        .await?;

    let Some((device_id, handle, alias, org_slug, org_name, punya, status)) = baris else {
        return Err(ApiError::TidakDitemukan("perangkat"));
    };

    let tampil = DeviceId::parse(device_id.trim())
        .map(|d| d.grouped())
        .unwrap_or_else(|_| device_id.clone());

    Ok(Sukses::baru(DiriResp {
        device_id: device_id.trim().to_string(),
        device_id_tampil: tampil,
        handle,
        alias,
        org_slug,
        org_name,
        punya_sandi_tetap: punya,
        status,
    }))
}

// ── Alias ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HandleReq {
    /// Kosong berarti menghapus alias.
    pub handle: Option<String>,
}

/// `PUT /api/v1/devices/self/handle`
pub async fn set_handle(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    perangkat: PerangkatTerautentikasi,
    axum::Json(req): axum::Json<HandleReq>,
) -> ApiResult<Sukses<serde_json::Value>> {
    let handle = match req.handle.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(h) => Some(periksa_handle(h)?),
        None => None,
    };

    let hasil: bool = sqlx::query_scalar("SELECT set_device_handle($1, $2, $3)")
        .bind(perangkat.device_uuid)
        .bind(perangkat.org_id)
        .bind(&handle)
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            // Basis data menegakkan keunikan; menerjemahkannya di sini
            // menghasilkan pesan yang dapat ditindaklanjuti pengguna alih-alih
            // "kesalahan internal".
            if let Some(db) = e.as_database_error() {
                if db.is_unique_violation() {
                    return ApiError::Konflik("alias itu sudah dipakai perangkat lain".into());
                }
            }
            ApiError::from(e)
        })?;

    if !hasil {
        return Err(ApiError::TidakDitemukan("perangkat"));
    }

    audit::catat(&state.db, audit::Entri {
        org_id: perangkat.org_id,
        user_id: None,
        ip,
        aksi: aksi::DEVICE_ALIAS_DIUBAH,
        payload: Some(serde_json::json!({
            "device_uuid": perangkat.device_uuid, "handle": handle,
        })),
    })
    .await;

    Ok(Sukses::baru(serde_json::json!({ "handle": handle })))
}

/// Memeriksa bentuk alias sebelum menyentuh basis data.
///
/// Batasannya sama persis dengan constraint di migrasi 0006. Diperiksa dua kali
/// dengan sengaja: yang di sini menghasilkan pesan yang menjelaskan, yang di
/// sana memastikan tidak ada jalur lain yang dapat melewatinya.
fn periksa_handle(h: &str) -> Result<String, ApiError> {
    let h = h.to_lowercase();

    if h.len() < 3 || h.len() > 32 {
        return Err(ApiError::Validasi(
            "alias harus 3 sampai 32 karakter".into(),
        ));
    }
    if h.len() == 9 && h.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError::Validasi(
            "alias tidak boleh berupa sembilan digit — bentuk itu milik nomor perangkat".into(),
        ));
    }
    if !h
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(ApiError::Validasi(
            "alias hanya boleh memuat huruf, angka, tanda hubung, dan garis bawah".into(),
        ));
    }
    if !h.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) {
        return Err(ApiError::Validasi(
            "alias harus diawali huruf atau angka".into(),
        ));
    }

    Ok(h)
}

// ── Kata sandi ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SandiReq {
    /// Bila benar, kata sandi sesi dibangkitkan ulang secara acak.
    #[serde(default)]
    pub rotasi_sesi: bool,
    /// Kata sandi tetap baru. `Some("")` berarti mematikan akses tanpa
    /// pengawasan; `None` berarti jangan sentuh.
    pub sandi_tetap: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SandiResp {
    /// Hanya terisi bila kata sandi sesi baru saja dirotasi.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_password: Option<String>,
    pub punya_sandi_tetap: bool,
}

/// `PUT /api/v1/devices/self/passwords`
pub async fn set_sandi(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    perangkat: PerangkatTerautentikasi,
    axum::Json(req): axum::Json<SandiReq>,
) -> ApiResult<Sukses<SandiResp>> {
    let sesi_baru = req.rotasi_sesi.then(rdp_core::password::generate);
    let sesi_hash = match &sesi_baru {
        Some(p) => Some(hash::hash(p).map_err(ApiError::Internal)?),
        None => None,
    };

    let (ubah_tetap, tetap_hash) = match req.sandi_tetap.as_deref() {
        None => (false, None),
        Some("") => (true, None),
        Some(p) => {
            periksa_sandi_tetap(p)?;
            (true, Some(hash::hash(p).map_err(ApiError::Internal)?))
        }
    };

    let hasil: bool = sqlx::query_scalar("SELECT set_device_passwords($1, $2, $3, $4, $5, $6)")
        .bind(perangkat.device_uuid)
        .bind(perangkat.org_id)
        .bind(&sesi_hash)
        .bind(&tetap_hash)
        .bind(req.rotasi_sesi)
        .bind(ubah_tetap)
        .fetch_one(&state.db)
        .await?;

    if !hasil {
        return Err(ApiError::TidakDitemukan("perangkat"));
    }

    audit::catat(&state.db, audit::Entri {
        org_id: perangkat.org_id,
        user_id: None,
        ip,
        aksi: aksi::DEVICE_SANDI_DIROTASI,
        payload: Some(serde_json::json!({
            "device_uuid": perangkat.device_uuid,
            "rotasi_sesi": req.rotasi_sesi,
            "ubah_sandi_tetap": ubah_tetap,
            "sandi_tetap_aktif": tetap_hash.is_some(),
        })),
    })
    .await;

    Ok(Sukses::baru(SandiResp {
        session_password: sesi_baru,
        punya_sandi_tetap: tetap_hash.is_some(),
    }))
}

fn periksa_sandi_tetap(p: &str) -> Result<(), ApiError> {
    if p.chars().count() < SANDI_TETAP_MIN {
        return Err(ApiError::Validasi(format!(
            "kata sandi tetap minimal {SANDI_TETAP_MIN} karakter"
        )));
    }
    let rendah = p.to_lowercase();
    if SANDI_TERLARANG.contains(&rendah.as_str()) {
        return Err(ApiError::Validasi(
            "kata sandi itu terlalu umum — mesin ini dapat diakses siapa pun yang menebaknya"
                .into(),
        ));
    }
    // Satu karakter berulang lolos panjang minimum tetapi tidak menahan apa pun.
    if p.chars().collect::<std::collections::HashSet<_>>().len() < 4 {
        return Err(ApiError::Validasi(
            "kata sandi tetap harus memuat setidaknya empat karakter berbeda".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_wajar_diterima() {
        assert_eq!(periksa_handle("pc-kantor").unwrap(), "pc-kantor");
        assert_eq!(periksa_handle("Laptop_01").unwrap(), "laptop_01");
        assert_eq!(periksa_handle("  PC-Gudang  ").unwrap_or_default(), "");
    }

    #[test]
    fn alias_dinormalkan_ke_huruf_kecil() {
        // Tanpa ini `PC-Kantor` dan `pc-kantor` menjadi dua perangkat berbeda
        // yang membingungkan siapa pun yang mengetiknya.
        assert_eq!(periksa_handle("PC-KANTOR").unwrap(), "pc-kantor");
    }

    #[test]
    fn alias_berupa_sembilan_digit_ditolak() {
        // Bentuk itu milik nomor perangkat. Alias yang menyamar sebagai nomor
        // membuat Quick Connect tidak dapat memutuskan mana yang dimaksud.
        let e = periksa_handle("123456789").unwrap_err().to_string();
        assert!(e.contains("sembilan digit"), "{e}");
        // Delapan dan sepuluh digit tetap boleh.
        assert!(periksa_handle("12345678").is_ok());
        assert!(periksa_handle("1234567890").is_ok());
    }

    #[test]
    fn alias_cacat_ditolak() {
        for buruk in ["ab", "pc kantor", "-awal", "_awal", "pc@kantor", "pc.kantor"] {
            assert!(periksa_handle(buruk).is_err(), "diterima padahal cacat: {buruk}");
        }
        assert!(periksa_handle(&"a".repeat(33)).is_err(), "terlalu panjang");
    }

    #[test]
    fn sandi_tetap_pendek_ditolak() {
        assert!(periksa_sandi_tetap("pendek").is_err());
        assert!(periksa_sandi_tetap("sembilan9").is_err(), "sembilan karakter");
        assert!(periksa_sandi_tetap("sepuluhKar").is_ok());
    }

    #[test]
    fn sandi_tetap_umum_ditolak() {
        assert!(periksa_sandi_tetap("1234567890").is_err());
        assert!(periksa_sandi_tetap("QWERTYUIOP").is_err(), "besar-kecil diabaikan");
    }

    #[test]
    fn sandi_tetap_seragam_ditolak() {
        // Lolos panjang minimum, tidak menahan apa pun.
        assert!(periksa_sandi_tetap("abababababab").is_err());
        assert!(periksa_sandi_tetap("aaaaaaaaaaaa").is_err());
    }

    #[test]
    fn ambang_lebih_ketat_dari_sandi_sesi() {
        // Kata sandi sesi delapan karakter karena dibangkitkan mesin. Yang
        // dipilih manusia harus lebih panjang untuk mendekati ketahanan yang
        // sama.
        assert!(SANDI_TETAP_MIN > rdp_core::password::LEN);
    }
}
