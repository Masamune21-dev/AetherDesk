//! Error API dan amplop respons.
//!
//! Bentuk amplop mengikuti API.md §3 supaya klien hanya perlu mengenal satu
//! struktur. Aturan yang mengikat di sini: **pesan internal tidak pernah bocor
//! ke pemanggil.** Setiap varian memutuskan sendiri apa yang layak dilihat
//! klien; detailnya masuk ke log, bukan ke respons.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("input tidak valid: {0}")]
    Validasi(String),

    #[error("kredensial tidak valid")]
    KredensialSalah,

    #[error("tidak terautentikasi")]
    TidakTerautentikasi,

    #[error("izin ditolak")]
    IzinDitolak,

    #[error("{0} tidak ditemukan")]
    TidakDitemukan(&'static str),

    #[error("konflik: {0}")]
    Konflik(String),

    #[error("terlalu banyak percobaan")]
    Dijeda { retry_after_seconds: u64 },

    /// Kegagalan Quick Connect. Sengaja **satu** varian untuk seluruh sebab —
    /// ID tidak ada, password salah, maupun sedang dijeda. Membedakannya di
    /// respons akan memberi tahu penyerang ID mana yang hidup.
    /// (QUICK_CONNECT.md §5.1)
    #[error("device ID atau password salah")]
    ConnectDitolak,

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Redis(#[from] redis::RedisError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    fn code(&self) -> &'static str {
        match self {
            Self::Validasi(_) => "VALIDATION_FAILED",
            Self::KredensialSalah => "INVALID_CREDENTIALS",
            Self::TidakTerautentikasi => "UNAUTHENTICATED",
            Self::IzinDitolak => "PERMISSION_DENIED",
            Self::TidakDitemukan(_) => "NOT_FOUND",
            Self::Konflik(_) => "CONFLICT",
            Self::Dijeda { .. } => "RATE_LIMITED",
            Self::ConnectDitolak => "CONNECT_REJECTED",
            Self::Database(_) | Self::Redis(_) | Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Validasi(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::KredensialSalah | Self::TidakTerautentikasi => StatusCode::UNAUTHORIZED,
            Self::IzinDitolak => StatusCode::FORBIDDEN,
            Self::TidakDitemukan(_) => StatusCode::NOT_FOUND,
            Self::Konflik(_) => StatusCode::CONFLICT,
            Self::Dijeda { .. } => StatusCode::TOO_MANY_REQUESTS,
            // Quick Connect yang ditolak adalah kegagalan autentikasi.
            Self::ConnectDitolak => StatusCode::UNAUTHORIZED,
            Self::Database(_) | Self::Redis(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    /// Pesan yang boleh dilihat klien.
    fn pesan_publik(&self) -> String {
        match self {
            // Error infrastruktur membocorkan bentuk sistem — nama tabel, host,
            // versi driver. Selalu diganti pesan generik.
            Self::Database(_) | Self::Redis(_) | Self::Internal(_) => {
                "Terjadi kesalahan internal".to_string()
            }
            lain => lain.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Sebab sesungguhnya dicatat di sini, sekali, lengkap.
        match &self {
            Self::Database(e) => tracing::error!(error = %e, "kesalahan database"),
            Self::Redis(e) => tracing::error!(error = %e, "kesalahan redis"),
            Self::Internal(e) => tracing::error!(error = ?e, "kesalahan internal"),
            lain => tracing::debug!(error = %lain, "permintaan ditolak"),
        }

        let mut body = json!({
            "error": {
                "code": self.code(),
                "message": self.pesan_publik(),
            }
        });

        if let Self::Dijeda {
            retry_after_seconds,
        } = &self
        {
            body["error"]["retry_after_seconds"] = json!(retry_after_seconds);
        }

        (self.status(), Json(body)).into_response()
    }
}

/// Amplop sukses standar (API.md §3).
#[derive(Debug, Serialize)]
pub struct Sukses<T> {
    pub data: T,
}

impl<T: Serialize> Sukses<T> {
    pub fn baru(data: T) -> Self {
        Self { data }
    }
}

impl<T: Serialize> IntoResponse for Sukses<T> {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_infrastruktur_tidak_membocorkan_detail() {
        let e = ApiError::Internal(anyhow::anyhow!("koneksi ke 10.0.0.5:5432 gagal"));
        let pesan = e.pesan_publik();
        assert!(!pesan.contains("10.0.0.5"), "IP internal bocor: {pesan}");
        assert!(!pesan.contains("5432"), "port internal bocor: {pesan}");
        assert_eq!(e.code(), "INTERNAL_ERROR");
    }

    #[test]
    fn connect_ditolak_selalu_pesan_sama() {
        // Sebab apa pun harus menghasilkan pesan identik.
        let e = ApiError::ConnectDitolak;
        assert_eq!(e.code(), "CONNECT_REJECTED");
        assert_eq!(e.pesan_publik(), "device ID atau password salah");
        assert_eq!(e.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn status_http_sesuai_semantik() {
        assert_eq!(
            ApiError::TidakDitemukan("device").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::Dijeda { retry_after_seconds: 900 }.status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            ApiError::IzinDitolak.status(),
            StatusCode::FORBIDDEN
        );
    }
}
