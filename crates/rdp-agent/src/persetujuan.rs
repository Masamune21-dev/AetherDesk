//! Siapa yang boleh masuk, dan siapa yang memutuskannya.
//!
//! QUICK_CONNECT.md §4.1 mewajibkan prompt persetujuan yang menyebutkan siapa
//! yang meminta. Agent berbasis browser memenuhinya sejak awal; agent native
//! tidak pernah bisa, karena ia tidak punya antarmuka untuk bertanya — dan
//! sampai sekarang ia **menerima setiap permintaan secara otomatis**.
//!
//! Modul ini menutup lubang itu, dan sekaligus mewujudkan apa yang biasa orang
//! harapkan dari aplikasi remote desktop: izinkan sekali, dan setelahnya orang
//! yang sama tidak perlu diizinkan lagi.
//!
//! ## Tiga mode, dan hanya satu yang tidak aman
//!
//! | Mode | Perilaku |
//! |---|---|
//! | `IzinkanSemua` | menerima siapa pun — hanya untuk mesin milik sendiri |
//! | `HanyaTepercaya` | menerima yang sudah ada di daftar, menolak sisanya |
//! | `Tanya` | bertanya lewat antarmuka, lalu mengingat jawabannya |
//!
//! Mode baku adalah `HanyaTepercaya`. Menolak permintaan yang tidak dikenal
//! lebih baik daripada menerimanya diam-diam: penolakan terlihat dan dapat
//! ditindaklanjuti, penerimaan diam-diam tidak.

use crate::dipercaya::Daftar;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Orang yang meminta akses, sebagaimana dilaporkan server.
///
/// Seluruh isinya berasal dari klaim token yang sudah diverifikasi, bukan dari
/// apa pun yang dikirim viewer. Prompt yang menampilkan nama pilihan penyerang
/// sendiri adalah prompt yang membantu penipuan, bukan mencegahnya.
#[derive(Debug, Clone)]
pub struct Peminta {
    pub user_id: Uuid,
    pub nama: String,
    pub email: String,
    pub ip: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keputusan {
    Tolak,
    IzinkanSekali,
    IzinkanSelalu,
}

/// Satu pertanyaan yang menunggu jawaban antarmuka.
#[derive(Debug)]
pub struct Permintaan {
    pub peminta: Peminta,
    pub jawab: tokio::sync::oneshot::Sender<Keputusan>,
}

#[derive(Debug, Clone)]
pub enum Mode {
    /// Menerima siapa pun. Dipakai `--izinkan-semua`.
    IzinkanSemua,
    /// Hanya yang sudah ada di daftar kepercayaan.
    HanyaTepercaya,
    /// Bertanya lewat kotak dialog sistem.
    ///
    /// Tanpa satu pun dependensi baru: `MessageBoxW` sudah ada di crate
    /// `windows` yang dipakai capture. Bentuknya sederhana dan tombolnya
    /// mengikuti bahasa sistem, tetapi ia muncul di depan, memaksa jawaban,
    /// dan itulah yang sebenarnya dibutuhkan — jendela yang lebih baik
    /// menyusul, bukan sebaliknya.
    Dialog,
    /// Bertanya lewat antarmuka yang dijalankan pemanggil.
    Tanya(tokio::sync::mpsc::UnboundedSender<Permintaan>),
}

/// Berapa lama menunggu jawaban manusia sebelum menganggapnya menolak.
///
/// Diam **bukan** persetujuan. Permintaan yang tidak dijawab — pemiliknya
/// sedang tidak di depan mesin, atau memang sengaja mengabaikannya — berakhir
/// sebagai penolakan, dan viewer diberi tahu alih-alih menunggu selamanya.
pub const BATAS_MENUNGGU: std::time::Duration = std::time::Duration::from_secs(45);

#[derive(Debug, Clone)]
pub struct Penjaga {
    daftar: Arc<Mutex<Daftar>>,
    mode: Mode,
}

impl Penjaga {
    pub fn baru(mode: Mode) -> Self {
        Self {
            daftar: Arc::new(Mutex::new(Daftar::muat())),
            mode,
        }
    }

    /// Daftar kepercayaan yang dipakai bersama antarmuka.
    pub fn daftar(&self) -> Arc<Mutex<Daftar>> {
        Arc::clone(&self.daftar)
    }

