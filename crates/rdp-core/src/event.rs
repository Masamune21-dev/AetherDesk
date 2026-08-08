//! Domain event dan abstraksi bus.
//!
//! ADR-013 mewajibkan trait ini ada **sejak commit pertama**, bukan ditambahkan
//! belakangan. Fase 0 memakai [`InProcessBus`]; saat Signal Server diekstrak
//! menjadi layanan terpisah, yang berubah hanya satu adapter — bukan setiap
//! pemanggil.

use crate::ids::{DeviceId, OrgId, SessionId, UserId};
use serde::{Deserialize, Serialize};

/// Event domain yang disiarkan lintas modul.
///
/// Penamaan mengikuti CODING_STANDARD.md §2.5: kata kerja bentuk lampau,
/// karena event menyatakan sesuatu yang **sudah** terjadi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    DeviceRegistered {
        org_id: OrgId,
        device_id: DeviceId,
    },
    DeviceOnline {
        device_id: DeviceId,
    },
    DeviceOffline {
        device_id: DeviceId,
        /// Benar bila transisi berasal dari putusnya koneksi, bukan dari TTL
        /// yang kedaluwarsa. Membedakan keduanya penting untuk temuan S-09.
        graceful: bool,
    },
    SessionStarted {
        session_id: SessionId,
        device_id: DeviceId,
        viewer_id: UserId,
    },
    SessionEnded {
        session_id: SessionId,
        duration_seconds: i64,
    },
    QuickConnectAttempted {
        device_id_input: String,
        outcome: QuickConnectOutcome,
    },
}

/// Hasil satu upaya Quick Connect. Dicatat untuk setiap upaya, termasuk yang
/// gagal — justru baris gagal itulah sinyal pemindaian ruang ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickConnectOutcome {
    Accepted,
    BadPassword,
    UnknownId,
    Throttled,
    RejectedByUser,
}

impl QuickConnectOutcome {
    /// Nilai yang disimpan ke kolom `outcome`.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::BadPassword => "bad_password",
            Self::UnknownId => "unknown_id",
            Self::Throttled => "throttled",
            Self::RejectedByUser => "rejected_by_user",
        }
    }

    /// Benar bila upaya ini patut dihitung terhadap batas laju.
    ///
    /// ID dengan check digit salah tidak pernah sampai ke sini — sudah ditolak
    /// sebelum menyentuh database (QUICK_CONNECT.md §5).
    pub fn counts_toward_rate_limit(&self) -> bool {
        matches!(self, Self::BadPassword | Self::UnknownId)
    }
}

/// Abstraksi bus event.
///
/// Sengaja minimal. Semakin sedikit yang dijanjikan trait ini, semakin mudah
/// menggantinya dengan NATS JetStream tanpa menyentuh pemanggil.
#[async_trait::async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent);
}

/// Implementasi Fase 0 — mencatat event lalu membuangnya.
///
/// Cukup selama seluruh modul hidup dalam satu proses. Menggantinya dengan
/// NATS adalah pekerjaan satu berkas.
#[derive(Debug, Default, Clone)]
pub struct InProcessBus;

#[async_trait::async_trait]
impl EventBus for InProcessBus {
    async fn publish(&self, event: DomainEvent) {
        // Aman memakai `Debug`: seluruh varian event dirancang bebas rahasia.
        // Password, token, dan kunci tidak pernah masuk ke dalamnya — lihat
        // `QuickConnectAttempted` yang hanya membawa ID yang diketik, bukan
        // password yang dicoba.
        tracing::info!(target: "domain_event", ?event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_db_str_stabil() {
        // Nilai ini masuk ke database — mengubahnya memutus data historis.
        assert_eq!(QuickConnectOutcome::Accepted.as_db_str(), "accepted");
        assert_eq!(QuickConnectOutcome::BadPassword.as_db_str(), "bad_password");
        assert_eq!(QuickConnectOutcome::UnknownId.as_db_str(), "unknown_id");
        assert_eq!(QuickConnectOutcome::Throttled.as_db_str(), "throttled");
        assert_eq!(
            QuickConnectOutcome::RejectedByUser.as_db_str(),
            "rejected_by_user"
        );
    }

    #[test]
    fn hanya_kegagalan_kredensial_yang_dihitung() {
        assert!(QuickConnectOutcome::BadPassword.counts_toward_rate_limit());
        assert!(QuickConnectOutcome::UnknownId.counts_toward_rate_limit());
        // Sudah dijeda: jangan memperpanjang jeda sendiri.
        assert!(!QuickConnectOutcome::Throttled.counts_toward_rate_limit());
        // Penolakan oleh pengguna bukan kegagalan kredensial.
        assert!(!QuickConnectOutcome::RejectedByUser.counts_toward_rate_limit());
        assert!(!QuickConnectOutcome::Accepted.counts_toward_rate_limit());
    }

    #[tokio::test]
    async fn in_process_bus_menerima_publish() {
        let bus = InProcessBus;
        bus.publish(DomainEvent::DeviceOffline {
            device_id: crate::ids::DeviceId::generate(),
            graceful: true,
        })
        .await;
    }
}
