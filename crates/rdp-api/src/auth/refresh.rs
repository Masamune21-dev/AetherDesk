//! Refresh token.
//!
//! ARCHITECTURE.md §6.2 menjanjikan access token 15 menit dengan refresh token
//! 7 hari, tetapi hanya bagian pertama yang sempat diimplementasikan. Akibatnya
//! terasa langsung pada pemakaian nyata: setiap 15 menit seluruh pemanggilan
//! API mulai menjawab 401, dan agent yang seharusnya berbagi layar berjam-jam
//! justru terputus di tengah jalan.
//!
//! Token disimpan di Redis dalam bentuk hash SHA-256, bukan apa adanya. Token
//! ini berentropi tinggi dan dibangkitkan CSPRNG, jadi SHA-256 sudah memadai —
//! Argon2 dirancang untuk melawan tebakan pada rahasia berentropi rendah, dan
//! tidak memberi manfaat tambahan di sini selain biaya.

use crate::error::{ApiError, ApiResult};
use rand::RngCore;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Masa berlaku refresh token — ARCHITECTURE.md §6.2.
pub const REFRESH_TTL_SECONDS: u64 = 604_800;

/// Panjang token dalam byte sebelum dikodekan.
const PANJANG_BYTE: usize = 32;

#[derive(Debug, Serialize, Deserialize)]
pub struct SesiRefresh {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub email: String,
}

/// Membangkitkan refresh token baru dari CSPRNG.
pub fn buat() -> String {
    let mut b = [0u8; PANJANG_BYTE];
    rand::thread_rng().fill_bytes(&mut b);
    hex(&b)
}

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

/// Kunci Redis untuk sebuah token. Token tidak pernah disimpan apa adanya.
fn kunci(token: &str) -> String {
    let d = Sha256::digest(token.as_bytes());
    format!("refresh:{}", hex(&d))
}

pub async fn simpan(
    redis: &mut redis::aio::ConnectionManager,
    token: &str,
    sesi: &SesiRefresh,
) -> ApiResult<()> {
    let nilai = serde_json::to_string(sesi)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("serialisasi sesi gagal: {e}")))?;
    let _: () = redis.set_ex(kunci(token), nilai, REFRESH_TTL_SECONDS).await?;
    Ok(())
}

/// Menukar refresh token dengan sesinya, sekaligus **menghapus** token itu.
///
/// Rotasi sekali pakai. Selain membatasi jendela penyalahgunaan token yang
/// bocor, ia juga membuat pemakaian ulang terdeteksi: token yang sudah dirotasi
/// tidak akan ditemukan lagi, dan kegagalan itu adalah sinyal bahwa ada salinan
/// yang beredar di tempat lain.
pub async fn tukar(
    redis: &mut redis::aio::ConnectionManager,
    token: &str,
) -> ApiResult<SesiRefresh> {
    let k = kunci(token);

    let nilai: Option<String> = redis.get(&k).await?;
    let Some(nilai) = nilai else {
        tracing::info!("refresh token tidak dikenal, kedaluwarsa, atau sudah dipakai");
        return Err(ApiError::TidakTerautentikasi);
    };

    // Dihapus sebelum token baru diterbitkan: bila proses gagal setelah titik
    // ini, pengguna cukup masuk ulang — jauh lebih baik daripada meninggalkan
    // token lama yang masih dapat dipakai.
    let _: () = redis.del(&k).await?;

    serde_json::from_str(&nilai)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("sesi refresh rusak: {e}")))
}

/// Mencabut satu refresh token, dipakai saat logout.
pub async fn cabut(redis: &mut redis::aio::ConnectionManager, token: &str) -> ApiResult<()> {
    let _: () = redis.del(kunci(token)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn token_cukup_panjang_dan_heksadesimal() {
        let t = buat();
        assert_eq!(t.len(), PANJANG_BYTE * 2);
        assert!(t.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn token_tidak_berulang() {
        let set: HashSet<String> = (0..500).map(|_| buat()).collect();
        assert_eq!(set.len(), 500, "ada tabrakan token");
    }

    #[test]
    fn kunci_tidak_memuat_token_asli() {
        // Kalau token bocor lewat dump Redis, seluruh gunanya hilang.
        let t = buat();
        let k = kunci(&t);
        assert!(!k.contains(&t), "token tersimpan apa adanya di kunci");
        assert!(k.starts_with("refresh:"));
    }

    #[test]
    fn kunci_deterministik() {
        let t = buat();
        assert_eq!(kunci(&t), kunci(&t));
    }

    #[test]
    fn token_berbeda_menghasilkan_kunci_berbeda() {
        assert_ne!(kunci(&buat()), kunci(&buat()));
    }

    #[test]
    fn ttl_sesuai_arsitektur() {
        assert_eq!(REFRESH_TTL_SECONDS, 604_800, "ARCHITECTURE.md §6.2: 7 hari");
    }
}
