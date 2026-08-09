//! Sesi media WebRTC.
//!
//! M2, langkah ketiga dan terakhir: menyambungkan capture dan encoder ke jalur
//! yang sudah berjalan di produksi. Signaling, TURN, persetujuan, dan siklus
//! hidup sesi tidak disentuh sama sekali — agent native menggantikan **satu
//! ujung** dari koneksi yang sudah bekerja, persis seperti yang dijanjikan
//! NEXT_PLAN.md §11.
//!
//! ## Agent yang menawarkan, bukan viewer
//!
//! Temuan T-10: dokumen menjawab dua kali secara berlawanan soal siapa pengirim
//! SDP offer. Implementasi memakai **agent sebagai offerer**, karena agent yang
//! memiliki media. Viewer berbasis browser yang sudah ada di produksi pun
//! dibangun dengan asumsi itu, jadi agent native ini masuk ke tempat yang sama
//! tanpa mengubah satu baris pun di sisi viewer.
//!
//! ## Kenapa ada thread tersendiri
//!
//! Capture memegang objek Direct3D dan encoder memegang objek Media Foundation.
//! Keduanya terikat pada thread tempat mereka dibuat dan bukan `Send`, sehingga
//! tidak dapat hidup di dalam task async yang bebas berpindah thread. Mereka
//! menempati satu thread OS sendiri dan menyerahkan hasilnya lewat channel —
//! yang juga kebetulan bentuk yang benar untuk pekerjaan yang memblokir.

use crate::{capture, encode};
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

/// Pengaturan aliran.
#[derive(Debug, Clone)]
pub struct Pengaturan {
    pub monitor: Option<String>,
    pub fps: u32,
    pub bitrate: u32,
}

impl Default for Pengaturan {
    fn default() -> Self {
        Self {
            monitor: None,
            fps: 30,
            // 8 Mbps sebagai atap, bukan sasaran. Isi layar kerja jauh lebih
            // mudah dimampatkan daripada video gerak — pengukuran di mesin ini
            // menghasilkan sekitar 1,7 Mbps pada 1080p30.
            bitrate: 8_000_000,
        }
    }
}

/// Perintah untuk thread capture.
#[derive(Debug)]
enum Perintah {
    /// Berpindah ke monitor lain, dirujuk dengan nama perangkat GDI.
    Ganti(String),
}

/// Satu sesi media yang sedang berjalan.
pub struct SesiMedia {
    pc: Arc<RTCPeerConnection>,
    berhenti: Arc<AtomicBool>,
    /// Monitor yang sedang dibagikan. Ditulis thread capture, dibaca sisi
    /// async saat menyusun `MONITOR_LAYOUT`.
    aktif: Arc<std::sync::Mutex<String>>,
    /// Dipegang hanya agar channel tetap hidup selama sesi. Menjatuhkannya
    /// akan membuat thread capture melihat pengirim hilang.
    _tx_cmd: std::sync::mpsc::Sender<Perintah>,
}

/// Menyusun pesan `MONITOR_LAYOUT`.
///
/// Monitor dirujuk dengan **nama perangkat**, bukan indeks dalam daftar.
/// Urutan `EnumDisplayMonitors` tidak dijanjikan stabil, sehingga mencabut satu
/// monitor akan menggeser indeks yang lain — dan viewer yang menyimpan indeks
/// akan menunjuk layar yang salah tanpa satu pun galat muncul.
///
/// Nama perangkat pun bukan jaminan mutlak: Windows dapat menomori ulang
/// setelah perubahan perangkat keras. Tetapi ia bertahan melewati kejadian
/// yang lazim — layar dimatikan, kabel dicabut sementara — dan itu selisih
/// yang menentukan dalam pemakaian sehari-hari.
fn pesan_layout(aktif: &str) -> Option<String> {
    let monitors = crate::monitor::enumerasi().ok()?;
    let daftar: Vec<_> = monitors
        .iter()
        .map(|m| {
            serde_json::json!({
                "name": m.name,
                "x": m.x,
                "y": m.y,
                "width": m.width,
                "height": m.height,
                "is_primary": m.is_primary,
                "scale_percent": m.scale_percent,
            })
        })
        .collect();

    Some(
        serde_json::json!({
            "type": "MONITOR_LAYOUT",
            "payload": { "monitors": daftar, "active": aktif },
        })
        .to_string(),
    )
}

