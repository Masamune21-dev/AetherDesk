//! Penentuan IP klien di belakang proxy.
//!
//! Layanan ini selalu berada di balik nginx pada `127.0.0.1`, sehingga alamat
//! peer TCP **selalu** loopback. Memakainya untuk pembatasan laju akan
//! menjadikan seluruh dunia satu ember yang sama — batas per IP menjadi tidak
//! berguna sekaligus berubah menjadi denial of service global.
//!
//! Rantai kepercayaan di deployment ini:
//!
//! ```text
//! pengunjung → Cloudflare → router (SNAT) → nginx → rdp-api
//!                             │                │
//!                             │                └─ set X-Real-IP dari $remote_addr
//!                             └─ nginx memulihkan $remote_addr dari
//!                                CF-Connecting-IP lewat snippet
//!                                cloudflare-real-ip.conf
//! ```
//!
//! Karena itu `X-Real-IP` dari nginx sudah merupakan IP pengunjung asli.
//! Header ini **hanya** boleh dipercaya karena tidak ada jalur lain menuju
//! layanan ini: port 8080 terikat ke loopback dan tidak pernah terekspos.

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::request::Parts,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// Nama header yang diisi nginx pada vhost `aetherdesk`.
const HEADER_REAL_IP: &str = "x-real-ip";

#[derive(Debug, Clone, Copy)]
pub struct IpKlien(pub IpAddr);

impl<S: Send + Sync> FromRequestParts<S> for IpKlien {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(ip) = parts
            .headers
            .get(HEADER_REAL_IP)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
        {
            return Ok(Self(ip));
        }

        // Tanpa header — hanya terjadi saat dipanggil langsung di localhost,
        // misalnya saat pengujian.
        if let Some(ConnectInfo(peer)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            return Ok(Self(peer.ip()));
        }

        tracing::warn!("IP klien tidak dapat ditentukan, memakai 0.0.0.0");
        Ok(Self(IpAddr::V4(Ipv4Addr::UNSPECIFIED)))
    }
}

impl std::fmt::Display for IpKlien {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    async fn ekstrak(req: Request<()>) -> IpAddr {
        let (mut parts, _) = req.into_parts();
        IpKlien::from_request_parts(&mut parts, &()).await.unwrap().0
    }

    #[tokio::test]
    async fn memakai_x_real_ip_bila_ada() {
        let req = Request::builder()
            .header(HEADER_REAL_IP, "203.0.113.195")
            .body(())
            .unwrap();
        assert_eq!(ekstrak(req).await, "203.0.113.195".parse::<IpAddr>().unwrap());
    }

    #[tokio::test]
    async fn menerima_ipv6() {
        let req = Request::builder()
            .header(HEADER_REAL_IP, "2001:db8::1")
            .body(())
            .unwrap();
        assert_eq!(ekstrak(req).await, "2001:db8::1".parse::<IpAddr>().unwrap());
    }

    #[tokio::test]
    async fn header_sampah_tidak_menjatuhkan_request() {
        let req = Request::builder()
            .header(HEADER_REAL_IP, "bukan-ip")
            .body(())
            .unwrap();
        // Jatuh ke fallback, bukan panik atau error.
        assert_eq!(ekstrak(req).await, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    }

    #[tokio::test]
    async fn spasi_di_sekitar_nilai_dimaafkan() {
        let req = Request::builder()
            .header(HEADER_REAL_IP, "  203.0.113.9  ")
            .body(())
            .unwrap();
        assert_eq!(ekstrak(req).await, "203.0.113.9".parse::<IpAddr>().unwrap());
    }
}
