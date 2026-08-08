# Web Dashboard Design Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** UI/UX Architect / Full-Stack Engineer

---

## 1. Technology Stack

| Layer | Technology |
|---|---|
| Backend Framework | Laravel 13 (PHP 8.4) |
| Frontend Framework | Vue 3 (Composition API) |
| Bridge | Inertia.js |
| Language | TypeScript (strict mode) |
| State Management | Pinia |
| UI Components | Headless UI + Tailwind CSS |
| Charts | Chart.js / ECharts |
| Real-time | Laravel Echo + WebSocket (Soketi/Reverb) |
| Build | Vite |

---

## 2. Halaman Dashboard

### 2.1 Device Management

- **Device List**: Tabel dengan filter (status, OS, group, tag), search, bulk actions.
- **Device Detail**: Info hardware, software inventory, connection history, metrics.
- **Device Groups**: CRUD grup perangkat, drag-and-drop assignment.
- **Device Map**: Geo-map lokasi perangkat (berdasarkan IP geolocation).

### 2.2 User & Access Management

- **User List**: CRUD users, role assignment, MFA status.
- **Role Management**: Custom roles, permission matrix editor.
- **SSO Configuration**: SAML/OIDC provider setup wizard.
- **SCIM Settings**: SCIM endpoint configuration.
- **API Tokens**: Generate/revoke API tokens per user.

### 2.3 Organization & Tenant

- **Organization Settings**: Nama, logo, billing, license.
- **Team Management**: Sub-teams dalam organisasi.
- **Policy Editor**: Kebijakan akses (clipboard policy, file transfer policy, recording policy).
- **Branding**: Custom logo, colors untuk white-label.

### 2.4 Session Management

- **Active Sessions**: Real-time list sesi aktif, terminate button.
- **Session History**: Riwayat semua sesi, filter by date/user/device.
- **Session Recording Playback**: Video player untuk session recordings.
- **Connection Logs**: Detail teknis setiap koneksi (latency, codec, bandwidth, duration).

### 2.5 Analytics & Monitoring

- **Overview Dashboard**: Jumlah perangkat online, sesi aktif hari ini, total bandwidth.
- **Performance Charts**: Latency trend, bandwidth usage, FPS distribution.
- **Device Health**: CPU/RAM/disk usage per device (from agent metrics).
- **Bandwidth Analytics**: Top consumers, daily/weekly/monthly trend.
- **Connection Success Rate**: Persentase koneksi berhasil vs gagal.

### 2.6 Audit & Compliance

- **Audit Log Viewer**: Searchable, filterable audit trail.
- **Audit Export**: Export ke CSV/PDF untuk auditor.
- **Compliance Dashboard**: Status checklist SOC2/GDPR.

### 2.7 System Configuration

- **STUN/TURN Servers**: Manage relay server list.
- **Update Management**: Set update channel per group, view update status.
- **Webhook Configuration**: Manage webhook endpoints dan events.
- **Notification Settings**: Email, Slack, webhook notification rules.

---

## 3. Real-time Features

Dashboard menggunakan WebSocket (Laravel Echo) untuk data real-time:

- Device online/offline status changes.
- Active session count updates.
- Notification alerts (security events, update available).
- Device health metric updates (jika device detail page terbuka).

---

## 4. Integrasi dengan Rust Backend

Web Dashboard berkomunikasi dengan Rust API Server melalui REST API:

```
┌──────────────┐     Inertia      ┌──────────────┐     REST/WS     ┌──────────────┐
│  Vue 3 SPA   │ ◄──────────────► │  Laravel 13  │ ◄─────────────► │  Rust API    │
│  (Frontend)  │                  │  (Web BFF)   │                 │  Server      │
└──────────────┘                  └──────────────┘                 └──────────────┘
```

Laravel bertindak sebagai BFF (Backend for Frontend):
- Handle SSO/session cookies untuk web.
- Proxy API calls ke Rust backend dengan server-side JWT.
- Server-side rendering (SSR) via Inertia untuk SEO-critical pages.
- Caching responses di Laravel Redis cache.
