//! Koneksi ke Signal Server.
//!
//! Menutup bagian terakhir M1: agent hadir di dashboard sebagai perangkat
//! online, dan Quick Connect dapat menemukannya.
//!
//! Yang **belum** ada adalah capture (M2). Karena itu permintaan sesi yang
//! masuk ditolak dengan alasan yang jelas, bukan didiamkan — viewer yang
//! menunggu tanpa jawaban jauh lebih membingungkan daripada penolakan yang
//! menyebutkan sebabnya.

use crate::{api::Klien, identitas::Konfigurasi};
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use rdp_core::DeviceKeypair;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

const BACKOFF_AWAL: Duration = Duration::from_secs(1);
const BACKOFF_MAKS: Duration = Duration::from_secs(30);

/// Jarak antar heartbeat.
///
/// Lebih rapat daripada yang dibutuhkan status kehadiran, karena kehadiran
/// bukan tugasnya — Signal Server yang memilikinya lewat sambungan WebSocket.
/// Heartbeat menandai keterjangkauan dan menyegarkan metadata.
const JARAK_HEARTBEAT: Duration = Duration::from_secs(60);

/// Ambang penyegaran token, dihitung mundur dari kedaluwarsanya.
///
/// Token diperbarui saat tersisa dua menit, bukan saat sudah kedaluwarsa.
/// Menunggu sampai gagal berarti setiap siklus token menghasilkan satu
/// heartbeat yang hilang.
const AMBANG_SEGAR: i64 = 120;

/// Token perangkat beserta umurnya.
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
pub async fn jalankan(konfig: Konfigurasi, kunci: DeviceKeypair) -> Result<()> {
    let klien = Klien::baru(konfig.api_base())?;
    let mut jeda = BACKOFF_AWAL;

    loop {
        match satu_sesi(&klien, &konfig, &kunci).await {
            Ok(()) => {
                tracing::warn!("koneksi signaling tertutup, menyambung ulang");
                // Koneksi yang sempat berdiri berarti servernya sehat; jangan
                // menghukum sambungan berikutnya dengan jeda yang sudah
                // membengkak dari kegagalan lama.
                jeda = BACKOFF_AWAL;
            }
            Err(e) => tracing::error!(error = %e, "sesi signaling gagal"),
        }

        tracing::info!(detik = jeda.as_secs(), "menunggu sebelum mencoba lagi");
        tokio::time::sleep(jeda).await;
        jeda = (jeda * 2).min(BACKOFF_MAKS);
    }
}

async fn satu_sesi(klien: &Klien, konfig: &Konfigurasi, kunci: &DeviceKeypair) -> Result<()> {
    let mut token = ambil_token(klien, konfig, kunci).await?;

    let url = konfig.ws_url();
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .with_context(|| format!("gagal menyambung ke {url}"))?;
    tracing::info!(%url, "signaling tersambung");

    let (mut tulis, mut baca) = ws.split();

    // AUTH wajib menjadi pesan pertama. `device_uuid` sengaja tetap dikirim
    // meskipun token sudah memuatnya — server memeriksa keduanya cocok, dan
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

    loop {
        tokio::select! {
            pesan = baca.next() => {
                let Some(pesan) = pesan else {
                    return Ok(()); // socket tertutup dari sisi server
                };
                match pesan.context("kesalahan membaca WebSocket")? {
                    Message::Text(t) => {
                        if let Some(balasan) = tangani(&t, konfig) {
                            tulis.send(Message::Text(balasan.into())).await?;
                        }
                    }
                    Message::Close(c) => {
                        tracing::warn!(?c, "server menutup koneksi");
                        return Ok(());
                    }
                    Message::Ping(p) => tulis.send(Message::Pong(p)).await?,
                    _ => {}
                }
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
}

/// Menangani satu pesan dari server. Mengembalikan balasan bila ada.
fn tangani(teks: &str, konfig: &Konfigurasi) -> Option<String> {
    let pesan: Value = serde_json::from_str(teks).ok()?;
    let tipe = pesan.get("type")?.as_str()?;
    let payload = pesan.get("payload");

    match tipe {
        "AUTH_OK" => {
            tracing::info!(
                device_id = %konfig.device_id,
                "terautentikasi sebagai agent — perangkat kini online"
            );
            None
        }

        "PING" => Some(json!({ "type": "PONG" }).to_string()),

        // M2 belum ada. Menolak dengan alasan yang tertulis jauh lebih baik
        // daripada mendiamkan viewer sampai kehabisan waktu.
        "SESSION_OFFER" => {
            let session_id = payload?.get("session_id")?.clone();
            let peminta = payload
                .and_then(|p| p.get("viewer_email"))
                .and_then(|v| v.as_str())
                .unwrap_or("tidak diketahui");
            tracing::warn!(%peminta, "permintaan sesi ditolak — capture belum ada (M2)");
            Some(
                json!({
                    "type": "SESSION_REJECT",
                    "payload": {
                        "session_id": session_id,
                        "reason": "agent native belum mendukung berbagi layar (M2)",
                    },
                })
                .to_string(),
            )
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
            None
        }

        lain => {
            tracing::debug!(tipe = lain, "pesan tidak ditangani");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn konfig() -> Konfigurasi {
        Konfigurasi {
            device_uuid: Uuid::nil(),
            device_id: "123456789".into(),
            server: "https://a.test".into(),
        }
    }

    #[test]
    fn ping_dibalas_pong() {
        let b = tangani(r#"{"type":"PING"}"#, &konfig()).unwrap();
        assert!(b.contains(r#""type":"PONG""#), "{b}");
    }

    #[test]
    fn session_offer_ditolak_dengan_alasan() {
        let masuk = r#"{"type":"SESSION_OFFER","payload":{
            "session_id":"11111111-1111-1111-1111-111111111111",
            "viewer_name":"A","viewer_email":"a@b.c","viewer_ip":"1.2.3.4"}}"#;
        let b = tangani(masuk, &konfig()).unwrap();
        assert!(b.contains("SESSION_REJECT"), "{b}");
        // session_id wajib ikut, kalau tidak server tidak tahu sesi mana yang
        // ditolak dan viewer tetap menunggu.
        assert!(b.contains("11111111-1111-1111-1111-111111111111"), "{b}");
        assert!(b.contains("M2"), "alasan penolakan tidak menyebutkan sebabnya: {b}");
    }

    #[test]
    fn auth_ok_tidak_perlu_balasan() {
        let m = r#"{"type":"AUTH_OK","payload":{"role":"agent",
                    "user_id":"00000000-0000-0000-0000-000000000000",
                    "org_id":"00000000-0000-0000-0000-000000000000"}}"#;
        assert!(tangani(m, &konfig()).is_none());
    }

    #[test]
    fn pesan_cacat_tidak_menjatuhkan_agent() {
        // Seluruh masukan ini datang dari jaringan.
        for buruk in [
            "",
            "bukan json",
            "{}",
            r#"{"type":123}"#,
            r#"{"type":"SESSION_OFFER"}"#,
            r#"{"type":"SESSION_OFFER","payload":{}}"#,
        ] {
            assert!(tangani(buruk, &konfig()).is_none(), "gagal pada: {buruk}");
        }
    }

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
}
