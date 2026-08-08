//! Registri koneksi aktif.
//!
//! Fase 0 berjalan pada satu node, jadi registri cukup berada di memori
//! (ADR-013). Saat Signal Server diskalakan ke banyak node, yang berubah hanya
//! isi modul ini: pencarian tujuan berpindah ke NATS, sementara pemanggilnya
//! tetap.

use crate::protocol::Keluar;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

pub type Pengirim = mpsc::UnboundedSender<String>;

#[derive(Debug, Clone)]
pub struct Koneksi {
    pub pengirim: Pengirim,
    pub user_id: Uuid,
    pub org_id: Uuid,
}

#[derive(Debug, Default)]
struct Isi {
    /// Agent, dikunci berdasarkan device UUID.
    agent: HashMap<Uuid, Koneksi>,
    /// Viewer, dikunci berdasarkan id koneksi — satu pengguna boleh membuka
    /// beberapa viewer sekaligus, jadi user_id tidak cukup sebagai kunci.
    viewer: HashMap<Uuid, Koneksi>,
    /// Peta sesi → (viewer koneksi, device) untuk merutekan SDP dan ICE.
    sesi: HashMap<Uuid, (Uuid, Uuid)>,
}

#[derive(Debug, Clone, Default)]
pub struct Registri(Arc<RwLock<Isi>>);

impl Registri {
    pub fn baru() -> Self {
        Self::default()
    }

    pub async fn daftar_agent(&self, device_uuid: Uuid, k: Koneksi) -> Option<Koneksi> {
        // Mengembalikan koneksi lama bila ada. Pemanggil bertanggung jawab
        // memutusnya — satu perangkat hanya boleh punya satu agent aktif,
        // dan koneksi basi akan menerima pesan yang seharusnya untuk yang baru.
        self.0.write().await.agent.insert(device_uuid, k)
    }

    pub async fn daftar_viewer(&self, conn_id: Uuid, k: Koneksi) {
        self.0.write().await.viewer.insert(conn_id, k);
    }

    pub async fn lepas_agent(&self, device_uuid: Uuid) {
        self.0.write().await.agent.remove(&device_uuid);
    }

    pub async fn lepas_viewer(&self, conn_id: Uuid) {
        let mut isi = self.0.write().await;
        isi.viewer.remove(&conn_id);
        isi.sesi.retain(|_, (v, _)| *v != conn_id);
    }

    pub async fn agent(&self, device_uuid: Uuid) -> Option<Koneksi> {
        self.0.read().await.agent.get(&device_uuid).cloned()
    }

    pub async fn agent_terhubung(&self, device_uuid: Uuid) -> bool {
        self.0.read().await.agent.contains_key(&device_uuid)
    }

    pub async fn catat_sesi(&self, session_id: Uuid, viewer_conn: Uuid, device_uuid: Uuid) {
        self.0
            .write()
            .await
            .sesi
            .insert(session_id, (viewer_conn, device_uuid));
    }

    pub async fn hapus_sesi(&self, session_id: Uuid) {
        self.0.write().await.sesi.remove(&session_id);
    }

    /// Menemukan lawan bicara dalam sebuah sesi.
    ///
    /// `dari_viewer` menentukan arah: pesan dari viewer diteruskan ke agent,
    /// dan sebaliknya.
    pub async fn lawan(&self, session_id: Uuid, dari_viewer: bool) -> Option<Koneksi> {
        let isi = self.0.read().await;
        let (viewer_conn, device_uuid) = isi.sesi.get(&session_id)?;
        if dari_viewer {
            isi.agent.get(device_uuid).cloned()
        } else {
            isi.viewer.get(viewer_conn).cloned()
        }
    }

    /// Memastikan koneksi ini memang peserta sesi tersebut.
    ///
    /// Tanpa pemeriksaan ini, siapa pun yang terautentikasi dapat menyuntikkan
    /// SDP atau kandidat ICE ke sesi orang lain hanya dengan menebak
    /// `session_id` — pembajakan sesi tanpa perlu menembus kripto apa pun.
    pub async fn peserta_sah(&self, session_id: Uuid, conn_id: Uuid, device_uuid: Option<Uuid>) -> bool {
        let isi = self.0.read().await;
        match isi.sesi.get(&session_id) {
            Some((viewer_conn, dev)) => {
                *viewer_conn == conn_id || device_uuid.is_some_and(|d| d == *dev)
            }
            None => false,
        }
    }

