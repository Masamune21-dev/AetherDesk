//! Klien HTTP ke API Server.
//!
//! Tiga panggilan saja, dan seluruhnya berhubungan dengan identitas: menukar
//! token enrolment, menukar tanda tangan dengan token perangkat, dan mengirim
//! heartbeat.

use anyhow::{bail, Context, Result};
use rdp_core::{device_key, DeviceKeypair};
use serde::Deserialize;
use uuid::Uuid;

/// Amplop respons standar (API.md §3).
#[derive(Debug, Deserialize)]
struct Amplop<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct AmplopGalat {
    error: Galat,
}

#[derive(Debug, Deserialize)]
struct Galat {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KredensialResp {
    ice_servers: Vec<IceServer>,
}

#[derive(Debug, Deserialize)]
pub struct EnrolResp {
    pub device_uuid: Uuid,
    pub device_id: String,
    pub device_id_tampil: String,
    pub session_password: String,
}

/// Respons token perangkat.
///
/// Server juga mengirim `org_id`, yang sengaja tidak diambil di sini: agent
/// tidak pernah memerlukannya, dan menyimpan tenant di sisi klien hanya
/// menciptakan salinan kedua yang dapat menyimpang dari isi token.
#[derive(Debug, Deserialize)]
pub struct TokenResp {
    pub access_token: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone)]
pub struct Klien {
    http: reqwest::Client,
    api_base: String,
}

impl Klien {
    pub fn baru(api_base: String) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .user_agent(concat!("rdp-agent/", env!("CARGO_PKG_VERSION")))
                .build()
                .context("gagal membangun klien HTTP")?,
            api_base,
        })
    }

    /// `POST /devices/enrol` — menukar token enrolment dengan pendaftaran.
    pub async fn enrol(
        &self,
        enrolment_token: &str,
        public_key: &[u8],
        alias: Option<&str>,
        hostname: Option<&str>,
    ) -> Result<EnrolResp> {
        let body = serde_json::json!({
            "enrolment_token": enrolment_token,
            "public_key": device_key::ke_base64(public_key),
            "os_type": os_type(),
            "hostname": hostname,
            "alias": alias,
        });

        self.kirim("/devices/enrol", &body, None).await
    }

    /// `POST /devices/token` — membuktikan diri dengan tanda tangan.
    ///
    /// Stempel waktu dan nonce dibangkitkan di sini, tepat sebelum
    /// ditandatangani. Membangkitkannya lebih awal lalu menyimpannya akan
    /// membuat request gagal karena stempelnya sudah basi begitu ada jeda tak
    /// terduga — dan gejalanya hanya "kredensial tidak valid".
    pub async fn token_perangkat(
        &self,
        device_uuid: Uuid,
        kunci: &DeviceKeypair,
    ) -> Result<TokenResp> {
        let timestamp = chrono::Utc::now().timestamp();
        let nonce = device_key::nonce_baru();
        let tantangan = device_key::tantangan(&device_uuid, timestamp, &nonce);
        let tanda_tangan = kunci.tanda_tangani(&tantangan);

        let body = serde_json::json!({
            "device_uuid": device_uuid,
            "timestamp": timestamp,
            "nonce": nonce,
            "signature": device_key::ke_base64(&tanda_tangan),
        });

        self.kirim("/devices/token", &body, None).await
    }

    /// `GET /turn-credentials`.
    ///
    /// Agent memerlukan relay sama seperti viewer. Tanpa ini, sesi akan gagal
    /// tepat pada jaringan yang paling membutuhkannya — di belakang Symmetric
    /// NAT, yang secara industri mencakup 10–20% kasus.
    pub async fn turn_credentials(&self, token: &str) -> Result<Vec<IceServer>> {
        let resp: KredensialResp = self.ambil("/turn-credentials", token).await?;
        Ok(resp.ice_servers)
    }

    /// `POST /devices/heartbeat`.
    pub async fn heartbeat(&self, token: &str, hostname: Option<&str>) -> Result<()> {
        let body = serde_json::json!({
            "hostname": hostname,
            "client_version": env!("CARGO_PKG_VERSION"),
        });
        let _: serde_json::Value = self.kirim("/devices/heartbeat", &body, Some(token)).await?;
        Ok(())
    }

    async fn kirim<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        token: Option<&str>,
    ) -> Result<T> {
        let mut req = self.http.post(format!("{}{path}", self.api_base)).json(body);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        self.jalankan(req, path).await
    }

    async fn ambil<T: serde::de::DeserializeOwned>(&self, path: &str, token: &str) -> Result<T> {
        let req = self
            .http
            .get(format!("{}{path}", self.api_base))
            .bearer_auth(token);
        self.jalankan(req, path).await
    }

    async fn jalankan<T: serde::de::DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
        path: &str,
    ) -> Result<T> {
        let resp = req
            .send()
            .await
            .with_context(|| format!("gagal menghubungi {}{path}", self.api_base))?;

        let status = resp.status();
        let teks = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            // Amplop galat API.md §3 lebih berguna daripada kode status
            // telanjang; bila badannya bukan bentuk itu, teks mentahnya tetap
            // lebih informatif daripada tidak sama sekali.
            if let Ok(g) = serde_json::from_str::<AmplopGalat>(&teks) {
                bail!("{} ({}): {}", g.error.code, status.as_u16(), g.error.message);
            }
            bail!("HTTP {} dari {path}: {}", status.as_u16(), potong(&teks, 200));
        }

        let amplop: Amplop<T> = serde_json::from_str(&teks)
            .with_context(|| format!("respons {path} bukan amplop yang dikenal"))?;
        Ok(amplop.data)
    }
}

/// `os_type` sesuai daftar yang diterima API.
pub fn os_type() -> &'static str {
    if cfg!(windows) {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    }
}

/// Nama host mesin.
///
/// Dibaca dari lingkungan alih-alih memanggil API sistem. Nilainya hanya
/// metadata tampilan di dashboard — tidak ada keputusan keamanan yang
/// bergantung padanya, jadi menambah dependensi demi ketepatan yang lebih
/// tinggi tidak sepadan.
pub fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.is_empty())
}

fn potong(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_type_termasuk_daftar_yang_diterima_api() {
        assert!(matches!(os_type(), "Windows" | "macOS" | "Linux"));
    }

    #[test]
    fn amplop_sukses_terbaca() {
        // `org_id` sengaja ikut di sini: respons server memang memuatnya, dan
        // klien harus tetap membacanya dengan benar meski tidak memakainya.
        let j = r#"{"data":{"access_token":"t","expires_in":900,
                    "org_id":"00000000-0000-0000-0000-000000000000"}}"#;
        let a: Amplop<TokenResp> = serde_json::from_str(j).unwrap();
        assert_eq!(a.data.access_token, "t");
        assert_eq!(a.data.expires_in, 900);
    }

    #[test]
    fn amplop_galat_terbaca() {
        let j = r#"{"error":{"code":"UNAUTHENTICATED","message":"tidak terautentikasi"}}"#;
        let g: AmplopGalat = serde_json::from_str(j).unwrap();
        assert_eq!(g.error.code, "UNAUTHENTICATED");
    }

    #[test]
    fn potong_aman_untuk_multibyte() {
        assert_eq!(potong("abc", 10), "abc");
        assert!(potong(&"モ".repeat(500), 20).chars().count() <= 21);
    }
}
