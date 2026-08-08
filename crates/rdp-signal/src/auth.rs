//! Verifikasi JWT.
//!
//! Signal Server hanya perlu **memverifikasi** token, tidak pernah
//! menerbitkannya. Itulah nilai praktis dari ADR-008: cukup kunci publik yang
//! dipasang di sini, sementara kunci privat tidak pernah meninggalkan API
//! Server. Dengan HMAC, layanan ini mau tidak mau ikut memegang kemampuan
//! menerbitkan token.
//!
//! Struktur `Claims` sengaja diduplikasi dari `rdp-api` alih-alih dipindahkan
//! ke `rdp-core`. Memindahkannya akan menarik `jsonwebtoken` beserta `ring` ke
//! dalam crate inti — yang kelak juga dipakai agent, tempat setiap megabyte
//! biner diperhitungkan (NFR-PER-06: agent < 20 MB).

use anyhow::{Context, Result};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub org: Uuid,
    pub email: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
}

#[derive(Clone)]
pub struct Verifier {
    kunci: DecodingKey,
    issuer: String,
}

impl std::fmt::Debug for Verifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Verifier")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl Verifier {
    pub fn dari_pem(public_path: &str, issuer: &str) -> Result<Self> {
        let pem = std::fs::read(public_path)
            .with_context(|| format!("gagal membaca kunci publik JWT di {public_path}"))?;
        Ok(Self {
            kunci: DecodingKey::from_ed_pem(&pem)
                .context("kunci publik JWT bukan Ed25519 PEM yang valid")?,
            issuer: issuer.to_string(),
        })
    }

    pub fn verifikasi(&self, token: &str) -> Option<Claims> {
        // Algoritma dikunci ke EdDSA — menutup serangan `alg: none` dan
        // penurunan ke HMAC yang memakai kunci publik sebagai secret.
        let mut v = Validation::new(Algorithm::EdDSA);
        v.set_issuer(&[&self.issuer]);
        v.validate_exp = true;

        match jsonwebtoken::decode::<Claims>(token, &self.kunci, &v) {
            Ok(d) => Some(d.claims),
            Err(e) => {
                tracing::debug!(error = %e, "verifikasi token gagal");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn berkas_hilang_menyebut_path() {
        let e = Verifier::dari_pem("/tidak/ada.pem", "aetherdesk").unwrap_err();
        assert!(e.to_string().contains("/tidak/ada.pem"));
    }

    #[test]
    fn pem_rusak_ditolak() {
        let p = std::env::temp_dir().join("aetherdesk_signal_bad.pem");
        std::fs::write(&p, b"bukan pem").unwrap();
        assert!(Verifier::dari_pem(p.to_str().unwrap(), "aetherdesk").is_err());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn claims_membawa_org() {
        let c = Claims {
            sub: Uuid::nil(),
            org: Uuid::nil(),
            email: "a@b.c".into(),
            exp: 0,
            iat: 0,
            iss: "aetherdesk".into(),
        };
        let j = serde_json::to_value(&c).unwrap();
        assert!(j.get("org").is_some());
    }
}
