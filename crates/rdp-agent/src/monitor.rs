//! Enumerasi monitor.
//!
//! Ini modul pertama yang benar-benar menyentuh API sistem operasi, dan sengaja
//! dikerjakan lebih dulu: ia dapat diuji tanpa capture, tanpa encode, dan tanpa
//! jaringan. Menjalankan agent lalu melihat ketiga layar Anda terdaftar dengan
//! koordinat yang benar adalah cara tercepat membuktikan fondasinya sehat.

use serde::{Deserialize, Serialize};

/// Satu monitor pada virtual desktop.
///
/// Koordinat memakai `i32`, **bukan** `u32`. Ini bukan kehati-hatian berlebihan:
/// monitor yang diletakkan di sebelah kiri monitor primer memiliki `x` negatif,
/// dan susunan itu sangat umum. Memakai tipe tak bertanda membuat bug yang hanya
/// muncul pada sebagian pengguna dan hampir mustahil direproduksi oleh
/// pengembang yang monitornya kebetulan tersusun rapi. (Temuan T-16)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monitor {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    /// Faktor penskalaan DPI dalam persen (100 = 96 DPI).
    pub scale_percent: u32,
}

impl Monitor {
    /// Batas kanan pada virtual desktop.
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    /// Batas bawah pada virtual desktop.
    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    /// Menerjemahkan koordinat relatif (0.0–1.0) menjadi titik absolut pada
    /// virtual desktop.
    ///
    /// Viewer mengirim posisi relatif terhadap monitor yang sedang dilihat,
    /// bukan piksel. Dengan begitu perbedaan resolusi, penskalaan DPI, dan
    /// ukuran jendela viewer tidak pernah menjadi urusan sisi pengirim —
    /// seluruh kelas bug pemetaan koordinat hilang di satu tempat ini.
    pub fn ke_absolut(&self, rel_x: f64, rel_y: f64) -> (i32, i32) {
        let rx = rel_x.clamp(0.0, 1.0);
        let ry = rel_y.clamp(0.0, 1.0);
        (
            self.x + (rx * self.width as f64).round() as i32,
            self.y + (ry * self.height as f64).round() as i32,
        )
    }
}

/// Kotak pembatas yang mencakup seluruh monitor.
pub fn bounding_box(monitors: &[Monitor]) -> Option<(i32, i32, u32, u32)> {
    let kiri = monitors.iter().map(|m| m.x).min()?;
    let atas = monitors.iter().map(|m| m.y).min()?;
    let kanan = monitors.iter().map(|m| m.right()).max()?;
    let bawah = monitors.iter().map(|m| m.bottom()).max()?;
    Some((kiri, atas, (kanan - kiri) as u32, (bawah - atas) as u32))
}

// ── Windows ──────────────────────────────────────────────────────────────────

/// Menyatakan proses ini sadar-DPI per monitor sebelum satu pun koordinat dibaca.
///
/// Tanpa ini Windows **memvirtualkan** seluruh koordinat dan ukuran yang
/// dilaporkannya: pada monitor 1920×1080 berskala 150%, proses yang tidak
/// sadar-DPI melihat 1280×720. Angkanya konsisten dan tampak masuk akal, jadi
/// kesalahannya tidak terlihat sampai dipakai — dan tiga tempat langsung
/// terpengaruh:
///
/// - `ke_absolut()` menghasilkan piksel yang meleset, sehingga `SendInput`
///   dengan `MOUSEEVENTF_ABSOLUTE` mengklik di tempat yang salah (M4)
/// - Desktop Duplication menyerahkan frame beresolusi **fisik**, yang tidak
///   akan cocok dengan tata letak yang sudah terlanjur dilaporkan (M2)
/// - Bounding box virtual desktop menyusut, sehingga monitor bisa saling tindih
///
/// Dipanggil lewat `Once` dari dalam `enumerasi()` supaya pemanggil tidak
/// mungkin lupa. Kegagalan tidak fatal — Windows lama tanpa
/// `SetProcessDpiAwarenessContext` tetap berjalan, hanya dengan koordinat
/// tervirtualisasi, dan itu tercatat di log.
#[cfg(windows)]
pub(crate) fn siapkan_dpi() {
    use std::sync::Once;
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };

    static SEKALI: Once = Once::new();
    SEKALI.call_once(|| unsafe {
        if let Err(e) = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
            tracing::warn!(
                error = %e,
                "gagal menyatakan kesadaran DPI per monitor — koordinat mungkin tervirtualisasi"
            );
        }
    });
}

/// Mengubah DPI mentah menjadi persen penskalaan (96 DPI = 100%).
///
/// Seluruh langkah baku Windows — 100, 125, 150, 175, 200, 225, 250% — jatuh
/// tepat pada bilangan bulat. Pembulatan ke terdekat hanya berlaku bagi
/// penskalaan kustom yang dimasukkan pengguna sendiri.
#[cfg(any(windows, test))]
fn dpi_ke_persen(dpi: u32) -> u32 {
    if dpi == 0 {
        100
    } else {
        (dpi * 100 + 48) / 96
    }
}