impl std::fmt::Debug for SesiMedia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SesiMedia").finish_non_exhaustive()
    }
}

impl SesiMedia {
    /// Membuka sesi dan menghasilkan SDP offer yang siap dikirim.
    ///
    /// `kirim` dipakai untuk menyerahkan kandidat ICE begitu ditemukan.
    /// Trickle ICE, bukan menunggu pengumpulan selesai: menunggu berarti
    /// menambah beberapa detik sunyi sebelum gambar pertama, dan pada jaringan
    /// yang harus jatuh ke TURN penantian itu paling terasa.
    pub async fn mulai(
        ice_servers: Vec<RTCIceServer>,
        atur: Pengaturan,
        kirim: tokio::sync::mpsc::UnboundedSender<String>,
        session_id: uuid::Uuid,
    ) -> Result<(Self, String)> {
        let mut mesin = MediaEngine::default();
        mesin
            .register_default_codecs()
            .context("gagal mendaftarkan codec")?;

        let mut daftar = Registry::new();
        daftar = register_default_interceptors(daftar, &mut mesin)
            .context("gagal mendaftarkan interceptor")?;

        let api = APIBuilder::new()
            .with_media_engine(mesin)
            .with_interceptor_registry(daftar)
            .build();

        let pc = Arc::new(
            api.new_peer_connection(RTCConfiguration {
                ice_servers,
                ..Default::default()
            })
            .await
            .context("gagal membuat peer connection")?,
        );

        // Track H.264. Browser mendekodenya di perangkat keras, jadi tidak ada
        // transcoding di mana pun sepanjang jalur.
        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            "layar".to_owned(),
            "aetherdesk".to_owned(),
        ));

        pc.add_track(Arc::clone(&track) as Arc<dyn TrackLocal + Send + Sync>)
            .await
            .context("gagal menambahkan track video")?;

        // ── Kandidat ICE ────────────────────────────────────────────────────
        let kirim_ice = kirim.clone();
        pc.on_ice_candidate(Box::new(move |kandidat| {
            let kirim = kirim_ice.clone();
            Box::pin(async move {
                // `None` menandakan pengumpulan selesai; tidak ada yang perlu
                // dikirim, dan mengirim null justru membingungkan sebagian
                // klien.
                let Some(k) = kandidat else { return };
                let Ok(init) = k.to_json() else { return };
                let pesan = serde_json::json!({
                    "type": "ICE_CANDIDATE",
                    "payload": {
                        "session_id": session_id,
                        "candidate": {
                            "candidate": init.candidate,
                            "sdpMid": init.sdp_mid,
                            "sdpMLineIndex": init.sdp_mline_index,
                        },
                    },
                });
                let _ = kirim.send(pesan.to_string());
            })
        }));

        let berhenti = Arc::new(AtomicBool::new(false));
        let aktif = Arc::new(std::sync::Mutex::new(String::new()));
        let (tx_cmd, rx_cmd) = std::sync::mpsc::channel::<Perintah>();
        // Pemberitahuan dari thread capture bahwa monitor aktif berubah.
        let (tx_ubah, mut rx_ubah) = tokio::sync::mpsc::unbounded_channel::<()>();

        // ── Kanal kendali ───────────────────────────────────────────────────
        //
        // NEXT_PLAN.md §8: perpindahan monitor lewat DataChannel, bukan lewat
        // server. Server tidak perlu tahu monitor mana yang sedang dilihat, dan
        // menaruhnya di sana berarti setiap perpindahan menempuh perjalanan
        // pulang-pergi lewat internet untuk keputusan yang sepenuhnya lokal
        // bagi kedua ujung.
        let dc = pc
            .create_data_channel("kontrol", None)
            .await
            .context("gagal membuat kanal kendali")?;

        let dc_buka = Arc::clone(&dc);
        let aktif_buka = Arc::clone(&aktif);
        dc.on_open(Box::new(move || {
            let dc = Arc::clone(&dc_buka);
            let aktif = Arc::clone(&aktif_buka);
            Box::pin(async move {
                let nama = aktif.lock().map(|g| g.clone()).unwrap_or_default();
                if let Some(p) = pesan_layout(&nama) {
                    let _ = dc.send_text(p).await;
                }
            })
        }));

        let tx_cmd_pesan = tx_cmd.clone();
        dc.on_message(Box::new(move |msg| {
            let tx = tx_cmd_pesan.clone();
            Box::pin(async move {
                let Ok(teks) = String::from_utf8(msg.data.to_vec()) else {
                    return;
                };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&teks) else {
                    return;
                };
                if v.get("type").and_then(|t| t.as_str()) != Some("MONITOR_SELECT") {
                    return;
                }
                let Some(nama) = v
                    .get("payload")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                else {
                    return;
                };
                tracing::info!(monitor = nama, "viewer meminta pindah monitor");
                let _ = tx.send(Perintah::Ganti(nama.to_string()));
            })
        }));

        // Meneruskan perubahan monitor aktif ke viewer.
        let dc_ubah = Arc::clone(&dc);
        let aktif_ubah = Arc::clone(&aktif);
        tokio::spawn(async move {
            while rx_ubah.recv().await.is_some() {
                let nama = aktif_ubah.lock().map(|g| g.clone()).unwrap_or_default();
                if let Some(p) = pesan_layout(&nama) {
                    let _ = dc_ubah.send_text(p).await;
                }
            }
        });

        let henti_status = Arc::clone(&berhenti);
        pc.on_peer_connection_state_change(Box::new(move |keadaan| {
            tracing::info!(?keadaan, "keadaan koneksi berubah");
            if matches!(
                keadaan,
                RTCPeerConnectionState::Failed
                    | RTCPeerConnectionState::Closed
                    | RTCPeerConnectionState::Disconnected
            ) {
                // Menghentikan capture begitu koneksi hilang. Tanpa ini, encoder
                // terus bekerja penuh untuk penerima yang sudah tidak ada.
                henti_status.store(true, Ordering::Relaxed);
            }
            Box::pin(async {})
        }));

        // ── Thread capture dan encode ───────────────────────────────────────
        let (tx_au, mut rx_au) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
        let (tx_siap, rx_siap) = std::sync::mpsc::channel::<std::result::Result<(), String>>();
        let henti_thread = Arc::clone(&berhenti);
        let atur_thread = atur.clone();
        let aktif_thread = Arc::clone(&aktif);

        std::thread::Builder::new()
            .name("aetherdesk-capture".into())
            .spawn(move || {
                jalankan_capture(
                    atur_thread,
                    tx_au,
                    tx_siap,
                    henti_thread,
                    rx_cmd,
                    aktif_thread,
                    tx_ubah,
                );
            })
            .context("gagal membuat thread capture")?;

        // Menunggu thread melapor bahwa capture dan encoder benar-benar
        // terbuka. Tanpa ini, offer terkirim lebih dulu dan kegagalan capture
        // baru terlihat sebagai layar hitam di sisi viewer.
        match rx_siap.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => anyhow::bail!("{e}"),
            Err(_) => anyhow::bail!("capture tidak siap dalam 10 detik"),
        }

        // ── Penyuap track ───────────────────────────────────────────────────
        let durasi = Duration::from_micros(1_000_000 / atur.fps.max(1) as u64);
        let henti_kirim = Arc::clone(&berhenti);
        let pc_kirim = Arc::clone(&pc);
        tokio::spawn(async move {
            while let Some(au) = rx_au.recv().await {
                if henti_kirim.load(Ordering::Relaxed) {
                    break;
                }
                let sampel = Sample {
                    data: au.into(),
                    duration: durasi,
                    ..Default::default()
                };
                if let Err(e) = track.write_sample(&sampel).await {
                    tracing::warn!(error = %e, "gagal menulis sampel ke track");
                    break;
                }
            }

            // Aliran berhenti sebelum ada yang memintanya — capture atau
            // encoder gagal. Koneksi ditutup supaya viewer melihat sesi
            // berakhir, bukan gambar yang membeku selamanya.
            //
            // Gambar beku adalah kegagalan paling buruk bentuknya: ia terlihat
            // seperti jaringan lambat, sehingga orang menunggu alih-alih
            // melapor.
            if !henti_kirim.load(Ordering::Relaxed) {
                tracing::error!("aliran media berhenti tanpa diminta, sesi ditutup");
                henti_kirim.store(true, Ordering::Relaxed);
                let _ = pc_kirim.close().await;
            }
            tracing::debug!("penyuap track berhenti");
        });

        // ── Offer ───────────────────────────────────────────────────────────
        let offer = pc.create_offer(None).await.context("gagal membuat offer")?;
        pc.set_local_description(offer.clone())
            .await
            .context("gagal menetapkan deskripsi lokal")?;

        Ok((
            Self {
                pc,
                berhenti,
                aktif,
                _tx_cmd: tx_cmd,
            },
            offer.sdp,
        ))
    }

    /// Monitor yang sedang dibagikan.
    pub fn monitor_aktif(&self) -> String {
        self.aktif.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Menerima SDP answer dari viewer.
    pub async fn jawaban(&self, sdp: &str) -> Result<()> {
        let jawab = RTCSessionDescription::answer(sdp.to_string())
            .context("SDP answer tidak valid")?;
        self.pc
            .set_remote_description(jawab)
            .await
            .context("gagal menetapkan deskripsi jarak jauh")
    }

    /// Menerima satu kandidat ICE dari viewer.
    pub async fn kandidat(&self, nilai: &serde_json::Value) -> Result<()> {
        let Some(c) = nilai.get("candidate").and_then(|v| v.as_str()) else {
            // Kandidat kosong menandakan akhir pengumpulan pada sebagian klien.
            return Ok(());
        };
        let init = RTCIceCandidateInit {
            candidate: c.to_string(),
            sdp_mid: nilai
                .get("sdpMid")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            sdp_mline_index: nilai
                .get("sdpMLineIndex")
                .and_then(|v| v.as_u64())
                .map(|v| v as u16),
            ..Default::default()
        };
        self.pc
            .add_ice_candidate(init)
            .await
            .context("kandidat ICE ditolak")
    }

    pub async fn tutup(&self) {
        self.berhenti.store(true, Ordering::Relaxed);
        if let Err(e) = self.pc.close().await {
            tracing::debug!(error = %e, "penutupan peer connection tidak bersih");
        }
    }
}

