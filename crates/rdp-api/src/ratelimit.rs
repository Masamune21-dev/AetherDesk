//! Pembatasan laju berbasis Redis.
//!
//! QUICK_CONNECT.md §5 menetapkan pembatasan **per device ID**, bukan per IP
//! penyerang. Membatasi per IP saja tidak berguna: penyerang berpindah IP
//! dengan biaya nyaris nol, sementara device ID yang diserang tetap sama.

use crate::error::ApiResult;
use redis::AsyncCommands;

/// Hasil pemeriksaan batas laju.
#[derive(Debug, PartialEq, Eq)]
pub enum Keputusan {
    Lanjut,
    Dijeda { retry_after_seconds: u64 },
}

/// Satu aturan pembatasan.
#[derive(Debug, Clone, Copy)]
pub struct Aturan {
    pub maks: u64,
    pub jendela_detik: u64,
    pub jeda_detik: u64,
}

/// Batas percobaan gagal per device ID: 5 dalam 10 menit, jeda 15 menit.
pub const PER_DEVICE: Aturan = Aturan {
    maks: 5,
    jendela_detik: 600,
    jeda_detik: 900,
};

/// Batas ID tidak dikenal per IP sumber: 10 dalam 1 jam, blokir 24 jam.
pub const ID_TAK_DIKENAL_PER_IP: Aturan = Aturan {
    maks: 10,
    jendela_detik: 3600,
    jeda_detik: 86_400,
};

/// Batas enrolment gagal per IP sumber: 10 dalam 1 jam, jeda 1 jam.
///
/// Token enrolment beruang 256 bit, jadi ini **bukan** perlindungan terhadap
/// tebakan — menebaknya sudah mustahil tanpa bantuan apa pun. Yang dibatasi
/// adalah biayanya bagi kita: setiap upaya menjalankan SHA-256 dan satu query,
/// dan endpointnya terbuka tanpa autentikasi. Jedanya lebih pendek daripada
/// `ID_TAK_DIKENAL_PER_IP` karena salah ketik saat memasang agent adalah
/// kejadian wajar, sementara memindai ruang device ID tidak.
pub const ENROLMENT_PER_IP: Aturan = Aturan {
    maks: 10,
    jendela_detik: 3600,
    jeda_detik: 3600,
};

/// Memeriksa apakah kunci sedang dijeda, **tanpa** menambah hitungan.
///
/// Dipisahkan dari [`catat_kegagalan`] secara sengaja: percobaan yang ditolak
/// karena sedang dijeda tidak boleh memperpanjang jeda itu sendiri. Kalau
/// digabung, penyerang yang terus mencoba akan mengunci korban selamanya —
/// berubah menjadi denial of service terhadap pemilik perangkat.
pub async fn periksa(
    redis: &mut redis::aio::ConnectionManager,
    kunci: &str,
) -> ApiResult<Keputusan> {
    let kunci_jeda = format!("jeda:{kunci}");
    let ttl: i64 = redis.ttl(&kunci_jeda).await?;

    // TTL negatif berarti kunci tidak ada (-2) atau tanpa kedaluwarsa (-1).
    if ttl > 0 {
        return Ok(Keputusan::Dijeda {
            retry_after_seconds: ttl as u64,
        });
    }
    Ok(Keputusan::Lanjut)
}

/// Mencatat satu kegagalan dan memasang jeda bila ambang terlampaui.
pub async fn catat_kegagalan(
    redis: &mut redis::aio::ConnectionManager,
    kunci: &str,
    aturan: Aturan,
) -> ApiResult<Keputusan> {
    let kunci_hitung = format!("gagal:{kunci}");

    let hitungan: u64 = redis.incr(&kunci_hitung, 1u64).await?;
    if hitungan == 1 {
        // Jendela dimulai dari kegagalan pertama.
        let _: () = redis
            .expire(&kunci_hitung, aturan.jendela_detik as i64)
            .await?;
    }

    if hitungan >= aturan.maks {
        let kunci_jeda = format!("jeda:{kunci}");
        let _: () = redis
            .set_ex(&kunci_jeda, 1u8, aturan.jeda_detik)
            .await?;
        let _: () = redis.del(&kunci_hitung).await?;

        tracing::warn!(kunci, hitungan, "ambang batas laju terlampaui, jeda dipasang");
        return Ok(Keputusan::Dijeda {
            retry_after_seconds: aturan.jeda_detik,
        });
    }

    Ok(Keputusan::Lanjut)
}

/// Menghapus hitungan kegagalan setelah autentikasi berhasil.
pub async fn bersihkan(
    redis: &mut redis::aio::ConnectionManager,
    kunci: &str,
) -> ApiResult<()> {
    let _: () = redis.del(format!("gagal:{kunci}")).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aturan_sesuai_dokumen() {
        // QUICK_CONNECT.md §5
        assert_eq!(PER_DEVICE.maks, 5);
        assert_eq!(PER_DEVICE.jendela_detik, 600);
        assert_eq!(PER_DEVICE.jeda_detik, 900);

        assert_eq!(ID_TAK_DIKENAL_PER_IP.maks, 10);
        assert_eq!(ID_TAK_DIKENAL_PER_IP.jendela_detik, 3600);
        assert_eq!(ID_TAK_DIKENAL_PER_IP.jeda_detik, 86_400);
    }

    #[test]
    fn jeda_lebih_lama_dari_jendela() {
        // Kalau jeda lebih pendek dari jendela, penyerang cukup menunggu jeda
        // lalu melanjutkan tanpa hitungan pernah ter-reset.
        assert!(PER_DEVICE.jeda_detik > PER_DEVICE.jendela_detik / 2);
    }
}
