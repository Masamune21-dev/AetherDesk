//! Koneksi ke Signal Server, dan siklus hidup sesi media.
//!
//! Menutup M1 sekaligus M2c: agent hadir di dashboard sebagai perangkat online,
//! dan ketika viewer meminta koneksi, ia benar-benar menyerahkan layarnya.
//!
//! Signaling, TURN, persetujuan, dan siklus hidup sesi di sisi server tidak
//! disentuh. Agent native menggantikan **satu ujung** dari koneksi yang sudah
//! bekerja — NEXT_PLAN.md §11.

use crate::{api::Klien, identitas::Konfigurasi, rtc};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use rdp_core::DeviceKeypair;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use webrtc::ice_transport::ice_server::RTCIceServer;

const BACKOFF_AWAL: Duration = Duration::from_secs(1);
const BACKOFF_MAKS: Duration = Duration::from_secs(30);

/// Jarak antar heartbeat.
///
/// Lebih rapat daripada yang dibutuhkan status kehadiran, karena kehadiran
/// bukan tugasnya — Signal Server yang memilikinya lewat sambungan WebSocket.
/// Heartbeat menandai keterjangkauan dan menyegarkan metadata.
const JARAK_HEARTBEAT: Duration = Duration::from_secs(60);

/// Ambang penyegaran token, dihitung mundur dari kedaluwarsanya.
const AMBANG_SEGAR: i64 = 120;

struct Token {
    nilai: String,
    diperoleh: Instant,
    berlaku_detik: i64,
}

impl Token {
    fn perlu_disegarkan(&self) -> bool {
        self.diperoleh.elapsed().as_secs() as i64 >= self.berlaku_detik - AMBANG_SEGAR
    }
}

async fn ambil_token(klien: &Klien, konfig: &Konfigurasi, kunci: &DeviceKeypair) -> Result<Token> {
    let t = klien
        .token_perangkat(konfig.device_uuid, kunci)
        .await
        .context("gagal memperoleh token perangkat")?;
    Ok(Token {
        nilai: t.access_token,
        diperoleh: Instant::now(),
        berlaku_detik: t.expires_in,
    })
}

/// Menjalankan agent sampai dihentikan.
pub async fn jalankan(
    konfig: Konfigurasi,
    kunci: DeviceKeypair,
    atur: rtc::Pengaturan,
    penjaga: crate::persetujuan::Penjaga,
) -> Result<()> {
    let klien = Klien::baru(konfig.api_base())?;
    let mut jeda = BACKOFF_AWAL;

    loop {
        match satu_sesi(&klien, &konfig, &kunci, &atur, &penjaga).await {
            Ok(()) => {
                tracing::warn!("koneksi signaling tertutup, menyambung ulang");
                // Koneksi yang sempat berdiri berarti servernya sehat; jangan
                // menghukum sambungan berikutnya dengan jeda yang sudah
                // membengkak dari kegagalan lama.
                jeda = BACKOFF_AWAL;
            }
            Err(e) => tracing::error!(error = %format!("{e:#}"), "sesi signaling gagal"),
        }

        tracing::info!(detik = jeda.as_secs(), "menunggu sebelum mencoba lagi");
        tokio::time::sleep(jeda).await;
        jeda = (jeda * 2).min(BACKOFF_MAKS);
    }
}

/// Sesi media yang sedang berjalan, bila ada.
struct Aktif {
    session_id: Uuid,
    media: rtc::SesiMedia,
}

