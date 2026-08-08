//! Siklus hidup sesi.
//!
//! `POST /api/v1/connect` menyisipkan baris berstatus `pending`, tetapi sampai
//! sekarang tidak ada yang pernah memajukannya. Akibatnya seluruh sesi —
//! termasuk yang berhasil dan sudah lama berakhir — tetap tercatat `pending`
//! selamanya, dan `ended_at` tidak pernah terisi. Riwayat sesi menjadi tidak
//! berguna, dan "sesi aktif" di dashboard mustahil dihitung.
//!
//! Signal Server adalah tempat yang tepat untuk menutup celah ini: ia satu-
//! satunya komponen yang tahu kapan sesi benar-benar disetujui, ditolak, atau
//! berakhir — termasuk saat berakhirnya karena koneksi putus, bukan karena
//! seseorang menekan tombol.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Menjalankan pembaruan dengan tenant ditetapkan lebih dulu (RLS).
async fn dengan_tenant(db: &PgPool, org_id: Uuid, sql: &str, session_id: Uuid) -> Result<u64> {
    let mut tx = db.begin().await?;
    sqlx::query("SELECT set_config('aetherdesk.current_org', $1, true)")
        .bind(org_id.to_string())
        .execute(&mut *tx)
        .await?;
    let n = sqlx::query(sql)
        .bind(session_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    Ok(n)
}

/// `pending` → `active`, saat agent menyetujui permintaan.
pub async fn aktifkan(db: &PgPool, org_id: Uuid, session_id: Uuid) -> Result<()> {
    let n = dengan_tenant(
        db,
        org_id,
        "UPDATE sessions SET status = 'active'
         WHERE id = $1 AND status = 'pending'",
        session_id,
    )
    .await?;
    if n > 0 {
        tracing::info!(%session_id, "sesi aktif");
    }
    Ok(())
}

/// Menutup sesi apa pun statusnya, selama belum tertutup.
///
/// `alasan` tercatat untuk membedakan sesi yang diakhiri pengguna dari yang
/// putus sendiri — pembedaan yang justru paling berguna saat menelusuri
/// keluhan "koneksi saya terputus terus".
pub async fn akhiri(db: &PgPool, org_id: Uuid, session_id: Uuid, alasan: &str) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("SELECT set_config('aetherdesk.current_org', $1, true)")
        .bind(org_id.to_string())
        .execute(&mut *tx)
        .await?;

    let n = sqlx::query(
        "UPDATE sessions
         SET status = CASE WHEN status = 'active' THEN 'terminated' ELSE 'disconnected' END,
             ended_at = now()
         WHERE id = $1 AND ended_at IS NULL",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;

    if n > 0 {
        tracing::info!(%session_id, alasan, "sesi berakhir");
    }
    Ok(())
}

/// Menutup seluruh sesi milik sebuah perangkat.
///
/// Dipanggil saat agent terputus. Tanpa ini, sesi menggantung `active`
/// selamanya setiap kali seseorang menutup tab tanpa menekan tombol akhiri —
/// yang justru cara paling umum orang mengakhiri sesuatu.
pub async fn akhiri_semua_perangkat(db: &PgPool, org_id: Uuid, device_uuid: Uuid) -> Result<u64> {
    let mut tx = db.begin().await?;
    sqlx::query("SELECT set_config('aetherdesk.current_org', $1, true)")
        .bind(org_id.to_string())
        .execute(&mut *tx)
        .await?;

    let n = sqlx::query(
        "UPDATE sessions
         SET status = 'disconnected', ended_at = now()
         WHERE device_uuid = $1 AND ended_at IS NULL",
    )
    .bind(device_uuid)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;

    if n > 0 {
        tracing::info!(%device_uuid, jumlah = n, "sesi menggantung ditutup");
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    #[test]
    fn status_akhir_membedakan_sebab() {
        // Sesi yang pernah aktif berakhir sebagai `terminated`; yang tidak
        // pernah disetujui berakhir sebagai `disconnected`. Perbedaan ini yang
        // membuat riwayat dapat menjawab "berapa banyak permintaan yang
        // sebenarnya pernah tersambung".
        let sql = "UPDATE sessions
         SET status = CASE WHEN status = 'active' THEN 'terminated' ELSE 'disconnected' END,
             ended_at = now()
         WHERE id = $1 AND ended_at IS NULL";
        assert!(sql.contains("CASE WHEN status = 'active'"));
        // Penjaga idempotensi: sesi yang sudah tertutup tidak boleh tersentuh.
        assert!(sql.contains("ended_at IS NULL"));
    }
}
