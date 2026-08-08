//! Identitas perangkat yang tersimpan di disk.
//!
//! Dua berkas, dan pemisahannya disengaja:
//!
//! | Berkas | Isi | Sifat |
//! |---|---|---|
//! | `device.json` | UUID, device ID, alamat server | boleh dibaca siapa saja |
//! | `device.key` | seed privat Ed25519, 32 byte | **rahasia** |
//!
//! Menggabungkan keduanya akan membuat satu-satunya berkas rahasia ikut
//! terbaca setiap kali sesuatu ingin tahu alamat server. Memisahkannya
//! membuat berkas rahasia hanya disentuh saat benar-benar perlu menandatangani.

use anyhow::{Context, Result};
use rdp_core::DeviceKeypair;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Alamat server baku bila tidak ditentukan saat enrolment.
pub const SERVER_BAKU: &str = "https://aetherdesk.masamune.my.id";

const BERKAS_KONFIG: &str = "device.json";
const BERKAS_KUNCI: &str = "device.key";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Konfigurasi {
    pub device_uuid: Uuid,
    pub device_id: String,
    pub server: String,
}

impl Konfigurasi {
    /// URL dasar API, tanpa garis miring di ujung.
    pub fn api_base(&self) -> String {
        format!("{}/api/v1", self.server.trim_end_matches('/'))
    }

    /// URL WebSocket signaling, diturunkan dari alamat server.
    ///
    /// Diturunkan, bukan disimpan terpisah. Menyimpan keduanya mengundang
    /// keadaan tempat agent berbicara ke API di satu host dan ke signaling di
    /// host lain — yang selalu berarti salah satunya usang.
    pub fn ws_url(&self) -> String {
        let dasar = self.server.trim_end_matches('/');
        let ws = dasar
            .strip_prefix("https://")
            .map(|s| format!("wss://{s}"))
            .or_else(|| dasar.strip_prefix("http://").map(|s| format!("ws://{s}")))
            .unwrap_or_else(|| format!("wss://{dasar}"));
        format!("{ws}/ws")
    }
}

/// Direktori tempat identitas disimpan.
pub fn direktori() -> Result<PathBuf> {
    // Variabel lingkungan lebih dulu: tanpanya, menguji dua identitas di satu
    // mesin memerlukan penghapusan yang sebenarnya.
    if let Ok(p) = std::env::var("AETHERDESK_DIR") {
        return Ok(PathBuf::from(p));
    }
    let dirs = directories::ProjectDirs::from("id", "masamune", "aetherdesk")
        .context("tidak dapat menentukan direktori konfigurasi pengguna")?;
    Ok(dirs.config_dir().to_path_buf())
}

pub fn path_konfig() -> Result<PathBuf> {
    Ok(direktori()?.join(BERKAS_KONFIG))
}

pub fn path_kunci() -> Result<PathBuf> {
    Ok(direktori()?.join(BERKAS_KUNCI))
}

/// Apakah perangkat ini sudah pernah ter-enrol.
pub fn sudah_enrol() -> bool {
    matches!((path_konfig(), path_kunci()), (Ok(k), Ok(s)) if k.is_file() && s.is_file())
}

pub fn muat_konfig() -> Result<Konfigurasi> {
    let p = path_konfig()?;
    let isi = std::fs::read_to_string(&p)
        .with_context(|| format!("belum ter-enrol — {} tidak ada", p.display()))?;
    serde_json::from_str(&isi).with_context(|| format!("{} rusak", p.display()))
}

pub fn muat_kunci() -> Result<DeviceKeypair> {
    let p = path_kunci()?;
    let seed = std::fs::read(&p)
        .with_context(|| format!("kunci perangkat tidak ada di {}", p.display()))?;
    DeviceKeypair::dari_seed(&seed)
        .with_context(|| format!("kunci perangkat di {} tidak valid", p.display()))
}

/// Menyimpan identitas hasil enrolment.
pub fn simpan(konfig: &Konfigurasi, kunci: &DeviceKeypair) -> Result<()> {
    let dir = direktori()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("gagal membuat {}", dir.display()))?;

    let p_kunci = path_kunci()?;
    std::fs::write(&p_kunci, kunci.seed())
        .with_context(|| format!("gagal menulis {}", p_kunci.display()))?;
    kunci_hanya_pemilik(&p_kunci)?;

    let p_konfig = path_konfig()?;
    std::fs::write(&p_konfig, serde_json::to_vec_pretty(konfig)?)
        .with_context(|| format!("gagal menulis {}", p_konfig.display()))?;

    Ok(())
}

/// Membatasi berkas kunci agar hanya dapat dibaca pemiliknya.
///
/// Pada Unix ini `chmod 600` dan selesai. Pada Windows tidak ada padanan satu
/// baris — izin default berkas di direktori profil pengguna sudah membatasi
/// akses ke pemilik dan administrator, tetapi itu **bukan** hal yang sama, dan
/// tidak jujur bila dibiarkan tanpa catatan.
///
/// Pengetatan ACL yang sesungguhnya menyusul bersama service Windows (ADR-010),
/// tempat kunci akan pindah ke penyimpanan milik LocalSystem.
#[cfg(unix)]
fn kunci_hanya_pemilik(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("gagal mengetatkan izin {}", p.display()))
}

#[cfg(not(unix))]
fn kunci_hanya_pemilik(p: &Path) -> Result<()> {
    tracing::debug!(
        path = %p.display(),
        "izin berkas kunci mengikuti ACL bawaan profil pengguna"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn konfig(server: &str) -> Konfigurasi {
        Konfigurasi {
            device_uuid: Uuid::nil(),
            device_id: "123456789".into(),
            server: server.into(),
        }
    }

    #[test]
    fn ws_url_mengikuti_skema_server() {
        assert_eq!(konfig("https://a.test").ws_url(), "wss://a.test/ws");
        assert_eq!(konfig("http://127.0.0.1:8080").ws_url(), "ws://127.0.0.1:8080/ws");
    }

    #[test]
    fn garis_miring_di_ujung_tidak_menghasilkan_url_ganda() {
        assert_eq!(konfig("https://a.test/").ws_url(), "wss://a.test/ws");
        assert_eq!(konfig("https://a.test/").api_base(), "https://a.test/api/v1");
    }

    #[test]
    fn server_tanpa_skema_dianggap_aman() {
        // Menebak ke arah TLS, bukan menjauh darinya. Salah tebak ke `ws://`
        // akan mengirim token perangkat melewati jaringan tanpa enkripsi.
        assert_eq!(konfig("a.test").ws_url(), "wss://a.test/ws");
    }

    #[test]
    fn api_base_berakhir_di_v1() {
        assert_eq!(konfig("https://a.test").api_base(), "https://a.test/api/v1");
    }

    #[test]
    fn konfigurasi_bolak_balik_json() {
        let k = konfig("https://a.test");
        let s = serde_json::to_string(&k).unwrap();
        let kembali: Konfigurasi = serde_json::from_str(&s).unwrap();
        assert_eq!(kembali.device_uuid, k.device_uuid);
        assert_eq!(kembali.server, k.server);
    }

    #[test]
    fn konfigurasi_tidak_pernah_memuat_kunci_privat() {
        // Pemisahan dua berkas hanya bermakna bila yang satu benar-benar tidak
        // memuat isi yang lain.
        let s = serde_json::to_string(&konfig("https://a.test")).unwrap();
        for terlarang in ["seed", "key", "private", "secret"] {
            assert!(!s.contains(terlarang), "{terlarang} muncul di device.json: {s}");
        }
    }
}