async fn satu_sesi(
    klien: &Klien,
    konfig: &Konfigurasi,
    kunci: &DeviceKeypair,
    atur: &rtc::Pengaturan,
    penjaga: &crate::persetujuan::Penjaga,
) -> Result<()> {
    let mut token = ambil_token(klien, konfig, kunci).await?;

    let url = konfig.ws_url();
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("gagal menyambung ke {url}"))?;
    tracing::info!(%url, "signaling tersambung");

    let (mut tulis, mut baca) = ws.split();

    // Kredensial TURN diambil sekarang, bukan saat sesi pertama diminta.
    // Relay yang tidak terkonfigurasi adalah kesalahan penyiapan, dan
    // menemukannya saat agent dinyalakan jauh lebih baik daripada menemukannya
    // ketika seseorang sedang menunggu layar muncul.
    let ice = ambil_ice(klien, &token.nilai).await;

    // Satu antrean keluar untuk semuanya. Kandidat ICE ditemukan di dalam
    // callback WebRTC yang tidak memegang socket, jadi ia perlu jalan pulang
    // yang tidak melibatkan kunci bersama.
    let (tx_keluar, mut rx_keluar) = mpsc::unbounded_channel::<String>();

    // AUTH wajib menjadi pesan pertama. `device_uuid` tetap dikirim meskipun
    // token sudah memuatnya — server memeriksa keduanya cocok, dan
    // ketidakcocokan berarti konfigurasi lokal sudah menyimpang dari identitas
    // yang sebenarnya dipegang.
    tulis
        .send(Message::Text(
            json!({
                "type": "AUTH",
                "payload": { "token": token.nilai, "device_uuid": konfig.device_uuid },
            })
            .to_string()
            .into(),
        ))
        .await
        .context("gagal mengirim AUTH")?;

    let mut detak = tokio::time::interval(JARAK_HEARTBEAT);
    detak.tick().await; // tick pertama selesai seketika

    let mut aktif: Option<Aktif> = None;

    loop {
        tokio::select! {
            pesan = baca.next() => {
                let Some(pesan) = pesan else { break };
                match pesan.context("kesalahan membaca WebSocket")? {
                    Message::Text(t) => {
                        tangani(&t, konfig, atur, &ice, penjaga, &tx_keluar, &mut aktif).await;
                    }
                    Message::Close(c) => {
                        tracing::warn!(?c, "server menutup koneksi");
                        break;
                    }
                    Message::Ping(p) => tulis.send(Message::Pong(p)).await?,
                    _ => {}
                }
            }

            Some(keluar) = rx_keluar.recv() => {
                tulis.send(Message::Text(keluar.into())).await?;
            }

            _ = detak.tick() => {
                if token.perlu_disegarkan() {
                    token = ambil_token(klien, konfig, kunci).await?;
                    tracing::debug!("token perangkat disegarkan");
                }
                if let Err(e) = klien
                    .heartbeat(&token.nilai, crate::api::hostname().as_deref())
                    .await
                {
                    // Heartbeat yang gagal bukan alasan memutus signaling.
                    // Keduanya jalur terpisah, dan yang menentukan kehadiran
                    // perangkat justru signaling yang masih hidup.
                    tracing::warn!(error = %e, "heartbeat gagal");
                }
            }
        }
    }

    if let Some(a) = aktif {
        a.media.tutup().await;
    }
    Ok(())
}

