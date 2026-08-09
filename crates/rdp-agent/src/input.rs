//! Injeksi mouse dan papan ketik.
//!
//! M4. Ini modul yang mengubah sifat produk: sampai sekarang agent hanya
//! *menunjukkan* layar, dan mulai di sini ia menyerahkan kendali atasnya.
//!
//! ## Kendali tidak menyala dengan sendirinya
//!
//! Seluruh modul ini tidak melakukan apa pun kecuali agent dijalankan dengan
//! `--izinkan-kendali`. NEXT_PLAN.md §7.1 mewajibkan izin diminta **per
//! tingkat**, dan agent native tidak punya antarmuka untuk bertanya di tengah
//! sesi. Yang tersisa sebagai persetujuan yang jujur adalah keputusan orang
//! yang menyalakan agent, diambil sebelum siapa pun tersambung.
//!
//! ## Koordinat relatif, bukan piksel
//!
//! Viewer mengirim posisi 0,0–1,0 terhadap **monitor yang sedang dilihat**, dan
//! agent yang menerjemahkannya. Ini menghapus seluruh kelas bug yang berasal
//! dari perbedaan resolusi, penskalaan DPI, dan ukuran jendela viewer — viewer
//! tidak perlu tahu apa pun tentang tata letak fisik mesin tujuan.
//! (NEXT_PLAN.md §6.2)
//!
//! ## Scancode, bukan virtual key
//!
//! Papan ketik memakai scancode. Mengirim virtual key membuat huruf yang
//! diketik bergantung pada tata letak yang aktif di mesin tujuan, sehingga
//! viewer ber-QWERTY yang mengakses mesin ber-AZERTY menghasilkan huruf yang
//! salah. (NEXT_PLAN.md §6.1)

use crate::monitor::Monitor;

/// Berapa lama injeksi dijeda setelah pengguna lokal menyentuh mesinnya.
///
/// NEXT_PLAN.md §7.2: "orang yang merebut kembali kendali mesinnya sendiri
/// harus selalu menang". Tiga detik cukup untuk menyelesaikan satu gerakan
/// tanpa harus melawan kursor jarak jauh, dan cukup pendek agar sesi yang sah
/// tidak terasa macet.
pub const JEDA_LOKAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Selisih piksel yang dianggap gerakan pengguna lokal, bukan derau.
const AMBANG_GESER: i32 = 8;

/// Tombol tetikus sebagaimana dikirim viewer (mengikuti `MouseEvent.button`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tombol {
    Kiri,
    Tengah,
    Kanan,
}

impl Tombol {
    pub fn dari_nomor(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Kiri),
            1 => Some(Self::Tengah),
            2 => Some(Self::Kanan),
            _ => None,
        }
    }
}

/// Memetakan koordinat relatif ke titik absolut pada virtual desktop, lalu ke
/// satuan 0–65535 yang diminta `SendInput`.
///
/// `SendInput` dengan `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK` tidak
/// menerima piksel: ia menerima pecahan dari **seluruh virtual desktop**.
/// Menghitungnya terhadap satu monitor saja akan membuat setiap klik pada
/// susunan multi-monitor mendarat di tempat yang salah — dan pada susunan
/// berkoordinat negatif, meleset ke arah yang berlawanan.
pub fn ke_satuan_sendinput(
    monitor: &Monitor,
    virt: (i32, i32, u32, u32),
    rel_x: f64,
    rel_y: f64,
) -> (i32, i32) {
    let (vx, vy, vw, vh) = virt;
    let (ax, ay) = monitor.ke_absolut(rel_x, rel_y);

    // Pembagi memakai lebar dikurangi satu: 65535 memetakan ke piksel
    // terakhir, bukan ke satu piksel di luarnya.
    let dx = ((ax - vx) as i64 * 65_535 / (vw.max(2) as i64 - 1)) as i32;
    let dy = ((ay - vy) as i64 * 65_535 / (vh.max(2) as i64 - 1)) as i32;

    (dx.clamp(0, 65_535), dy.clamp(0, 65_535))
}

