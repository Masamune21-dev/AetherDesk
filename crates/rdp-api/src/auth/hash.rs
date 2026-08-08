//! Hashing password dengan Argon2id.
//!
//! PRD §12.2 menetapkan Argon2id. Parameternya mengikuti rekomendasi OWASP
//! 2024: 19 MiB memori, 2 iterasi, paralelisme 1.

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};

fn argon2() -> Argon2<'static> {
    let params = Params::new(19_456, 2, 1, None).expect("parameter Argon2 valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub fn hash(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow!("gagal melakukan hash password: {e}"))
}

/// Memverifikasi password terhadap hash tersimpan.
///
/// Mengembalikan `false` — bukan error — bila hash rusak atau tidak dikenali.
/// Pemanggil memperlakukan hash cacat sebagaimana password salah, sehingga
/// baris database yang rusak tidak berubah menjadi celah autentikasi.
pub fn verify(password: &str, stored: &str) -> bool {
    match PasswordHash::new(stored) {
        Ok(parsed) => argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(e) => {
            tracing::warn!(error = %e, "hash password tersimpan tidak dapat diparsing");
            false
        }
    }
}

/// Hash umpan yang dibangkitkan sekali saat proses start.
///
/// Sengaja **tidak** ditulis sebagai konstanta literal: hash yang salah ketik
/// akan gagal diparsing dalam hitungan mikrodetik, dan justru menciptakan
/// selisih waktu yang seharusnya dihilangkan. Membangkitkannya menjamin
/// formatnya benar dan biayanya identik dengan verifikasi asli.
static DUMMY_HASH: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    hash("kata sandi umpan yang tidak pernah dipakai siapa pun")
        .expect("hashing umpan harus berhasil saat start")
});

/// Menjalankan pekerjaan hashing dengan biaya setara verifikasi asli.
///
/// Dipakai saat identitas **tidak** ditemukan. Tanpa ini, jalur "user tidak
/// ada" selesai jauh lebih cepat daripada jalur "password salah", dan selisih
/// waktunya menjadi oracle yang memberi tahu penyerang akun atau device ID
/// mana yang hidup. (QUICK_CONNECT.md §5.1)
pub fn verify_dummy(password: &str) {
    let _ = verify(password, &DUMMY_HASH);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_lalu_verify_berhasil() {
        let h = hash("rahasia-yang-panjang").unwrap();
        assert!(verify("rahasia-yang-panjang", &h));
    }

    #[test]
    fn password_salah_ditolak() {
        let h = hash("benar").unwrap();
        assert!(!verify("salah", &h));
    }

    #[test]
    fn salt_berbeda_tiap_hash() {
        let a = hash("sama").unwrap();
        let b = hash("sama").unwrap();
        assert_ne!(a, b, "salt tidak diacak");
        assert!(verify("sama", &a) && verify("sama", &b));
    }

    #[test]
    fn hash_memakai_argon2id() {
        let h = hash("x").unwrap();
        assert!(h.starts_with("$argon2id$"), "algoritma salah: {h}");
    }

    #[test]
    fn hash_rusak_ditolak_bukan_panik() {
        assert!(!verify("apa pun", "bukan-hash-sama-sekali"));
        assert!(!verify("apa pun", ""));
        assert!(!verify("apa pun", "$argon2id$rusak"));
    }

    #[test]
    fn hash_umpan_berformat_valid() {
        // Bila hash umpan tidak dapat diparsing, `verify` akan kembali dalam
        // mikrodetik dan justru menciptakan oracle waktu yang ingin dihindari.
        assert!(
            PasswordHash::new(&DUMMY_HASH).is_ok(),
            "hash umpan tidak valid: {}",
            &*DUMMY_HASH
        );
        assert!(DUMMY_HASH.starts_with("$argon2id$"));
    }

    #[test]
    fn verify_dummy_memakan_waktu_sebanding_verifikasi_asli() {
        let nyata = hash("password asli").unwrap();

        let t0 = std::time::Instant::now();
        let _ = verify("tebakan salah", &nyata);
        let durasi_nyata = t0.elapsed();

        let t1 = std::time::Instant::now();
        verify_dummy("tebakan salah");
        let durasi_umpan = t1.elapsed();

        // Keduanya menjalankan Argon2id penuh, jadi harus berada pada orde
        // besaran yang sama. Ambang longgar supaya tidak rapuh di CI.
        let rasio = durasi_nyata.as_secs_f64() / durasi_umpan.as_secs_f64().max(1e-9);
        assert!(
            (0.2..5.0).contains(&rasio),
            "selisih waktu terlalu besar: nyata {durasi_nyata:?}, umpan {durasi_umpan:?}"
        );
    }
}
