//! Identitas perangkat berbasis Ed25519.
//!
//! Agent berjalan tanpa pengawasan di mesin yang tidak kita kendalikan, jadi ia
//! tidak boleh menyimpan kredensial pengguna. Bila mesin itu dibongkar, yang
//! bocor harus **satu perangkat**, bukan akun manusia beserta seluruh perangkat
//! organisasinya.
//!
//! Karena itu setiap agent memegang keypair miliknya sendiri. Kunci privat
//! tidak pernah meninggalkan mesin, dan server hanya menyimpan kunci publik.
//! Untuk membuktikan dirinya, agent menandatangani tantangan yang memuat
//! stempel waktu dan nonce, lalu menukarnya dengan JWT perangkat berumur
//! pendek.
//!
//! ## Kenapa modul ini ada di crate inti
//!
//! Penanda tangan (agent) dan pemverifikasi (API Server) **wajib** membangun
//! byte tantangan yang identik. Menduplikasi pembentukannya di dua crate adalah
//! undangan untuk perbedaan satu karakter yang gejalanya cuma "tanda tangan
//! ditolak", tanpa petunjuk sisi mana yang keliru. Satu fungsi bersama
//! menghapus kelas bug itu seluruhnya.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::Rng;
use uuid::Uuid;

/// Panjang kunci publik maupun seed privat Ed25519.
pub const KEY_LEN: usize = 32;

/// Panjang tanda tangan Ed25519.
pub const SIG_LEN: usize = 64;

/// Penanda domain untuk tantangan autentikasi perangkat.
///
/// **Ini bukan hiasan.** ADR-008 mewajibkan kunci perangkat yang sama kelak
/// menandatangani SDP. Bila kedua keperluan menandatangani byte telanjang,
/// tanda tangan yang dikumpulkan dari satu alur dapat diputar ulang sebagai
/// tanda tangan yang sah di alur lain. Awalan domain membuat kedua ruang pesan
/// terpisah dan tidak pernah bersinggungan.
///
/// Nomor versi ikut di dalamnya supaya format tantangan dapat berubah kelak
/// tanpa membuat tanda tangan lama tiba-tiba berlaku pada format baru.
pub const DOMAIN_AUTH: &str = "aetherdesk-device-auth:v1";

/// Selisih stempel waktu maksimum yang masih diterima, dalam detik.
///
/// Nilainya kompromi: terlalu ketat membuat agent bermesin jam sedikit meleset
/// gagal login terus-menerus, terlalu longgar memperlebar jendela pemutaran
/// ulang bila pencatatan nonce gagal. Enam puluh detik cukup untuk hanyutan jam
/// yang wajar tanpa NTP.
pub const SKEW_MAX_SECONDS: i64 = 60;

/// Jumlah byte acak pada nonce.
pub const NONCE_BYTES: usize = 16;

/// Membangun byte tantangan yang ditandatangani agent.
///
/// Bentuknya sengaja dibuat tidak ambigu: seluruh bagian dipisahkan `:` dan
/// tidak satu pun dapat memuat `:` sendiri — UUID dan bilangan bulat jelas
/// tidak bisa, dan nonce dibangkitkan dari alfabet heksadesimal. Tanpa jaminan
/// itu, dua kombinasi berbeda dapat menghasilkan string yang sama dan satu
/// tanda tangan berlaku untuk keduanya.
pub fn tantangan(device_uuid: &Uuid, unix_ts: i64, nonce: &str) -> String {
    format!("{DOMAIN_AUTH}:{device_uuid}:{unix_ts}:{nonce}")
}

/// Membangkitkan nonce heksadesimal untuk satu upaya autentikasi.
pub fn nonce_baru() -> String {
    let mut rng = rand::thread_rng();
    let byte: [u8; NONCE_BYTES] = rng.gen();
    byte.iter().map(|b| format!("{b:02x}")).collect()
}

/// Apakah stempel waktu masih berada di dalam jendela yang diterima.
///
/// Memeriksa **kedua** arah. Menolak hanya stempel yang terlalu tua akan
/// membiarkan penyerang mengirim stempel jauh di masa depan dan menyimpan
/// tanda tangannya untuk dipakai berhari-hari kemudian.
pub fn stempel_waktu_segar(unix_ts: i64, sekarang: i64) -> bool {
    (sekarang - unix_ts).abs() <= SKEW_MAX_SECONDS
}

