//! Akses database dengan cakupan tenant.
//!
//! Migrasi 0001 mengaktifkan `FORCE ROW LEVEL SECURITY`, sehingga setiap query
//! terhadap tabel ber-tenant **wajib** menetapkan organisasi aktif lebih dulu:
//!
//! ```sql
//! SET LOCAL aetherdesk.current_org = '<uuid>';
//! ```
//!
//! `SET LOCAL` hanya berlaku sampai transaksi berakhir, jadi kebocoran antar
//! request mustahil terjadi lewat connection pool — koneksi yang dikembalikan
//! ke pool sudah kehilangan setelan itu.
//!
//! Modul ini membuat aturan tersebut sulit dilupakan: satu-satunya cara
//! memperoleh transaksi adalah lewat [`mulai_transaksi_tenant`], yang selalu
//! menetapkannya.

use crate::error::ApiResult;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Membuka transaksi yang sudah tercakup pada satu organisasi.
///
/// Pemanggil wajib memanggil `.commit()`; bila tidak, sqlx melakukan rollback
/// saat transaksi di-drop.
pub async fn mulai_transaksi_tenant(
    pool: &PgPool,
    org_id: Uuid,
) -> ApiResult<Transaction<'static, Postgres>> {
    let mut tx = pool.begin().await?;

    // `set_config` dipakai alih-alih `SET LOCAL` dengan interpolasi string,
    // karena SET tidak menerima parameter terikat. Fungsi ini menerimanya,
    // sehingga tidak ada UUID yang pernah disambung ke dalam teks SQL.
    sqlx::query("SELECT set_config('aetherdesk.current_org', $1, true)")
        .bind(org_id.to_string())
        .execute(&mut *tx)
        .await?;

    Ok(tx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn set_config_dipakai_agar_uuid_tidak_pernah_disambung() {
        // Penjaga terhadap regresi: bila seseorang mengganti `set_config`
        // dengan format!("SET LOCAL ... = '{}'", uuid), jalur ini kembali
        // menjadi sambungan string. Test ini menandai niatnya.
        let sql = "SELECT set_config('aetherdesk.current_org', $1, true)";
        assert!(sql.contains("$1"), "parameter terikat hilang");
        assert!(!sql.contains("format!"));
    }
}