// ── Windows ──────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK,
        MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    fn kirim(masukan: &[INPUT]) {
        let n = unsafe { SendInput(masukan, std::mem::size_of::<INPUT>() as i32) };
        if n as usize != masukan.len() {
            // Kegagalan paling lazim: UIPI menolak karena proses tujuan berjalan
            // pada tingkat integritas lebih tinggi, atau layar aman sedang
            // aktif. Bukan alasan menjatuhkan sesi.
            tracing::debug!(
                terkirim = n,
                diminta = masukan.len(),
                "SendInput ditolak sebagian"
            );
        }
    }

    fn mouse(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, data: i32) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    // `mouseData` bertipe u32 di API, tetapi delta roda
                    // bertanda. Win32 menafsirkannya sebagai i32 dalam
                    // komplemen dua, jadi konversinya memang sekadar
                    // penafsiran ulang bit.
                    mouseData: data as u32,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    pub fn gerak(monitor: &Monitor, virt: (i32, i32, u32, u32), rel_x: f64, rel_y: f64) {
        let (dx, dy) = ke_satuan_sendinput(monitor, virt, rel_x, rel_y);
        kirim(&[mouse(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            dx,
            dy,
            0,
        )]);
    }

    pub fn tombol(t: Tombol, tekan: bool) {
        let flags = match (t, tekan) {
            (Tombol::Kiri, true) => MOUSEEVENTF_LEFTDOWN,
            (Tombol::Kiri, false) => MOUSEEVENTF_LEFTUP,
            (Tombol::Tengah, true) => MOUSEEVENTF_MIDDLEDOWN,
            (Tombol::Tengah, false) => MOUSEEVENTF_MIDDLEUP,
            (Tombol::Kanan, true) => MOUSEEVENTF_RIGHTDOWN,
            (Tombol::Kanan, false) => MOUSEEVENTF_RIGHTUP,
        };
        kirim(&[mouse(flags, 0, 0, 0)]);
    }

    pub fn gulir(delta: i32) {
        kirim(&[mouse(MOUSEEVENTF_WHEEL, 0, 0, delta)]);
    }

    pub fn papan_ketik(scancode: u16, tekan: bool, extended: bool) {
        let mut flags = KEYEVENTF_SCANCODE;
        if !tekan {
            flags |= KEYEVENTF_KEYUP;
        }
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        kirim(&[INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: Default::default(),
                    wScan: scancode,
                    dwFlags: KEYBD_EVENT_FLAGS(flags.0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }]);
    }

    pub fn posisi_kursor() -> Option<(i32, i32)> {
        let mut p = Default::default();
        unsafe { GetCursorPos(&mut p) }.ok()?;
        Some((p.x, p.y))
    }
}

#[cfg(windows)]
pub use win::{gerak, gulir, papan_ketik, posisi_kursor, tombol};

// ── Platform lain ────────────────────────────────────────────────────────────

#[cfg(not(windows))]
pub fn gerak(_m: &Monitor, _v: (i32, i32, u32, u32), _x: f64, _y: f64) {}
#[cfg(not(windows))]
pub fn tombol(_t: Tombol, _tekan: bool) {}
#[cfg(not(windows))]
pub fn gulir(_d: i32) {}
#[cfg(not(windows))]
pub fn papan_ketik(_s: u16, _tekan: bool, _e: bool) {}
#[cfg(not(windows))]
pub fn posisi_kursor() -> Option<(i32, i32)> {
    None
}

// ── Jeda saat pengguna lokal mengambil alih ──────────────────────────────────

/// Menahan injeksi ketika pengguna lokal menggerakkan tetikus fisiknya.
///
/// Caranya membandingkan posisi kursor yang sebenarnya dengan posisi terakhir
/// yang **kita** tempatkan. Bila keduanya menyimpang lebih jauh daripada
/// ambang, berarti ada tangan lain di mesin itu.
///
/// Ini heuristik, bukan mekanisme yang pasti. Cara yang benar-benar dapat
/// membedakan input fisik dari input suntikan adalah low-level hook dengan
/// bendera `LLMHF_INJECTED`, dan itu menuntut message loop tersendiri.
/// Heuristik ini menangkap kasus yang sebenarnya penting — seseorang meraih
/// tetikusnya dan ingin mesinnya kembali — tanpa menambah thread yang harus
/// dijaga hidupnya.
#[derive(Debug)]
pub struct PenjagaLokal {
    terakhir_ditempatkan: Option<(i32, i32)>,
    dijeda_sampai: Option<std::time::Instant>,
}

impl Default for PenjagaLokal {
    fn default() -> Self {
        Self::baru()
    }
}

impl PenjagaLokal {
    pub fn baru() -> Self {
        Self {
            terakhir_ditempatkan: None,
            dijeda_sampai: None,
        }
    }

    /// Apakah injeksi sedang ditahan.
    pub fn dijeda(&mut self) -> bool {
        // Sebelum memutuskan, periksa apakah kursor sudah berpindah sendiri.
        if let (Some(sekarang), Some(kita)) = (posisi_kursor(), self.terakhir_ditempatkan) {
            let geser = (sekarang.0 - kita.0).abs().max((sekarang.1 - kita.1).abs());
            if geser > AMBANG_GESER {
                self.dijeda_sampai = Some(std::time::Instant::now() + JEDA_LOKAL);
                self.terakhir_ditempatkan = None;
            }
        }

        match self.dijeda_sampai {
            Some(sampai) if std::time::Instant::now() < sampai => true,
            Some(_) => {
                self.dijeda_sampai = None;
                false
            }
            None => false,
        }
    }