/// Menangani satu pesan dari server.
async fn tangani(
    teks: &str,
    konfig: &Konfigurasi,
    atur: &rtc::Pengaturan,
    ice: &[RTCIceServer],
    penjaga: &crate::persetujuan::Penjaga,
    tx: &mpsc::UnboundedSender<String>,
    aktif: &mut Option<Aktif>,
) {
    let Ok(pesan) = serde_json::from_str::<Value>(teks) else {
        tracing::debug!("pesan bukan JSON");
        return;
    };
    let Some(tipe) = pesan.get("type").and_then(|v| v.as_str()) else {
        return;
    };
    let payload = pesan.get("payload");

    match tipe {
        "AUTH_OK" => {
            tracing::info!(
                device_id = %konfig.device_id,
                "terautentikasi sebagai agent — perangkat kini online"
            );
        }

        "PING" => {
            let _ = tx.send(json!({ "type": "PONG" }).to_string());
        }

        "SESSION_OFFER" => {
            let Some(sid) = ambil_uuid(payload, "session_id") else {
                tracing::warn!("SESSION_OFFER tanpa session_id");
                return;
            };
            let peminta = crate::persetujuan::Peminta {
                // Tanpa identitas yang stabil, tidak ada yang dapat diingat.
                // Server selalu mengirimkannya; ketiadaannya berarti server
                // lebih tua daripada agent ini, dan menebak siapa pemintanya
                // bukan pilihan yang sah.
                user_id: match ambil_uuid(payload, "viewer_user_id") {
                    Some(u) => u,
                    None => {
                        tracing::error!("SESSION_OFFER tanpa viewer_user_id — permintaan ditolak");
                        let _ = tx.send(tolak(sid, "server tidak mengirim identitas peminta"));
                        return;
                    }
                },
                nama: medan_teks(payload, "viewer_name")
                    .unwrap_or_else(|| "tidak diketahui".into()),
                email: medan_teks(payload, "viewer_email").unwrap_or_default(),
                ip: medan_teks(payload, "viewer_ip").unwrap_or_default(),
            };

            // Persetujuan diminta **sebelum** capture disiapkan. Membuka
            // Desktop Duplication lebih dulu berarti mesin mulai menangkap
            // layarnya untuk permintaan yang mungkin ditolak.
            if !penjaga.putuskan(&peminta).await {
                tracing::info!(%sid, peminta = %peminta.email, "permintaan sesi ditolak");
                let _ = tx.send(tolak(sid, "permintaan tidak disetujui di perangkat tujuan"));
                return;
            }

            // Satu sesi pada satu waktu. Sesi kedua akan berebut capture yang
            // sama, dan Desktop Duplication hanya boleh dipegang satu pihak.
            if let Some(lama) = aktif.take() {
                tracing::info!(sesi_lama = %lama.session_id, "sesi lama ditutup");
                lama.media.tutup().await;
            }

            match rtc::SesiMedia::mulai(ice.to_vec(), atur.clone(), tx.clone(), sid).await {
                Ok((media, sdp)) => {
                    tracing::info!(%sid, peminta = %peminta.email, "sesi diterima, offer dikirim");
                    let _ = tx.send(
                        json!({ "type": "SESSION_ACCEPT", "payload": { "session_id": sid } })
                            .to_string(),
                    );
                    let _ = tx.send(
                        json!({
                            "type": "SDP_OFFER",
                            "payload": { "session_id": sid, "sdp": sdp },
                        })
                        .to_string(),
                    );
                    *aktif = Some(Aktif { session_id: sid, media });
                }
                Err(e) => {
                    tracing::error!(%sid, error = %format!("{e:#}"), "gagal memulai media");
                    let _ = tx.send(tolak(sid, &format!("agent gagal memulai capture: {e}")));
                }
            }
        }

        "SDP_ANSWER" => {
            let Some(sdp) = payload.and_then(|p| p.get("sdp")).and_then(|v| v.as_str()) else {
                return;
            };
            match aktif.as_ref() {
                Some(a) if cocok(payload, a.session_id) => {
                    if let Err(e) = a.media.jawaban(sdp).await {
                        tracing::error!(error = %format!("{e:#}"), "SDP answer ditolak");
                    } else {
                        tracing::info!(sid = %a.session_id, "SDP answer diterima");
                    }
                }
                _ => tracing::warn!("SDP answer untuk sesi yang tidak aktif"),
            }
        }

        "ICE_CANDIDATE" => {
            let Some(k) = payload.and_then(|p| p.get("candidate")) else {
                return;
            };
            match aktif.as_ref() {
                Some(a) if cocok(payload, a.session_id) => {
                    if let Err(e) = a.media.kandidat(k).await {
                        tracing::debug!(error = %e, "kandidat ICE ditolak");
                    }
                }
                _ => tracing::debug!("kandidat ICE untuk sesi yang tidak aktif"),
            }
        }

        "SESSION_END" => {
            if let Some(a) = aktif.take() {
                tracing::info!(sid = %a.session_id, "sesi diakhiri");
                a.media.tutup().await;
            }
        }

        "ERROR" => {
            let kode = payload
                .and_then(|p| p.get("code"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let pesan_galat = payload
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            tracing::error!(kode, pesan = pesan_galat, "server menolak");
        }

        lain => tracing::debug!(tipe = lain, "pesan tidak ditangani"),
    }
}

/// Mengambil daftar server ICE, dengan STUN publik sebagai jaring terakhir.
async fn ambil_ice(klien: &Klien, token: &str) -> Vec<RTCIceServer> {
    match klien.turn_credentials(token).await {
        Ok(daftar) => {
            let ice = ubah_ice(daftar);
            let relay = ice.iter().any(|s| s.urls.iter().any(|u| u.starts_with("turn:")));
            if relay {
                tracing::info!(server = ice.len(), "kredensial TURN diperoleh, relay tersedia");
            } else {
                // Jaringan di belakang Symmetric NAT — secara industri 10–20%
                // kasus — tidak akan pernah tersambung tanpa relay. Pantas
                // terlihat sejak sekarang, bukan sebagai kegagalan misterius
                // pada sesi pertama.
                tracing::warn!("server ICE tidak memuat relay TURN; NAT ketat akan gagal");
            }
            ice
        }
        Err(e) => {
            tracing::warn!(error = %e, "kredensial TURN tidak diperoleh, hanya STUN publik");
            vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }]
        }
    }
}

