//! Konfigurasi runtime.
//!
//! ARCHITECTURE.md §13.1 menetapkan hierarki: environment variable menimpa
//! berkas config, yang menimpa default terkompilasi. Fase 0 baru menerapkan
//! lapisan environment variable — lapisan berkas TOML menyusul saat ada
//! parameter yang memang layak diletakkan di sana.

use anyhow::{Context, Result};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub bind: String,
    pub db_max_connections: u32,
    pub db_acquire_timeout: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: required("AETHERDESK_DB_URL")?,
            redis_url: required("AETHERDESK_REDIS_URL")?,
            bind: optional("AETHERDESK_API_BIND", "127.0.0.1:8080"),
            db_max_connections: optional("AETHERDESK_DB_MAX_CONN", "10")
                .parse()
                .context("AETHERDESK_DB_MAX_CONN harus berupa angka")?,
            db_acquire_timeout: Duration::from_secs(5),
        })
    }
}

fn required(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("environment variable {key} belum diset"))
}

fn optional(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_memakai_default_saat_kosong() {
        assert_eq!(
            optional("AETHERDESK_VARIABEL_YANG_PASTI_TIDAK_ADA", "fallback"),
            "fallback"
        );
    }

    #[test]
    fn required_gagal_dengan_pesan_menyebut_nama_variabel() {
        let e = required("AETHERDESK_VARIABEL_YANG_PASTI_TIDAK_ADA").unwrap_err();
        assert!(e.to_string().contains("AETHERDESK_VARIABEL_YANG_PASTI_TIDAK_ADA"));
    }
}
