//! Penerbitan dan verifikasi JWT.
//!
//! ADR-008 menetapkan **EdDSA (Ed25519)**, bukan HMAC. Dokumen sebelumnya
//! saling bertentangan: contoh token memakai `RS256` sementara konfigurasi
//! menyediakan `jwt_secret` tunggal yang menyiratkan kunci simetris.
//!
//! Bedanya bukan kosmetik. Dengan HMAC, setiap layanan yang perlu
//! **memverifikasi** token juga memegang kemampuan **menerbitkannya**. Dengan
//! Ed25519, kunci privat cukup ada di API Server; verifier lain hanya
//! memerlukan kunci publik.

use crate::error::{ApiError, ApiResult};
use anyhow::{Context, Result};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Masa berlaku access token — ARCHITECTURE.md §6.2.
pub const ACCESS_TOKEN_TTL_SECONDS: i64 = 900;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — user id.
    pub sub: Uuid,
    /// Organisasi tempat token ini berlaku.
    pub org: Uuid,
    pub email: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
}

impl Claims {
    pub fn user_id(&self) -> Uuid {
        self.sub
    }
    pub fn org_id(&self) -> Uuid {
        self.org
    }
}

#[derive(Clone)]
pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    issuer: String,
}

impl std::fmt::Debug for JwtKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Jangan pernah mencetak material kunci.
        f.debug_struct("JwtKeys")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl JwtKeys {
    /// Memuat keypair Ed25519 dari berkas PEM PKCS#8.
    pub fn from_pem_files(private_path: &str, public_path: &str, issuer: &str) -> Result<Self> {
        let priv_pem = std::fs::read(private_path)
            .with_context(|| format!("gagal membaca kunci privat JWT di {private_path}"))?;
        let pub_pem = std::fs::read(public_path)
            .with_context(|| format!("gagal membaca kunci publik JWT di {public_path}"))?;

        Ok(Self {
            encoding: EncodingKey::from_ed_pem(&priv_pem)
                .context("kunci privat JWT bukan Ed25519 PEM yang valid")?,
            decoding: DecodingKey::from_ed_pem(&pub_pem)
                .context("kunci publik JWT bukan Ed25519 PEM yang valid")?,
            issuer: issuer.to_string(),
        })
    }

    pub fn terbitkan(&self, user_id: Uuid, org_id: Uuid, email: &str) -> ApiResult<String> {
        let sekarang = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: user_id,
            org: org_id,
            email: email.to_string(),
            iat: sekarang,
            exp: sekarang + ACCESS_TOKEN_TTL_SECONDS,
            iss: self.issuer.clone(),
        };

        jsonwebtoken::encode(&Header::new(Algorithm::EdDSA), &claims, &self.encoding)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("gagal menerbitkan token: {e}")))
    }

    pub fn verifikasi(&self, token: &str) -> ApiResult<Claims> {
        // Algoritma dikunci ke EdDSA. Tanpa ini, penyerang dapat mengirim
        // token beralgoritma `none` atau menurunkannya ke HMAC memakai kunci
        // publik sebagai secret — kelas serangan klasik pada JWT.
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.set_issuer(&[&self.issuer]);
        validation.validate_exp = true;

        jsonwebtoken::decode::<Claims>(token, &self.decoding, &validation)
            .map(|d| d.claims)
            .map_err(|e| {
                tracing::debug!(error = %e, "verifikasi token gagal");
                ApiError::TidakTerautentikasi
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_tidak_valid_ditolak_dengan_pesan_jelas() {
        let dir = std::env::temp_dir();
        let p = dir.join("aetherdesk_test_bad.pem");
        std::fs::write(&p, b"bukan pem").unwrap();

        let hasil = JwtKeys::from_pem_files(
            p.to_str().unwrap(),
            p.to_str().unwrap(),
            "aetherdesk",
        );
        assert!(hasil.is_err());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn berkas_hilang_menyebut_path() {
        let hasil = JwtKeys::from_pem_files(
            "/path/yang/pasti/tidak/ada.pem",
            "/path/yang/pasti/tidak/ada.pub.pem",
            "aetherdesk",
        );
        let pesan = hasil.unwrap_err().to_string();
        assert!(pesan.contains("/path/yang/pasti/tidak/ada.pem"), "{pesan}");
    }

    #[test]
    fn ttl_sesuai_arsitektur() {
        assert_eq!(ACCESS_TOKEN_TTL_SECONDS, 900, "ARCHITECTURE.md §6.2: 15 menit");
    }

    #[test]
    fn claims_membawa_tenant() {
        // org wajib ada di token: seluruh query runtime memakainya untuk
        // menetapkan `aetherdesk.current_org` sebelum menyentuh tabel ber-RLS.
        let c = Claims {
            sub: Uuid::nil(),
            org: Uuid::nil(),
            email: "a@b.c".into(),
            exp: 0,
            iat: 0,
            iss: "aetherdesk".into(),
        };
        let j = serde_json::to_value(&c).unwrap();
        assert!(j.get("org").is_some(), "claim org hilang");
        assert!(j.get("sub").is_some(), "claim sub hilang");
    }
}
