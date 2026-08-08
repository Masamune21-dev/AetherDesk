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

/// Masa berlaku token perangkat.
///
/// Sama pendeknya dengan token pengguna, dan sengaja: agent dapat memperbarui
/// tokennya kapan saja tanpa campur tangan manusia — ia memegang kunci
/// privatnya sendiri. Tidak ada alasan memberinya umur lebih panjang, sementara
/// umur pendek membatasi kerugian bila token bocor dari memori proses.
pub const DEVICE_TOKEN_TTL_SECONDS: i64 = 900;

/// Jenis subjek yang dirujuk sebuah token.
pub mod jenis {
    /// Token milik manusia yang login lewat dashboard.
    pub const PENGGUNA: &str = "user";
    /// Token milik agent yang membuktikan diri dengan kunci perangkat.
    pub const PERANGKAT: &str = "device";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — user id, atau device uuid pada token perangkat.
    pub sub: Uuid,
    /// Organisasi tempat token ini berlaku.
    pub org: Uuid,
    #[serde(default)]
    pub email: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,

    /// Jenis subjek: `user` atau `device`.
    ///
    /// `#[serde(default)]` bukan kemalasan. Token pengguna yang sudah beredar
    /// saat migrasi ini dirilis tidak memuat klaim ini sama sekali, dan
    /// masing-masing masih berlaku sampai 15 menit ke depan. Tanpa nilai baku,
    /// setiap sesi yang sedang berjalan akan tertolak serentak begitu layanan
    /// di-restart.
    #[serde(default = "jenis_baku")]
    pub typ: String,

    /// Device UUID pada token perangkat; kosong pada token pengguna.
    ///
    /// Diletakkan terpisah dari `sub` supaya kode yang membaca `sub` sebagai
    /// user id tidak diam-diam memperoleh device uuid saat token perangkat
    /// lewat — kekeliruan yang akan menghasilkan kebocoran lintas-subjek yang
    /// sulit terlihat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev: Option<Uuid>,
}

fn jenis_baku() -> String {
    jenis::PENGGUNA.to_string()
}

impl Claims {
    pub fn user_id(&self) -> Uuid {
        self.sub
    }
    pub fn org_id(&self) -> Uuid {
        self.org
    }

    pub fn adalah_perangkat(&self) -> bool {
        self.typ == jenis::PERANGKAT
    }

    /// Device UUID, hanya bila token ini memang token perangkat.
    ///
    /// Memeriksa `typ` **dan** `dev` sekaligus. Memeriksa `dev` saja akan
    /// menerima token pengguna yang kebetulan membawa klaim `dev` — dan klaim
    /// itu berasal dari JWT yang kita terbitkan sendiri, jadi satu-satunya cara
    /// hal itu terjadi adalah bug di sisi penerbitan. Justru bug seperti itulah
    /// yang perlu ditahan di sini, bukan diandaikan tidak ada.
    pub fn device_uuid(&self) -> Option<Uuid> {
        if self.adalah_perangkat() {
            self.dev
        } else {
            None
        }
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
            typ: jenis::PENGGUNA.to_string(),
            dev: None,
        };

