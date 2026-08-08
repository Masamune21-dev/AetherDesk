//! Kehadiran perangkat.
//!
//! Menutup temuan **S-09**. ARCHITECTURE.md §8.4 mengandalkan TTL Redis 90
//! detik untuk menandai perangkat offline, sehingga mesin yang mati mendadak
//! tetap tampil online sampai satu setengah menit. Itu bertabrakan dengan
//! FR-DEV-06 yang menjanjikan status real-time, dan membuat teknisi mencoba
//! menghubungi mesin yang sudah tidak ada.
//!
//! Di sini transisi offline terjadi **langsung** saat WebSocket putus. TTL
//! tetap dipertahankan, tetapi turun perannya menjadi jaring pengaman untuk
//! kasus yang tidak menghasilkan event putus — proses dibunuh paksa, node
//! signal mati, atau jaringan hilang tanpa FIN.

use anyhow::Result;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

/// TTL jaring pengaman. Tetap 90 detik sesuai ARCHITECTURE.md §8.4.
const TTL_DETIK: u64 = 90;

fn kunci(device_uuid: Uuid) -> String {
    format!("device:{device_uuid}:online")
}

pub async fn tandai_online(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
    org_id: Uuid,
    device_uuid: Uuid,
) -> Result<()> {
    let _: () = redis.set_ex(kunci(device_uuid), 1u8, TTL_DETIK).await?;

    let mut tx = db.begin().await?;
    sqlx::query("SELECT set_config('aetherdesk.current_org', $1, true)")
        .bind(org_id.to_string())
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE devices SET status = 'online', last_heartbeat = now() WHERE id = $1")
        .bind(device_uuid)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    tracing::info!(%device_uuid, "perangkat online");
    Ok(())
}

pub async fn tandai_offline(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
    org_id: Uuid,
    device_uuid: Uuid,
) -> Result<()> {
    let _: () = redis.del(kunci(device_uuid)).await?;

    let mut tx = db.begin().await?;
    sqlx::query("SELECT set_config('aetherdesk.current_org', $1, true)")
        .bind(org_id.to_string())
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE devices SET status = 'offline' WHERE id = $1")
        .bind(device_uuid)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    tracing::info!(%device_uuid, "perangkat offline");
    Ok(())
}

/// Memperpanjang TTL selama koneksi masih hidup.
pub async fn perbarui(
    redis: &mut redis::aio::ConnectionManager,
    device_uuid: Uuid,
) -> Result<()> {
    let _: () = redis.expire(kunci(device_uuid), TTL_DETIK as i64).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_kunci_sesuai_arsitektur() {
        // ARCHITECTURE.md §10.3 menetapkan pola `device:{id}:online`.
        let id = Uuid::nil();
        assert_eq!(kunci(id), format!("device:{id}:online"));
    }

    #[test]
    fn ttl_masih_tiga_kali_interval_ping() {
        // Ping dikirim tiap 25 detik; TTL 90 detik memberi ruang tiga kali
        // gagal sebelum jaring pengaman bekerja.
        assert_eq!(TTL_DETIK, 90);
        assert!(TTL_DETIK >= 25 * 3);
    }
}
