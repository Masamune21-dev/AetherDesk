# API Design Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** Backend Architect

---

## 1. Ikhtisar API

Platform mengekspos tiga jenis API:

| Tipe API | Protokol | Use Case |
|---|---|---|
| **REST API** | HTTPS (JSON) | CRUD operations, dashboard, integrasi pihak ketiga |
| **gRPC API** | HTTP/2 + Protobuf | Komunikasi inter-service, high-perf agent-to-server |
| **WebSocket API** | WSS | Signaling real-time, presence, notifikasi |

Base URL: `https://api.rdp.io/v1`

---

## 2. Autentikasi API

Semua request harus menyertakan Bearer token JWT di header `Authorization`.

```
Authorization: Bearer eyJhbGciOiJSUzI1NiIs...
```

Untuk API Machine-to-Machine (M2M), gunakan API Token yang di-generate dari Web Dashboard:

```
Authorization: Bearer rdp_api_xxxxxxxxxxxxxxxxxxxxxxxx
```

---

## 3. Format Response Standar

### Success Response

```json
{
  "data": { ... },
  "meta": {
    "request_id": "req_7f2a9b3c",
    "timestamp": "2026-08-07T10:00:00Z"
  }
}
```

### Error Response

```json
{
  "error": {
    "code": "DEVICE_NOT_FOUND",
    "message": "Device with ID dev_xyz not found",
    "details": [],
    "request_id": "req_7f2a9b3c"
  }
}
```

### Paginated Response

```json
{
  "data": [ ... ],
  "meta": {
    "page": 1,
    "per_page": 25,
    "total": 150,
    "total_pages": 6
  }
}
```

---

## 4. Kode Error HTTP

| Kode | Nama | Deskripsi |
|---|---|---|
| 200 | OK | Sukses |
| 201 | Created | Resource berhasil dibuat |
| 204 | No Content | Sukses tanpa response body |
| 400 | Bad Request | Input tidak valid |
| 401 | Unauthorized | Token tidak ada atau expired |
| 403 | Forbidden | Tidak memiliki izin |
| 404 | Not Found | Resource tidak ditemukan |
| 409 | Conflict | Konflik data (duplikat) |
| 422 | Unprocessable Entity | Validasi gagal |
| 429 | Too Many Requests | Rate limit terlampaui |
| 500 | Internal Server Error | Error internal |
| 503 | Service Unavailable | Service sedang maintenance |

---

## 5. Rate Limiting

Rate limit menggunakan sliding window algorithm, disimpan di Redis.

| Scope | Limit | Window |
|---|---|---|
| Per IP (unauthenticated) | 60 requests | 1 menit |
| Per User (authenticated) | 300 requests | 1 menit |
| Per API Token | 1000 requests | 1 menit |
| Login endpoint | 5 requests | 5 menit |

Response header rate limit:

```
X-RateLimit-Limit: 300
X-RateLimit-Remaining: 298
X-RateLimit-Reset: 1691395200
```

---

## 6. Endpoint REST API

### 6.1 Authentication

| Method | Path | Deskripsi |
|---|---|---|
| POST | `/auth/login` | Login dengan email + password |
| POST | `/auth/mfa` | Verifikasi MFA (TOTP) |
| POST | `/auth/refresh` | Refresh access token |
| POST | `/auth/logout` | Logout (revoke tokens) |
| POST | `/auth/sso/saml` | SAML SSO callback |
| POST | `/auth/sso/oidc` | OIDC SSO callback |

### 6.2 Users

| Method | Path | Deskripsi |
|---|---|---|
| GET | `/users` | List users (paginated, filterable) |
| GET | `/users/{id}` | Get user detail |
| POST | `/users` | Create user |
| PATCH | `/users/{id}` | Update user |
| DELETE | `/users/{id}` | Delete user |
| POST | `/users/{id}/mfa/enable` | Enable MFA |
| POST | `/users/{id}/mfa/disable` | Disable MFA |

### 6.3 Devices

| Method | Path | Deskripsi |
|---|---|---|
| GET | `/devices` | List devices (filterable: status, group, os_type) |
| GET | `/devices/{id}` | Get device detail + metrics |
| POST | `/devices/register` | Register new device (agent) |
| PATCH | `/devices/{id}` | Update device metadata |
| DELETE | `/devices/{id}` | Unregister device |
| POST | `/devices/{id}/wake` | Send Wake-on-LAN |
| POST | `/devices/{id}/reboot` | Remote reboot |
| POST | `/devices/{id}/command` | Execute remote command |
| GET | `/devices/{id}/inventory` | Get hardware/software inventory |
| GET | `/devices/{id}/metrics` | Get device performance metrics |

### 6.4 Sessions