    /// Memutuskan apakah satu permintaan sesi diterima.
    pub async fn putuskan(&self, p: &Peminta) -> bool {
        if matches!(self.mode, Mode::IzinkanSemua) {
            tracing::warn!(
                peminta = %p.email,
                "permintaan diterima tanpa persetujuan — mode --izinkan-semua"
            );
            return true;
        }

        if self.sudah_dipercaya(p.user_id) {
            tracing::info!(peminta = %p.email, "diterima — sudah ada di daftar kepercayaan");
            return true;
        }

        if matches!(self.mode, Mode::Dialog) {
            let salinan = p.clone();
            // Dialog memblokir thread-nya sampai dijawab, jadi ia tidak boleh
            // menempati thread runtime async.
            let keputusan = tokio::task::spawn_blocking(move || dialog::tanya(&salinan))
                .await
                .unwrap_or(Keputusan::Tolak);
            return self.terapkan(p, keputusan);
        }

        let Mode::Tanya(pengirim) = &self.mode else {
            tracing::warn!(
                peminta = %p.email,
                "permintaan ditolak — belum dipercaya dan tidak ada antarmuka untuk bertanya"
            );
            return false;
        };

        let (jawab, tunggu) = tokio::sync::oneshot::channel();
        if pengirim
            .send(Permintaan {
                peminta: p.clone(),
                jawab,
            })
            .is_err()
        {
            tracing::error!("antarmuka persetujuan hilang — permintaan ditolak");
            return false;
        }

        let keputusan = match tokio::time::timeout(BATAS_MENUNGGU, tunggu).await {
            Ok(Ok(k)) => k,
            // Jendela ditutup tanpa menjawab, atau waktunya habis. Keduanya
            // bukan persetujuan.
            Ok(Err(_)) => Keputusan::Tolak,
            Err(_) => {
                tracing::info!(peminta = %p.email, "tidak dijawab dalam batas waktu — ditolak");
                Keputusan::Tolak
            }
        };

        self.terapkan(p, keputusan)
    }

    fn terapkan(&self, p: &Peminta, k: Keputusan) -> bool {
        match k {
            Keputusan::Tolak => false,
            Keputusan::IzinkanSekali => true,
            Keputusan::IzinkanSelalu => {
                self.percayai(p);
                true
            }
        }
    }

    fn sudah_dipercaya(&self, user_id: Uuid) -> bool {
        let Ok(mut d) = self.daftar.lock() else {
            // Mutex yang teracuni berarti ada thread yang panik sambil
            // memegangnya. Menolak lebih aman daripada menebak isinya.
            tracing::error!("daftar kepercayaan tidak dapat dibaca — permintaan ditolak");
            return false;
        };
        if d.dipercaya(user_id) {
            d.catat_pemakaian(user_id);
            let _ = d.simpan();
            true
        } else {
            false
        }
    }

    fn percayai(&self, p: &Peminta) {
        let Ok(mut d) = self.daftar.lock() else {
            tracing::error!("daftar kepercayaan tidak dapat ditulis");
            return;
        };
        d.tambah(p.user_id, &p.email);
        match d.simpan() {
            Ok(()) => tracing::info!(peminta = %p.email, "ditambahkan ke daftar kepercayaan"),
            // Izinnya tetap berlaku untuk sesi ini; yang gagal hanya
            // mengingatnya. Pengguna akan ditanya lagi lain kali, dan itu
            // jauh lebih baik daripada sesi yang gagal.
            Err(e) => tracing::error!(error = %e, "gagal menyimpan daftar kepercayaan"),
        }
    }
}

// ── Kotak dialog sistem ──────────────────────────────────────────────────────

#[cfg(windows)]
mod dialog {
    use super::{Keputusan, Peminta};
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDNO, IDYES, MB_ICONWARNING, MB_SETFOREGROUND, MB_SYSTEMMODAL,
        MB_YESNOCANCEL,
    };