fn ubah_ice(daftar: Vec<crate::api::IceServer>) -> Vec<RTCIceServer> {
    daftar
        .into_iter()
        .map(|s| RTCIceServer {
            urls: s.urls,
            username: s.username.unwrap_or_default(),
            credential: s.credential.unwrap_or_default(),
        })
        .collect()
}

/// Pesan penolakan sesi, dengan alasan yang tertulis.
///
/// Viewer yang menunggu tanpa jawaban jauh lebih membingungkan daripada
/// penolakan yang menyebut sebabnya.
fn tolak(sid: Uuid, alasan: &str) -> String {
    json!({
        "type": "SESSION_REJECT",
        "payload": { "session_id": sid, "reason": alasan },
    })
    .to_string()
}

fn medan_teks(payload: Option<&Value>, kunci: &str) -> Option<String> {
    payload
        .and_then(|p| p.get(kunci))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn ambil_uuid(payload: Option<&Value>, kunci: &str) -> Option<Uuid> {
    payload
        .and_then(|p| p.get(kunci))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Apakah pesan ini memang untuk sesi yang sedang aktif.
///
/// Server sudah memeriksa keanggotaan sesi, tetapi memeriksanya lagi di sini
/// murah dan menutup kelas bug yang berbeda: pesan yang tiba terlambat dari
/// sesi sebelumnya, yang akan merusak sesi yang baru saja dimulai.
fn cocok(payload: Option<&Value>, sid: Uuid) -> bool {
    ambil_uuid(payload, "session_id") == Some(sid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_disegarkan_sebelum_kedaluwarsa() {
        let baru = Token {
            nilai: String::new(),
            diperoleh: Instant::now(),
            berlaku_detik: 900,
        };
        assert!(!baru.perlu_disegarkan());

        // Token yang umurnya lebih pendek daripada ambang harus langsung
        // dianggap perlu disegarkan, bukan dipakai sampai gagal.
        let pendek = Token {
            nilai: String::new(),
            diperoleh: Instant::now(),
            berlaku_detik: AMBANG_SEGAR - 1,
        };
        assert!(pendek.perlu_disegarkan());
    }

    #[test]
    fn backoff_terbatas() {
        let mut j = BACKOFF_AWAL;
        for _ in 0..20 {
            j = (j * 2).min(BACKOFF_MAKS);
        }
        assert_eq!(j, BACKOFF_MAKS, "backoff tumbuh tanpa batas");
    }

    #[test]
    fn session_id_terbaca_dari_payload() {
        let p = json!({ "session_id": "11111111-1111-1111-1111-111111111111" });
        assert_eq!(
            ambil_uuid(Some(&p), "session_id").map(|u| u.to_string()),
            Some("11111111-1111-1111-1111-111111111111".to_string())
        );
    }

    #[test]
    fn session_id_cacat_tidak_menjatuhkan_agent() {
        for buruk in [json!({}), json!({ "session_id": 5 }), json!({ "session_id": "bukan-uuid" })]
        {
            assert_eq!(ambil_uuid(Some(&buruk), "session_id"), None);
        }
        assert_eq!(ambil_uuid(None, "session_id"), None);
    }

    #[test]
    fn pesan_dari_sesi_lama_ditolak() {
        // Jawaban atau kandidat yang tiba terlambat dari sesi sebelumnya tidak
        // boleh merusak sesi yang baru dimulai.
        let sekarang = Uuid::new_v4();
        let lama = json!({ "session_id": Uuid::new_v4().to_string() });
        assert!(!cocok(Some(&lama), sekarang));

        let benar = json!({ "session_id": sekarang.to_string() });
        assert!(cocok(Some(&benar), sekarang));
    }

    #[test]
    fn kredensial_ice_dipetakan_tanpa_option() {
        // webrtc-rs memakai String kosong, bukan Option, untuk server tanpa
        // kredensial. Memetakan None menjadi "null" akan membuat STUN publik
        // ditolak sebagai kredensial tidak sah.
        let daftar = vec![
            crate::api::IceServer {
                urls: vec!["stun:a".into()],
                username: None,
                credential: None,
            },
            crate::api::IceServer {
                urls: vec!["turn:b".into()],
                username: Some("u".into()),
                credential: Some("p".into()),
            },
        ];
        let hasil = ubah_ice(daftar);
        assert_eq!(hasil[0].username, "");
        assert_eq!(hasil[1].username, "u");
        assert_eq!(hasil[1].credential, "p");
    }
}
