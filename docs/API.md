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

Base URL: `https://<host>/api/v1`

> **Diselaraskan dengan implementasi (2026-08-09).** Dokumen ini sebelumnya
> memakai `/v1` pada satu bagian dan `/api/v1` pada bagian lain (temuan R-05).
> Bentuk yang benar dan berlaku adalah **`/api/v1`** — nginx meneruskan URI apa
> adanya tanpa memotong prefiks.
>
> Endpoint kesehatan sengaja **tidak** diversikan karena sifatnya operasional
> dan harus tetap stabil melewati pergantian versi: `/api/health` dan
> `/api/health/ready`.

---

## 2. Autentikasi API

Semua request harus menyertakan Bearer token JWT di header `Authorization`.

```
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJFZERTQSJ9...
```

Header token yang benar-benar diterbitkan adalah `{"typ":"JWT","alg":"EdDSA"}`.
Contoh lama pada dokumen ini memakai `RS256`, dan itu keliru — lihat ADR-008.

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
| Quick Connect per device ID | 5 gagal | jeda 15 menit |
| Quick Connect per IP (ID tak dikenal) | 10 | blokir 24 jam |

Pembatasan Quick Connect diterapkan **per device ID**, bukan per IP penyerang:
penyerang berpindah IP dengan biaya nyaris nol, sementara device ID yang
diserang tetap sama. Pemeriksaan jeda dipisahkan dari pencatatan kegagalan agar
percobaan yang sudah dijeda tidak memperpanjang jedanya sendiri — bila digabung,
penyerang dapat mengunci pemilik perangkat selamanya.

Response header rate limit:

```
X-RateLimit-Limit: 300
X-RateLimit-Remaining: 298
X-RateLimit-Reset: 1691395200
```

---

## 6. Endpoint REST API

### 6.1 Authentication

| Method | Path | Status | Deskripsi |
|---|---|---|---|
| POST | `/auth/bootstrap` | **terpasang** | Membuat organisasi pertama; menolak selamanya setelah ada satu organisasi |
| POST | `/auth/login` | **terpasang** | Login dengan `org_slug` + email + password |
| POST | `/auth/refresh` | **terpasang** | Menukar refresh token dengan pasangan baru (rotasi sekali pakai) |
| POST | `/auth/logout` | **terpasang** | Mencabut refresh token |
| GET | `/auth/me` | **terpasang** | Profil pengguna dan organisasinya |
| POST | `/auth/mfa` | belum | Verifikasi MFA (TOTP) |
| POST | `/auth/sso/saml` | belum | SAML SSO callback |
| POST | `/auth/sso/oidc` | belum | OIDC SSO callback |

#### `org_slug` wajib pada login

```json
POST /api/v1/auth/login
{ "org_slug": "contoh-teknologi", "email": "erik@msp.id", "password": "..." }
```

Ini konsekuensi langsung perbaikan **T-05**. Email hanya unik **per
organisasi**, bukan global — tanpa itu, satu orang tidak dapat menjadi anggota
dua organisasi, dan skenario MSP di UC-03 mustahil. Akibatnya
`email + password` saja tidak lagi menunjuk ke satu orang: dua organisasi boleh
sama-sama memiliki `erik@msp.id`.

#### Respons login

```json
{
  "data": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJFZERTQSJ9...",
    "refresh_token": "82cd98699ddb5027...",
    "token_type": "Bearer",
    "expires_in": 900
  }
}
```

Access token ditandatangani **EdDSA (Ed25519)**, bukan `RS256` seperti contoh
lama di dokumen ini, dan bukan HMAC seperti yang disiratkan `jwt_secret` pada
konfigurasi. Lihat ADR-008: kunci privat hanya ada di API Server, sementara
Signal Server cukup memegang kunci publik.

Refresh token berlaku 7 hari dan **berotasi sekali pakai** — token lama langsung
dihapus saat ditukar, sehingga pemakaian ulang terdeteksi.

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

| Method | Path | Status | Deskripsi |
|---|---|---|---|
| POST | `/devices` | **terpasang** | Mendaftarkan perangkat; mengembalikan Device ID dan kata sandi sesi |
| GET | `/devices` | **terpasang** | Daftar perangkat organisasi aktif |
| POST | `/devices/{uuid}/rotate-password` | **terpasang** | Membangkitkan ulang kata sandi sesi |
| GET | `/devices/{id}` | belum | Detail perangkat beserta metrik |
| PATCH | `/devices/{id}` | belum | Mengubah metadata |
| DELETE | `/devices/{id}` | belum | Menghapus pendaftaran |
| POST | `/devices/{id}/wake` | belum | Wake-on-LAN |
| POST | `/devices/{id}/reboot` | belum | Reboot jarak jauh |
| POST | `/devices/{id}/command` | belum | Eksekusi perintah — lihat peringatan S-12 |
| GET | `/devices/{id}/inventory` | belum | Inventaris hardware dan software |

Respons pendaftaran memuat kata sandi sesi dalam bentuk asli. **Ini
satu-satunya kali** nilai itu dikirim; setelahnya hanya hash Argon2id yang
tersimpan.

### 6.3.1 Quick Connect

| Method | Path | Status |
|---|---|---|
| POST | `/connect` | **terpasang** |

```json
POST /api/v1/connect
{ "device_id": "942716382", "password": "3W65EMBJ" }
```

Perilakunya dirancang di [QUICK_CONNECT.md](./QUICK_CONNECT.md) dan mengikat:

- Check digit divalidasi **sebelum** menyentuh database
- Seluruh sebab kegagalan menghasilkan respons yang **identik**, karena
  membedakannya memberi tahu penyerang device ID mana yang hidup
- Lama respons dinormalkan ke lantai tetap 250 ms
- Pembatasan laju **per device ID**, bukan per IP penyerang

Keberhasilan mengembalikan `status: "pending_approval"`. Kata sandi yang benar
memberi hak **meminta** koneksi, bukan mendapatkannya — agent tetap harus
menampilkan prompt persetujuan.

### 6.3.2 Kredensial TURN

| Method | Path | Status |
|---|---|---|
| GET | `/turn-credentials` | **terpasang** |

Mengembalikan daftar ICE server beserta pasangan HMAC berumur 6 jam. Rahasia
bersama tidak pernah meninggalkan server. Lihat DEPLOYMENT_PLAN.md §7.1.

### 6.4 Sessions

| Method | Path | Status | Deskripsi |
|---|---|---|---|
| GET | `/sessions` | **terpasang** | Riwayat sesi organisasi aktif |
| GET | `/audit-logs` | **terpasang** | Jejak audit organisasi aktif |
| GET | `/sessions/{id}` | belum | Detail satu sesi |
| POST | `/sessions/{id}/terminate` | belum | Mengakhiri sesi dari sisi server |
| GET | `/sessions/{id}/recording` | belum | URL rekaman sesi |

Pembuatan sesi terjadi lewat `POST /connect`, bukan `POST /sessions`.

Status sesi berpindah `pending` → `active` → `terminated`/`disconnected`.
Perpindahannya digerakkan Signal Server, karena ia satu-satunya komponen yang
tahu kapan sesi benar-benar disetujui, ditolak, atau putus. Sesi yang pernah
aktif berakhir sebagai `terminated`; yang tidak pernah disetujui berakhir
sebagai `disconnected` — pembedaan itu yang membuat riwayat dapat menjawab
berapa permintaan yang sungguh tersambung.

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