/// Keypair perangkat. Kunci privat tidak pernah dikirim ke mana pun.
pub struct DeviceKeypair {
    signing: SigningKey,
}

// `SigningKey` tidak mengimplementasikan Debug yang aman untuk dicetak, dan
// crate ini memaksa `missing_debug_implementations`. Implementasi manual yang
// tidak pernah menyentuh material kunci menyelesaikan keduanya sekaligus.
impl std::fmt::Debug for DeviceKeypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceKeypair")
            .field("public_key", &self.public_key_base64())
            .finish_non_exhaustive()
    }
}

impl DeviceKeypair {
    /// Membangkitkan keypair baru dari CSPRNG sistem.
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            signing: SigningKey::generate(&mut rng),
        }
    }

    /// Memuat keypair dari seed privat 32 byte.
    pub fn dari_seed(seed: &[u8]) -> crate::Result<Self> {
        let seed: [u8; KEY_LEN] = seed
            .try_into()
            .map_err(|_| crate::CoreError::KunciTidakValid)?;
        Ok(Self {
            signing: SigningKey::from_bytes(&seed),
        })
    }

    /// Seed privat, untuk disimpan ke disk dengan izin ketat.
    pub fn seed(&self) -> [u8; KEY_LEN] {
        self.signing.to_bytes()
    }

    /// Kunci publik mentah, yang didaftarkan ke server.
    pub fn public_key(&self) -> [u8; KEY_LEN] {
        self.signing.verifying_key().to_bytes()
    }

    pub fn public_key_base64(&self) -> String {
        ke_base64(&self.public_key())
    }

    /// Menandatangani tantangan.
    pub fn tanda_tangani(&self, pesan: &str) -> [u8; SIG_LEN] {
        self.signing.sign(pesan.as_bytes()).to_bytes()
    }
}

/// Memverifikasi tanda tangan atas sebuah tantangan.
///
/// Mengembalikan `bool`, bukan `Result`, karena pemanggil tidak boleh
/// membedakan sebab kegagalannya. "Kunci publik cacat" dan "tanda tangan salah"
/// harus menghasilkan respons yang sama — selisih apa pun di antara keduanya
/// adalah oracle.
pub fn verifikasi(public_key: &[u8], pesan: &str, tanda_tangan: &[u8]) -> bool {
    let Ok(pk): std::result::Result<[u8; KEY_LEN], _> = public_key.try_into() else {
        return false;
    };
    let Ok(sig): std::result::Result<[u8; SIG_LEN], _> = tanda_tangan.try_into() else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_bytes(&pk) else {
        return false;
    };
    vk.verify(pesan.as_bytes(), &Signature::from_bytes(&sig))
        .is_ok()
}

// ── Pengkodean ───────────────────────────────────────────────────────────────
//
// Satu alfabet untuk seluruh sistem. Base64 punya varian standar dan URL-safe
// yang berbeda pada dua karakter saja, dan padding yang boleh ada atau tidak —
// cukup untuk membuat dua sisi gagal saling memahami dengan gejala yang
// menyesatkan. Helper ini memastikan pilihannya dibuat satu kali.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

