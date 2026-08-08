# Architecture Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** Principal Software Architect

---

## Daftar Isi

1. [System Architecture Overview](#1-system-architecture-overview)
2. [Design Principles](#2-design-principles)
3. [Architecture Decision Records](#3-architecture-decision-records)
4. [Component Architecture](#4-component-architecture)
5. [Communication Patterns](#5-communication-patterns)
6. [Authentication Flow](#6-authentication-flow)
7. [Session Flow](#7-session-flow)
8. [Network Connection Flows](#8-network-connection-flows)
9. [Data Flows](#9-data-flows)
10. [Infrastructure Architecture](#10-infrastructure-architecture)
11. [Module Structure](#11-module-structure)
12. [Error Handling Strategy](#12-error-handling-strategy)
13. [Configuration Management](#13-configuration-management)

---

## 1. System Architecture Overview

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                              REMOTE DESKTOP PLATFORM                                      │
│                           Gambaran Arsitektur Sistem                                      │
└──────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│  CLIENT LAYER                                                                           │
│                                                                                         │
│  ┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────────────┐ │
│  │    Desktop Viewer   │    │    Desktop Agent     │    │      Web Dashboard          │ │
│  │    (Rust + Tauri)   │    │    (Rust, native)    │    │  (Laravel 13 + Vue 3)       │ │
│  │                     │    │                      │    │                             │ │
│  │  • Connection Mgr   │    │  • Screen Capture    │    │  • Device Management        │ │
│  │  • Video Renderer   │    │  • Input Handler     │    │  • User Management          │ │
│  │  • Input Sender     │    │  • Audio Capture     │    │  • Session History          │ │
│  │  • File Manager     │    │  • File Transfer     │    │  • Analytics                │ │
│  │  • Address Book     │    │  • Clipboard Sync    │    │  • Policy Management        │ │
│  │  • Chat UI          │    │  • System Tray       │    │  • Audit Logs               │ │
│  └─────────┬───────────┘    └──────────┬───────────┘    └───────────────┬─────────────┘ │
│            │                           │                                │               │
└────────────┼───────────────────────────┼────────────────────────────────┼───────────────┘
             │                           │                                │
             │         WebRTC (P2P)      │                                │ HTTPS/WSS
             │◄─────────────────────────►│                                │
             │                           │                                │
             │         HTTPS/WSS         │        HTTPS/WSS               │
             └──────────┬────────────────┘────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│  EDGE / CDN LAYER                                                                       │
│                                                                                         │
│  ┌─────────────────────────────────────────────────────────────────────────────────┐   │
│  │                    Cloudflare / HAProxy / Nginx                                  │   │
│  │          TLS Termination • Rate Limiting • WAF • DDoS Protection                │   │
│  └─────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                         │
│  ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────────────────────┐  │
│  │  STUN Cluster     │  │  TURN Cluster      │  │  CDN (Static Assets)             │  │
│  │                   │  │                    │  │                                   │  │
│  │  • coturn         │  │  • coturn/Pion     │  │  • JS/CSS bundles                 │  │
│  │  • Geo-distributed│  │  • Geo-distributed │  │  • Agent installers               │  │
│  │  • 50+ PoP global │  │  • E2E relay only  │  │  • Documentation                 │  │
│  └───────────────────┘  └───────────────────┘  └───────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│  APPLICATION LAYER (Kubernetes Cluster)                                                 │
│                                                                                         │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │  API Server      │  │  Signal Server   │  │  Relay Server    │  │  Web Server    │ │
│  │  (Rust/Axum)     │  │  (Rust/Axum/WS)  │  │  (Rust)          │  │  (Laravel 13)  │ │
│  │                  │  │                  │  │                  │  │                │ │
│  │  • REST API      │  │  • Session Mgmt  │  │  • Packet relay  │  │  • Dashboard   │ │
│  │  • gRPC          │  │  • ICE Signal    │  │  • E2E encrypted │  │  • Inertia SSR │ │
│  │  • Auth/JWT      │  │  • WebSocket     │  │  • Bandwidth QoS │  │  • API proxy   │ │
│  │  • Webhook       │  │  • Presence      │  │  • Load balanced │  │  • Auth        │ │
│  └────────┬─────────┘  └────────┬─────────┘  └─────────────────┘  └───────┬────────┘ │
│           │                     │                                           │          │
│  ┌────────┴─────────────────────┴───────────────────────────────────────────┴───────┐  │
│  │                         NATS JetStream Message Bus                               │  │
│  │     • Event-driven pub/sub   • Session events   • Device events   • Audit events │  │
│  └────────┬─────────────────────────────────────────────────────────────────────────┘  │
│           │                                                                             │
│  ┌────────┴─────────────────────────────────────────────────────────────────────────┐  │
│  │  Supporting Services                                                              │  │
│  │                                                                                   │  │
│  │  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐  │  │
│  │  │ Update Service │  │  Notification  │  │ Metrics/OTel   │  │  Audit Service │  │  │
│  │  │                │  │  Service       │  │  Collector     │  │                │  │  │
│  │  └────────────────┘  └────────────────┘  └────────────────┘  └────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│  DATA LAYER                                                                             │
│                                                                                         │
│  ┌────────────────────────┐  ┌──────────────────────┐  ┌───────────────────────────┐  │
│  │  PostgreSQL HA         │  │  Redis Cluster        │  │  Object Storage (S3)      │  │
│  │                        │  │                       │  │                           │  │
│  │  Primary               │  │  • Session cache      │  │  • Session recordings     │  │
│  │  ├── Replica 1        │  │  • Device presence    │  │  • File transfer temp     │  │
│  │  └── Replica 2        │  │  • Rate limiting       │  │  • Audit log archives     │  │
│  │                        │  │  • JWT revocation     │  │  • Agent installers       │  │
│  │  PgBouncer connection  │  │  • Pub/sub            │  │  • Software updates       │  │
│  │  pooling               │  │  • Distributed lock   │  └───────────────────────────┘  │
│  └────────────────────────┘  └──────────────────────┘                                  │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│  OBSERVABILITY LAYER                                                                    │
│                                                                                         │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────┐ │
│  │  Prometheus      │  │  Grafana         │  │  Jaeger          │  │  ELK Stack     │ │
│  │  (Metrics)       │  │  (Dashboards)    │  │  (Tracing)       │  │  (Logs)        │ │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘  └────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Design Principles

### 2.1 Zero Trust Architecture

Tidak ada komponen yang dipercaya secara implisit. Setiap request harus:
- **Diautentikasi** — verify identity (device cert + user JWT)
- **Diotorisasi** — verify permission (RBAC/ABAC check)
- **Dienkripsi** — data tidak boleh plaintext in transit atau at rest
- **Diaudit** — semua akses dicatat dalam audit log immutable

### 2.2 Defense in Depth

Berlapis-lapis pertahanan keamanan:

```
Internet
    │
    ▼
[Layer 1: Edge Security]
Cloudflare WAF + DDoS Protection + Rate Limiting
    │
    ▼
[Layer 2: Transport Security]
TLS 1.3 mandatory, Certificate Pinning
    │
    ▼
[Layer 3: Authentication]
JWT + Device Certificate (mTLS), MFA
    │
    ▼
[Layer 4: Authorization]
RBAC + ABAC checks per request
    │
    ▼
[Layer 5: Application Security]
Input validation, OWASP mitigations
    │
    ▼
[Layer 6: Session Security]
E2E encryption, nonce/replay protection
    │
    ▼
[Layer 7: Data Security]
Encryption at rest, data minimization
```

### 2.3 Least Privilege

Setiap komponen, service, dan user mendapat access minimal yang diperlukan:
- Agent hanya bisa membuat session untuk device-nya sendiri
- Viewer hanya bisa connect ke device yang diizinkan
- Service account hanya bisa akses tabel/API yang dibutuhkan
- Plugin berjalan dalam sandbox dengan API terbatas

### 2.4 Fail Safe

Sistem gagal ke mode aman:
- Jika auth service down → reject semua request (bukan allow all)
- Jika enkripsi gagal → terminate session (bukan kirim plaintext)
- Jika policy check error → deny access (bukan allow)
- Jika relay down → fallback ke relay lain (bukan drop encryption)

### 2.5 Separation of Concerns

| Concern | Komponen |
|---------|----------|
| Identity & Auth | API Server + JWT Service |
| Signaling | Signal Server |
| Data Relay | Relay Server (E2E, tidak bisa decrypt) |
| Business Logic | API Server |
| Presentation | Web Server + Tauri Viewer |
| Observability | Prometheus + Grafana + Jaeger |

### 2.6 Modular Monolith → Microservices Evolution

Dimulai sebagai modular monolith dengan clear module boundaries. Setiap modul:
- Memiliki domain model tersendiri
- Berkomunikasi via message bus (NATS) untuk async, atau internal function call
- Dapat di-extract menjadi microservice saat beban membutuhkan

---

## 3. Architecture Decision Records

### ADR-001: Rust sebagai Primary Language

| | Detail |
|---|---|
| **Status** | Accepted |
| **Context** | Perlu bahasa yang aman, cepat, dan efisien untuk agent dan backend |
| **Decision** | Gunakan Rust untuk agent, backend API, signal server, relay server |
| **Rationale** | Memory safety tanpa GC, zero-cost abstractions, sistem-level control, async via Tokio, ekosistem networking matang (Quinn/QUIC, rustls) |
| **Alternatives** | Go (GC overhead, kurang safe), C++ (memory unsafe), Python (terlalu lambat) |
| **Consequences** | Learning curve lebih tinggi, developer pool lebih kecil, tapi runtime performance superior dan security lebih baik |

### ADR-002: NATS JetStream vs RabbitMQ

| | Detail |
|---|---|
| **Status** | Accepted |
| **Decision** | Gunakan NATS JetStream |
| **Rationale** | |
| | • **Latency**: NATS < 1ms vs RabbitMQ ~1-5ms — kritis untuk signaling real-time |
| | • **Throughput**: NATS 10M+ msg/s vs RabbitMQ ~1M msg/s |
| | • **Operasional**: NATS lebih sederhana (single binary, embedded clustering) |
| | • **Protocol fit**: NATS lebih cocok untuk ephemeral signaling; JetStream untuk durable events |
| | • **Memory**: NATS lebih lightweight |
| **Alternatives** | RabbitMQ (AMQP, lebih mature tapi berat), Kafka (over-engineered untuk use case ini) |
| **Consequences** | Ekosistem RabbitMQ lebih besar, tapi NATS lebih fit untuk real-time latency requirements |

### ADR-003: Tauri vs Electron untuk Desktop Viewer

| | Detail |
|---|---|
| **Status** | Accepted |
| **Decision** | Gunakan Tauri dengan Rust backend + Vue 3 frontend |
| **Rationale** | |
| | • **Bundle size**: Tauri ~10MB vs Electron ~100MB+ |
| | • **Memory**: Tauri ~50MB vs Electron ~300MB+ |
| | • **Native access**: Tauri memberi akses ke Rust codebase yang sama |
| | • **Security**: No Node.js in renderer, lebih kecil attack surface |
| | • **GPU**: Bisa akses platform native GPU APIs via Rust |
| **Alternatives** | Electron (V8, lebih mature ecosystem tapi resource boros), Qt (C++, complex), Flutter Desktop |
| **Consequences** | Webview rendering bisa berbeda tiap platform, tapi UI logic shared via Vue 3 |

### ADR-004: WebRTC untuk Streaming

| | Detail |
|---|---|
| **Status** | Accepted |
| **Decision** | WebRTC sebagai transport layer untuk screen streaming |
| **Rationale** | |
| | • **NAT traversal**: Built-in ICE, STUN, TURN support |
| | • **Standardized**: Implementasi ada di semua platform |
| | • **Latency**: Dirancang untuk real-time, UDP-based |
| | • **Security**: DTLS-SRTP mandatory |
| | • **Adaptability**: Built-in congestion control, adaptive bitrate |
| **Alternatives** | Custom UDP protocol (butuh reinvent NAT traversal), QUIC (lebih baru, ekosistem belum matang), RTP langsung (butuh signaling sendiri) |
| **Consequences** | Depend pada WebRTC library (Pion/Go atau webrtc-rs), overhead signaling via ICE |

### ADR-005: Modular Monolith dengan Evolution Path

| | Detail |
|---|---|
| **Status** | Accepted |
| **Decision** | Mulai dengan modular monolith, extract microservices bila skala membutuhkan |
| **Rationale** | |
| | • **Kesederhanaan**: Single deployment, single database (partitioned), lebih mudah debug |
| | • **Performance**: In-process communication, no network overhead antar modul |
| | • **Evolusi bertahap**: Extract modul yang skalanya berbeda (signal server vs API server) |
| | • **Developer experience**: Lebih mudah onboarding, refactor, dan testing |
| **Module boundaries** | auth, devices, sessions, billing, notifications, updates — each is own Rust module |
| **Extract candidates** | Signal Server (WebSocket heavy), Relay Server (network I/O heavy), Metrics Collector |

### ADR-006: PostgreSQL sebagai Primary Database

| | Detail |
|---|---|
| **Status** | Accepted |
| **Decision** | PostgreSQL sebagai satu-satunya SQL database |
| **Rationale** | JSONB untuk semi-structured data, partitioning untuk logs, excellent full-text search, mature HA dengan Patroni, row-level security untuk multi-tenant |
| **Alternatives** | MySQL (kurang fitur JSON/advanced queries), CockroachDB (overhead distributed SQL), ScyllaDB (no joins, lebih cocok untuk time-series only) |

---

## 4. Component Architecture

### 4.1 Agent Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                      DESKTOP AGENT                           │
│                    (Rust, native binary)                     │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Core Services                                          │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │ │
│  │  │  Watchdog     │  │  Crash Rcvry │  │  Auto Update │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Capture Pipeline                                       │ │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐ │ │
│  │  │  Screen Capture  │  │  Audio Capture               │ │ │
│  │  │  ┌─ DXGI (Win) ─┐│  │  ┌─ WASAPI (Win) ──────────┐│ │ │
│  │  │  └─ SCKit (Mac) ─┘│  │  └─ CoreAudio (Mac) ───────┘│ │ │
│  │  └──────────────────┘  └──────────────────────────────┘ │ │
│  │                                                         │ │
│  │  ┌──────────────────────────────────────────────────┐   │ │
│  │  │  Hardware Encoder                                 │   │ │
│  │  │  ┌─ NVENC ─┐ ┌─ QuickSync ─┐ ┌─ AMF ─┐          │   │ │
│  │  │  └─────────┘ └─────────────┘ └───────┘          │   │ │
│  │  │  ┌─ VideoToolbox (Mac) ─┐ ┌─ Software fallback ─┐│   │ │
│  │  │  └─────────────────────┘ └────────────────────┘ │   │ │
│  │  └──────────────────────────────────────────────────┘   │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Input Handler                                          │ │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐ │ │
│  │  │  Keyboard Hook   │  │  Mouse Hook                  │ │ │
│  │  └──────────────────┘  └──────────────────────────────┘ │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Network Stack                                          │ │
│  │  ┌───────────────┐  ┌───────────────┐  ┌─────────────┐ │ │
│  │  │  WebRTC Peer  │  │  WS Client    │  │  REST Client│ │ │
│  │  │  (webrtc-rs)  │  │  (Signal)     │  │  (Reg/Auth) │ │ │
│  │  └───────────────┘  └───────────────┘  └─────────────┘ │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  System Integration                                     │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │ │
│  │  │  System Tray │  │  OS Service  │  │  Permissions │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### 4.2 Viewer Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                      DESKTOP VIEWER                          │
│                    (Rust + Tauri + Vue 3)                    │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  UI Layer (Vue 3 + TypeScript)                          │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │ │
│  │  │  Address Book│  │  Session Tab │  │  File Manager│  │ │
│  │  │  Device List │  │  Canvas Rndr │  │  Terminal    │  │ │
│  │  │  Settings    │  │  Toolbar     │  │  Chat        │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Tauri Backend (Rust)                                   │ │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐ │ │
│  │  │  Connection Mgr  │  │  Session State               │ │ │
│  │  │  • P2P/Relay     │  │  • Multi-session             │ │ │
│  │  │  • ICE handling  │  │  • Recording                 │ │ │
│  │  └──────────────────┘  └──────────────────────────────┘ │ │
│  │                                                         │ │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐ │ │
│  │  │  Decode Pipeline │  │  Render Pipeline             │ │ │
│  │  │  • H264/H265/AV1 │  │  • GPU-accelerated           │ │ │
│  │  │  • Audio decode  │  │  • WebGL/Metal/D3D12         │ │ │
│  │  └──────────────────┘  └──────────────────────────────┘ │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### 4.3 API Server Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                       API SERVER                             │
│                     (Rust / Axum)                            │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Transport Layer                                        │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │ │
│  │  │  REST (Axum) │  │  gRPC (Tonic)│  │  WebSocket   │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Middleware Pipeline                                    │ │
│  │  TLS → Rate Limit → Auth (JWT/mTLS) → RBAC → Validate  │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Domain Modules                                         │ │
│  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ │ │
│  │  │ Auth │ │Device│ │Sssion│ │ File │ │ Org  │ │Notify│ │ │
│  │  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ │ │
│  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐          │ │
│  │  │Audit │ │Update│ │Plugin│ │Metric│ │Webhk │          │ │
│  │  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘          │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Infrastructure                                         │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │ │
│  │  │  PgBouncer   │  │  Redis       │  │  NATS Client │  │ │
│  │  │  (DB pool)   │  │  (Cache)     │  │  (Events)    │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

---

## 5. Communication Patterns

### 5.1 Synchronous (Request-Response)

```
Client ──► REST API ──► Handler ──► Domain Logic ──► Database
       ◄──          ◄──         ◄──              ◄──
```

Digunakan untuk: CRUD operations, auth, device management, settings.

### 5.2 Asynchronous (Event-Driven via NATS)

```
Producer ──► NATS Subject ──► [Persisted if JetStream] ──► Consumer(s)
                               • Multiple consumers
                               • At-least-once delivery
                               • Replay capability
```

Digunakan untuk: session events, audit logs, notifications, device status changes, webhook triggers.

### 5.3 Real-time (WebSocket)

```
Client ──WebSocket──► Signal Server
                              │
                              ├──► NATS (broadcast ke semua signal nodes)
                              └──► Redis (presence/state)
```

Digunakan untuk: session signaling, presence updates, real-time notifications.

### 5.4 Media (WebRTC DataChannel + RTP)

```
Agent ──ICE/DTLS──► [P2P Direct] ──► Viewer
     └──TURN──────► [Relay] ─────────┘
      (jika P2P gagal)
```

Digunakan untuk: screen streaming, audio, input forwarding, file transfer, clipboard sync.

---

## 6. Authentication Flow

### 6.1 Device Registration Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        DEVICE REGISTRATION FLOW                         │
└─────────────────────────────────────────────────────────────────────────┘

  Agent                    API Server              CA Service          Database
    │                           │                       │                 │
    │  1. Generate Ed25519      │                       │                 │
    │     keypair (device)      │                       │                 │
    │     stored in secure      │                       │                 │
    │     keystore              │                       │                 │
    │                           │                       │                 │
    │  2. POST /api/v1/devices/register                 │                 │
    │     Body: { device_id, public_key, system_info,  │                 │
    │             install_token, signature }            │                 │
    │──────────────────────────►│                       │                 │
    │                           │  3. Validate install  │                 │
    │                           │     token             │                 │
    │                           │  4. Verify signature  │                 │
    │                           │─────────────────────►│                 │
    │                           │  5. Sign device CSR   │                 │
    │                           │◄─────────────────────│                 │
    │                           │  6. Store device +    │                 │
    │                           │     certificate       │                 │
    │                           │──────────────────────────────────────►│
    │  7. Response: {           │                       │                 │
    │     device_certificate,   │                       │                 │
    │     device_id_confirmed,  │                       │                 │
    │     server_certificate,   │                       │                 │
    │     heartbeat_interval    │                       │                 │
    │  }                        │                       │                 │
    │◄──────────────────────────│                       │                 │
    │  8. Store server cert     │                       │                 │
    │     Pin server public key │                       │                 │
```

### 6.2 User Authentication Flow (JWT + MFA)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         USER AUTH FLOW                                  │
└─────────────────────────────────────────────────────────────────────────┘

  Client (Viewer/Web)          API Server              Redis
        │                           │                    │
        │  POST /api/v1/auth/login  │                    │
        │  { email, password }      │                    │
        │──────────────────────────►│                    │
        │                           │ Argon2id verify    │
        │                           │ (password check)   │
        │                           │                    │
        │◄── 200: { mfa_required:   │                    │
        │     true, mfa_token }  ───│                    │
        │                           │                    │
        │  POST /api/v1/auth/mfa    │                    │
        │  { mfa_token, totp_code } │                    │
        │──────────────────────────►│                    │
        │                           │ Verify TOTP        │
        │                           │                    │
        │                           │ Store refresh token│
        │                           │────────────────────►
        │◄── 200: {                 │                    │
        │     access_token (JWT),   │                    │
        │     refresh_token,        │                    │
        │     expires_in: 900       │                    │
        │  }                     ───│                    │
        │                           │                    │
        │  [15 min later]           │                    │
        │  POST /api/v1/auth/refresh│                    │
        │  { refresh_token }        │                    │
        │──────────────────────────►│                    │
        │                           │ Verify refresh     │
        │                           │ token not revoked  │
        │                           │────────────────────►
        │◄── 200: { new access_token, new refresh_token}─│
```

### 6.3 Session Authentication Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      SESSION AUTH FLOW (mTLS)                           │
└─────────────────────────────────────────────────────────────────────────┘

  Viewer                  Signal Server               Agent
    │                           │                       │
    │  1. WebSocket connect     │                       │
    │     + Client TLS cert     │                       │
    │     (user identity)       │                       │
    │──────────────────────────►│                       │
    │                           │ 2. Verify viewer JWT  │
    │                           │    Verify device perm │
    │  3. Request session to    │                       │
    │     device_id: "ABC123"   │                       │
    │──────────────────────────►│                       │
    │                           │ 4. Forward request    │
    │                           │    to agent (if online)│
    │                           │──────────────────────►│
    │                           │ 5. Agent: prompt user │
    │                           │    OR auto-accept     │
    │                           │    (unattended mode)  │
    │                           │ 6. Agent: accept      │
    │                           │◄──────────────────────│
    │ 7. ICE offer forwarded    │                       │
    │◄──────────────────────────│                       │
    │ 8. SDP + ICE negotiation  │                       │
    │──────────────────────────►│──────────────────────►│
    │                           │                       │
    │ 9. DTLS-SRTP handshake    │                       │
    │   (E2E key exchange)      │                       │
    │◄──────────────────────────────────────────────────│
    │                           │                       │
    │ 10. Streaming begins      │                       │
    │◄══════════════════════════════════════════════════│
    │     (P2P or via TURN,     │                       │
    │      Signal Server tidak  │                       │
    │      bisa baca konten)    │                       │
```

---

## 7. Session Flow

### 7.1 Complete Session Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         SESSION LIFECYCLE                                       │
└─────────────────────────────────────────────────────────────────────────────────┘

  INITIATION
  ──────────
  Viewer                Signal Server                   Agent
    │                        │                             │
    │  HELLO {version,       │                             │
    │   viewer_id, token}    │                             │
    │───────────────────────►│                             │
    │  AUTH_OK {session_id}  │                             │
    │◄───────────────────────│                             │
    │  CONNECT {device_id,   │                             │
    │   quality, codecs}     │                             │
    │───────────────────────►│                             │
    │                        │  INCOMING {viewer_id,       │
    │                        │   session_id, requester}   │
    │                        │────────────────────────────►│
    │                        │                             │ [Unattended: auto-accept]
    │                        │                             │ [Attended: show prompt]
    │                        │  ACCEPT {session_id, caps}  │
    │                        │◄────────────────────────────│
    │  ACCEPTED {sdp_offer}  │                             │
    │◄───────────────────────│                             │
  NEGOTIATION
  ───────────
    │  SDP_ANSWER            │                             │
    │───────────────────────►│────────────────────────────►│
    │  ICE_CANDIDATE [...]   │                             │
    │◄──────────────────────►│◄───────────────────────────►│
    │  (DTLS-SRTP setup)     │                             │
    │◄────────────────────────────────────────────────────►│
  ACTIVE SESSION
  ──────────────
    │                        │                             │
    │◄════ SCREEN frames ═══════════════════════════════════│
    │════  KEYBOARD events ══════════════════════════════════►│
    │════  MOUSE events  ═══════════════════════════════════►│
    │◄════ AUDIO packets ═══════════════════════════════════│
    │◄══►  CLIPBOARD sync ══════════════════════════════════►│
    │◄══►  FILE transfer  ══════════════════════════════════►│
    │◄══►  CHAT messages  ══════════════════════════════════►│
    │──── PING ──────────────────────────────────────────────►│
    │◄─── PONG ─────────────────────────────────────────────│
  TERMINATION
  ───────────
    │  DISCONNECT {reason}   │                             │
    │───────────────────────►│────────────────────────────►│
    │                        │  Session stats logged       │
    │                        │  Recording finalized        │
    │  SESSION_END {stats}   │                             │
    │◄───────────────────────│                             │
```

---

## 8. Network Connection Flows

### 8.1 ICE/P2P Connection Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     P2P CONNECTION ESTABLISHMENT (ICE)                  │
└─────────────────────────────────────────────────────────────────────────┘

  Viewer (NAT A)           Signal Server           Agent (NAT B)
       │                        │                        │
       │  1. Gather ICE candidates:                     │
       │    a) Host candidates                          │
       │       (local IPs)                              │
       │    b) STUN candidates ──►STUN Server           │
       │       (public IP:port)   └──►response          │
       │    c) TURN relay cands ──►TURN Server          │
       │       (allocated relay)   └──►relay allocated  │
       │                        │                        │
       │  2. Send SDP + ICE candidates                  │
       │───────────────────────►│                        │
       │                        │ 3. Forward to agent   │
       │                        │───────────────────────►│
       │                        │ 4. Agent gathers its  │
       │                        │    ICE candidates      │
       │                        │    (same process)     │
       │                        │                        │
       │                        │◄───────────────────────│
       │◄───────────────────────│  5. SDP answer + candidates
       │                        │                        │
       │  6. ICE Connectivity Checks (STUN binding)     │
       │◄───────────────────────────────────────────────►│
       │  Multiple candidate pairs tested               │
       │                        │                        │
       │  7. Best path selected:                        │
       │     Priority: host > srflx > relay             │
       │                        │                        │
       │  8. DTLS Handshake (E2E key exchange)          │
       │◄───────────────────────────────────────────────►│
       │                        │                        │
       │◄═══════ SRTP streaming over P2P UDP ═══════════►│
```

### 8.2 Relay Fallback Flow (TURN)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                  TURN RELAY FLOW (P2P FALLBACK)                         │
└─────────────────────────────────────────────────────────────────────────┘

  Viewer           TURN Server           Agent
    │                   │                   │
    │  1. TURN Allocate │                   │
    │  { username, password (HMAC) }        │
    │──────────────────►│                   │
    │  2. Allocation    │                   │
    │◄─────────────────►│                   │
    │  relay: 1.2.3.4:5678                  │
    │                   │  3. Permission    │
    │──────────────────►│   CreatePermission│
    │                   │   for agent IP    │
    │                   │  4. ChannelBind   │
    │──────────────────►│   (efficient UDP)│
    │                   │                   │
    │                   │  [Agent side also allocates TURN relay]
    │                   │                   │
    │  5. DTLS over TURN│                   │
    │◄──────────────────────────────────────►│
    │  (E2E: TURN server tidak decrypt DTLS) │
    │                   │                   │
    │◄══ SRTP media via TURN relay ════════►│
    │  (E2E encrypted, TURN hanya forward)  │
    │                   │                   │
    │  Note: TURN refresh setiap 5 menit    │
    │──────────────────►│                   │
```

### 8.3 Reconnect Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         RECONNECT FLOW                                  │
└─────────────────────────────────────────────────────────────────────────┘

  Viewer                Signal Server               Agent
    │                        │                        │
    │  ✗ Connection lost     │                        │
    │  (timeout / network)   │                        │
    │                        │                        │
    │  1. Wait 1s (backoff 1)│                        │
    │  2. Reconnect attempt  │                        │
    │───────────────────────►│                        │
    │  ERROR: 503 (server    │                        │
    │  busy / agent gone)    │                        │
    │                        │                        │
    │  3. Wait 2s (backoff 2)│                        │
    │  4. Reconnect attempt  │                        │
    │───────────────────────►│                        │
    │                        │  5. Agent re-checked  │
    │                        │───────────────────────►│
    │                        │◄───────────────────────│
    │  6. Session resume offer│                       │
    │     { prev_session_id } │                       │
    │◄───────────────────────│                        │
    │  7. Session resumed!   │                        │
    │  (clipboard, state     │                        │
    │   preserved if < 60s)  │                        │
    │◄════════════════════════════════════════════════│
    │                        │                        │
    │  Max backoff: 30s      │                        │
    │  Max retries: 10       │                        │
    │  After max: show UI    │                        │
    │  "Connection lost,     │                        │
    │   try manual reconnect"│                        │
```

### 8.4 Heartbeat Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           HEARTBEAT SYSTEM                              │
└─────────────────────────────────────────────────────────────────────────┘

  Agent                   API Server                  Redis
    │                          │                         │
    │  [Every 30 seconds]      │                         │
    │  POST /api/v1/devices/heartbeat                   │
    │  { device_id, status, metrics }                   │
    │─────────────────────────►│                         │
    │                          │ Update presence:        │
    │                          │ SET device:{id}:online 1│
    │                          │ EX 90  (3x interval)    │
    │                          │────────────────────────►│
    │  200: { next_in: 30 }    │                         │
    │◄─────────────────────────│                         │
    │                          │                         │
    │  [If no heartbeat 90s]   │                         │
    │                          │ TTL expired →           │
    │                          │ NATS: device.offline    │
    │                          │ Notify connected viewers│
    │                          │ Log offline event       │
    │                          │                         │
  Viewer side:                 │                         │
    │  WebSocket connected      │                         │
    │  to Signal Server         │                         │
    │  [Every 25s] PING ───────────────────────────────  │
    │  PONG ◄──────────────────────────────────────────  │
    │                           │                         │
    │  [If no PONG 30s]         │                         │
    │  Reconnect attempt        │                         │
```

---

## 9. Data Flows

### 9.1 Screen Capture Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         SCREEN CAPTURE PIPELINE (AGENT)                         │
└─────────────────────────────────────────────────────────────────────────────────┘

  ┌─────────────┐  GPU Frame   ┌──────────────┐  Dirty Rect  ┌──────────────────┐
  │  OS Capture │─────────────►│  Frame Diff  │─────────────►│  Hardware Encoder│
  │             │             │  Detection   │             │                  │
  │ Win: DXGI   │             │             │             │  NVENC (NV)       │
  │  Desktop    │             │ Compare with │             │  QuickSync (Intel)│
  │  Duplication│             │  prev frame  │             │  AMF (AMD)        │
  │             │             │  using GPU   │             │  VideoToolbox(Mac)│
  │ Mac: Screen │             │  shader      │             │  x264 (software)  │
  │  CaptureKit │             │             │             │                  │
  └─────────────┘             └──────────────┘             └────────┬─────────┘
        │                                                           │
        │ Capture interval:                                         │ NAL units
        │ 60fps → 16.7ms                                            │ (H264/H265/AV1)
        │ Dynamic: reduce if                                        ▼
        │ network slow                                 ┌──────────────────────────┐
                                                       │  Packetizer              │
                                                       │                          │
                                                       │ • Split into RTP packets │
                                                       │ • Max 1200 bytes/packet  │
                                                       │ • Add sequence number    │
                                                       │ • Add timestamp          │
                                                       │ • Add E2E encryption     │
                                                       └────────────┬─────────────┘
                                                                    │
                                                                    ▼
                                                       ┌──────────────────────────┐
                                                       │  WebRTC Transport        │
                                                       │                          │
                                                       │ • SRTP (DTLS-keyed)      │
                                                       │ • RTCP feedback          │
                                                       │ • Congestion control     │
                                                       │ • Bandwidth estimation   │
                                                       └────────────┬─────────────┘
                                                                    │ UDP
                                                                    ▼
                                                           ─────────────────
                                                              P2P or TURN
                                                           ─────────────────
                                                                    │
                                                                    ▼
                                                       ┌──────────────────────────┐
                                                       │  Viewer: Jitter Buffer   │
                                                       │  Reassemble RTP → frames │
                                                       └────────────┬─────────────┘
                                                                    │
                                                                    ▼
                                                       ┌──────────────────────────┐
                                                       │  Hardware Decoder        │
                                                       │  D3D12 / Metal / VAAPI   │
                                                       └────────────┬─────────────┘
                                                                    │ YUV frames
                                                                    ▼
                                                       ┌──────────────────────────┐
                                                       │  GPU Renderer            │
                                                       │  • YUV→RGB conversion    │
                                                       │  • Display on canvas     │
                                                       │  • Cursor overlay        │
                                                       └──────────────────────────┘
```

### 9.2 File Transfer Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         FILE TRANSFER FLOW                              │
└─────────────────────────────────────────────────────────────────────────┘

  Viewer                                                Agent
    │                                                     │
    │  1. FILE_TRANSFER_INIT { files: [...], direction } │
    │──────────────────────────────────────────────────►│
    │  2. FILE_TRANSFER_ACCEPT { transfer_id, chunk_size}│
    │◄──────────────────────────────────────────────────│
    │                                                     │
    │  3. For each file:                                 │
    │     a. Compute SHA-256 checksum (parallel)        │
    │     b. Split into chunks (256KB default)          │
    │     c. Compress each chunk (Zstd)                 │
    │     d. Encrypt chunk (AES-256-GCM, session key)   │
    │                                                     │
    │  4. FILE_CHUNK { transfer_id, file_idx, chunk_idx, │
    │                  data, checksum }                  │
    │──────────────────────────────────────────────────►│ (repeated)
    │  5. FILE_CHUNK_ACK { received, write_ok }         │
    │◄──────────────────────────────────────────────────│
    │                                                     │
    │  [If NAK or timeout: retransmit chunk]            │
    │                                                     │
    │  6. FILE_COMPLETE { file_idx, total_sha256 }      │
    │──────────────────────────────────────────────────►│
    │  7. Agent verify SHA-256                           │
    │  8. FILE_VERIFY_OK or FILE_VERIFY_FAIL            │
    │◄──────────────────────────────────────────────────│
    │                                                     │
    │  [On network disconnect:]                          │
    │  Resume: send remaining chunks using chunk_idx     │
    │  (agent tracks received chunks in temp manifest)  │
```

### 9.3 Clipboard Sync Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       CLIPBOARD SYNC FLOW                               │
└─────────────────────────────────────────────────────────────────────────┘

  Viewer Side                                       Agent Side
       │                                                 │
       │  User Ctrl+C → clipboard event                 │
       │                                                 │
       │  1. Read clipboard content                      │
       │  2. Detect type: text/image/html/files         │
       │  3. If > 1MB: stream mode                      │
       │  4. Compress (Zstd)                            │
       │  5. Encrypt (AES-256-GCM)                      │
       │                                                 │
       │  CLIPBOARD { type, size, data }                │
       │──────────────────────────────────────────────►│
       │                                                 │ 6. Decrypt
       │                                                 │ 7. Decompress
       │                                                 │ 8. Set OS clipboard
       │  CLIPBOARD_ACK                                  │
       │◄──────────────────────────────────────────────│
       │                                                 │
       │  [Agent side paste: same flow, reversed]       │
       │                                                 │
       │  Policy checks:                                │
       │  • Clipboard disabled? → drop                  │
       │  • File clipboard disabled? → drop             │
       │  • Size limit exceeded? → truncate + warn      │
```

### 9.4 Audio Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           AUDIO FLOW                                    │
└─────────────────────────────────────────────────────────────────────────┘

  Agent                                               Viewer
    │                                                    │
    │  OS Audio Output (WASAPI/CoreAudio loopback)       │
    │  ┌─────────────────────────────────────────────┐  │
    │  │  PCM 48kHz, 16-bit, stereo                  │  │
    │  └──────────────┬──────────────────────────────┘  │
    │                 │                                  │
    │  ┌──────────────▼──────────────────────────────┐  │
    │  │  Opus Encoder (48kHz, 64-128 kbps)          │  │
    │  │  Frame size: 20ms (960 samples)             │  │
    │  └──────────────┬──────────────────────────────┘  │
    │                 │                                  │
    │  ┌──────────────▼──────────────────────────────┐  │
    │  │  RTP Packetizer (WebRTC audio track)         │  │
    │  └──────────────┬──────────────────────────────┘  │
    │                 │ SRTP (encrypted)                 │
    │◄════════════════╪══════════════════════════════════►
    │                 │                                  │
    │                 │                ┌─────────────────▼──────────────────────┐
    │                 │                │  Jitter Buffer (20-100ms adaptive)     │
    │                 │                └─────────────────┬──────────────────────┘
    │                 │                                  │
    │                 │                ┌─────────────────▼──────────────────────┐
    │                 │                │  Opus Decoder                           │
    │                 │                └─────────────────┬──────────────────────┘
    │                 │                                  │
    │                 │                ┌─────────────────▼──────────────────────┐
    │                 │                │  OS Audio Output (WASAPI/CoreAudio)     │
    │                 │                └────────────────────────────────────────┘
```

---

## 10. Infrastructure Architecture

### 10.1 Kubernetes Deployment

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                        KUBERNETES CLUSTER                                           │
│                                                                                     │
│  Namespace: rdp-prod                                                                │
│                                                                                     │
│  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│  │  Ingress (Nginx / Traefik)                                                   │   │
│  │  api.rdp.io → api-service                                                    │   │
│  │  signal.rdp.io → signal-service                                              │   │
│  │  app.rdp.io → web-service                                                    │   │
│  └──────────────────────┬──────────────────────────────────────────────────────┘   │
│                         │                                                           │
│  ┌──────────────────────┼──────────────────────────────────────────────────────┐   │
│  │  Application Pods    │                                                       │   │
│  │  ┌──────────────┐  ┌─┴─────────────┐  ┌─────────────┐  ┌────────────────┐  │   │
│  │  │ api-pod      │  │ signal-pod     │  │ relay-pod   │  │ web-pod         │  │   │
│  │  │ (x5 replicas)│  │ (x5 replicas)  │  │ (x10 repl.) │  │ (x3 replicas)  │  │   │
│  │  │              │  │                │  │             │  │                │  │   │
│  │  │ HPA: 2-20    │  │ HPA: 2-20      │  │ HPA: 2-50   │  │ HPA: 1-10      │  │   │
│  │  └──────────────┘  └───────────────┘  └─────────────┘  └────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                     │
│  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│  │  Stateful Services                                                           │   │
│  │  ┌──────────────────────────┐  ┌──────────────────────────────────────────┐ │   │
│  │  │  PostgreSQL (Patroni)    │  │  Redis Cluster                           │ │   │
│  │  │  Primary + 2 Replicas    │  │  3 Master + 3 Replica nodes              │ │   │
│  │  │  PgBouncer (connection   │  │  Sentinel for failover                   │ │   │
│  │  │  pool)                   │  └──────────────────────────────────────────┘ │   │
│  │  └──────────────────────────┘                                               │   │
│  │  ┌──────────────────────────────────────────────────────────────────────┐   │   │
│  │  │  NATS JetStream Cluster (3 nodes)                                     │   │   │
│  │  └──────────────────────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                     │
│  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│  │  Observability                                                               │   │
│  │  Prometheus • Grafana • Jaeger • Alertmanager • OpenTelemetry Collector     │   │
│  └─────────────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

### 10.2 Database Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        DATABASE ARCHITECTURE                            │
└─────────────────────────────────────────────────────────────────────────┘

  Application Servers
       │
       ▼
  ┌─────────────────────────────────────────────────────────────────────┐
  │  PgBouncer (Connection Pool)                                         │
  │  Transaction mode, 100 connections per app server                   │
  └──────────┬──────────────────────────────────────────────────────────┘
             │
             ├──► [Writes] ──► PostgreSQL Primary
             │                      │
             │                      ├──► Streaming Replication (sync)
             │                      │         └──► Replica 1 (hot standby)
             │                      │
             │                      └──► Streaming Replication (async)
             │                                └──► Replica 2 (analytics/reports)
             │
             └──► [Reads] ──► PostgreSQL Replica 1 / Replica 2
                               (HAProxy load balancing)

  Table Partitioning Strategy:
  ┌──────────────────────────────────────────────────────────────────┐
  │  connection_logs → partitioned by month (range)                   │
  │  audit_logs → partitioned by month (range)                        │
  │  session_recordings → partitioned by month (range)                │
  │  device_metrics → partitioned by day (range), retained 90 days    │
  │  performance_metrics → retained 30 days then archived to S3       │
  └──────────────────────────────────────────────────────────────────┘
```

### 10.3 Cache Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         REDIS CLUSTER                                   │
└─────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────┐
  │  Redis Cluster (6 nodes: 3 master + 3 replica)                       │
  │                                                                      │
  │  Cache Keys Structure:                                               │
  │  ┌────────────────────────────────────────────────────────────────┐  │
  │  │  device:{device_id}:online   → 1 (TTL: 90s, set by heartbeat) │  │
  │  │  device:{device_id}:info     → JSON (TTL: 5min)               │  │
  │  │  session:{session_id}:state  → JSON (TTL: session lifetime)   │  │
  │  │  user:{user_id}:permissions  → JSON (TTL: 5min)              │  │
  │  │  org:{org_id}:settings       → JSON (TTL: 60min)             │  │
  │  │  ratelimit:{ip}:{endpoint}   → counter (TTL: window)         │  │
  │  │  jwt:blacklist:{jti}         → 1 (TTL: token expiry)         │  │
  │  │  mfa_temp:{token}            → JSON (TTL: 5min)              │  │
  │  │  lock:{resource}             → lock_id (TTL: 30s)            │  │
  │  └────────────────────────────────────────────────────────────────┘  │
  └──────────────────────────────────────────────────────────────────────┘
```

---

## 11. Module Structure

### 11.1 Rust Workspace

```
remote-desktop-platform/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── rdp-core/                 # Core types, protocol, crypto
│   │   ├── src/
│   │   │   ├── protocol/         # Packet definitions, serialization
│   │   │   ├── crypto/           # AES-GCM, ChaCha20, Ed25519, X25519
│   │   │   ├── types/            # DeviceId, SessionId, UserId newtype wrappers
│   │   │   └── error/            # Error types
│   │   └── Cargo.toml
│   │
│   ├── rdp-agent/                # Desktop Agent binary
│   │   ├── src/
│   │   │   ├── capture/          # Screen, audio capture
│   │   │   │   ├── dxgi.rs       # Windows DXGI Desktop Duplication
│   │   │   │   ├── screencapturekit.rs  # macOS ScreenCaptureKit
│   │   │   │   └── audio.rs      # WASAPI / CoreAudio
│   │   │   ├── encoder/          # Hardware/software encoding
│   │   │   │   ├── nvenc.rs      # NVIDIA NVENC
│   │   │   │   ├── quicksync.rs  # Intel Quick Sync
│   │   │   │   ├── amf.rs        # AMD AMF
│   │   │   │   ├── videotoolbox.rs  # Apple VideoToolbox
│   │   │   │   └── software.rs   # x264/x265 software fallback
│   │   │   ├── input/            # Keyboard/mouse hook
│   │   │   ├── network/          # WebRTC client, signaling
│   │   │   ├── service/          # OS service integration
│   │   │   ├── tray/             # System tray
│   │   │   ├── update/           # Auto-update
│   │   │   ├── watchdog/         # Crash recovery
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   │
│   ├── rdp-viewer/               # Desktop Viewer (Tauri)
│   │   ├── src/
│   │   │   ├── connection/       # Connection manager
│   │   │   ├── decoder/          # Video/audio decoder
│   │   │   ├── renderer/         # GPU rendering
│   │   │   ├── input/            # Keyboard/mouse capture
│   │   │   ├── file_transfer/    # File manager
│   │   │   ├── recording/        # Session recording
│   │   │   └── main.rs
│   │   ├── frontend/             # Vue 3 + TypeScript UI
│   │   │   ├── src/
│   │   │   │   ├── components/
│   │   │   │   ├── views/
│   │   │   │   ├── stores/       # Pinia
│   │   │   │   └── composables/
│   │   │   └── package.json
│   │   └── Cargo.toml
│   │
│   ├── rdp-api/                  # API Server (Axum)
│   │   ├── src/
│   │   │   ├── routes/           # Route handlers
│   │   │   ├── middleware/       # Auth, rate limit, logging
│   │   │   ├── domain/           # Domain modules
│   │   │   │   ├── auth/
│   │   │   │   ├── devices/
│   │   │   │   ├── sessions/
│   │   │   │   ├── organizations/
│   │   │   │   ├── users/
│   │   │   │   ├── files/
│   │   │   │   ├── audit/
│   │   │   │   ├── updates/
│   │   │   │   └── webhooks/
│   │   │   ├── infrastructure/   # DB, Redis, NATS adapters
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   │
│   ├── rdp-signal/               # Signal Server (WebSocket)
│   │   ├── src/
│   │   │   ├── session/          # Session management
│   │   │   ├── presence/         # Device online/offline
│   │   │   ├── ice/              # ICE signaling
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   │
│   ├── rdp-relay/                # Relay Server
│   │   ├── src/
│   │   │   ├── relay/            # Packet forwarding
│   │   │   ├── bandwidth/        # QoS, rate limiting
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   │
│   └── rdp-proto/                # Protocol buffer definitions
│       ├── proto/
│       │   ├── session.proto
│       │   ├── device.proto
│       │   └── control.proto
│       └── Cargo.toml
│
├── web/                          # Web Dashboard
│   ├── app/                      # Laravel 13
│   │   ├── Http/
│   │   ├── Models/
│   │   └── Services/
│   ├── resources/
│   │   └── js/                   # Vue 3 + Inertia + TypeScript
│   │       ├── Components/
│   │       ├── Pages/
│   │       ├── Stores/           # Pinia
│   │       └── Types/
│   └── ...
│
├── infra/                        # Infrastructure as Code
│   ├── kubernetes/               # Helm charts
│   ├── docker/                   # Dockerfiles
│   └── terraform/                # Cloud infrastructure
│
└── docs/                         # Documentation (this folder)
```

---

## 12. Error Handling Strategy

### 12.1 Error Taxonomy

```rust
// rdp-core/src/error/mod.rs

pub enum RdpError {
    // Network errors
    NetworkTimeout,
    ConnectionRefused,
    NatTraversalFailed,
    TurnAllocationFailed,

    // Auth errors
    InvalidToken,
    TokenExpired,
    DeviceCertInvalid,
    PermissionDenied,
    MfaRequired,

    // Session errors
    SessionNotFound,
    SessionExpired,
    DeviceOffline,
    AgentBusy,

    // Protocol errors
    ProtocolVersionMismatch,
    InvalidPacket,
    DecryptionFailed,
    ReplayDetected,

    // Resource errors
    EncoderInitFailed,
    CapturePermissionDenied,
    InsufficientBandwidth,

    // Infrastructure
    DatabaseError(sqlx::Error),
    CacheError(redis::RedisError),
    MessagingError(async_nats::Error),
}
```

### 12.2 Error Propagation

- Agent dan Viewer menggunakan `color-eyre` untuk rich error context
- API Server menggunakan `thiserror` untuk typed errors → HTTP status mapping
- All errors dilog dengan `tracing` (structured), dikirim ke OpenTelemetry

---

## 13. Configuration Management

### 13.1 Configuration Hierarchy

```
Priority (tinggi ke rendah):
1. Environment variables            → RDP_DB_URL=...
2. Config file (TOML)              → /etc/rdp/config.toml
3. Default values dalam binary     → compiled defaults
```

### 13.2 Config Structure

```toml
# /etc/rdp-api/config.toml

[server]
bind = "0.0.0.0:8080"
tls_cert = "/certs/server.crt"
tls_key = "/certs/server.key"

[database]
url = "${RDP_DB_URL}"           # env var interpolation
max_connections = 20
min_connections = 5

[redis]
url = "${RDP_REDIS_URL}"
pool_size = 20

[nats]
url = "${RDP_NATS_URL}"

[auth]
jwt_secret = "${RDP_JWT_SECRET}"
access_token_ttl_seconds = 900
refresh_token_ttl_seconds = 604800

[security]
device_cert_validity_days = 365
tls_min_version = "1.3"

[features]
session_recording = true
file_transfer_max_size_mb = 10240
```

---

*Dokumen ini merupakan blueprint arsitektur lengkap. Setiap perubahan arsitektur besar harus didokumentasikan sebagai ADR baru.*
