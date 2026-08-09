//! Menjalankan agent otomatis saat pengguna masuk.
//!
//! Memakai kunci registri `Run` milik **pengguna**
//! (`HKEY_CURRENT_USER`), bukan yang milik mesin.
//!
//! Itu bukan pilihan kenyamanan. Kunci per-mesin memerlukan hak administrator
//! dan menjalankan agent untuk **setiap** orang yang masuk ke komputer itu —
//! termasuk yang tidak pernah menyetujui apa pun. Agent ini menangkap layar
//! sesi interaktif seseorang; ia berhak hidup hanya di sesi orang yang memang
//! memasangnya.
//!
//! Jalan yang benar untuk mesin bersama adalah service LocalSystem yang
//! meluncurkan session agent lewat `WTSQueryUserToken` — ADR-010. Sampai itu
//! ada, per-pengguna adalah satu-satunya bentuk yang jujur.

use anyhow::{Context, Result};

/// Nama nilai di dalam kunci `Run`.
pub const NAMA: &str = "AetherDesk";

const KUNCI: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Perintah yang dijalankan saat masuk.
///
/// Dikutip karena path dapat memuat spasi — `C:\Program Files\...` tanpa
/// tanda kutip akan ditafsirkan sebagai beberapa argumen, dan gejalanya
/// adalah agent yang tidak pernah menyala tanpa satu pun pesan.
fn perintah(argumen: &[String]) -> Result<String> {
    let exe = std::env::current_exe().context("tidak dapat menentukan lokasi program")?;
    let mut s = format!("\"{}\" gui", exe.display());
    for a in argumen {
        s.push(' ');
        s.push_str(a);
    }
    Ok(s)
}

#[cfg(windows)]
mod win {
    use super::{KUNCI, NAMA};
    use anyhow::{Context, Result};
    use windows::core::HSTRING;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SAM_FLAGS, REG_SZ,
    };

    /// Membuka kunci `Run`, yang selalu ada di setiap pemasangan Windows.
    ///
    /// Sengaja `Open`, bukan `Create`: kunci ini tidak perlu dibuat, dan
    /// versi `Create` menuntut fitur `Win32_Security` beserta
    /// `SECURITY_ATTRIBUTES` untuk sesuatu yang tidak pernah kita gunakan.
    fn buka(akses: REG_SAM_FLAGS) -> Result<HKEY> {
        let mut kunci = HKEY::default();
        let hasil = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(KUNCI), 0, akses, &mut kunci)
        };
        if hasil.is_err() {
            anyhow::bail!("tidak dapat membuka kunci registri Run: {hasil:?}");
        }
        Ok(kunci)
    }

    pub fn pasang(perintah: &str) -> Result<()> {
        let kunci = buka(KEY_WRITE)?;
        // UTF-16 beserta terminator nol. Registri menyimpan panjang dalam byte,
        // dan nilai tanpa terminator akan terbaca menyatu dengan sampah di
        // belakangnya oleh sebagian pembaca.
        let data: Vec<u16> = perintah.encode_utf16().chain(std::iter::once(0)).collect();
        let byte = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2)
        };

        let hasil = unsafe {
            RegSetValueExW(kunci, &HSTRING::from(NAMA), 0, REG_SZ, Some(byte))
        };
        unsafe { let _ = RegCloseKey(kunci); }
        hasil.ok().context("gagal menulis nilai registri")
    }

    pub fn hapus() -> Result<bool> {
        let kunci = buka(KEY_WRITE)?;
        let hasil = unsafe { RegDeleteValueW(kunci, &HSTRING::from(NAMA)) };
        unsafe { let _ = RegCloseKey(kunci); }

        // Nilai yang memang tidak ada bukan kegagalan: hasil akhirnya sama
        // dengan yang diminta.
        if hasil == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        hasil.ok().context("gagal menghapus nilai registri")?;
        Ok(true)
    }

    pub fn terpasang() -> Option<String> {
        let kunci = buka(KEY_READ).ok()?;
        let nama = HSTRING::from(NAMA);

        let mut ukuran = 0u32;
        let hasil = unsafe {
            RegQueryValueExW(kunci, &nama, None, None, None, Some(&mut ukuran))
        };
        if hasil.is_err() || ukuran == 0 {
            unsafe { let _ = RegCloseKey(kunci); }
            return None;
        }

        let mut buf = vec![0u8; ukuran as usize];
        let hasil = unsafe {
            RegQueryValueExW(kunci, &nama, None, None, Some(buf.as_mut_ptr()), Some(&mut ukuran))
        };
        unsafe { let _ = RegCloseKey(kunci); }
        if hasil.is_err() {
            return None;
        }

        let lebar: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        Some(String::from_utf16_lossy(&lebar))
    }
}

#[cfg(not(windows))]
mod win {
    use anyhow::Result;
    pub fn pasang(_p: &str) -> Result<()> {
        anyhow::bail!("jalan otomatis hanya tersedia di Windows")
    }
    pub fn hapus() -> Result<bool> {
        anyhow::bail!("jalan otomatis hanya tersedia di Windows")
    }
    pub fn terpasang() -> Option<String> {
        None
    }
}

/// Memasang agent agar berjalan saat pengguna masuk.
pub fn pasang(argumen: &[String]) -> Result<String> {
    let p = perintah(argumen)?;
    win::pasang(&p)?;
    Ok(p)
}

pub fn hapus() -> Result<bool> {
    win::hapus()
}

pub fn terpasang() -> Option<String> {
    win::terpasang()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perintah_selalu_mengutip_path() {
        // Path dengan spasi tanpa tanda kutip ditafsirkan sebagai beberapa
        // argumen, dan agent tidak pernah menyala tanpa satu pun pesan.
        let p = perintah(&[]).unwrap();
        assert!(p.starts_with('"'), "path tidak dikutip: {p}");
        assert!(p.contains("\" gui"), "subperintah gui hilang: {p}");
    }

    #[test]
    fn argumen_ikut_diteruskan() {
        let p = perintah(&["--izinkan-kendali".into(), "--fps".into(), "60".into()]).unwrap();
        assert!(p.ends_with("gui --izinkan-kendali --fps 60"), "{p}");
    }

    #[test]
    fn nama_nilai_stabil() {
        // Nilai ini hidup di registri pengguna. Mengubahnya meninggalkan entri
        // lama yang tetap menjalankan biner versi sebelumnya.
        assert_eq!(NAMA, "AetherDesk");
        assert!(KUNCI.ends_with(r"CurrentVersion\Run"));
    }
}
