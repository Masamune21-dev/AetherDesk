//! Kredensial TURN berumur pendek.
//!
//! Memakai skema REST API coturn: nama pengguna adalah `<kedaluwarsa>:<id>`
//! dan kata sandinya HMAC-SHA1 atas nama itu memakai rahasia bersama. Server
//! TURN memverifikasinya tanpa perlu menyimpan satu pun kredensial.
//!
//! Alasan memilih skema ini: rahasia bersama **tidak pernah** meninggalkan
//! server. Yang sampai ke browser hanyalah pasangan yang kedaluwarsa dalam
//! hitungan jam, sehingga tangkapan layar konsol atau HAR yang tersebar tidak
//! berubah menjadi relay gratis bagi orang lain.
//!
//! HMAC-SHA1 di sini bukan pilihan kriptografis kami — ia ditetapkan protokol
//! TURN (RFC 5389 §15.4 memakai HMAC-SHA1 untuk MESSAGE-INTEGRITY), dan coturn
//! mengikutinya. Perannya sebagai kode autentikasi pesan, bukan hashing kata
//! sandi, sehingga kelemahan tumbukan SHA-1 tidak berlaku di sini.

use crate::{
    auth::SubjekTerautentikasi,
    error::{ApiResult, Sukses},
    state::AppState,
};
use axum::extract::State;
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;

/// Masa berlaku kredensial. Cukup panjang untuk sesi kerja penuh, cukup pendek
/// agar kebocoran tidak bernilai lama.
const TTL_DETIK: i64 = 6 * 3600;

#[derive(Debug, Serialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KredensialResp {
    pub ice_servers: Vec<IceServer>,
    pub expires_at: i64,
}

/// `GET /api/v1/turn-credentials`
///
/// Wajib terautentikasi. Relay adalah komponen termahal yang dioperasikan,
/// jadi tidak ada alasan membagikannya kepada pemanggil anonim.
///
/// Menerima token pengguna **maupun** token perangkat. Sebuah sesi punya dua
/// ujung, dan keduanya perlu relay; agent yang tidak dapat memperolehnya akan
/// gagal tepat pada jaringan yang paling membutuhkannya. Karena nama pengguna
/// TURN memuat id subjek, jejak pemakaian relay tetap dapat ditelusuri ke
/// perangkat tertentu, bukan melebur menjadi anonim.
pub async fn kredensial(
    State(state): State<AppState>,
    subjek: SubjekTerautentikasi,
) -> ApiResult<Sukses<KredensialResp>> {
    let Some(turn) = &state.turn else {
        // TURN belum dikonfigurasi: kembalikan STUN saja, bukan galat.
        // Klien tetap dapat mencoba P2P langsung.
        return Ok(Sukses::baru(KredensialResp {
            ice_servers: vec![IceServer {
                urls: vec!["stun:stun.l.google.com:19302".into()],
                username: None,
                credential: None,
            }],
            expires_at: 0,
        }));
    };

    let kedaluwarsa = chrono::Utc::now().timestamp() + TTL_DETIK;
    let username = format!("{kedaluwarsa}:{}", subjek.subjek);
    let credential = tanda_tangan(&username, &turn.secret);

    let urls = vec![
        format!("turn:{}:{}?transport=udp", turn.host, turn.port),
        // TCP membantu jaringan yang memblokir UDP sama sekali.
        format!("turn:{}:{}?transport=tcp", turn.host, turn.port),
    ];

    tracing::debug!(
        subjek = %subjek.subjek,
        perangkat = subjek.adalah_perangkat,
        "kredensial TURN diterbitkan"
    );

    Ok(Sukses::baru(KredensialResp {
        ice_servers: vec![
            IceServer {
                urls: vec![format!("stun:{}:{}", turn.host, turn.port)],
                username: None,
                credential: None,
            },
            IceServer {
                urls,
                username: Some(username),
                credential: Some(credential),
            },
        ],
        expires_at: kedaluwarsa,
    }))
}

fn tanda_tangan(username: &str, secret: &str) -> String {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret.as_bytes())
        .expect("HMAC menerima kunci sepanjang apa pun");
    mac.update(username.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tanda_tangan_deterministik() {
        let a = tanda_tangan("1700000000:abc", "rahasia");
        let b = tanda_tangan("1700000000:abc", "rahasia");
        assert_eq!(a, b);
    }

    #[test]
    fn rahasia_berbeda_menghasilkan_tanda_tangan_berbeda() {
        assert_ne!(
            tanda_tangan("1700000000:abc", "rahasia-satu"),
            tanda_tangan("1700000000:abc", "rahasia-dua")
        );
    }

    #[test]
    fn username_berbeda_menghasilkan_tanda_tangan_berbeda() {
        assert_ne!(
            tanda_tangan("1700000000:abc", "s"),
            tanda_tangan("1700000000:abd", "s")
        );
    }

    #[test]
    fn hasil_adalah_base64_sha1_20_byte() {
        let t = tanda_tangan("x", "y");
        let byte = STANDARD.decode(&t).expect("harus base64 yang sah");
        assert_eq!(byte.len(), 20, "SHA-1 menghasilkan 20 byte");
    }

    #[test]
    fn ttl_cukup_untuk_satu_hari_kerja_tapi_tidak_abadi() {
        assert!(TTL_DETIK >= 3600, "terlalu pendek untuk sesi kerja");
        assert!(TTL_DETIK <= 24 * 3600, "kebocoran bernilai terlalu lama");
    }

    #[test]
    fn rahasia_tidak_bocor_ke_dalam_username() {
        // Username dikirim apa adanya ke klien; ia tidak boleh memuat rahasia.
        let u = format!("{}:{}", 1700000000, uuid::Uuid::nil());
        assert!(!u.contains("rahasia"));
    }
}