    pub async fn jumlah(&self) -> (usize, usize) {
        let isi = self.0.read().await;
        (isi.agent.len(), isi.viewer.len())
    }
}

/// Mengirim pesan ke sebuah koneksi. Gagal kirim berarti penerima sudah pergi.
pub fn kirim(k: &Koneksi, pesan: &Keluar) -> bool {
    k.pengirim.send(pesan.ke_json()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn koneksi() -> (Koneksi, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Koneksi {
                pengirim: tx,
                user_id: Uuid::new_v4(),
                org_id: Uuid::new_v4(),
            },
            rx,
        )
    }

    #[tokio::test]
    async fn agent_terdaftar_dan_ditemukan() {
        let r = Registri::baru();
        let dev = Uuid::new_v4();
        let (k, _rx) = koneksi();
        assert!(r.daftar_agent(dev, k).await.is_none());
        assert!(r.agent_terhubung(dev).await);
        r.lepas_agent(dev).await;
        assert!(!r.agent_terhubung(dev).await);
    }

    #[tokio::test]
    async fn agent_kedua_menggantikan_dan_mengembalikan_yang_lama() {
        let r = Registri::baru();
        let dev = Uuid::new_v4();
        let (k1, _rx1) = koneksi();
        let (k2, _rx2) = koneksi();
        r.daftar_agent(dev, k1).await;
        let lama = r.daftar_agent(dev, k2).await;
        assert!(lama.is_some(), "koneksi lama harus dikembalikan untuk diputus");
    }

    #[tokio::test]
    async fn rute_sesi_dua_arah() {
        let r = Registri::baru();
        let dev = Uuid::new_v4();
        let vconn = Uuid::new_v4();
        let sess = Uuid::new_v4();

        let (ka, _ra) = koneksi();
        let (kv, _rv) = koneksi();
        r.daftar_agent(dev, ka).await;
        r.daftar_viewer(vconn, kv).await;
        r.catat_sesi(sess, vconn, dev).await;

        assert!(r.lawan(sess, true).await.is_some(), "viewer -> agent");
        assert!(r.lawan(sess, false).await.is_some(), "agent -> viewer");
    }

    #[tokio::test]
    async fn pihak_luar_bukan_peserta_sah() {
        let r = Registri::baru();
        let dev = Uuid::new_v4();
        let vconn = Uuid::new_v4();
        let sess = Uuid::new_v4();
        let (ka, _ra) = koneksi();
        let (kv, _rv) = koneksi();
        r.daftar_agent(dev, ka).await;
        r.daftar_viewer(vconn, kv).await;
        r.catat_sesi(sess, vconn, dev).await;

        assert!(r.peserta_sah(sess, vconn, None).await, "viewer peserta");
        assert!(r.peserta_sah(sess, Uuid::new_v4(), Some(dev)).await, "agent peserta");
        assert!(
            !r.peserta_sah(sess, Uuid::new_v4(), None).await,
            "pihak luar tidak boleh dianggap peserta"
        );
        assert!(
            !r.peserta_sah(Uuid::new_v4(), vconn, None).await,
            "sesi tidak dikenal harus ditolak"
        );
    }

    #[tokio::test]
    async fn viewer_lepas_membersihkan_sesinya() {
        let r = Registri::baru();
        let dev = Uuid::new_v4();
        let vconn = Uuid::new_v4();
        let sess = Uuid::new_v4();
        let (ka, _ra) = koneksi();
        let (kv, _rv) = koneksi();
        r.daftar_agent(dev, ka).await;
        r.daftar_viewer(vconn, kv).await;
        r.catat_sesi(sess, vconn, dev).await;

        r.lepas_viewer(vconn).await;
        assert!(
            !r.peserta_sah(sess, vconn, None).await,
            "sesi harus ikut terhapus saat viewer pergi"
        );
    }
}