#[cfg(windows)]
pub fn enumerasi() -> anyhow::Result<Vec<Monitor>> {
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

    siapkan_dpi();

    unsafe extern "system" fn callback(
        h: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let keluar = &mut *(data.0 as *mut Vec<Monitor>);

        let mut info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        if GetMonitorInfoW(h, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
            let r = info.monitorInfo.rcMonitor;
            let nama = String::from_utf16_lossy(&info.szDevice)
                .trim_end_matches('\0')
                .to_string();

            // DPI efektif, bukan DPI mentah panel: inilah angka yang dipakai
            // Windows untuk menata jendela, jadi ini pula yang perlu diketahui
            // viewer saat menggambar kursor dan menyekala teks.
            let mut dpi_x = 0u32;
            let mut dpi_y = 0u32;
            let skala = match GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) {
                Ok(()) => dpi_ke_persen(dpi_x),
                Err(_) => 100,
            };

            keluar.push(Monitor {
                id: keluar.len() as u32,
                name: if nama.is_empty() {
                    format!("Display {}", keluar.len() + 1)
                } else {
                    nama
                },
                x: r.left,
                y: r.top,
                width: (r.right - r.left) as u32,
                height: (r.bottom - r.top) as u32,
                is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                scale_percent: skala,
            });
        }
        BOOL(1)
    }

    let mut hasil: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(callback),
            LPARAM(&mut hasil as *mut _ as isize),
        );
    }

    if hasil.is_empty() {
        anyhow::bail!("tidak ada monitor terdeteksi");
    }
    Ok(hasil)
}

// ── Platform lain ────────────────────────────────────────────────────────────

#[cfg(not(windows))]
pub fn enumerasi() -> anyhow::Result<Vec<Monitor>> {
    // macOS memakai CGGetActiveDisplayList; Linux memakai XRandR atau Wayland.
    // Keduanya menyusul — Windows dikerjakan lebih dulu sesuai keputusan di
    // NEXT_PLAN.md §10.
    anyhow::bail!("enumerasi monitor belum diimplementasikan pada platform ini")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: u32, x: i32, y: i32, w: u32, h: u32, primary: bool) -> Monitor {
        Monitor {
            id,
            name: format!("Display {id}"),
            x,
            y,
            width: w,
            height: h,
            is_primary: primary,
            scale_percent: 100,
        }
    }

    #[test]
    fn monitor_di_kiri_berkoordinat_negatif() {
        // Susunan yang paling sering merusak implementasi: monitor sekunder
        // diletakkan di KIRI monitor primer.
        let kiri = m(1, -1920, 0, 1920, 1080, false);
        assert!(kiri.x < 0, "koordinat kiri harus negatif");
        assert_eq!(kiri.right(), 0);
    }

    #[test]
    fn bounding_box_mencakup_koordinat_negatif() {
        let ms = vec![
            m(0, 0, 0, 1920, 1080, true),
            m(1, -1920, 0, 1920, 1080, false),
            m(2, 1920, -200, 2560, 1440, false),
        ];
        let (x, y, w, h) = bounding_box(&ms).unwrap();
        assert_eq!(x, -1920);
        assert_eq!(y, -200);
        assert_eq!(w, 1920 + 1920 + 2560);
        assert_eq!(h, 1440.max(1080 + 200) as u32);
    }

    #[test]
    fn koordinat_relatif_dipetakan_ke_monitor_yang_benar() {
        let kiri = m(1, -1920, 0, 1920, 1080, false);
        assert_eq!(kiri.ke_absolut(0.0, 0.0), (-1920, 0));
        assert_eq!(kiri.ke_absolut(1.0, 1.0), (0, 1080));
        assert_eq!(kiri.ke_absolut(0.5, 0.5), (-960, 540));
    }

    #[test]
    fn koordinat_di_luar_rentang_dijepit() {
        // Viewer yang buggy tidak boleh membuat kursor melompat ke monitor lain.
        let utama = m(0, 0, 0, 1920, 1080, true);
        assert_eq!(utama.ke_absolut(-5.0, 2.0), (0, 1080));
        assert_eq!(utama.ke_absolut(9.9, -9.9), (1920, 0));
    }

    #[test]
    fn resolusi_berbeda_tetap_terpetakan_benar() {
        let empat_k = m(2, 1920, 0, 3840, 2160, false);
        assert_eq!(empat_k.ke_absolut(0.5, 0.5), (1920 + 1920, 1080));
    }

    #[test]
    fn bounding_box_kosong_untuk_daftar_kosong() {
        assert!(bounding_box(&[]).is_none());
    }

    #[test]
    fn langkah_penskalaan_baku_windows_bulat() {
        // Nilai-nilai inilah yang benar-benar muncul di pengaturan tampilan.
        for (dpi, persen) in [
            (96, 100),
            (120, 125),
            (144, 150),
            (168, 175),
            (192, 200),
            (216, 225),
            (240, 250),
        ] {
            assert_eq!(dpi_ke_persen(dpi), persen, "DPI {dpi}");
        }
    }

    #[test]
    fn dpi_nol_dianggap_seratus_persen() {
        // GetDpiForMonitor yang gagal meninggalkan variabel keluarannya apa
        // adanya; menganggapnya 0% akan membuat viewer membagi dengan nol.
        assert_eq!(dpi_ke_persen(0), 100);
    }
}
