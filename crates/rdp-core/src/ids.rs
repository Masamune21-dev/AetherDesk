//! Newtype pembungkus untuk identitas domain.
//!
//! Mengikuti CODING_STANDARD.md §2.5: `DeviceId`, `SessionId`, dan `UserId`
//! adalah value object dengan validasi, bukan alias `String`. Ini mencegah
//! kelas bug yang paling membosankan sekaligus paling sering terjadi —
//! menukar dua ID saat memanggil fungsi.

use crate::damm;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Panjang device ID dalam digit, termasuk check digit.
pub const DEVICE_ID_LEN: usize = 9;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdError {
    #[error("device ID harus {DEVICE_ID_LEN} digit, diterima {0}")]
    PanjangSalah(usize),
    #[error("device ID hanya boleh berisi digit desimal")]
    BukanDigit,
    #[error("check digit device ID tidak cocok")]
    CheckDigitSalah,
}

/// Device ID sembilan digit — delapan digit acak diikuti check digit Damm.
///
/// Ini adalah **alamat, bukan rahasia**. Seluruh kekuatan autentikasi berada
/// pada password sesi; lihat [`crate::password`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeviceId(String);

impl DeviceId {
    /// Membangkitkan device ID baru dari RNG kriptografis.
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let mut digits = String::with_capacity(DEVICE_ID_LEN);
        for _ in 0..DEVICE_ID_LEN - 1 {
            let d: u8 = rng.gen_range(0..10);
            digits.push((b'0' + d) as char);
        }
        let cd = damm::check_digit(digits.as_bytes()).expect("hanya berisi digit ASCII");
        digits.push((b'0' + cd) as char);
        Self(digits)
    }

    /// Memvalidasi input pengguna. Spasi dan tanda hubung dibuang lebih dulu,
    /// karena ID ditampilkan berkelompok (`942 716 382`) dan orang akan
    /// mengetiknya persis seperti yang mereka lihat.
    pub fn parse(input: &str) -> Result<Self, IdError> {
        let cleaned: String = input
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();

        if cleaned.len() != DEVICE_ID_LEN {
            return Err(IdError::PanjangSalah(cleaned.len()));
        }
        if !cleaned.bytes().all(|b| b.is_ascii_digit()) {
            return Err(IdError::BukanDigit);
        }
        if !damm::is_valid(cleaned.as_bytes()) {
            return Err(IdError::CheckDigitSalah);
        }
        Ok(Self(cleaned))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Bentuk tampilan berkelompok tiga: `942 716 382`.
    pub fn grouped(&self) -> String {
        format!("{} {} {}", &self.0[0..3], &self.0[3..6], &self.0[6..9])
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for DeviceId {
    type Error = IdError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::parse(&v)
    }
}

impl From<DeviceId> for String {
    fn from(v: DeviceId) -> Self {
        v.0
    }
}

macro_rules! uuid_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub uuid::Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }
            pub fn as_uuid(&self) -> uuid::Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<uuid::Uuid> for $name {
            fn from(u: uuid::Uuid) -> Self {
                Self(u)
            }
        }
    };
}

uuid_newtype!(UserId, "Identitas user, primary key tabel `users`.");
uuid_newtype!(OrgId, "Identitas organisasi (tenant).");
uuid_newtype!(SessionId, "Identitas sesi remote.");
uuid_newtype!(DeviceUuid, "Primary key internal tabel `devices`.");

#[cfg(test)]
mod tests {
    use super::*;

    /// ID valid yang dihitung manual; dipakai sebagai fixture agar test
    /// tidak bergantung pada `generate()` yang sedang diuji.
    const FIXTURE: &str = "942716382";

    #[test]
    fn fixture_memang_valid() {
        assert!(damm::is_valid(FIXTURE.as_bytes()));
    }

    #[test]
    fn generate_selalu_menghasilkan_id_valid() {
        for _ in 0..2000 {
            let id = DeviceId::generate();
            assert_eq!(id.as_str().len(), DEVICE_ID_LEN);
            assert!(DeviceId::parse(id.as_str()).is_ok(), "invalid: {id}");
        }
    }

    #[test]
    fn generate_tidak_menghasilkan_nilai_tetap() {
        let a = DeviceId::generate();
        let mut berbeda = false;
        for _ in 0..50 {
            if DeviceId::generate() != a {
                berbeda = true;
                break;
            }
        }
        assert!(berbeda, "generate() tampak deterministik");
    }

    #[test]
    fn parse_menerima_bentuk_berkelompok() {
        let id = DeviceId::parse(FIXTURE).unwrap();
        assert_eq!(DeviceId::parse(&id.grouped()).unwrap(), id);
        assert_eq!(DeviceId::parse("942-716-382").unwrap(), id);
    }

    #[test]
    fn parse_menolak_input_cacat() {
        assert_eq!(DeviceId::parse("12345"), Err(IdError::PanjangSalah(5)));
        assert_eq!(DeviceId::parse("12345678a"), Err(IdError::BukanDigit));
    }

    #[test]
    fn parse_menolak_check_digit_salah() {
        let mut bytes = FIXTURE.as_bytes().to_vec();
        bytes[8] = b'0' + ((bytes[8] - b'0') + 1) % 10;
        let rusak = String::from_utf8(bytes).unwrap();
        assert_eq!(DeviceId::parse(&rusak), Err(IdError::CheckDigitSalah));
    }

    #[test]
    fn grouped_berformat_tiga_tiga_tiga() {
        let g = DeviceId::parse(FIXTURE).unwrap().grouped();
        assert_eq!(g, "942 716 382");
    }

    #[test]
    fn roundtrip_serde() {
        let id = DeviceId::parse(FIXTURE).unwrap();
        let j = serde_json::to_string(&id).unwrap();
        assert_eq!(j, format!("\"{FIXTURE}\""));
    }

    #[test]
    fn id_berbeda_tipe_tidak_tertukar() {
        assert_ne!(UserId::new().to_string(), OrgId::new().to_string());
    }
}
