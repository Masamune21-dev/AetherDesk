# System Design Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** System Architect

---

## 1. Modular Monolith Design & Evolution Path

Sistem dirancang sebagai **Modular Monolith** menggunakan Rust workspace. Setiap modul dibatasi secara ketat menggunakan domain boundaries dan hanya berkomunikasi via NATS JetStream (async) atau internal API interfaces (sync). Hal ini memungkinkan ekstraksi modul menjadi microservices secara instan tanpa refactor besar.

### Domain Boundaries

```
┌────────────────────────────────────────────────────────────────────────┐
│                        MODULAR MONOLITH (Rust)                         │
│                                                                        │
│   ┌──────────────┐      ┌──────────────┐      ┌──────────────┐         │
│   │  Auth Mod    │      │  Device Mod  │      │  Session Mod │         │
│   │  (mTLS/JWT)  │      │  (Inventory) │      │  (Signaling) │         │
│   └──────┬───────┘      └──────┬───────┘      └──────┬───────┘         │
│          │                     │                     │                 │
│  ────────┼─────────────────────┼─────────────────────┼───────────────  │
│          │                     │                     │                 │
│          ▼                     ▼                     ▼                 │
│   ┌──────────────────────────────────────────────────────────┐         │
│   │                NATS JetStream (Message Bus)              │         │
│   └────────────────────────────┬─────────────────────────────┘         │
│                                │                                       │
│                                ▼                                       │
│   ┌──────────────────────────────────────────────────────────┐         │
│   │                 Shared Database (PostgreSQL)             │         │
│   │            (Schema partitioned by module prefix)         │         │
│   └──────────────────────────────────────────────────────────┘         │
└────────────────────────────────────────────────────────────────────────┘
```

#### Evolusi ke Microservices

Saat beban (traffic) meningkat:
1. **Signal Mod** diekstrak menjadi `rdp-signal` microservice (heavy WebSocket connections).
2. **Relay Mod** diekstrak menjadi `rdp-relay` microservice (heavy network I/O).
3. **Billing/Org Mod** diekstrak menjadi service tersendiri jika diintegrasikan dengan sistem enterprise lain.

---

## 2. Distributed State & Concurrency

### Kunci Konsistensi Data
- **Redis Cluster** bertindak sebagai *source of truth* untuk real-time state (perangkat online/offline, statistik sesi aktif, rate limit, dynamic lock).
- **PostgreSQL HA** menyimpan metadata persisten (users, organizations, audit logs, licenses, configuration).

### Penanganan Concurrency
1. **Distributed Lock (Redis Redlock)**: Digunakan saat meregistrasi agent baru atau membuat sesi untuk mencegah *race condition*.
2. **Tokio Async Runtime**: Digunakan pada agent dan backend untuk menangani ribuan tugas concurrent I/O (network packets, file chunks) secara efisien tanpa thread blocking.
3. **Database Optimistic Concurrency Control (OCC)**: Menggunakan kolom `version` (integer) pada tabel kritis seperti `devices` dan `settings` untuk mendeteksi konflik update data.

---

## 3. High Availability & Disaster Recovery

### Strategi Pemulihan (Failover)

| Komponen | Kegagalan Skenario | Mekanisme Failover | Target Pemulihan (RTO) |
|---|---|---|---|
| **API/Signal Server** | Node K8s mati | Ingress secara otomatis me-route traffic ke replika pods sehat lain. Pod baru di-schedule. | < 5 detik |
| **Relay Server** | Relay server overload/mati | Client (agent/viewer) mendeteksi timeout, meminta relay server baru dari API, dan me-reconnect sesi (resume session). | < 2 detik |
| **PostgreSQL Primary** | DB utama crash | Patroni mempromosikan Replica 1 menjadi Primary baru. PgBouncer me-route ulang query. | < 10 detik (RTO), RPO = 0 |
| **Redis Master** | Redis master crash | Redis Sentinel mempromosikan Redis Replica menjadi Master baru. | < 3 detik |
| **NATS Message Bus** | Node NATS mati | RAFT consensus di NATS JetStream menunjuk leader baru untuk stream. | < 1 detik |

---

## 4. Cache & Database Optimization

### Skema Caching Redis

| Tipe Data | Format Kunci (Key) | Struktur Data Redis | Kebijakan TTL |
|---|---|---|---|
| **Presence Status** | `device:{id}:online` | String `1` | 90 detik (heartbeat interval 30s) |
| **User Session** | `user:session:{token_jti}` | String `user_id` | Mengikuti masa kadaluarsa JWT |
| **Device Info** | `device:{id}:metadata` | Hash | 5 menit |
| **Organization Policy** | `org:{id}:policies` | Hash | 30 menit |
| **Rate Limit Window** | `rate:{ip}:{endpoint}` | String (counter) | 1 menit |

### Optimasi Query Database
- **Index**: Index B-Tree pada kolom pencarian (`devices.device_id`, `users.email`, `sessions.session_id`).
- **Partitioning Table**: Tabel `audit_logs` dan `connection_logs` dipartisi secara bulanan berdasarkan range waktu.
- **Connection Pooling**: PgBouncer dipasang secara lokal pada setiap node app server dalam mode `transaction pooling` untuk meminimalkan handshake overhead.

---

## 5. Structured Logging & Distributed Tracing

### Format Structured Logging (JSON)

Semua komponen sistem menghasilkan log terstruktur yang mudah diparsing oleh mesin analitik.

```json
{
  "timestamp": "2026-08-07T14:32:01.002Z",
  "level": "INFO",
  "service": "rdp-api",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "span_id": "00f067aa0ba902b7",
  "message": "Session created successfully",
  "context": {
    "session_id": "sess_89f02a",
    "viewer_id": "usr_7812",
    "device_id": "dev_3290",
    "connection_type": "P2P",
    "client_ip": "203.0.113.195"
  }
}
```

### Distributed Tracing dengan OpenTelemetry & Jaeger

Tracing diimplementasikan pada setiap request lifecycle:
1. **Viewer/Dashboard** men-generate `traceparent` (W3C standard header).
2. **API Server** menangkap header, membuat span baru, dan menyebarkannya via HTTP headers ke service internal.
3. **NATS JetStream** menyebarkan trace context pada metadata pesan untuk melacak operasi async.
4. **Jaeger Collector** mengumpulkan data trace dari seluruh sistem untuk visualisasi performa.

```
[Web Dashboard] ───► [API Server] ───► [NATS Message] ───► [Worker Service]
    │                     │                                     │
    └─────────────────────┴───────► [Jaeger] ◄──────────────────┘
```

---

*Desain ini menjamin sistem Remote Desktop Platform dapat beroperasi dengan keandalan tinggi, skalabilitas linier, dan kepatuhan penuh terhadap standar audit enterprise.*
