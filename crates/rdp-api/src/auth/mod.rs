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

/// Ekstraktor untuk endpoint yang membutuhkan user terautentikasi.
///
/// Menaruhnya sebagai ekstraktor, bukan middleware, membuat kebutuhan auth
/// terbaca langsung dari tanda tangan handler — sebuah handler yang tidak
/// menyebut `Terautentikasi` memang tidak terlindungi, dan itu terlihat saat
/// review, bukan tersembunyi di tabel routing.
#[derive(Debug, Clone)]
pub struct Terautentikasi(pub Claims);

impl FromRequestParts<AppState> for Terautentikasi {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
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

        Ok(Self(state.jwt.verifikasi(token)?))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn prefix_bearer_wajib_persis() {
        // Menegaskan bentuk yang diterima; `bearer` huruf kecil dan `Bearer:`
        // sama-sama ditolak oleh strip_prefix.
        assert_eq!("Bearer abc".strip_prefix("Bearer "), Some("abc"));
        assert_eq!("bearer abc".strip_prefix("Bearer "), None);
        assert_eq!("Bearer".strip_prefix("Bearer "), None);
    }
}