    /// Mencatat ke mana kita baru saja menempatkan kursor.
    pub fn catat_penempatan(&mut self) {
        self.terakhir_ditempatkan = posisi_kursor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, w: u32, h: u32) -> Monitor {
        Monitor {
            id: 0,
            name: "uji".into(),
            x,
            y,
            width: w,
            height: h,
            is_primary: true,
            scale_percent: 100,
        }
    }

    #[test]
    fn pojok_monitor_tunggal_memetakan_ke_ujung_rentang() {
        let m = monitor(0, 0, 1920, 1080);
        let v = (0, 0, 1920, 1080);
        assert_eq!(ke_satuan_sendinput(&m, v, 0.0, 0.0), (0, 0));
        assert_eq!(ke_satuan_sendinput(&m, v, 1.0, 1.0), (65_535, 65_535));
    }

    #[test]
    fn tengah_monitor_tunggal_mendekati_setengah() {
        let m = monitor(0, 0, 1920, 1080);
        let (x, y) = ke_satuan_sendinput(&m, (0, 0, 1920, 1080), 0.5, 0.5);
        assert!((x - 32_767).abs() < 40, "x meleset: {x}");
        assert!((y - 32_767).abs() < 40, "y meleset: {y}");
    }

    #[test]
    fn monitor_berkoordinat_negatif_dipetakan_benar() {
        // Susunan mesin uji: DISPLAY2 tegak di kiri-atas, virtual desktop
        // 3000×1920 mulai dari (-1080, -406). Inilah bentuk yang membongkar
        // implementasi yang menghitung terhadap satu monitor saja.
        let kiri = monitor(-1080, -406, 1080, 1920);
        let virt = (-1080, -406, 3000, 1920);

        // Pojok kiri-atas monitor itu adalah pojok kiri-atas virtual desktop.
        assert_eq!(ke_satuan_sendinput(&kiri, virt, 0.0, 0.0), (0, 0));

        // Pojok kanan-bawahnya berada di tengah-kanan virtual desktop, bukan
        // di ujungnya — monitor ini hanya menempati sebagian.
        let (x, y) = ke_satuan_sendinput(&kiri, virt, 1.0, 1.0);
        assert!(x < 30_000, "x seharusnya belum mencapai ujung: {x}");
        assert_eq!(y, 65_535, "tinggi monitor ini penuh mengisi desktop");
    }

    #[test]
    fn monitor_primer_pada_susunan_negatif() {
        // Monitor primer di (0,0) berada di sepertiga kanan virtual desktop.
        let utama = monitor(0, 0, 1920, 1080);
        let virt = (-1080, -406, 3000, 1920);
        let (x, _) = ke_satuan_sendinput(&utama, virt, 0.0, 0.0);
        assert!(x > 20_000, "monitor primer terlalu ke kiri: {x}");
    }

    #[test]
    fn koordinat_di_luar_rentang_tetap_di_dalam_desktop() {
        // Viewer yang buggy tidak boleh membuat kursor melompat keluar layar.
        let m = monitor(0, 0, 1920, 1080);
        let v = (0, 0, 1920, 1080);
        for (rx, ry) in [(-5.0, 2.0), (9.9, -9.9), (f64::NAN, 0.5)] {
            let (x, y) = ke_satuan_sendinput(&m, v, rx, ry);
            assert!((0..=65_535).contains(&x), "x keluar rentang: {x}");
            assert!((0..=65_535).contains(&y), "y keluar rentang: {y}");
        }
    }

    #[test]
    fn virtual_desktop_sangat_kecil_tidak_membagi_nol() {
        let m = monitor(0, 0, 1, 1);
        let (x, y) = ke_satuan_sendinput(&m, (0, 0, 1, 1), 0.5, 0.5);
        assert!((0..=65_535).contains(&x));
        assert!((0..=65_535).contains(&y));
    }

    #[test]
    fn nomor_tombol_mengikuti_mouseevent_browser() {
        assert_eq!(Tombol::dari_nomor(0), Some(Tombol::Kiri));
        assert_eq!(Tombol::dari_nomor(1), Some(Tombol::Tengah));
        assert_eq!(Tombol::dari_nomor(2), Some(Tombol::Kanan));
        assert_eq!(Tombol::dari_nomor(3), None, "tombol samping belum didukung");
    }

    #[test]
    fn jeda_lokal_cukup_lama_untuk_satu_gerakan() {
        assert!(JEDA_LOKAL >= std::time::Duration::from_secs(2));
        assert!(JEDA_LOKAL <= std::time::Duration::from_secs(10));
    }
}
