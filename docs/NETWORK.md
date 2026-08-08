# Network Routing & Architecture Specification

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** System Architect / Network Engineer

---

## 1. Topologi Jaringan Global

Remote Desktop Platform menggunakan topologi global terdistribusi untuk meminimalkan latensi data plane.

```
       Viewer (Jakarta) ────────────────► Anycast DNS
              │                                │
              │ (RTT: 5ms)                     ▼
              ▼                     TURN Server (Singapura)
        [P2P Direct Path]                      ▲
     (gagal karena Symmetric NAT)              │ (RTT: 8ms)
              │                                │
              ▼                                │
      Relay Tunnel (UDP Encrypted) ────────────┘
              │
              ▼
       Agent (Bandung)
```

---

## 2. Dynamic Port Allocations

Berikut adalah daftar port yang wajib dibuka pada infrastruktur jaringan:

### 2.1 Server-Side Ports

| Service | Port | Protokol | Deskripsi |
|---|---|---|---|
| Ingress / Web / API | `443` | TCP | HTTPS REST API, gRPC-Web, dan WebSocket signaling |
| STUN Server | `3478` | UDP/TCP | NAT Discovery & Binding |
| TURN Server | `3478` | UDP/TCP | TURN Allocation (Fallback) |
| TURN Server (Secure)| `5349` | UDP/TCP | TURN over TLS / DTLS |
| TURN Media Relay | `49152 - 65535` | UDP | Alokasi port dinamis untuk packet forwarding |
| Internal gRPC | `50051` | TCP | Komunikasi inter-service dalam private network |

### 2.2 Client-Side Ports (Agent & Viewer)

- **Outbound**: Harus diizinkan melakukan koneksi keluar (outbound) ke port `443` TCP, `3478` UDP/TCP, dan dynamic range `49152-65535` UDP.
- **Inbound**: Tidak ada port inbound yang wajib dibuka pada router client. Seluruh koneksi inbound ditangani via STUN hole punching (P2P) atau di-relay via TURN server outbound tunnel.

---

## 3. Firewall & NAT Traversal Mechanics

### 3.1 UDP Hole Punching Flow
1. **Signaling**: Viewer dan Agent bertukar IP publik & port lokal (ICE candidates) via Signal Server.
2. **Probing**: Kedua client mengirimkan paket UDP kosong (STUN binding requests) ke alamat publik pihak lawan secara bersamaan.
3. **NAT Mapping creation**: Pengiriman paket keluar memaksa NAT masing-masing pihak membuat entri *mapping* di firewall-nya.
4. **State establishment**: Setelah salah satu paket berhasil menembus port mapping lawan, status koneksi UDP berubah menjadi `established` (P2P langsung berhasil).

### 3.2 Symmetric NAT Handling
Jika salah satu pihak berada di belakang Symmetric NAT (IP/port destination berubah-ubah per request):
- UDP hole punching kemungkinan besar gagal (tingkat keberhasilan < 10%).
- ICE agent secara otomatis mendeteksi kegagalan konektivitas langsung dan mengalihkan jalur data plane ke **TURN Relay** melalui port `443` (TCP/UDP) yang menyerupai trafik HTTPS biasa untuk menghindari pemblokiran firewall.