| Method | Path | Deskripsi |
|---|---|---|
| GET | `/sessions` | List sessions (active/history) |
| GET | `/sessions/{id}` | Get session detail |
| POST | `/sessions` | Create session (initiate connection) |
| POST | `/sessions/{id}/terminate` | Terminate session |
| GET | `/sessions/{id}/recording` | Get session recording URL |

### 6.5 Organizations

| Method | Path | Deskripsi |
|---|---|---|
| GET | `/organizations/{id}` | Get organization detail |
| PATCH | `/organizations/{id}` | Update organization |
| GET | `/organizations/{id}/groups` | List device groups |
| POST | `/organizations/{id}/groups` | Create device group |
| GET | `/organizations/{id}/members` | List organization members |

### 6.6 Audit Logs

| Method | Path | Deskripsi |
|---|---|---|
| GET | `/audit-logs` | List audit logs (filterable, paginated) |
| GET | `/audit-logs/export` | Export audit logs (CSV) |

### 6.7 Webhooks

| Method | Path | Deskripsi |
|---|---|---|
| GET | `/webhooks` | List webhooks |
| POST | `/webhooks` | Create webhook |
| PATCH | `/webhooks/{id}` | Update webhook |
| DELETE | `/webhooks/{id}` | Delete webhook |
| POST | `/webhooks/{id}/test` | Test webhook delivery |

---

## 7. Filtering & Sorting

Query parameters standar:

```
GET /api/v1/devices?status=online&os_type=Windows&sort=-last_heartbeat&page=2&per_page=50
```

| Parameter | Deskripsi |
|---|---|
| `sort` | Field untuk sorting. Prefix `-` untuk descending |
| `page` | Nomor halaman (default: 1) |
| `per_page` | Jumlah item per halaman (default: 25, max: 100) |
| `search` | Full-text search pada field yang relevan |
| `filter[field]` | Filter berdasarkan field spesifik |

---

## 8. API Versioning

API menggunakan URL path versioning: `/v1/`, `/v2/`.

Kebijakan:
- Versi baru (`v2`) dirilis saat ada breaking changes.
- Versi lama (`v1`) didukung minimal 12 bulan setelah `v2` rilis.
- Header `Sunset` dikirimkan 6 bulan sebelum versi didepresiasi.

---

## 9. WebSocket API (Signaling)

Endpoint: `wss://signal.rdp.io/v1/ws`

### Format Pesan

```json
{
  "type": "SESSION_REQUEST",
  "payload": { "device_id": "dev_abc123" },
  "request_id": "msg_001",
  "timestamp": 1691395200
}
```

### Tipe Pesan

| Type | Direction | Deskripsi |
|---|---|---|
| `AUTH` | Client → Server | Autentikasi WebSocket |
| `AUTH_OK` | Server → Client | Autentikasi berhasil |
| `SESSION_REQUEST` | Viewer → Server | Minta koneksi ke device |
| `SESSION_OFFER` | Server → Agent | Forward permintaan ke agent |
| `SESSION_ACCEPT` | Agent → Server | Agent menerima koneksi |
| `SESSION_REJECT` | Agent → Server | Agent menolak koneksi |
| `SDP_OFFER` | Agent → Viewer | WebRTC SDP offer |
| `SDP_ANSWER` | Viewer → Agent | WebRTC SDP answer |
| `ICE_CANDIDATE` | Bidirectional | ICE candidate exchange |
| `SESSION_END` | Bidirectional | Sesi berakhir |
| `DEVICE_STATUS` | Server → Client | Device online/offline |
| `PING` / `PONG` | Bidirectional | Keep-alive |

---

## 10. gRPC API

Digunakan untuk komunikasi internal antar service dan koneksi agent-to-server yang membutuhkan performa tinggi.

```protobuf
syntax = "proto3";
package rdp.v1;

service DeviceService {
  rpc Register(RegisterRequest) returns (RegisterResponse);
  rpc Heartbeat(HeartbeatRequest) returns (HeartbeatResponse);
  rpc StreamMetrics(stream MetricsData) returns (MetricsAck);
}

service SessionService {
  rpc Create(CreateSessionRequest) returns (CreateSessionResponse);
  rpc Terminate(TerminateRequest) returns (TerminateResponse);
  rpc StreamEvents(stream SessionEvent) returns (stream SessionEvent);
}
```

---

## 11. OpenAPI / Swagger

Dokumentasi OpenAPI 3.1 tersedia di:
- **Swagger UI**: `https://api.rdp.io/docs`
- **OpenAPI JSON**: `https://api.rdp.io/openapi.json`
- **Redoc**: `https://api.rdp.io/redoc`

Spesifikasi di-generate otomatis dari kode Rust menggunakan crate `utoipa` dan di-validasi di CI pipeline.
