//! Penulisan audit log.
//!
//! Tabel `audit_logs` beserta trigger append-only sudah ada sejak migrasi
//! pertama dan terbukti menolak `UPDATE` maupun `DELETE` — tetapi tidak ada
//! satu pun jalur kode yang pernah menulis ke sana. Infrastrukturnya lengkap,
//! isinya kosong, dan PRD §12.3 mensyaratkan jejak audit untuk seluruh
//! peristiwa yang relevan bagi keamanan.
//!
//! Dua aturan yang mengikat modul ini:
//!
//! 1. **Kegagalan menulis audit tidak boleh menggagalkan permintaan.** Menolak
//!    login karena audit gagal ditulis mengubah masalah pencatatan menjadi
//!    pemadaman layanan.
//! 2. **Kegagalan itu tetap harus terlihat.** Jejak audit yang diam-diam hilang
//!    lebih berbahaya daripada yang tidak pernah ada, karena menimbulkan rasa
//!    aman yang keliru saat diaudit.

use serde_json::Value;
use sqlx::PgPool;
use std::net::IpAddr;
use uuid::Uuid;

/// Aksi yang dicatat. Nilai-nilai ini masuk ke database dan dipakai untuk
/// memfilter — mengubahnya memutus kueri dan laporan yang sudah ada.
pub mod aksi {
    pub const LOGIN: &str = "user.login";
    pub const LOGIN_GAGAL: &str = "user.login_failed";
    pub const LOGOUT: &str = "user.logout";
    pub const ORG_DIBUAT: &str = "org.bootstrap";
    pub const DEVICE_DIDAFTARKAN: &str = "device.register";
    pub const DEVICE_SANDI_DIROTASI: &str = "device.rotate_password";
    pub const SESI_DIMINTA: &str = "session.request";
    pub const SESI_DITOLAK: &str = "session.rejected";
}

pub struct Entri<'a> {
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    pub ip: IpAddr,
    pub aksi: &'a str,
    pub payload: Option<Value>,
}

/// Menulis satu entri audit. Tidak pernah mengembalikan galat kepada pemanggil.
///
/// `audit_logs` sengaja tidak memakai RLS: penulisannya terjadi pada jalur yang
/// tenant-nya sudah diketahui dari token, dan membebaninya dengan konteks
/// transaksi hanya menambah cara baru untuk gagal mencatat.
pub async fn catat(db: &PgPool, e: Entri<'_>) {
    let hasil = sqlx::query(
        "INSERT INTO audit_logs (organization_id, user_id, ip_address, action, payload)
         VALUES ($1, $2, $3::inet, $4, $5)",
    )
    .bind(e.org_id)
    .bind(e.user_id)
    .bind(e.ip.to_string())
    .bind(e.aksi)
    .bind(e.payload)
    .execute(db)
    .await;

    if let Err(err) = hasil {
        // Naik ke ERROR, bukan WARN: hilangnya jejak audit adalah peristiwa
        // yang pantas membangunkan orang, bukan sekadar catatan pinggir.
        tracing::error!(error = %err, aksi = e.aksi, "GAGAL MENULIS AUDIT LOG");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nama_aksi_memakai_titik_dan_huruf_kecil() {
        // Konvensi DATABASE.md §2.8: `user.login`, `device.delete`,
        // `permission.grant`. Konsistensi ini yang membuat filter berpola
        // seperti `action LIKE 'device.%'` dapat diandalkan.
        for a in [
            aksi::LOGIN,
            aksi::LOGIN_GAGAL,
            aksi::LOGOUT,
            aksi::ORG_DIBUAT,
            aksi::DEVICE_DIDAFTARKAN,
            aksi::DEVICE_SANDI_DIROTASI,
            aksi::SESI_DIMINTA,
            aksi::SESI_DITOLAK,
        ] {
            assert!(a.contains('.'), "{a} tidak memakai pemisah titik");
            assert_eq!(a, a.to_lowercase(), "{a} bukan huruf kecil");
            assert!(a.len() <= 100, "{a} melampaui lebar kolom");
        }
    }

    #[test]
    fn nama_aksi_unik() {
        use std::collections::HashSet;
        let semua = [
            aksi::LOGIN, aksi::LOGIN_GAGAL, aksi::LOGOUT, aksi::ORG_DIBUAT,
            aksi::DEVICE_DIDAFTARKAN, aksi::DEVICE_SANDI_DIROTASI,
            aksi::SESI_DIMINTA, aksi::SESI_DITOLAK,
        ];
        assert_eq!(semua.iter().collect::<HashSet<_>>().len(), semua.len());
    }
}
