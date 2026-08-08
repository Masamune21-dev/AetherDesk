//! # rdp-core
//!
//! Tipe inti dan aturan domain AetherDesk yang dibagi seluruh crate.
//!
//! Crate ini sengaja **tidak** bergantung pada framework web, driver database,
//! maupun message bus. Batas itu yang membuat ADR-005 (modular monolith dengan
//! jalur evolusi ke microservices) benar-benar dapat ditegakkan, bukan sekadar
//! dijanjikan di dokumen.
//!
//! ## Modul
//!
//! - [`damm`] — check digit untuk device ID
//! - [`ids`] — newtype identitas domain
//! - [`password`] — password sesi sekali pakai
//! - [`event`] — domain event dan abstraksi [`event::EventBus`]
//! - [`error`] — taksonomi error inti

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations, rust_2018_idioms)]

pub mod damm;
pub mod error;
pub mod event;
pub mod ids;
pub mod password;

pub use error::{CoreError, Result};
pub use event::{DomainEvent, EventBus, InProcessBus, QuickConnectOutcome};
pub use ids::{DeviceId, DeviceUuid, OrgId, SessionId, UserId};

/// Versi protokol wire yang diimplementasikan crate ini.
///
/// Dinaikkan hanya saat terjadi perubahan yang memutus kompatibilitas pada
/// tata letak packet di REMOTE_PROTOCOL.md.
pub const PROTOCOL_VERSION: u8 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versi_protokol_sesuai_spesifikasi() {
        // REMOTE_PROTOCOL.md §2 menetapkan versi saat ini adalah 1.
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn reexport_dapat_dipakai() {
        let _ = DeviceId::generate();
        let _ = UserId::new();
        let _: Result<()> = Ok(());
    }
}
