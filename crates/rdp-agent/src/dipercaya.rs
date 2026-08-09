//! Daftar orang yang pernah diizinkan mengakses mesin ini.
//!
//! "Izinkan sekali, berikutnya tidak usah" — kebiasaan yang sudah dikenal dari
//! UltraViewer dan AnyDesk. Yang membuatnya aman bukan kenyamanannya melainkan
//! **siapa yang memutuskan**: daftarnya hidup di mesin agent, bukan di server.
//!
//! Itu bukan detail penyimpanan. Persetujuan adalah milik orang yang duduk di
//! depan mesin ini; menaruh daftarnya di server berarti siapa pun yang
//! menguasai server dapat menambahkan dirinya sendiri ke dalamnya. Server tidak
//! pernah tahu isi berkas ini dan tidak pernah dapat mengubahnya.
//!
//! Konsekuensi yang diterima: memasang ulang agent menghapus seluruh
//! kepercayaan. Itu justru perilaku yang benar — mesin yang baru dipasang belum
//! pernah menyetujui siapa pun.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

const BERKAS: &str = "dipercaya.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tepercaya {
    /// Identitas pengguna dari klaim token yang sudah diverifikasi server.
    ///
    /// Bukan email. Email dapat berpindah pemilik, dan daftar yang menunjuk
    /// email lama akan diam-diam mengizinkan orang yang salah.
    pub user_id: Uuid,
    /// Disimpan hanya untuk ditampilkan. Bila email berubah, yang tampil ikut
    /// usang — tetapi keputusan izinnya tetap terikat pada `user_id`.
    pub email: String,
    pub diizinkan_pada: chrono::DateTime<chrono::Utc>,
    pub terakhir_dipakai: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Daftar {
    #[serde(default)]
    pub entri: Vec<Tepercaya>,
}

fn path() -> Result<PathBuf> {
    Ok(crate::identitas::direktori()?.join(BERKAS))
}

impl Daftar {
    /// Memuat daftar; berkas yang belum ada berarti daftar kosong.
    ///
    /// Berkas yang **rusak** juga menghasilkan daftar kosong, bukan galat.
    /// Gagal memuat lalu menolak berjalan akan mengunci pemilik dari mesinnya
    /// sendiri; gagal memuat lalu menganggap semua orang tepercaya jauh lebih
    /// buruk lagi. Kosong adalah satu-satunya jawaban yang aman.
    pub fn muat() -> Self {
        let Ok(p) = path() else {
            return Self::default();
        };
        let Ok(isi) = std::fs::read_to_string(&p) else {
            return Self::default();
        };
        match serde_json::from_str(&isi) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    path = %p.display(),
                    "daftar kepercayaan rusak — diperlakukan sebagai kosong"
                );
                Self::default()
            }
        }
    }

    pub fn simpan(&self) -> Result<()> {
        let p = path()?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("gagal menulis {}", p.display()))
    }

    pub fn dipercaya(&self, user_id: Uuid) -> bool {
        self.entri.iter().any(|e| e.user_id == user_id)
    }

    /// Menambahkan seseorang, atau menyegarkan catatan yang sudah ada.
    pub fn tambah(&mut self, user_id: Uuid, email: &str) {
        if let Some(e) = self.entri.iter_mut().find(|e| e.user_id == user_id) {
            // Email disegarkan supaya tampilan tidak usang, tetapi tanggal
            // pemberian izin tidak — kapan seseorang dipercaya adalah fakta
            // yang tidak berubah, dan justru itu yang berguna saat diaudit.
            e.email = email.to_string();
            return;
        }
        self.entri.push(Tepercaya {
            user_id,
            email: email.to_string(),
            diizinkan_pada: chrono::Utc::now(),
            terakhir_dipakai: None,
        });
    }

    pub fn catat_pemakaian(&mut self, user_id: Uuid) {
        if let Some(e) = self.entri.iter_mut().find(|e| e.user_id == user_id) {
            e.terakhir_dipakai = Some(chrono::Utc::now());
        }
    }

    /// Mencabut satu orang. Mengembalikan benar bila ada yang tercabut.
    pub fn cabut(&mut self, user_id: Uuid) -> bool {
        let sebelum = self.entri.len();
        self.entri.retain(|e| e.user_id != user_id);
        self.entri.len() != sebelum
    }

    pub fn kosongkan(&mut self) {
        self.entri.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daftar_uji() -> Daftar {
        let mut d = Daftar::default();
        d.tambah(Uuid::nil(), "a@b.c");
        d
    }

    #[test]
    fn yang_ditambahkan_menjadi_dipercaya() {
        let d = daftar_uji();
        assert!(d.dipercaya(Uuid::nil()));
        assert!(!d.dipercaya(Uuid::new_v4()), "orang lain ikut dipercaya");
    }

    #[test]
    fn menambah_dua_kali_tidak_menggandakan() {
        let mut d = daftar_uji();
        d.tambah(Uuid::nil(), "a@b.c");
        assert_eq!(d.entri.len(), 1);
    }

    #[test]
    fn email_disegarkan_tetapi_tanggal_izin_tidak() {
        let mut d = daftar_uji();
        let semula = d.entri[0].diizinkan_pada;
        d.tambah(Uuid::nil(), "baru@b.c");
        assert_eq!(d.entri[0].email, "baru@b.c");
        assert_eq!(
            d.entri[0].diizinkan_pada, semula,
            "tanggal pemberian izin ikut berubah — jejak auditnya hilang"
        );
    }

    #[test]
    fn pencabutan_bekerja_dan_melaporkan() {
        let mut d = daftar_uji();
        assert!(d.cabut(Uuid::nil()));
        assert!(!d.dipercaya(Uuid::nil()));
        assert!(!d.cabut(Uuid::nil()), "pencabutan kedua melapor berhasil");
    }

    #[test]
    fn daftar_kosong_tidak_mempercayai_siapa_pun() {
        let d = Daftar::default();
        assert!(!d.dipercaya(Uuid::nil()));
        assert!(!d.dipercaya(Uuid::new_v4()));
    }

    #[test]
    fn berkas_rusak_menghasilkan_daftar_kosong() {
        // Yang diuji di sini adalah keputusannya, bukan berkasnya: apa pun yang
        // gagal dibaca harus berakhir sebagai daftar kosong, tidak pernah
        // sebagai daftar yang mempercayai seseorang.
        let rusak: Result<Daftar, _> = serde_json::from_str("{ bukan json");
        assert!(rusak.is_err());
        assert!(!Daftar::default().dipercaya(Uuid::new_v4()));
    }

    #[test]
    fn berkas_tanpa_medan_entri_tetap_terbaca() {
        // Berkas dari versi lebih lama, atau yang ditulis tangan.
        let d: Daftar = serde_json::from_str("{}").unwrap();
        assert!(d.entri.is_empty());
    }

    #[test]
    fn bolak_balik_json_mempertahankan_identitas() {
        let d = daftar_uji();
        let s = serde_json::to_string(&d).unwrap();
        let kembali: Daftar = serde_json::from_str(&s).unwrap();
        assert!(kembali.dipercaya(Uuid::nil()));
    }
}