pub fn ke_base64(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

pub fn dari_base64(s: &str) -> crate::Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(s.trim())
        .map_err(|_| crate::CoreError::KunciTidakValid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn tanda_tangan_sendiri_terverifikasi() {
        let kp = DeviceKeypair::generate();
        let pesan = tantangan(&Uuid::new_v4(), 1_770_000_000, &nonce_baru());
        let sig = kp.tanda_tangani(&pesan);
        assert!(verifikasi(&kp.public_key(), &pesan, &sig));
    }

    #[test]
    fn kunci_lain_ditolak() {
        let a = DeviceKeypair::generate();
        let b = DeviceKeypair::generate();
        let pesan = tantangan(&Uuid::new_v4(), 1_770_000_000, "abc");
        let sig = a.tanda_tangani(&pesan);
        assert!(!verifikasi(&b.public_key(), &pesan, &sig));
    }

    #[test]
    fn pesan_diubah_satu_karakter_ditolak() {
        let kp = DeviceKeypair::generate();
        let dev = Uuid::new_v4();
        let sig = kp.tanda_tangani(&tantangan(&dev, 1_770_000_000, "abc"));
        // Stempel waktu bergeser satu detik.
        let lain = tantangan(&dev, 1_770_000_001, "abc");
        assert!(!verifikasi(&kp.public_key(), &lain, &sig));
    }

    #[test]
    fn tantangan_selalu_berawalan_domain() {
        // Properti inti dari pemisahan domain: tanda tangan autentikasi tidak
        // boleh pernah berlaku sebagai tanda tangan SDP, dan sebaliknya.
        let t = tantangan(&Uuid::nil(), 0, "n");
        assert!(t.starts_with(DOMAIN_AUTH), "{t}");
        assert!(t.starts_with("aetherdesk-device-auth:v1:"));
    }

    #[test]
    fn tantangan_tidak_ambigu_antar_bagian() {
        // Dua kombinasi berbeda tidak boleh menghasilkan string yang sama.
        // Bila nonce boleh memuat ':', pasangan di bawah ini akan bertabrakan.
        let a = tantangan(&Uuid::nil(), 1, "2:x");
        let b = tantangan(&Uuid::nil(), 12, "x");
        assert_ne!(a, b);
    }

    #[test]
    fn nonce_heksadesimal_dan_tanpa_pemisah() {
        let n = nonce_baru();
        assert_eq!(n.len(), NONCE_BYTES * 2);
        assert!(n.chars().all(|c| c.is_ascii_hexdigit()), "{n}");
        assert!(!n.contains(':'), "nonce merusak pemisah tantangan");
    }

    #[test]
    fn nonce_tidak_berulang() {
        let set: HashSet<String> = (0..500).map(|_| nonce_baru()).collect();
        assert_eq!(set.len(), 500);
    }

    #[test]
    fn stempel_waktu_masa_depan_juga_ditolak() {
        let now = 1_770_000_000;
        assert!(stempel_waktu_segar(now, now));
        assert!(stempel_waktu_segar(now - SKEW_MAX_SECONDS, now));
        assert!(stempel_waktu_segar(now + SKEW_MAX_SECONDS, now));
        assert!(!stempel_waktu_segar(now - SKEW_MAX_SECONDS - 1, now));
        // Tanpa batas atas, tanda tangan bertanggal jauh di depan dapat
        // disimpan lalu dipakai berhari-hari kemudian.
        assert!(!stempel_waktu_segar(now + SKEW_MAX_SECONDS + 1, now));
    }

    #[test]
    fn seed_bolak_balik() {
        let kp = DeviceKeypair::generate();
        let lagi = DeviceKeypair::dari_seed(&kp.seed()).unwrap();
        assert_eq!(kp.public_key(), lagi.public_key());
    }

    #[test]
    fn seed_panjang_salah_ditolak() {
        assert!(DeviceKeypair::dari_seed(&[0u8; 16]).is_err());
        assert!(DeviceKeypair::dari_seed(&[]).is_err());
    }

    #[test]
    fn verifikasi_tidak_panik_pada_masukan_cacat() {
        // Seluruh input ini datang dari jaringan; tidak satu pun boleh
        // menjatuhkan proses.
        let kp = DeviceKeypair::generate();
        let pesan = tantangan(&Uuid::nil(), 0, "n");
        let sig = kp.tanda_tangani(&pesan);

        assert!(!verifikasi(&[], &pesan, &sig));
        assert!(!verifikasi(&[0u8; 31], &pesan, &sig));
        assert!(!verifikasi(&[0u8; 33], &pesan, &sig));
        assert!(!verifikasi(&kp.public_key(), &pesan, &[]));
        assert!(!verifikasi(&kp.public_key(), &pesan, &[0u8; 63]));
    }

    #[test]
    fn base64_bolak_balik_dan_url_safe() {
        let kp = DeviceKeypair::generate();
        let s = kp.public_key_base64();
        assert_eq!(dari_base64(&s).unwrap(), kp.public_key().to_vec());
        // URL-safe tanpa padding: tiga karakter ini tidak boleh muncul.
        assert!(!s.contains('+') && !s.contains('/') && !s.contains('='), "{s}");
    }

    #[test]
    fn base64_cacat_ditolak_tanpa_panik() {
        assert!(dari_base64("bukan base64 !!!").is_err());
    }

    #[test]
    fn debug_tidak_membocorkan_kunci_privat() {
        let kp = DeviceKeypair::generate();
        let d = format!("{kp:?}");
        let seed_b64 = ke_base64(&kp.seed());
        assert!(!d.contains(&seed_b64), "seed privat muncul di Debug: {d}");
    }
}