        self.encode(&claims)
    }

    /// Menerbitkan token untuk sebuah perangkat.
    ///
    /// `sub` diisi device uuid, bukan user id. Tidak ada pengguna di balik
    /// token ini — agent berjalan tanpa pengawasan, dan menautkannya ke akun
    /// manusia mana pun akan membuat jejak audit berbohong tentang siapa yang
    /// melakukan apa.
    pub fn terbitkan_perangkat(&self, device_uuid: Uuid, org_id: Uuid) -> ApiResult<String> {
        let sekarang = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: device_uuid,
            org: org_id,
            email: String::new(),
            iat: sekarang,
            exp: sekarang + DEVICE_TOKEN_TTL_SECONDS,
            iss: self.issuer.clone(),
            typ: jenis::PERANGKAT.to_string(),
            dev: Some(device_uuid),
        };

        self.encode(&claims)
    }

    fn encode(&self, claims: &Claims) -> ApiResult<String> {
        jsonwebtoken::encode(&Header::new(Algorithm::EdDSA), claims, &self.encoding)
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

    fn claims_pengguna() -> Claims {
        Claims {
            sub: Uuid::nil(),
            org: Uuid::nil(),
            email: "a@b.c".into(),
            exp: 0,
            iat: 0,
            iss: "aetherdesk".into(),
            typ: jenis::PENGGUNA.into(),
            dev: None,
        }
    }

    #[test]
    fn claims_membawa_tenant() {
        // org wajib ada di token: seluruh query runtime memakainya untuk
        // menetapkan `aetherdesk.current_org` sebelum menyentuh tabel ber-RLS.
        let j = serde_json::to_value(claims_pengguna()).unwrap();
        assert!(j.get("org").is_some(), "claim org hilang");
        assert!(j.get("sub").is_some(), "claim sub hilang");
    }

    #[test]
    fn token_lama_tanpa_klaim_typ_tetap_terbaca_sebagai_pengguna() {
        // Token yang sudah beredar saat migrasi ini dirilis tidak memuat `typ`
        // maupun `dev`, dan masing-masing masih berlaku sampai 15 menit. Tanpa
        // nilai baku, seluruh sesi berjalan akan tertolak serentak saat
        // layanan di-restart.
        let lama = r#"{"sub":"00000000-0000-0000-0000-000000000000",
                       "org":"00000000-0000-0000-0000-000000000000",
                       "email":"a@b.c","exp":0,"iat":0,"iss":"aetherdesk"}"#;
        let c: Claims = serde_json::from_str(lama).unwrap();
        assert_eq!(c.typ, jenis::PENGGUNA);
        assert!(!c.adalah_perangkat());
        assert_eq!(c.device_uuid(), None);
    }

    #[test]
    fn token_pengguna_tidak_pernah_menghasilkan_device_uuid() {
        // Bahkan bila klaim `dev` entah bagaimana ikut terbawa, `typ` yang
        // menentukan. Ini yang menahan bug penerbitan agar tidak berubah
        // menjadi kebocoran lintas-subjek.
        let mut c = claims_pengguna();
        c.dev = Some(Uuid::new_v4());
        assert_eq!(c.device_uuid(), None, "token pengguna menyamar jadi perangkat");
    }

    #[test]
    fn token_perangkat_membawa_uuid_pada_sub_dan_dev() {
        let dev = Uuid::new_v4();
        let org = Uuid::new_v4();
        let c = Claims {
            sub: dev,
            org,
            email: String::new(),
            exp: 0,
            iat: 0,
            iss: "aetherdesk".into(),
            typ: jenis::PERANGKAT.into(),
            dev: Some(dev),
        };
        assert!(c.adalah_perangkat());
        assert_eq!(c.device_uuid(), Some(dev));
        assert_eq!(c.org_id(), org);
    }

    #[test]
    fn email_kosong_tidak_diserialisasi_menjadi_null() {
        // Token perangkat tidak punya email. Bentuknya harus tetap string
        // kosong, bukan null — `email` di sisi deserialisasi bukan Option.
        let c = Claims {
            email: String::new(),
            typ: jenis::PERANGKAT.into(),
            dev: Some(Uuid::nil()),
            ..claims_pengguna()
        };
        let j = serde_json::to_string(&c).unwrap();
        let kembali: Claims = serde_json::from_str(&j).unwrap();
        assert_eq!(kembali.email, "");
        assert!(kembali.adalah_perangkat());
    }

    #[test]
    fn ttl_perangkat_tidak_lebih_panjang_dari_pengguna() {
        // Agent dapat memperbarui tokennya sendiri kapan saja, jadi tidak ada
        // alasan memberinya umur lebih panjang daripada sesi manusia.
        assert!(DEVICE_TOKEN_TTL_SECONDS <= ACCESS_TOKEN_TTL_SECONDS);
    }
}