    pub fn tanya(p: &Peminta) -> Keputusan {
        // Isi prompt mengikuti QUICK_CONNECT.md §4.1: siapa yang meminta, dari
        // mana, dan apa yang dapat ia lakukan. Semuanya berasal dari klaim
        // token yang sudah diverifikasi server, bukan dari apa pun yang
        // dikirim viewer — prompt yang menampilkan nama pilihan penyerang
        // sendiri justru membantu penipuan.
        let pesan = format!(
            "{}\n\
             {}\n\
             Dari  {}\n\n\
             Orang ini akan dapat MELIHAT LAYAR mesin ini, dan menggerakkan \
             mouse serta mengetik bila kendali diizinkan.\n\n\
             Ya      — izinkan, dan jangan tanya lagi untuk orang ini\n\
             Tidak   — izinkan hanya untuk sesi ini\n\
             Batal   — tolak\n\n\
             Bila Anda tidak sedang meminta bantuan siapa pun, pilih Batal.",
            p.nama,
            p.email,
            if p.ip.is_empty() { "alamat tidak diketahui" } else { &p.ip },
        );

        let hasil = unsafe {
            MessageBoxW(
                None,
                &HSTRING::from(pesan),
                &HSTRING::from("AetherDesk — permintaan akses jarak jauh"),
                // SYSTEMMODAL memastikan kotaknya muncul di atas apa pun yang
                // sedang tampil. Permintaan yang tersembunyi di balik jendela
                // lain sama saja dengan tidak bertanya.
                MB_YESNOCANCEL | MB_ICONWARNING | MB_SYSTEMMODAL | MB_SETFOREGROUND,
            )
        };

        match hasil {
            IDYES => Keputusan::IzinkanSelalu,
            IDNO => Keputusan::IzinkanSekali,
            // Termasuk Batal, tombol tutup, dan kegagalan menampilkan kotaknya
            // sama sekali. Apa pun yang bukan persetujuan tegas adalah
            // penolakan.
            _ => Keputusan::Tolak,
        }
    }
}

#[cfg(not(windows))]
mod dialog {
    use super::{Keputusan, Peminta};
    pub fn tanya(_p: &Peminta) -> Keputusan {
        Keputusan::Tolak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peminta() -> Peminta {
        Peminta {
            user_id: Uuid::nil(),
            nama: "Uji".into(),
            email: "a@b.c".into(),
            ip: "203.0.113.1".into(),
        }
    }

    #[tokio::test]
    async fn mode_izinkan_semua_menerima_yang_tidak_dikenal() {
        let p = Penjaga::baru(Mode::IzinkanSemua);
        assert!(p.putuskan(&peminta()).await);
    }

    #[tokio::test]
    async fn tanpa_antarmuka_yang_tidak_dikenal_ditolak() {
        // Inilah perubahan perilaku terpenting: sebelum modul ini ada, agent
        // native menerima setiap permintaan secara otomatis.
        let p = Penjaga::baru(Mode::HanyaTepercaya);
        assert!(!p.putuskan(&peminta()).await);
    }

    #[tokio::test]
    async fn jawaban_izinkan_sekali_tidak_mengingat() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let penjaga = Penjaga::baru(Mode::Tanya(tx));

        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let _ = req.jawab.send(Keputusan::IzinkanSekali);
            }
        });

        assert!(penjaga.putuskan(&peminta()).await);
        assert!(
            !penjaga.daftar().lock().unwrap().dipercaya(Uuid::nil()),
            "izin sekali ikut tersimpan"
        );
    }

    #[tokio::test]
    async fn penolakan_dihormati() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let penjaga = Penjaga::baru(Mode::Tanya(tx));
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let _ = req.jawab.send(Keputusan::Tolak);
            }
        });
        assert!(!penjaga.putuskan(&peminta()).await);
    }

    #[tokio::test]
    async fn antarmuka_yang_hilang_berarti_menolak() {
        // Pengirim dibuat lalu penerimanya langsung dijatuhkan, meniru jendela
        // yang tertutup. Diam tidak boleh berubah menjadi izin.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(rx);
        let penjaga = Penjaga::baru(Mode::Tanya(tx));
        assert!(!penjaga.putuskan(&peminta()).await);
    }

    #[tokio::test(start_paused = true)]
    async fn tidak_dijawab_berarti_menolak() {
        // Permintaan yang dibiarkan menggantung — pemiliknya sedang tidak di
        // depan mesin. Menunggu selamanya akan membuat viewer mengira
        // koneksinya bermasalah.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let penjaga = Penjaga::baru(Mode::Tanya(tx));
        tokio::spawn(async move {
            // Diterima, lalu sengaja tidak dijawab dan tidak dijatuhkan.
            let mut simpan = Vec::new();
            while let Some(req) = rx.recv().await {
                simpan.push(req);
            }
        });
        assert!(!penjaga.putuskan(&peminta()).await);
    }

    #[test]
    fn batas_menunggu_masuk_akal_bagi_manusia() {
        // Cukup untuk beralih jendela dan membaca prompt, tidak cukup untuk
        // ditinggal pergi.
        assert!(BATAS_MENUNGGU >= std::time::Duration::from_secs(20));
        assert!(BATAS_MENUNGGU <= std::time::Duration::from_secs(120));
    }
}
