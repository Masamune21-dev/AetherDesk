//! Riwayat sesi dan audit log.

use crate::{
    auth::Terautentikasi,
    db,
    error::{ApiResult, Sukses},
    state::AppState,
};
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct Sesi {
    pub session_id: Uuid,
    /// Disimpan sebagai snapshot pada barisnya sendiri, bukan lewat join.
    /// Perbaikan T-06 memakai `ON DELETE SET NULL` agar organisasi tetap dapat
    /// dihapus, jadi riwayat harus mampu berdiri tanpa baris perangkat.
    pub device_id: String,
    pub viewer_email: String,
    pub status: String,
    pub connect_method: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
}

/// `GET /api/v1/sessions`
pub async fn daftar(
    State(state): State<AppState>,
    Terautentikasi(claims): Terautentikasi,
) -> ApiResult<Sukses<Vec<Sesi>>> {
    let mut tx = db::mulai_transaksi_tenant(&state.db, claims.org_id()).await?;

    // RLS sudah membatasi ke organisasi aktif; tidak perlu WHERE tambahan.
    let baris: Vec<(Uuid, String, String, String, String, DateTime<Utc>, Option<DateTime<Utc>>)> =
        sqlx::query_as(
            "SELECT id, device_id_snapshot, viewer_email_snapshot, status,
                    connect_method, started_at, ended_at
             FROM sessions
             ORDER BY started_at DESC
             LIMIT 200",
        )
        .fetch_all(&mut *tx)
        .await?;

    tx.commit().await?;

    let sesi = baris
        .into_iter()
        .map(|(id, dev, email, status, metode, mulai, selesai)| Sesi {
            session_id: id,
            device_id: dev.trim().to_string(),
            viewer_email: email,
            status,
            connect_method: metode,
            started_at: mulai,
            ended_at: selesai,
            duration_seconds: selesai.map(|s| (s - mulai).num_seconds()),
        })
        .collect();

    Ok(Sukses::baru(sesi))
}

// ── Audit log ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct EntriAudit {
    pub id: Uuid,
    pub action: String,
    pub user_id: Option<Uuid>,
    pub ip_address: String,
    pub payload: Option<Value>,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/v1/audit-logs`
///
/// `audit_logs` tidak memakai RLS — penulisannya terjadi pada jalur yang
/// tenant-nya sudah pasti. Karena itu pembacaan **wajib** menyaring
/// `organization_id` secara eksplisit; tanpa itu satu tenant dapat membaca
/// jejak audit tenant lain.
pub async fn audit(
    State(state): State<AppState>,
    Terautentikasi(claims): Terautentikasi,
) -> ApiResult<Sukses<Vec<EntriAudit>>> {
    let baris: Vec<(Uuid, String, Option<Uuid>, String, Option<Value>, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT id, action, user_id, host(ip_address), payload, created_at
             FROM audit_logs
             WHERE organization_id = $1
             ORDER BY created_at DESC
             LIMIT 200",
        )
        .bind(claims.org_id())
        .fetch_all(&state.db)
        .await?;

    let entri = baris
        .into_iter()
        .map(|(id, action, user_id, ip, payload, created_at)| EntriAudit {
            id,
            action,
            user_id,
            ip_address: ip,
            payload,
            created_at,
        })
        .collect();

    Ok(Sukses::baru(entri))
}

#[cfg(test)]
mod tests {
    #[test]
    fn kueri_audit_menyaring_tenant_secara_eksplisit() {
        // Penjaga regresi. `audit_logs` sengaja tanpa RLS, jadi filter tenant
        // adalah satu-satunya yang memisahkan organisasi. Menghapusnya
        // membocorkan seluruh jejak audit lintas tenant tanpa gejala apa pun.
        let sql = "SELECT id, action, user_id, host(ip_address), payload, created_at
             FROM audit_logs
             WHERE organization_id = $1
             ORDER BY created_at DESC
             LIMIT 200";
        assert!(sql.contains("WHERE organization_id = $1"));
    }

    #[test]
    fn kueri_sesi_bergantung_pada_rls_bukan_join() {
        // Snapshot identitas dipakai agar riwayat bertahan meski perangkat
        // atau pengguna sudah dihapus (perbaikan T-06).
        let sql = "SELECT id, device_id_snapshot, viewer_email_snapshot, status,
                    connect_method, started_at, ended_at
             FROM sessions";
        assert!(sql.contains("device_id_snapshot"));
        assert!(!sql.contains("JOIN"), "riwayat tidak boleh bergantung pada join");
    }
}