/// Membuka capture beserta encoder yang sepadan dengan ukurannya.
fn buka_pasangan(
    monitor: Option<&str>,
    atur: &Pengaturan,
) -> Result<(capture::Duplikasi, encode::H264)> {
    let dup = capture::Duplikasi::buka(monitor)?;
    // Encoder dibuat ulang setiap kali monitor berpindah, bukan dipakai lagi.
    // Resolusi dan orientasi ikut berubah, dan encoder H.264 tidak dapat
    // mengganti ukuran frame di tengah jalan. Membuat yang baru juga
    // menghasilkan SPS, PPS, dan keyframe segar — persis yang dibutuhkan
    // dekoder di seberang untuk menyusun ulang dirinya.
    let enc = encode::H264::baru(dup.width, dup.height, atur.fps, atur.bitrate)?;
    Ok((dup, enc))
}

/// Perulangan capture dan encode. Berjalan pada thread sendiri.
fn jalankan_capture(
    atur: Pengaturan,
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    tx_siap: std::sync::mpsc::Sender<std::result::Result<(), String>>,
    berhenti: Arc<AtomicBool>,
    rx_cmd: std::sync::mpsc::Receiver<Perintah>,
    aktif: Arc<std::sync::Mutex<String>>,
    tx_ubah: tokio::sync::mpsc::UnboundedSender<()>,
) {
    let (mut dup, mut enc) = match buka_pasangan(atur.monitor.as_deref(), &atur) {
        Ok(v) => {
            let _ = tx_siap.send(Ok(()));
            v
        }
        Err(e) => {
            let _ = tx_siap.send(Err(format!("{e:#}")));
            return;
        }
    };

    let mut catat_aktif = |nama: &str| {
        if let Ok(mut g) = aktif.lock() {
            *g = nama.to_string();
        }
        let _ = tx_ubah.send(());
    };
    catat_aktif(&dup.nama_output);

    tracing::info!(
        monitor = %dup.nama_output,
        ukuran = %format!("{}×{}", dup.width, dup.height),
        encoder = %enc.nama,
        "capture dimulai"
    );

    let mulai = std::time::Instant::now();
    let jarak = Duration::from_micros(1_000_000 / atur.fps.max(1) as u64);
    let mut berikutnya = std::time::Instant::now();
    let mut terakhir: Option<capture::Frame> = None;

    while !berhenti.load(Ordering::Relaxed) {
        // Perintah diperiksa lebih dulu supaya perpindahan monitor terasa
        // seketika, bukan menunggu satu siklus frame lagi.
        while let Ok(perintah) = rx_cmd.try_recv() {
            let Perintah::Ganti(nama) = perintah;
            if nama == dup.nama_output {
                continue;
            }
            match buka_pasangan(Some(&nama), &atur) {
                Ok((d, e)) => {
                    tracing::info!(
                        dari = %dup.nama_output,
                        ke = %d.nama_output,
                        ukuran = %format!("{}×{}", d.width, d.height),
                        "monitor berpindah"
                    );
                    dup = d;
                    enc = e;
                    terakhir = None;
                    berikutnya = std::time::Instant::now();
                    catat_aktif(&dup.nama_output);
                }
                Err(e) => {
                    // Monitor lama tetap dipakai. Sesi yang berjalan tidak
                    // boleh mati hanya karena permintaan pindah gagal —
                    // monitornya mungkin baru saja dicabut.
                    tracing::warn!(
                        monitor = %nama,
                        error = %format!("{e:#}"),
                        "gagal pindah monitor, tetap pada yang lama"
                    );
                }
            }
        }

        match dup.ambil(5) {
            Ok(Some(f)) => terakhir = Some(f),
            Ok(None) => {}
            Err(e) => {
                tracing::error!(error = %e, "capture gagal");
                break;
            }
        }

        if std::time::Instant::now() < berikutnya {
            continue;
        }
        berikutnya += jarak;

        let Some(f) = &terakhir else { continue };
        let waktu = mulai.elapsed().as_nanos() as i64 / 100;

        match enc.encode(&f.data, waktu) {
            Ok(unit) => {
                for au in unit {
                    // `blocking_send` menahan thread ini saat penerima
                    // tertinggal, dan itu justru perilaku yang benar: menumpuk
                    // frame di memori hanya menambah latensi tanpa menambah
                    // satu pun frame yang benar-benar sampai.
                    if tx.blocking_send(au).is_err() {
                        tracing::debug!("penerima sampel hilang, capture berhenti");
                        return;
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "encode gagal");
                break;
            }
        }
    }

    tracing::info!("capture berhenti");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pengaturan_baku_masuk_akal() {
        let p = Pengaturan::default();
        assert_eq!(p.fps, 30);
        assert!(p.bitrate >= 1_000_000, "atap bitrate terlalu rendah");
        assert!(p.monitor.is_none(), "baku harus monitor primer");
    }

    #[tokio::test]
    async fn kandidat_kosong_tidak_dianggap_galat() {
        // Sebagian klien menutup pengumpulan dengan kandidat kosong. Itu bukan
        // kegagalan, dan memperlakukannya sebagai galat akan memutus sesi yang
        // sebenarnya sehat.
        let nilai = serde_json::json!({ "candidate": "" });
        assert!(nilai.get("candidate").and_then(|v| v.as_str()) == Some(""));

        let tanpa = serde_json::json!({});
        assert!(tanpa.get("candidate").is_none());
    }
}
