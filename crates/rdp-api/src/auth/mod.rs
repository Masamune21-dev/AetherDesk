//! Autentikasi: hashing password, JWT, dan ekstraktor request.

pub mod hash;
pub mod jwt;
pub mod refresh;

use crate::{
    error::ApiError,
    state::AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use jwt::Claims;
use uuid::Uuid;

/// Mengambil dan memverifikasi token dari header `Authorization`.
fn klaim_dari_header(parts: &Parts, state: &AppState) -> Result<Claims, ApiError> {
    let header = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::TidakTerautentikasi)?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or(ApiError::TidakTerautentikasi)?
        .trim();

    if token.is_empty() {
        return Err(ApiError::TidakTerautentikasi);
    }

    state.jwt.verifikasi(token)
}

/// Ekstraktor untuk endpoint yang membutuhkan user terautentikasi.
///
/// Menaruhnya sebagai ekstraktor, bukan middleware, membuat kebutuhan auth
/// terbaca langsung dari tanda tangan handler — sebuah handler yang tidak
/// menyebut `Terautentikasi` memang tidak terlindungi, dan itu terlihat saat
/// review, bukan tersembunyi di tabel routing.
///
/// **Token perangkat ditolak di sini.** Sejak identitas perangkat ada, kedua
/// jenis token ditandatangani kunci yang sama dan sama-sama lolos verifikasi
/// kriptografis. Tanpa pemeriksaan `typ`, agent mana pun dapat memanggil
/// seluruh endpoint pengguna — mendaftarkan perangkat baru, menerbitkan token
/// enrolment, membuka sesi ke perangkat lain di organisasinya. Perangkat yang
/// disusupi lalu menjadi pijakan untuk seluruh organisasi, bukan satu mesin.
#[derive(Debug, Clone)]
pub struct Terautentikasi(pub Claims);

impl FromRequestParts<AppState> for Terautentikasi {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let claims = klaim_dari_header(parts, state)?;

        if claims.adalah_perangkat() {
            tracing::warn!(
                device = ?claims.dev,
                "token perangkat dipakai pada endpoint pengguna"
            );
            return Err(ApiError::IzinDitolak);
        }

        Ok(Self(claims))
    }
}

/// Ekstraktor untuk endpoint yang dipanggil agent memakai token perangkat.
///
/// Kebalikannya juga ditegakkan: token pengguna **tidak** diterima di sini.
/// Bukan demi keamanan — pengguna memang berhak atas perangkatnya sendiri —
/// melainkan supaya `device_uuid` tidak pernah ambigu. Endpoint seperti
/// heartbeat menulis baris milik satu perangkat tertentu, dan satu-satunya
/// sumber yang tidak dapat dipalsukan pemanggil adalah klaim di dalam token.
#[derive(Debug, Clone)]
pub struct PerangkatTerautentikasi {
    pub device_uuid: Uuid,
    pub org_id: Uuid,
}

impl FromRequestParts<AppState> for PerangkatTerautentikasi {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let claims = klaim_dari_header(parts, state)?;

        let Some(device_uuid) = claims.device_uuid() else {
            return Err(ApiError::IzinDitolak);
        };

        Ok(Self {
            device_uuid,
            org_id: claims.org_id(),
        })
    }
}

/// Ekstraktor untuk endpoint yang melayani **keduanya**.
///
/// Sengaja langka. Sebagian besar endpoint memang milik salah satu pihak saja,
/// dan memisahkannya adalah yang menahan perangkat tersusupi agar tidak menjadi
/// pijakan ke seluruh organisasi.
///
/// Kredensial TURN adalah pengecualian yang sah: relay dibutuhkan oleh **kedua
/// ujung** sebuah sesi, dan agent yang tidak dapat memperolehnya akan gagal
/// tepat pada jaringan yang paling membutuhkannya.
#[derive(Debug, Clone)]
pub struct SubjekTerautentikasi {
    pub org_id: Uuid,
    /// User id, atau device uuid bila pemanggilnya perangkat.
    pub subjek: Uuid,
    pub adalah_perangkat: bool,
}

impl FromRequestParts<AppState> for SubjekTerautentikasi {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let claims = klaim_dari_header(parts, state)?;
        Ok(Self {
            org_id: claims.org_id(),
            subjek: claims.sub,
            adalah_perangkat: claims.adalah_perangkat(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::jenis;

    #[test]
    fn prefix_bearer_wajib_persis() {
        // Menegaskan bentuk yang diterima; `bearer` huruf kecil dan `Bearer:`
        // sama-sama ditolak oleh strip_prefix.
        assert_eq!("Bearer abc".strip_prefix("Bearer "), Some("abc"));
        assert_eq!("bearer abc".strip_prefix("Bearer "), None);
        assert_eq!("Bearer".strip_prefix("Bearer "), None);
    }

    fn klaim(typ: &str, dev: Option<Uuid>) -> Claims {
        Claims {
            sub: dev.unwrap_or_else(Uuid::new_v4),
            org: Uuid::new_v4(),
            email: String::new(),
            exp: 0,
            iat: 0,
            iss: "aetherdesk".into(),
            typ: typ.into(),
            dev,
        }
    }

    #[test]
    fn token_perangkat_bukan_token_pengguna() {
        // Properti yang menahan perangkat tersusupi agar tidak menjadi pijakan
        // ke seluruh organisasi. Ekstraktornya sendiri butuh AppState untuk
        // diuji utuh, jadi yang dikunci di sini adalah predikat yang
        // dipakainya.
        let perangkat = klaim(jenis::PERANGKAT, Some(Uuid::new_v4()));
        assert!(perangkat.adalah_perangkat(), "endpoint pengguna akan menerimanya");

        let pengguna = klaim(jenis::PENGGUNA, None);
        assert!(!pengguna.adalah_perangkat());
        assert_eq!(pengguna.device_uuid(), None, "endpoint perangkat akan menerimanya");
    }

    #[test]
    fn token_perangkat_tanpa_klaim_dev_ditolak() {
        // Bentuk yang seharusnya mustahil, karena kita sendiri yang
        // menerbitkan token. Tetap ditahan: bug penerbitan tidak boleh
        // berubah menjadi lubang otorisasi.
        let cacat = klaim(jenis::PERANGKAT, None);
        assert_eq!(cacat.device_uuid(), None);
    }
}
