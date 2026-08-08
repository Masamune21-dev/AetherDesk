//! Pesan signaling.
//!
//! Bentuk amplop mengikuti API.md §9. Payload memakai `serde` internally-tagged
//! sehingga satu enum mencakup seluruh tipe pesan sekaligus menjadi dokumentasi
//! protokol yang tidak bisa basi — menambah varian tanpa menanganinya akan
//! ditolak compiler.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Pesan dari klien ke server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Masuk {
    /// Autentikasi. Wajib menjadi pesan pertama.
    Auth {
        token: String,
        /// Diisi bila koneksi ini melayani sebuah perangkat (peran agent).
        /// Kosong berarti peran viewer.
        device_uuid: Option<Uuid>,
    },
    /// Viewer meminta koneksi ke perangkat.
    SessionRequest { session_id: Uuid, device_uuid: Uuid },
    /// Agent menerima permintaan.
    SessionAccept { session_id: Uuid },
    /// Agent menolak permintaan.
    SessionReject { session_id: Uuid, reason: Option<String> },
    /// Pertukaran SDP. Diteruskan apa adanya — server tidak pernah membacanya.
    SdpOffer { session_id: Uuid, sdp: String },
    SdpAnswer { session_id: Uuid, sdp: String },
    /// Kandidat ICE.
    IceCandidate {
        session_id: Uuid,
        candidate: serde_json::Value,
    },
    SessionEnd { session_id: Uuid },
    Pong,
}

/// Pesan dari server ke klien.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Keluar {
    AuthOk {
        role: &'static str,
        user_id: Uuid,
        org_id: Uuid,
    },
    /// Diteruskan ke agent saat ada viewer meminta koneksi.
    SessionOffer {
        session_id: Uuid,
        viewer_name: String,
        viewer_email: String,
        /// Ditampilkan pada prompt persetujuan; QUICK_CONNECT.md §4.1
        /// mensyaratkan pengguna tahu siapa yang meminta dan dari mana.
        viewer_ip: String,
    },
    SessionAccepted { session_id: Uuid },
    SessionRejected { session_id: Uuid, reason: Option<String> },
    SdpOffer { session_id: Uuid, sdp: String },
    SdpAnswer { session_id: Uuid, sdp: String },
    IceCandidate {
        session_id: Uuid,
        candidate: serde_json::Value,
    },
    SessionEnd { session_id: Uuid },
    DeviceStatus { device_uuid: Uuid, status: String },
    Ping,
    Error { code: String, message: String },
}

impl Keluar {
    pub fn error(code: &str, message: &str) -> Self {
        Self::Error {
            code: code.to_string(),
            message: message.to_string(),
        }
    }

    pub fn ke_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            tracing::error!(error = %e, "gagal menserialisasi pesan keluar");
            r#"{"type":"ERROR","payload":{"code":"INTERNAL","message":"serialisasi gagal"}}"#
                .to_string()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amplop_masuk_sesuai_api_md() {
        let j = r#"{"type":"AUTH","payload":{"token":"abc","device_uuid":null}}"#;
        let m: Masuk = serde_json::from_str(j).unwrap();
        assert!(matches!(m, Masuk::Auth { .. }));
    }

    #[test]
    fn tipe_pesan_screaming_snake_case() {
        let j = Keluar::AuthOk {
            role: "viewer",
            user_id: Uuid::nil(),
            org_id: Uuid::nil(),
        }
        .ke_json();
        assert!(j.contains(r#""type":"AUTH_OK""#), "{j}");
    }

    #[test]
    fn sdp_diteruskan_apa_adanya() {
        // Server tidak boleh menormalkan, memformat ulang, atau memvalidasi
        // isi SDP — tanda tangan device key (ADR-008) dihitung atas byte
        // aslinya, jadi perubahan sekecil apa pun akan membatalkannya.
        let asli = "v=0\r\no=- 123 2 IN IP4 127.0.0.1\r\n";
        let pesan = Keluar::SdpOffer {
            session_id: Uuid::nil(),
            sdp: asli.to_string(),
        };
        let j = pesan.ke_json();
        let kembali: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(kembali["payload"]["sdp"].as_str().unwrap(), asli);
    }

    #[test]
    fn pesan_tak_dikenal_ditolak() {
        let j = r#"{"type":"BUKAN_TIPE_APA_PUN","payload":{}}"#;
        assert!(serde_json::from_str::<Masuk>(j).is_err());
    }

    #[test]
    fn ke_json_tidak_pernah_panik() {
        let s = Keluar::error("X", "pesan").ke_json();
        assert!(s.contains("ERROR"));
    }
}
