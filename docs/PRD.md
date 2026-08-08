# Product Requirements Document (PRD)

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** Principal Software Architect

---

## Daftar Isi

1. [Vision](#1-vision)
2. [Mission](#2-mission)
3. [Target Market](#3-target-market)
4. [User Persona](#4-user-persona)
5. [Business Goals](#5-business-goals)
6. [Success Metrics](#6-success-metrics)
7. [Competitor Analysis](#7-competitor-analysis)
8. [Use Cases](#8-use-cases)
9. [Functional Requirements](#9-functional-requirements)
10. [Non-Functional Requirements](#10-non-functional-requirements)
11. [Performance Requirements](#11-performance-requirements)
12. [Security Requirements](#12-security-requirements)
13. [Scalability Requirements](#13-scalability-requirements)
14. [Accessibility](#14-accessibility)
15. [Internationalization](#15-internationalization)
16. [Offline Mode](#16-offline-mode)
17. [Future Expansion](#17-future-expansion)
18. [Milestone](#18-milestone)
19. [Roadmap](#19-roadmap)

---

## 1. Vision

Menjadi platform remote desktop enterprise paling aman, performa tertinggi, dan paling modern di dunia — dibangun dengan arsitektur zero-trust, end-to-end encryption, dan teknologi Rust untuk performa mendekati native.

### Pilar Vision

| Pilar | Deskripsi |
|-------|-----------|
| **Security First** | Zero-trust architecture, E2E encryption di mana relay server tidak dapat mendekripsi konten sesi |
| **Performance** | Sub-100ms latency WAN, sub-16ms latency LAN, 60fps rendering dengan hardware acceleration |
| **Enterprise Ready** | Multi-tenant, SSO, RBAC/ABAC, LDAP/AD, SCIM, audit logging, SOC2 compliance |
| **Cross Platform** | Windows, macOS sebagai target awal; Linux, Android, iOS sebagai ekspansi |
| **Open Architecture** | Plugin SDK, API-first design, webhook, extensible protocol |

---

## 2. Mission

Memberikan solusi remote desktop yang:

1. **Lebih cepat** dari TeamViewer dan AnyDesk — target latency end-to-end < 50ms pada LAN, < 100ms pada WAN
2. **Lebih aman** dari semua kompetitor — zero-trust, mutual TLS, device certificates, E2E encryption
3. **Lebih ringan** dari RustDesk — penggunaan CPU < 5% saat idle, < 15% saat streaming pada hardware modern
4. **Lebih enterprise** dari semua alternatif open-source — multi-tenant, SCIM provisioning, session recording, compliance-ready
5. **Lebih extensible** — Plugin SDK, Script Automation, REST/gRPC/WebSocket API

---

## 3. Target Market

### 3.1 Segmen Primer

| Segmen | Ukuran Pasar | Prioritas |
|--------|-------------|-----------|
| Managed Service Providers (MSP) | $50B+ global IT services market | P0 |
| Enterprise IT Departments | Perusahaan dengan 500+ karyawan | P0 |
| Small-Medium Business (SMB) | Perusahaan 10-500 karyawan | P1 |
| Educational Institutions | Universitas, sekolah, lab komputer | P1 |

### 3.2 Segmen Sekunder

| Segmen | Ukuran Pasar | Prioritas |
|--------|-------------|-----------|
| Healthcare | Rumah sakit, klinik, telemedicine | P2 |
| Government | Instansi pemerintah, militer | P2 |
| Financial Services | Bank, asuransi, fintech | P2 |
| Remote Workers / Freelancers | Individu dengan kebutuhan akses jarak jauh | P3 |

### 3.3 Geografi Target

- **Fase 1:** Asia Tenggara (Indonesia, Malaysia, Singapura, Thailand)
- **Fase 2:** Asia Pasifik (Jepang, Korea, Australia, India)
- **Fase 3:** Eropa & Amerika Utara
- **Fase 4:** Global

---

## 4. User Persona

### 4.1 IT Administrator — "Andi"

| Atribut | Detail |
|---------|--------|
| **Usia** | 28-45 tahun |
| **Peran** | System Administrator / IT Manager |
| **Organisasi** | Perusahaan 200-2000 karyawan |
| **Goals** | Mengelola 500+ perangkat, remote troubleshooting cepat, unattended access, deployment massal |
| **Pain Points** | TeamViewer mahal untuk skala besar, AnyDesk sering terputus, RustDesk kurang fitur enterprise |
| **Needs** | Multi-tenant, LDAP integration, bulk deployment, session recording, audit trail |
| **Tech Savviness** | Tinggi |
| **Frequency** | Harian, 4-8 jam/hari |

### 4.2 Help Desk Technician — "Budi"

| Atribut | Detail |
|---------|--------|
| **Usia** | 22-35 tahun |
| **Peran** | L1/L2 Support Technician |
| **Organisasi** | MSP yang mengelola 50+ klien |
| **Goals** | Resolve tiket cepat, remote ke perangkat klien tanpa instalasi kompleks, file transfer cepat |
| **Pain Points** | Banyak tool berbeda per klien, session recording manual, sulit handover sesi ke L2/L3 |
| **Needs** | Technician mode, session transfer, chat, clipboard sync, remote terminal, quick connect |
| **Tech Savviness** | Menengah-Tinggi |
| **Frequency** | Harian, 6-10 jam/hari |

### 4.3 End User — "Citra"

| Atribut | Detail |
|---------|--------|
| **Usia** | 25-55 tahun |
| **Peran** | Karyawan yang membutuhkan bantuan IT |
| **Organisasi** | Berbagai ukuran |
| **Goals** | Mendapatkan bantuan IT cepat, privasi terjaga, kontrol atas sesi remote |
| **Pain Points** | Takut privasi dilanggar, tidak mengerti cara install, proses persetujuan membingungkan |
| **Needs** | Permission prompt jelas, privacy mode, session end button, simple UI, one-click connect |
| **Tech Savviness** | Rendah-Menengah |
| **Frequency** | Bulanan, 1-2x/bulan |

### 4.4 Enterprise Security Officer — "Diana"

| Atribut | Detail |
|---------|--------|
| **Usia** | 30-50 tahun |
| **Peran** | CISO / Security Analyst |
| **Organisasi** | Enterprise 1000+ karyawan |
| **Goals** | Compliance, audit trail lengkap, zero-trust enforcement, kontrol akses granular |
| **Pain Points** | Remote desktop tools sering menjadi vektor serangan, kurang visibility, sulit audit |
| **Needs** | SOC2 compliance, session recording wajib, MFA enforcement, device certificate, SIEM integration |
| **Tech Savviness** | Tinggi |
| **Frequency** | Mingguan review, real-time alerting |

### 4.5 MSP Owner — "Erik"

| Atribut | Detail |
|---------|--------|
| **Usia** | 30-55 tahun |
| **Peran** | Pemilik/Direktur MSP |
| **Organisasi** | MSP dengan 10-100 teknisi, 50-500 klien |
| **Goals** | Margin tinggi, skala tanpa menambah biaya lisensi proporsional, white-label capability |
| **Pain Points** | Biaya TeamViewer/AnyDesk terlalu tinggi per teknisi, tidak bisa multi-tenant, branding terbatas |
| **Needs** | Multi-tenant, per-client billing, white-label, API integration dengan PSA/RMM, bulk pricing |
| **Tech Savviness** | Menengah |
| **Frequency** | Dashboard review harian |

---

## 5. Business Goals

### 5.1 Tahun 1

| Goal | Target | Metrik |
|------|--------|--------|
| MVP Release | Q2 2027 | Fitur core lengkap, Windows + macOS |
| Early Adopters | 500 organisasi | Registrasi aktif |
| Active Devices | 10,000 perangkat terkelola | DAU devices |
| Revenue | $500K ARR | Subscription revenue |
| NPS | > 40 | Net Promoter Score |

### 5.2 Tahun 2

| Goal | Target | Metrik |
|------|--------|--------|
| Enterprise Release | Q2 2028 | SSO, SCIM, multi-tenant, session recording |
| Organizations | 5,000 organisasi | Registrasi aktif |
| Active Devices | 100,000 perangkat | DAU devices |
| Revenue | $5M ARR | Subscription revenue |
| NPS | > 50 | Net Promoter Score |

### 5.3 Tahun 3

| Goal | Target | Metrik |
|------|--------|--------|
| Global Scale | Q2 2029 | Multi-region, semua platform |
| Organizations | 50,000 organisasi | Registrasi aktif |
| Active Devices | 1,000,000 perangkat | DAU devices |
| Revenue | $30M ARR | Subscription revenue |
| Market Position | Top 5 remote desktop global | Analyst ranking |

---

## 6. Success Metrics

### 6.1 Performance KPIs

| Metrik | Target LAN | Target WAN | Kompetitor Terbaik |
|--------|-----------|-----------|-------------------|
| Latency (input-to-display) | < 16ms | < 100ms | AnyDesk ~20ms LAN |
| Frame Rate | 60 fps | 30-60 fps | Parsec 60fps |
| Connection Time | < 2 detik | < 5 detik | AnyDesk ~3s |
| CPU Usage (idle) | < 1% | < 1% | RustDesk ~2% |
| CPU Usage (streaming) | < 10% | < 15% | TeamViewer ~20% |
| RAM Usage (agent) | < 50 MB | < 50 MB | RustDesk ~80MB |
| RAM Usage (viewer) | < 150 MB | < 150 MB | TeamViewer ~200MB |
| Bandwidth (1080p 30fps) | — | < 3 Mbps | AnyDesk ~2-5Mbps |
| Bandwidth (4K 60fps) | — | < 15 Mbps | Parsec ~15Mbps |
| Startup Time (agent) | < 1 detik | < 1 detik | — |

### 6.2 Reliability KPIs

| Metrik | Target |
|--------|--------|
| Uptime (control plane) | 99.99% |
| Uptime (relay/TURN) | 99.95% |
| Session Success Rate | > 99.5% |
| Auto-Reconnect Success | > 95% |
| Crash Rate | < 0.1% per session |
| Mean Time to Recovery | < 30 detik |

### 6.3 Business KPIs

| Metrik | Target |
|--------|--------|
| Customer Churn (monthly) | < 3% |
| Trial-to-Paid Conversion | > 15% |
| Support Ticket Resolution | < 4 jam (P1), < 24 jam (P2) |
| Deployment Time (enterprise) | < 1 hari |
| API Adoption | > 30% pelanggan enterprise |

---

## 7. Competitor Analysis

### 7.1 Perbandingan Fitur

| Fitur | **Kami** | TeamViewer | AnyDesk | RustDesk | Parsec | Chrome RD |
|-------|---------|------------|---------|----------|--------|-----------|
| Cross Platform | ✅ Win/Mac/Lin/And/iOS | ✅ Semua | ✅ Semua | ✅ Semua | ⚠️ Win/Mac/Lin | ⚠️ Chrome OS + plugin |
| E2E Encryption | ✅ Zero-trust | ✅ AES-256 | ✅ AES-256 | ✅ | ⚠️ | ✅ |
| P2P Connection | ✅ WebRTC | ✅ Proprietary | ✅ Proprietary | ✅ | ✅ | ❌ |
| Hardware Encoding | ✅ NVENC/QSV/AMF/VT | ✅ | ✅ | ⚠️ Partial | ✅ | ❌ |
| H265/AV1 | ✅ | ❌ | ❌ | ❌ | ⚠️ H265 | ❌ |
| Multi-Monitor | ✅ Unlimited | ✅ | ✅ | ⚠️ Basic | ✅ | ⚠️ |
| File Transfer | ✅ Resume/Parallel | ✅ | ✅ | ✅ Basic | ❌ | ❌ |
| Session Recording | ✅ | ✅ Enterprise | ✅ Enterprise | ❌ | ❌ | ❌ |
| Unattended Access | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| SSO/SAML/OIDC | ✅ | ✅ Enterprise | ⚠️ Limited | ❌ | ❌ | ✅ Google |
| LDAP/AD | ✅ | ✅ Enterprise | ⚠️ | ❌ | ❌ | ❌ |
| SCIM | ✅ | ⚠️ | ❌ | ❌ | ❌ | ❌ |
| RBAC/ABAC | ✅ | ✅ RBAC | ⚠️ Basic | ❌ | ❌ | ❌ |
| Multi-Tenant | ✅ | ✅ | ⚠️ | ❌ | ❌ | ❌ |
| Plugin SDK | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| API (REST/gRPC) | ✅ | ✅ REST | ⚠️ | ❌ | ❌ | ❌ |
| Webhook | ✅ | ⚠️ | ❌ | ❌ | ❌ | ❌ |
| Wake-on-LAN | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| Remote Reboot | ✅ | ✅ | ✅ | ⚠️ | ❌ | ❌ |
| Remote Terminal | ✅ | ⚠️ | ❌ | ❌ | ❌ | ❌ |
| Whiteboard | ✅ | ⚠️ | ❌ | ❌ | ❌ | ❌ |
| Voice/Video Call | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Hardware Inventory | ✅ | ✅ Enterprise | ❌ | ❌ | ❌ | ❌ |
| Self-Hosted | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |
| Open Protocol | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |

### 7.2 Perbandingan Harga (per bulan, estimasi)

| Tier | **Kami** | TeamViewer | AnyDesk | RustDesk |
|------|---------|------------|---------|----------|
| Free (personal) | ✅ 1 device | ✅ Limited | ✅ Limited | ✅ Unlimited |
| Starter (1 user) | $15 | $50 | $15 | $0 (self-host) |
| Professional (5 users) | $49 | $100 | $30 | $0 (self-host) |
| Enterprise (unlimited) | Custom | Custom ($200+) | Custom | $0 (self-host) |

### 7.3 Perbandingan Performa

| Metrik | **Kami (Target)** | TeamViewer | AnyDesk | RustDesk | Parsec |
|--------|-----------------|------------|---------|----------|--------|
| Latency LAN | <16ms | ~30ms | ~20ms | ~25ms | ~15ms |
| Latency WAN | <100ms | ~150ms | ~100ms | ~120ms | ~80ms |
| Max FPS | 60 | 30-60 | 60 | 30-60 | 60 |
| CPU (streaming) | <10% | ~20% | ~15% | ~15% | ~10% |
| RAM (agent) | <50MB | ~150MB | ~80MB | ~80MB | ~100MB |
| Startup | <1s | ~3s | ~2s | ~2s | ~3s |

### 7.4 Kelemahan Kompetitor yang Menjadi Peluang

1. **TeamViewer**: Harga tinggi, resource berat, sering dipersepsikan sebagai bloatware
2. **AnyDesk**: Fitur enterprise terbatas, pernah mengalami security breach (2024)
3. **RustDesk**: Kurang fitur enterprise (SSO, SCIM, session recording), UI kurang polish
4. **Parsec**: Fokus gaming bukan enterprise, tidak ada file transfer/unattended access
5. **Chrome Remote Desktop**: Terlalu basic, tergantung Google ecosystem

---

## 8. Use Cases

### UC-01: Remote IT Support (Attended)

| Aspek | Detail |
|-------|--------|
| **Aktor** | Help Desk Technician + End User |
| **Trigger** | End user melaporkan masalah via tiket |
| **Precondition** | Agent terinstall di perangkat end user |
| **Flow** | 1. Teknisi memilih perangkat dari dashboard → 2. Kirim permintaan remote → 3. End user menerima prompt persetujuan → 4. Sesi remote dimulai → 5. Teknisi troubleshoot → 6. Sesi selesai → 7. Session recording tersimpan |
| **Post-condition** | Tiket resolved, session log tersimpan untuk audit |

### UC-02: Unattended Server Access

| Aspek | Detail |
|-------|--------|
| **Aktor** | System Administrator |
| **Trigger** | Server membutuhkan maintenance |
| **Precondition** | Agent terinstall dengan unattended access enabled, password/key configured |
| **Flow** | 1. Admin membuka viewer → 2. Pilih server dari device list → 3. Authenticate (password + MFA) → 4. Langsung terhubung tanpa persetujuan di sisi remote → 5. Maintenance selesai → 6. Disconnect |
| **Post-condition** | Audit log tercatat, session recording tersimpan |

### UC-03: Multi-Tenant MSP Management

| Aspek | Detail |
|-------|--------|
| **Aktor** | MSP Technician |
| **Trigger** | Klien MSP membutuhkan support |
| **Precondition** | Organisasi klien terdaftar di tenant MSP |
| **Flow** | 1. Teknisi login ke dashboard MSP → 2. Pilih tenant/klien → 3. Lihat device list klien tersebut → 4. Remote ke perangkat → 5. Resolve masalah → 6. Log tercatat di tenant klien |
| **Post-condition** | Billing per-tenant terhitung, audit trail terisolasi per tenant |

### UC-04: Remote Training Session

| Aspek | Detail |
|-------|--------|
| **Aktor** | Trainer + Multiple Trainees |
| **Trigger** | Jadwal training |
| **Precondition** | Multi-user session enabled |
| **Flow** | 1. Trainer memulai sesi → 2. Trainees bergabung sebagai viewers → 3. Trainer mendemonstrasikan di shared screen → 4. Trainer menggunakan annotation/whiteboard → 5. Laser pointer untuk highlight → 6. Q&A via chat → 7. Sesi direkam |
| **Post-condition** | Recording tersedia untuk playback |

### UC-05: File Transfer Massal

| Aspek | Detail |
|-------|--------|
| **Aktor** | IT Admin |
| **Trigger** | Deployment software ke banyak perangkat |
| **Precondition** | File transfer enabled, sufficient bandwidth |
| **Flow** | 1. Admin pilih multiple devices → 2. Initiate file transfer → 3. Files di-chunk, compress, encrypt → 4. Parallel upload ke semua target → 5. Progress tracking per device → 6. Checksum verification → 7. Completion report |
| **Post-condition** | Files terkirim dan terverifikasi di semua target |

### UC-06: Wake-on-LAN + Remote Reboot

| Aspek | Detail |
|-------|--------|
| **Aktor** | System Administrator |
| **Trigger** | Server perlu di-restart remotely |
| **Precondition** | WoL enabled di BIOS, agent terdaftar |
| **Flow** | 1. Admin pilih device → 2. Kirim WoL magic packet melalui agent lain di subnet yang sama → 3. Tunggu device online → 4. Connect → 5. Jika perlu: remote reboot (termasuk Safe Mode) → 6. Reconnect setelah reboot |
| **Post-condition** | Device online dan operational |

### UC-07: Session Recording & Compliance

| Aspek | Detail |
|-------|--------|
| **Aktor** | Security Officer |
| **Trigger** | Audit requirement |
| **Precondition** | Session recording policy aktif |
| **Flow** | 1. Policy mengharuskan recording untuk semua sesi → 2. Sesi remote dimulai → 3. Recording otomatis aktif → 4. Sesi selesai → 5. Recording tersimpan terenkripsi → 6. Auditor dapat playback sesi → 7. Export audit log |
| **Post-condition** | Compliance evidence tersimpan |

### UC-08: SSO Enterprise Onboarding

| Aspek | Detail |
|-------|--------|
| **Aktor** | IT Admin (Enterprise) |
| **Trigger** | Onboarding platform ke organisasi |
| **Precondition** | Organisasi memiliki IdP (Okta, Azure AD, etc.) |
| **Flow** | 1. Admin configure SSO (SAML/OIDC) → 2. Map groups ke roles (RBAC) → 3. Enable SCIM provisioning → 4. Users auto-provisioned → 5. Users login via SSO → 6. Access sesuai role |
| **Post-condition** | Zero-touch user provisioning aktif |

### UC-09: Remote Terminal / Command Execution

| Aspek | Detail |
|-------|--------|
| **Aktor** | System Administrator |
| **Trigger** | Perlu menjalankan command tanpa full remote desktop |
| **Precondition** | Terminal access enabled, appropriate permissions |
| **Flow** | 1. Admin pilih device → 2. Buka remote terminal → 3. Terminal session (PowerShell/CMD/Bash/zsh) → 4. Execute commands → 5. Output streaming real-time → 6. Session logged |
| **Post-condition** | Commands executed, output logged |

### UC-10: Hardware/Software Inventory

| Aspek | Detail |
|-------|--------|
| **Aktor** | IT Asset Manager |
| **Trigger** | Quarterly asset audit |
| **Precondition** | Inventory collection enabled pada agent |
| **Flow** | 1. Agent collect hardware info (CPU, RAM, disk, GPU, peripherals) → 2. Agent collect software list (installed apps, versions) → 3. Data sync ke server → 4. Dashboard menampilkan inventory → 5. Export report (CSV/PDF) → 6. Alert jika ada unauthorized software |
| **Post-condition** | Asset database up-to-date |

### UC-11: Remote Printing

| Aspek | Detail |
|-------|--------|
| **Aktor** | Remote Worker |
| **Trigger** | Perlu print dokumen dari remote machine ke local printer |
| **Precondition** | Remote printing enabled, local printer configured |
| **Flow** | 1. User membuka dokumen di remote session → 2. Print ke virtual "Remote Printer" → 3. Print job captured → 4. Data dikirim ke viewer secara terenkripsi → 5. Viewer forward ke local printer → 6. Dokumen tercetak |
| **Post-condition** | Dokumen tercetak di lokasi user |

### UC-12: Script Automation & Scheduled Tasks

| Aspek | Detail |
|-------|--------|
| **Aktor** | IT Admin |
| **Trigger** | Maintenance rutin |
| **Precondition** | Script library tersedia, permissions granted |
| **Flow** | 1. Admin membuat script (PowerShell/Bash) → 2. Target devices/groups dipilih → 3. Schedule (immediate/cron) → 4. Script dijalankan pada target → 5. Output collected → 6. Report generated |
| **Post-condition** | Automated maintenance completed |

### UC-13: Bandwidth-Constrained Remote Access

| Aspek | Detail |
|-------|--------|
| **Aktor** | Field Technician dengan koneksi terbatas |
| **Trigger** | Remote access dari lokasi dengan bandwidth rendah |
| **Precondition** | Adaptive bitrate enabled |
| **Flow** | 1. Connect ke device → 2. Network quality detection → 3. Auto-adjust: resolusi turun, FPS turun, compression naik → 4. Switch ke codec lebih efisien (AV1) → 5. QoS profile "low-bandwidth" → 6. Tetap usable meskipun <1Mbps |
| **Post-condition** | Remote session functional pada bandwidth rendah |

### UC-14: Multi-Monitor Professional Use

| Aspek | Detail |
|-------|--------|
| **Aktor** | Developer/Designer yang remote |
| **Trigger** | Remote ke workstation dengan multiple monitors |
| **Precondition** | Multi-monitor support enabled |
| **Flow** | 1. Connect ke workstation → 2. Detect semua monitor remote → 3. Thumbnail preview tiap monitor → 4. Pilih: single monitor / all monitors / span → 5. Independent window per monitor → 6. Hot-plug detection jika monitor ditambah/dilepas |
| **Post-condition** | Full multi-monitor experience |

### UC-15: BYOD Secure Access

| Aspek | Detail |
|-------|--------|
| **Aktor** | Karyawan yang akses dari personal device |
| **Trigger** | Akses corporate resource dari luar |
| **Precondition** | BYOD policy configured, device compliance check |
| **Flow** | 1. User install viewer di personal device → 2. Login via SSO + MFA → 3. Device compliance check (OS version, antivirus, encryption) → 4. Jika compliant: access granted → 5. Jika tidak: remediation steps ditampilkan → 6. Semua akses diaudit, clipboard/file transfer bisa di-restrict berdasarkan policy |
| **Post-condition** | Secure access tanpa expose data ke personal device |

### UC-16: Voice/Video Communication During Session

| Aspek | Detail |
|-------|--------|
| **Aktor** | Technician + End User |
| **Trigger** | Perlu komunikasi saat remote session |
| **Flow** | 1. Selama remote session aktif → 2. Technician atau user initiate voice call → 3. WebRTC audio channel established → 4. Optional: video call → 5. Optional: chat text → 6. Komunikasi berjalan bersamaan dengan remote desktop |

### UC-17: Kiosk / Digital Signage Management

| Aspek | Detail |
|-------|--------|
| **Aktor** | IT Admin retail/hospitality |
| **Trigger** | Kiosk perlu update atau troubleshoot |
| **Flow** | 1. Agent berjalan sebagai Windows service pada kiosk → 2. Unattended access → 3. Admin remote kapan saja → 4. Update content → 5. Restart jika diperlukan → 6. Monitor health via dashboard |

---

## 9. Functional Requirements

### 9.1 Connection Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-CON-01 | P2P connection via WebRTC (STUN/TURN/ICE) | P0 |
| FR-CON-02 | Relay fallback ketika P2P gagal | P0 |
| FR-CON-03 | Auto-reconnect dengan session resumption | P0 |
| FR-CON-04 | Connection timeout configurable | P1 |
| FR-CON-05 | Concurrent sessions per device (configurable limit) | P1 |
| FR-CON-06 | Network quality detection dan adaptive behavior | P0 |
| FR-CON-07 | Geo-routing ke relay/TURN terdekat | P1 |
| FR-CON-08 | Connection statistics real-time (latency, bandwidth, FPS, packet loss) | P0 |

### 9.2 Screen Streaming

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-STR-01 | Screen capture: DXGI Desktop Duplication (Windows), ScreenCaptureKit (macOS) | P0 |
| FR-STR-02 | Hardware encoding: NVENC, QuickSync, AMF, VideoToolbox | P0 |
| FR-STR-03 | Software encoding fallback (x264, libvpx) | P0 |
| FR-STR-04 | Codec support: H.264, H.265, AV1 | P0 (H264), P1 (H265/AV1) |
| FR-STR-05 | Adaptive bitrate berdasarkan network condition | P0 |
| FR-STR-06 | Adaptive FPS (10-60 fps) | P0 |
| FR-STR-07 | Adaptive resolution scaling | P1 |
| FR-STR-08 | Dirty region detection — hanya encode area yang berubah | P0 |
| FR-STR-09 | Cursor rendering (local cursor untuk low latency) | P0 |
| FR-STR-10 | Color space: SDR (sRGB), HDR (rencana) | P0 (SDR), P3 (HDR) |

### 9.3 Input Handling

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-INP-01 | Keyboard input forwarding (semua key combinations) | P0 |
| FR-INP-02 | Mouse input forwarding (click, move, scroll, drag) | P0 |
| FR-INP-03 | Special keys: Ctrl+Alt+Del, Win key, PrintScreen | P0 |
| FR-INP-04 | Keyboard layout mapping cross-platform | P1 |
| FR-INP-05 | Touch input support (untuk viewer di tablet) | P2 |
| FR-INP-06 | Stylus/pen input (rencana) | P3 |

### 9.4 File Transfer

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-FIL-01 | Bidirectional file transfer | P0 |
| FR-FIL-02 | Drag-and-drop file transfer | P1 |
| FR-FIL-03 | Resume interrupted transfers | P0 |
| FR-FIL-04 | Pause/resume transfers | P1 |
| FR-FIL-05 | Parallel file upload (multiple files simultaneously) | P1 |
| FR-FIL-06 | Folder transfer (recursive) | P0 |
| FR-FIL-07 | Compression selama transfer (LZ4/Zstd) | P0 |
| FR-FIL-08 | Encryption selama transfer (AES-256-GCM) | P0 |
| FR-FIL-09 | Checksum verification (SHA-256) | P0 |
| FR-FIL-10 | Conflict resolution (overwrite/rename/skip) | P1 |
| FR-FIL-11 | Progress indicator per file dan keseluruhan | P0 |
| FR-FIL-12 | Transfer speed display | P1 |
| FR-FIL-13 | File size limit configurable | P1 |

### 9.5 Clipboard

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-CLB-01 | Text clipboard sync bidirectional | P0 |
| FR-CLB-02 | Image clipboard sync | P1 |
| FR-CLB-03 | HTML/rich text clipboard sync | P2 |
| FR-CLB-04 | File clipboard sync (copy file, paste di remote) | P2 |
| FR-CLB-05 | Large clipboard handling (>1MB) dengan streaming | P1 |
| FR-CLB-06 | Clipboard history (configurable retention) | P2 |
| FR-CLB-07 | Clipboard policy (disable per session/role) | P1 |

### 9.6 Audio

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-AUD-01 | Remote audio playback (audio dari remote machine) | P0 |
| FR-AUD-02 | Microphone passthrough (local mic → remote app) | P1 |
| FR-AUD-03 | Noise suppression | P2 |
| FR-AUD-04 | Echo cancellation | P2 |
| FR-AUD-05 | Audio codec: Opus (primary), AAC (fallback) | P0 |
| FR-AUD-06 | Audio mute/unmute controls | P0 |
| FR-AUD-07 | Volume control | P1 |

### 9.7 Video / Camera

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-VID-01 | Camera sharing (local camera → remote apps) | P2 |
| FR-VID-02 | Virtual camera driver | P2 |
| FR-VID-03 | Adaptive resolution (camera) | P2 |
| FR-VID-04 | Bandwidth optimization for camera stream | P2 |

### 9.8 Multi-Monitor

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-MON-01 | Detect semua monitor remote | P0 |
| FR-MON-02 | Single monitor view | P0 |
| FR-MON-03 | All monitors view (combined) | P1 |
| FR-MON-04 | Independent window per monitor | P1 |
| FR-MON-05 | Monitor hot-plug detection | P2 |
| FR-MON-06 | Monitor thumbnail preview | P1 |
| FR-MON-07 | Monitor rename (custom label) | P2 |
| FR-MON-08 | Unlimited monitor support | P1 |

### 9.9 Chat & Communication

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-COM-01 | Text chat during session | P0 |
| FR-COM-02 | Voice call (WebRTC audio) | P1 |
| FR-COM-03 | Video call (WebRTC video) | P2 |
| FR-COM-04 | Chat history per session | P1 |

### 9.10 Session Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-SES-01 | Session recording (video) | P1 |
| FR-SES-02 | Session playback | P1 |
| FR-SES-03 | Session transfer (hand-off ke teknisi lain) | P2 |
| FR-SES-04 | Multi-user session (multiple viewers) | P2 |
| FR-SES-05 | Technician mode vs Customer mode | P1 |
| FR-SES-06 | Temporary session (one-time code) | P0 |
| FR-SES-07 | Permanent device (unattended access) | P0 |
| FR-SES-08 | Session timeout configurable | P1 |
| FR-SES-09 | Session notes/tags | P2 |

### 9.11 Remote Operations

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-ROP-01 | Remote terminal (PowerShell/CMD/Bash/zsh) | P1 |
| FR-ROP-02 | Remote reboot (normal, safe mode) | P1 |
| FR-ROP-03 | Wake-on-LAN | P1 |
| FR-ROP-04 | Remote command execution (scripted) | P1 |
| FR-ROP-05 | Script automation (PowerShell/Bash scripts) | P2 |
| FR-ROP-06 | Scheduled tasks | P2 |
| FR-ROP-07 | Process manager (view/kill processes) | P2 |
| FR-ROP-08 | Service manager (start/stop/restart services) | P2 |
| FR-ROP-09 | Registry editor (Windows) | P3 |
| FR-ROP-10 | Remote printing | P2 |

### 9.12 Annotation & Collaboration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-ANN-01 | Whiteboard overlay | P2 |
| FR-ANN-02 | Annotation tools (pen, arrow, rectangle, text) | P2 |
| FR-ANN-03 | Laser pointer | P2 |
| FR-ANN-04 | Screen capture/screenshot | P1 |

### 9.13 Device Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-DEV-01 | Device registration dan enrollment | P0 |
| FR-DEV-02 | Device groups | P0 |
| FR-DEV-03 | Device tags | P1 |
| FR-DEV-04 | Device search dan filtering | P0 |
| FR-DEV-05 | Address book / favorites | P0 |
| FR-DEV-06 | Device online/offline status real-time | P0 |
| FR-DEV-07 | Hardware inventory collection | P2 |
| FR-DEV-08 | Software inventory collection | P2 |
| FR-DEV-09 | Device health monitoring (CPU, RAM, disk, network) | P2 |
| FR-DEV-10 | Bandwidth analytics per device | P2 |

### 9.14 Identity & Access Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-IAM-01 | User registration dan authentication | P0 |
| FR-IAM-02 | MFA/2FA (TOTP, WebAuthn/FIDO2) | P0 |
| FR-IAM-03 | SSO via SAML 2.0 | P1 |
| FR-IAM-04 | SSO via OIDC | P1 |
| FR-IAM-05 | LDAP/Active Directory integration | P1 |
| FR-IAM-06 | SCIM provisioning | P2 |
| FR-IAM-07 | RBAC (Role-Based Access Control) | P0 |
| FR-IAM-08 | ABAC (Attribute-Based Access Control) | P2 |
| FR-IAM-09 | API tokens (per user, scoped) | P1 |
| FR-IAM-10 | Device certificates (X.509) | P0 |

### 9.15 Organization & Tenant

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-ORG-01 | Multi-tenant architecture | P1 |
| FR-ORG-02 | Organization management | P0 |
| FR-ORG-03 | Team management within organization | P1 |
| FR-ORG-04 | Organization-level policies | P1 |
| FR-ORG-05 | Cross-organization access (controlled) | P2 |

### 9.16 Integration & Extensibility

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-INT-01 | REST API (public) | P0 |
| FR-INT-02 | gRPC API (internal + high-perf clients) | P1 |
| FR-INT-03 | WebSocket API (real-time events) | P0 |
| FR-INT-04 | Webhook (configurable events) | P1 |
| FR-INT-05 | Plugin SDK (Rust) | P2 |
| FR-INT-06 | SIEM integration (Splunk, ELK, etc.) | P2 |
| FR-INT-07 | PSA/RMM integration (ConnectWise, Datto, etc.) | P3 |

### 9.17 Auto Update

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-UPD-01 | Auto-update agent | P0 |
| FR-UPD-02 | Update channels: Stable, Beta, Nightly | P1 |
| FR-UPD-03 | Enterprise update server (self-hosted) | P2 |
| FR-UPD-04 | Delta updates (binary diff) | P2 |
| FR-UPD-05 | Rollback capability | P1 |
| FR-UPD-06 | Digital signature verification | P0 |
| FR-UPD-07 | Update policy per organization | P1 |

### 9.18 Web Dashboard

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-WEB-01 | Device management dashboard | P0 |
| FR-WEB-02 | User management | P0 |
| FR-WEB-03 | Organization/tenant management | P1 |
| FR-WEB-04 | Session history dan logs | P0 |
| FR-WEB-05 | Real-time device status | P0 |
| FR-WEB-06 | Analytics dashboard (connections, bandwidth, performance) | P1 |
| FR-WEB-07 | Audit log viewer | P1 |
| FR-WEB-08 | Policy management | P1 |
| FR-WEB-09 | Update management | P1 |
| FR-WEB-10 | Notification center | P2 |

---

## 10. Non-Functional Requirements

### 10.1 Performance

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-PER-01 | API response time (p50) | < 50ms |
| NFR-PER-02 | API response time (p99) | < 200ms |
| NFR-PER-03 | WebSocket message latency | < 10ms |
| NFR-PER-04 | Database query time (p95) | < 50ms |
| NFR-PER-05 | Cache hit ratio | > 90% |
| NFR-PER-06 | Agent binary size | < 20 MB |
| NFR-PER-07 | Viewer binary size | < 50 MB |

### 10.2 Reliability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-REL-01 | Control plane uptime | 99.99% (52 min downtime/tahun) |
| NFR-REL-02 | Data plane uptime | 99.95% (4.4 jam downtime/tahun) |
| NFR-REL-03 | Zero data loss | RPO = 0 untuk critical data |
| NFR-REL-04 | Recovery time | RTO < 5 menit |
| NFR-REL-05 | Graceful degradation | Tetap berfungsi walau komponen non-critical down |

### 10.3 Scalability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-SCA-01 | Concurrent connections | 1,000,000+ |
| NFR-SCA-02 | Registered devices | 10,000,000+ |
| NFR-SCA-03 | API requests per second | 50,000+ |
| NFR-SCA-04 | WebSocket connections per node | 100,000+ |
| NFR-SCA-05 | Horizontal scaling | Linear scalability dengan penambahan node |

### 10.4 Maintainability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-MAI-01 | Code coverage (unit test) | > 80% |
| NFR-MAI-02 | Code coverage (integration test) | > 60% |
| NFR-MAI-03 | Documentation coverage | 100% public API |
| NFR-MAI-04 | CI/CD pipeline | < 15 menit full pipeline |
| NFR-MAI-05 | Deployment rollback | < 5 menit |

### 10.5 Availability

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-AVA-01 | Multi-region deployment | >= 3 region |
| NFR-AVA-02 | No single point of failure | Semua komponen redundant |
| NFR-AVA-03 | Blue-green deployment | Zero-downtime deployment |
| NFR-AVA-04 | Auto-scaling | React to load within 60 detik |

---

## 11. Performance Requirements

### 11.1 Streaming Performance

| Skenario | Latency | FPS | Bandwidth | CPU Agent | CPU Viewer |
|----------|---------|-----|-----------|-----------|------------|
| LAN 1080p optimal | < 16ms | 60 | < 5 Mbps | < 8% | < 5% |
| LAN 4K optimal | < 20ms | 60 | < 20 Mbps | < 15% | < 10% |
| WAN 1080p good (50Mbps) | < 50ms | 60 | < 5 Mbps | < 10% | < 8% |
| WAN 1080p moderate (10Mbps) | < 80ms | 30 | < 3 Mbps | < 10% | < 8% |
| WAN 1080p poor (2Mbps) | < 150ms | 15-20 | < 2 Mbps | < 8% | < 5% |
| WAN via relay | < 200ms | 30 | < 5 Mbps | < 12% | < 10% |

### 11.2 File Transfer Performance

| Skenario | Throughput Target |
|----------|------------------|
| LAN (1Gbps) | > 500 Mbps |
| WAN (100Mbps) | > 80 Mbps |
| WAN (10Mbps) | > 8 Mbps |
| Via relay | > 5 Mbps |

### 11.3 Memory Budget

| Komponen | Idle | Active Session | Peak |
|----------|------|---------------|------|
| Agent | < 30 MB | < 80 MB | < 150 MB |
| Viewer (1 session) | < 80 MB | < 150 MB | < 300 MB |
| Viewer (5 sessions) | < 150 MB | < 400 MB | < 700 MB |

### 11.4 Startup Performance

| Event | Target |
|-------|--------|
| Agent cold start | < 1 detik |
| Agent warm start (background) | < 200ms |
| Viewer launch | < 2 detik |
| Connection establishment (P2P) | < 3 detik |
| Connection establishment (relay) | < 5 detik |
| First frame displayed | < 500ms setelah connected |

---

## 12. Security Requirements

### 12.1 Encryption

| Layer | Requirement |
|-------|-------------|
| Transport | TLS 1.3 (minimum) untuk semua komunikasi control plane |
| Session | E2E encryption menggunakan AES-256-GCM atau ChaCha20-Poly1305 |
| Key Exchange | X25519 ECDH |
| Signing | Ed25519 untuk device identity dan code signing |
| Relay | Relay server TIDAK BOLEH dapat mendekripsi konten sesi (E2E mandatory) |
| Storage | Sensitive data at rest encrypted dengan AES-256 |

### 12.2 Authentication & Authorization

| Requirement | Detail |
|-------------|--------|
| Device Identity | Ed25519 keypair per device, X.509 device certificate |
| User Auth | Username/password (Argon2id), SSO, MFA |
| Session Token | JWT dengan short expiry (15 min), refresh token (7 hari) |
| MFA | TOTP (RFC 6238), WebAuthn/FIDO2 |
| Mutual Auth | mTLS antara agent dan server |
| RBAC | Role hierarchy: Owner > Admin > Manager > Technician > Viewer > Guest |
| ABAC | Attribute-based policies (time, location, device compliance) |

### 12.3 Security Controls

| Control | Detail |
|---------|--------|
| Rate Limiting | Per IP, per user, per API endpoint |
| Brute Force | Account lockout setelah 5 failed attempts (30 min cooldown) |
| Replay Protection | Nonce + timestamp pada setiap authenticated request |
| Perfect Forward Secrecy | Ephemeral key exchange per session |
| Certificate Pinning | Pin server certificate di agent dan viewer |
| Secure Update | Code signing (Ed25519), hash verification, HTTPS-only |
| Tamper Detection | Binary integrity check, config file integrity |
| Audit Log | Immutable audit log untuk semua security-relevant events |
| OWASP Top 10 | Mitigasi untuk semua OWASP Top 10 (2025) |

### 12.4 Compliance Readiness

| Standard | Status |
|----------|--------|
| SOC 2 Type II | Designed for (Year 2 target) |
| GDPR | Compliant by design (data minimization, right to deletion) |
| HIPAA | Ready (BAA support, encryption, audit trail) |
| ISO 27001 | Designed for (Year 3 target) |

---

## 13. Scalability Requirements

### 13.1 Horizontal Scaling Targets

| Komponen | Single Node | Cluster (10 nodes) | Cluster (100 nodes) |
|----------|-------------|--------------------|--------------------|
| API Server | 5,000 rps | 50,000 rps | 500,000 rps |
| WebSocket Server | 100,000 conn | 1,000,000 conn | 10,000,000 conn |
| Signal Server | 10,000 sessions | 100,000 sessions | 1,000,000 sessions |
| TURN Server | 5,000 relays | 50,000 relays | 500,000 relays |
| Database (read) | 10,000 qps | 100,000 qps (replicas) | — |
| Database (write) | 5,000 qps | 5,000 qps (single primary) | Sharded |

### 13.2 Geo-Distribution

| Region | Komponen | Tujuan |
|--------|----------|--------|
| Asia Pacific (SG, TK, SY) | Full stack | Primary market |
| US (Virginia, Oregon) | Full stack | NA market |
| Europe (Frankfurt, London) | Full stack | EU market |
| Edge (50+ PoP) | STUN/TURN | Low latency connection establishment |

---

## 14. Accessibility

| Requirement | Standard | Detail |
|-------------|----------|--------|
| Web Dashboard | WCAG 2.1 AA | Semua fungsi dapat diakses via keyboard |
| Screen Reader | ARIA labels | Semua komponen interaktif memiliki label |
| Color Contrast | 4.5:1 minimum | Text contrast ratio |
| Focus Indicators | Visible | Semua focusable elements memiliki visible focus |
| Font Scaling | 200% | Dashboard tetap usable pada 200% zoom |
| Viewer | Keyboard navigation | Semua toolbar actions via keyboard shortcut |
| High Contrast | Supported | High contrast mode untuk viewer toolbar |

---

## 15. Internationalization

### 15.1 Bahasa Target

| Fase | Bahasa |
|------|--------|
| Fase 1 | English, Bahasa Indonesia |
| Fase 2 | Chinese (Simplified), Japanese, Korean |
| Fase 3 | German, French, Spanish, Portuguese |
| Fase 4 | Arabic (RTL), Thai, Vietnamese, Hindi |

### 15.2 Technical Requirements

| Requirement | Detail |
|-------------|--------|
| i18n Framework | Vue I18n (web), Fluent (Rust/Tauri) |
| String Externalization | Semua user-facing strings di resource files |
| Date/Time | Locale-aware formatting, timezone support |
| Number Format | Locale-aware (decimal separator, currency) |
| RTL Layout | Dashboard dan viewer mendukung RTL |
| Unicode | Full Unicode support (emoji, CJK, Arabic) |
| Pluralization | ICU MessageFormat |

---

## 16. Offline Mode

| Fitur | Offline Capability |
|-------|-------------------|
| Device List | Cached locally, sync saat online |
| Address Book | Full offline access |
| Connection History | Cached locally |
| Direct LAN Connection | Berfungsi tanpa internet (mDNS discovery) |
| Settings | Full offline access |
| File Transfer Queue | Queued, execute saat online |
| Viewer Preferences | Full offline access |

---

## 17. Future Expansion

### 17.1 Platform Expansion

| Platform | Timeline | Scope |
|----------|----------|-------|
| Linux (Agent + Viewer) | Phase 2 | X11/Wayland capture, full feature parity |
| Android Viewer | Phase 2 | View + basic input, touch-optimized |
| Android Agent | Phase 3 | Screen share, limited remote control |
| iOS Viewer | Phase 3 | View + basic input, App Store compliant |
| Web Viewer | Phase 2 | Browser-based viewer via WebRTC |
| ChromeOS | Phase 3 | Via web viewer atau Android app |

### 17.2 Feature Expansion

| Feature | Timeline | Deskripsi |
|---------|----------|-----------|
| USB Redirection | Phase 3 | Forward USB devices ke remote machine |
| Smart Card | Phase 3 | Smart card passthrough untuk authentication |
| AI Troubleshooting | Phase 4 | AI-assisted diagnostics dan solution suggestion |
| AR Annotations | Phase 4 | Augmented reality overlay untuk on-site guidance |
| HDR Support | Phase 3 | High Dynamic Range screen streaming |
| Wayland Native | Phase 2 | Native Wayland screen capture (Linux) |
| Zero-Install Viewer | Phase 2 | WebAssembly-based viewer, no installation needed |

---

## 18. Milestone

### Phase 1: Foundation (Bulan 1-6)

| Milestone | Target | Deliverable |
|-----------|--------|-------------|
| M1.1 | Bulan 1 | Core protocol, basic agent (Windows screen capture, keyboard, mouse) |
| M1.2 | Bulan 2 | Basic viewer (Tauri), P2P connection via WebRTC, H.264 encoding |
| M1.3 | Bulan 3 | Authentication, device registration, basic API server |
| M1.4 | Bulan 4 | File transfer (basic), clipboard sync, macOS agent |
| M1.5 | Bulan 5 | Web dashboard (basic), user management, device management |
| M1.6 | Bulan 6 | **MVP Release** — Windows + macOS, P2P, relay fallback, basic features |

### Phase 2: Enterprise (Bulan 7-12)

| Milestone | Target | Deliverable |
|-----------|--------|-------------|
| M2.1 | Bulan 7 | SSO (SAML/OIDC), MFA, RBAC |
| M2.2 | Bulan 8 | Multi-tenant, organization management, team management |
| M2.3 | Bulan 9 | Session recording, audit logging, compliance features |
| M2.4 | Bulan 10 | Linux agent/viewer, Android viewer (basic) |
| M2.5 | Bulan 11 | Remote terminal, script automation, hardware inventory |
| M2.6 | Bulan 12 | **Enterprise Release** — Full enterprise features, 3 platforms |

### Phase 3: Scale (Bulan 13-18)

| Milestone | Target | Deliverable |
|-----------|--------|-------------|
| M3.1 | Bulan 13 | Multi-region deployment, geo-routing |
| M3.2 | Bulan 14 | Plugin SDK, webhook, API v2 |
| M3.3 | Bulan 15 | H.265/AV1 encoding, HDR planning |
| M3.4 | Bulan 16 | iOS viewer, Android agent, web viewer |
| M3.5 | Bulan 17 | SCIM, LDAP/AD, ABAC |
| M3.6 | Bulan 18 | **Scale Release** — 1M+ devices, all platforms, plugin ecosystem |

### Phase 4: Intelligence (Bulan 19-24)

| Milestone | Target | Deliverable |
|-----------|--------|-------------|
| M4.1 | Bulan 19 | AI-assisted troubleshooting (prototype) |
| M4.2 | Bulan 20 | USB redirection, smart card support |
| M4.3 | Bulan 21 | Voice/video call, whiteboard, annotation |
| M4.4 | Bulan 22 | Remote printing, advanced QoS |
| M4.5 | Bulan 23 | SOC 2 Type II certification |
| M4.6 | Bulan 24 | **Intelligence Release** — AI features, full compliance, mature platform |

---

## 19. Roadmap

```
2027                              2028                              2029
Q1      Q2      Q3      Q4      Q1      Q2      Q3      Q4      Q1      Q2
├───────┼───────┼───────┼───────┼───────┼───────┼───────┼───────┼───────┤

Phase 1: Foundation (MVP)
├═══════════════════════════╗
│ Core Protocol & Agent     ║
│ Viewer (Tauri)            ║
│ WebRTC P2P + Relay        ║
│ Basic Dashboard           ║
│ Win + macOS               ║
╚═══════════════════════════╝
         ▼
         Phase 2: Enterprise
         ├═══════════════════════════╗
         │ SSO/MFA/RBAC             ║
         │ Multi-tenant             ║
         │ Session Recording        ║
         │ Linux + Android Viewer   ║
         │ Remote Terminal          ║
         │ Audit & Compliance       ║
         ╚═══════════════════════════╝
                  ▼
                  Phase 3: Scale
                  ├═══════════════════════════╗
                  │ Multi-region              ║
                  │ Plugin SDK & Webhooks     ║
                  │ H.265 / AV1              ║
                  │ iOS + Web Viewer          ║
                  │ SCIM / LDAP / ABAC       ║
                  │ 1M+ Devices              ║
                  ╚═══════════════════════════╝
                           ▼
                           Phase 4: Intelligence
                           ├═══════════════════════════╗
                           │ AI Troubleshooting        ║
                           │ USB/Smart Card            ║
                           │ Voice/Video/Whiteboard    ║
                           │ Remote Printing           ║
                           │ SOC 2 Type II             ║
                           │ Mature Platform           ║
                           ╚═══════════════════════════╝
```

---

## Lampiran

### A. Glosarium

| Term | Definisi |
|------|----------|
| Agent | Software yang berjalan di perangkat remote yang akan dikontrol |
| Viewer | Software yang digunakan untuk melihat dan mengontrol perangkat remote |
| P2P | Peer-to-Peer, koneksi langsung antara agent dan viewer |
| TURN | Traversal Using Relays around NAT, relay server untuk koneksi yang tidak bisa P2P |
| STUN | Session Traversal Utilities for NAT, server untuk NAT discovery |
| ICE | Interactive Connectivity Establishment, framework untuk menemukan path terbaik |
| E2E | End-to-End encryption |
| MFA | Multi-Factor Authentication |
| RBAC | Role-Based Access Control |
| ABAC | Attribute-Based Access Control |
| MSP | Managed Service Provider |
| SSO | Single Sign-On |
| SCIM | System for Cross-domain Identity Management |

### B. Referensi

1. WebRTC Specification — W3C
2. STUN RFC 5389
3. TURN RFC 5766
4. ICE RFC 8445
5. TLS 1.3 RFC 8446
6. OAuth 2.0 RFC 6749
7. SAML 2.0 Specification
8. SCIM RFC 7644
9. SOC 2 Trust Service Criteria

---

*Dokumen ini adalah living document dan akan diperbarui seiring perkembangan produk.*
