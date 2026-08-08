//! State bersama yang dibagi ke seluruh handler.

use crate::auth::jwt::JwtKeys;
use rdp_core::InProcessBus;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: redis::aio::ConnectionManager,
    pub jwt: JwtKeys,
    /// ADR-013: bus in-process untuk Fase 0. Mengganti ke NATS berarti
    /// mengganti tipe ini menjadi `Arc<dyn EventBus>` — pemanggil tidak berubah.
    pub events: Arc<InProcessBus>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `ConnectionManager` tidak mengimplementasikan Debug, dan PgPool akan
        // mencetak connection string berisi password bila diteruskan begitu
        // saja. `JwtKeys` memuat material kunci privat.
        f.debug_struct("AppState")
            .field("db", &"PgPool")
            .field("redis", &"ConnectionManager")
            .field("jwt", &self.jwt)
            .finish()
    }
}
