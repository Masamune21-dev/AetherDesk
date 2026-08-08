//! Taksonomi error inti.
//!
//! Menggantikan `RdpError` yang digambarkan di ARCHITECTURE.md §12.1, dengan
//! satu perubahan penting: varian infrastruktur (`DatabaseError`, `CacheError`,
//! `MessagingError`) **tidak** ada di sini. Menaruhnya di crate inti akan
//! memaksa `rdp-core` bergantung pada `sqlx`, `redis`, dan `async-nats` —
//! merusak batas modul yang justru dijanjikan ADR-005.
//!
//! Setiap crate infrastruktur mendefinisikan error-nya sendiri dan
//! mengonversinya ke sini di batas domain.

use crate::ids::IdError;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    // ── Jaringan ─────────────────────────────────────────────────────────────
    #[error("koneksi habis waktu")]
    NetworkTimeout,
    #[error("koneksi ditolak")]
    ConnectionRefused,
    #[error("NAT traversal gagal")]
    NatTraversalFailed,
    #[error("alokasi TURN gagal")]
    TurnAllocationFailed,

    // ── Autentikasi ──────────────────────────────────────────────────────────
    #[error("token tidak valid")]
    InvalidToken,
    #[error("token kedaluwarsa")]
    TokenExpired,
    #[error("sertifikat device tidak valid")]
    DeviceCertInvalid,
    /// Material kunci cacat — panjang salah, base64 rusak, atau titik kurva
    /// yang tidak sah.
    ///
    /// Ketiganya sengaja dilebur menjadi satu varian. Yang membocorkan
    /// informasi bukanlah pesannya, melainkan **perbedaan** antar sebab: ia
    /// memberi tahu penyerang seberapa jauh tebakannya sudah benar. Karena
    /// perbedaan itu sudah hilang di sini, pesannya sendiri aman disampaikan.
    #[error("kunci perangkat tidak valid")]
    KunciTidakValid,
    #[error("izin ditolak")]
    PermissionDenied,
    #[error("MFA diperlukan")]
    MfaRequired,

    // ── Sesi ─────────────────────────────────────────────────────────────────
    #[error("sesi tidak ditemukan")]
    SessionNotFound,
    #[error("sesi kedaluwarsa")]
    SessionExpired,
    #[error("perangkat sedang offline")]
    DeviceOffline,
    #[error("agent sedang sibuk")]
    AgentBusy,

    // ── Protokol ─────────────────────────────────────────────────────────────
    #[error("versi protokol tidak cocok: viewer {viewer}, agent {agent}")]
    ProtocolVersionMismatch { viewer: u8, agent: u8 },
    #[error("packet tidak valid")]
    InvalidPacket,
    #[error("dekripsi gagal")]
    DecryptionFailed,
    #[error("replay terdeteksi pada sequence {0}")]
    ReplayDetected(u32),
    #[error("tanda tangan SDP tidak sah")]
    SdpSignatureInvalid,

    // ── Identitas ────────────────────────────────────────────────────────────
    #[error(transparent)]
    Id(#[from] IdError),

    // ── Sumber daya ──────────────────────────────────────────────────────────
    #[error("inisialisasi encoder gagal")]
    EncoderInitFailed,
    #[error("izin capture layar ditolak")]
    CapturePermissionDenied,
    #[error("bandwidth tidak mencukupi")]
    InsufficientBandwidth,
}

impl CoreError {
    /// Kode stabil yang dikirim ke klien. Nilai ini adalah bagian dari kontrak
    /// API — mengubahnya memutus klien yang sudah terpasang.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NetworkTimeout => "NETWORK_TIMEOUT",
            Self::ConnectionRefused => "CONNECTION_REFUSED",
            Self::NatTraversalFailed => "NAT_TRAVERSAL_FAILED",
            Self::TurnAllocationFailed => "TURN_ALLOCATION_FAILED",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::DeviceCertInvalid => "DEVICE_CERT_INVALID",
            Self::KunciTidakValid => "INVALID_DEVICE_KEY",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::MfaRequired => "MFA_REQUIRED",
            Self::SessionNotFound => "SESSION_NOT_FOUND",
            Self::SessionExpired => "SESSION_EXPIRED",
            Self::DeviceOffline => "DEVICE_OFFLINE",
            Self::AgentBusy => "AGENT_BUSY",
            Self::ProtocolVersionMismatch { .. } => "PROTOCOL_VERSION_MISMATCH",
            Self::InvalidPacket => "INVALID_PACKET",
            Self::DecryptionFailed => "DECRYPTION_FAILED",
            Self::ReplayDetected(_) => "REPLAY_DETECTED",
            Self::SdpSignatureInvalid => "SDP_SIGNATURE_INVALID",
            Self::Id(_) => "INVALID_DEVICE_ID",
            Self::EncoderInitFailed => "ENCODER_INIT_FAILED",
            Self::CapturePermissionDenied => "CAPTURE_PERMISSION_DENIED",
            Self::InsufficientBandwidth => "INSUFFICIENT_BANDWIDTH",
        }
    }

    /// Status HTTP yang sepadan, untuk dipetakan di lapisan API.
    pub fn http_status(&self) -> u16 {
        match self {
            Self::InvalidToken | Self::TokenExpired | Self::DeviceCertInvalid => 401,
            Self::KunciTidakValid => 400,
            Self::PermissionDenied => 403,
            Self::MfaRequired => 403,
            Self::SessionNotFound => 404,
            Self::Id(_) | Self::InvalidPacket | Self::ProtocolVersionMismatch { .. } => 400,
            Self::DeviceOffline | Self::AgentBusy => 409,
            Self::SessionExpired => 410,
            Self::NetworkTimeout => 504,
            _ => 500,
        }
    }

    /// Benar bila pesan error boleh sampai ke klien apa adanya.
    ///
    /// Error infrastruktur dan kriptografi tidak pernah boleh — detailnya
    /// membocorkan bentuk sistem kepada penyerang.
    pub fn is_safe_to_expose(&self) -> bool {
        !matches!(
            self,
            Self::DecryptionFailed | Self::ReplayDetected(_) | Self::SdpSignatureInvalid
        )
    }
}

pub type Result<T, E = CoreError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kode_error_unik() {
        use std::collections::HashSet;
        let contoh = [
            CoreError::NetworkTimeout,
            CoreError::InvalidToken,
            CoreError::TokenExpired,
            CoreError::PermissionDenied,
            CoreError::SessionNotFound,
            CoreError::DeviceOffline,
            CoreError::InvalidPacket,
            CoreError::DecryptionFailed,
            CoreError::SdpSignatureInvalid,
        ];
        let kode: HashSet<_> = contoh.iter().map(|e| e.code()).collect();
        assert_eq!(kode.len(), contoh.len(), "ada kode error yang bertabrakan");
    }

    #[test]
    fn kegagalan_kripto_tidak_pernah_diekspos() {
        assert!(!CoreError::DecryptionFailed.is_safe_to_expose());
        assert!(!CoreError::ReplayDetected(42).is_safe_to_expose());
        assert!(!CoreError::SdpSignatureInvalid.is_safe_to_expose());
        assert!(CoreError::DeviceOffline.is_safe_to_expose());
    }

    #[test]
    fn id_error_terkonversi_otomatis() {
        let e: CoreError = IdError::BukanDigit.into();
        assert_eq!(e.code(), "INVALID_DEVICE_ID");
        assert_eq!(e.http_status(), 400);
    }
}
