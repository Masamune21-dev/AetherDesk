# AetherDesk

Platform remote desktop enterprise dengan arsitektur zero-trust, enkripsi
end-to-end, dan agent berbasis Rust.

**Status:** Fase 0 — implementasi control plane sedang berjalan.
Spesifikasi lengkap ada di [`docs/`](./docs), catatan pengerjaan di [`worklog.md`](./worklog.md).

---

## Penamaan

Produk ini bernama **AetherDesk**. Awalan crate dan nama layanan memakai `rdp-`
(`rdp-core`, `rdp-api`, `rdp-signal`) sebagai singkatan dari *remote desktop platform*.

> **Catatan:** awalan `rdp-` berbenturan dengan protokol Remote Desktop Protocol
> milik Microsoft dan menyulitkan pencarian. Penggantian ke awalan `aether-`
> sedang dipertimbangkan dan lebih murah dilakukan sekarang daripada nanti.
> Lihat temuan R-01.

---

## Peta Dokumen

Dokumen normatif — sumber kebenaran, konflik diselesaikan ke arah dokumen ini:

| Dokumen | Isi |
|---|---|
| [PRD.md](./docs/PRD.md) | Visi, persona, 130+ functional requirement, target performa, milestone |
| [ARCHITECTURE.md](./docs/ARCHITECTURE.md) | Arsitektur sistem, ADR, alur autentikasi, alur sesi, struktur modul |
| [SYSTEM_DESIGN.md](./docs/SYSTEM_DESIGN.md) | Modular monolith, state terdistribusi, HA, caching, tracing |
| [REMOTE_PROTOCOL.md](./docs/REMOTE_PROTOCOL.md) | Spesifikasi protokol biner — header, tipe packet, keamanan |
| [DATABASE.md](./docs/DATABASE.md) | Skema PostgreSQL, ERD, strategi partisi |
| [API.md](./docs/API.md) | REST, gRPC, dan WebSocket API |
| [SECURITY.md](./docs/SECURITY.md) | Zero trust, policy engine ABAC, sandboxing agent |

Desain per subsistem:

| Dokumen | Isi |
|---|---|
| [NETWORK.md](./docs/NETWORK.md) | Topologi jaringan, alokasi port, NAT traversal |
| [STREAMING.md](./docs/STREAMING.md) | Pipeline capture, akselerasi GPU, integrasi codec hardware |
| [AUDIO.md](./docs/AUDIO.md) | Pipeline audio, perbandingan codec, echo cancellation |
| [VIDEO.md](./docs/VIDEO.md) | Camera sharing dan virtual camera driver |
| [MULTI_MONITOR.md](./docs/MULTI_MONITOR.md) | Deteksi monitor, mode tampilan, hot-plug |
| [FILE_TRANSFER.md](./docs/FILE_TRANSFER.md) | Chunking, kompresi, siklus transfer, resume |
| [CLIPBOARD.md](./docs/CLIPBOARD.md) | Sinkronisasi clipboard, penanganan data besar, policy |
| [AGENT.md](./docs/AGENT.md) | Remote shell/PTY, inventaris hardware & software, task manager |
| [VIEWER.md](./docs/VIEWER.md) | Tata letak UI viewer, floating toolbar, overlay statistik |
| [WEB.md](./docs/WEB.md) | Dashboard web — halaman, fitur real-time |
| [SYNC.md](./docs/SYNC.md) | Wake-on-LAN, remote power, remote printing |

Operasional:

| Dokumen | Isi |
|---|---|
| [NEXT_PLAN.md](./docs/NEXT_PLAN.md) | **Rencana berikutnya** — kendali penuh, multi-monitor, agent native |
| [DEPLOYMENT_PLAN.md](./docs/DEPLOYMENT_PLAN.md) | **Rencana deploy Fase 0 yang sedang dijalankan** — server, nginx, DNS, SSL |
| [DEPLOYMENT.md](./docs/DEPLOYMENT.md) | Topologi Kubernetes untuk skala penuh (belum dijalankan) |
| [DEVOPS.md](./docs/DEVOPS.md) | Pipeline CI/CD, code signing, distribusi rilis |
| [TESTING.md](./docs/TESTING.md) | Piramida pengujian, integration test, load test |
| [CODING_STANDARD.md](./docs/CODING_STANDARD.md) | Standar Rust dan TypeScript, pola arsitektur, konvensi git |
| [CONTRIBUTING.md](./docs/CONTRIBUTING.md) | Prasyarat, setup lokal, alur kontribusi |
| [ROADMAP.md](./docs/ROADMAP.md) | Empat fase pengembangan, 24 bulan |
| [CHANGELOG.md](./docs/CHANGELOG.md) | Riwayat perubahan |

---

## Urutan Baca yang Disarankan

**Baru bergabung:** PRD §1-8 → ARCHITECTURE §1-3 → SYSTEM_DESIGN §1

**Mengerjakan backend:** ARCHITECTURE §4.3, §11 → API.md → DATABASE.md → CODING_STANDARD.md

**Mengerjakan agent:** ARCHITECTURE §4.1 → REMOTE_PROTOCOL.md → STREAMING.md → AGENT.md

**Mengerjakan viewer:** ARCHITECTURE §4.2 → VIEWER.md → MULTI_MONITOR.md

**Mengerjakan infrastruktur:** DEPLOYMENT_PLAN.md → DEVOPS.md → NETWORK.md

---

## Status Review

Review menyeluruh atas seluruh spesifikasi menghasilkan **53 temuan**
(8 Blocker, 21 Tinggi, 14 Sedang, 10 Rendah). Perbaikan sedang berjalan —
riwayatnya terlihat di git log dan [`worklog.md`](./worklog.md).

Laporan review: <https://claude.ai/code/artifact/567d8bcf-d1a4-4f6c-9098-285312a0c398>

---

## Lisensi

Belum ditentukan. PRD §7.1 mengklaim "Open Protocol" dan "Self-Hosted" sebagai
pembeda terhadap kompetitor — keduanya menyiratkan keputusan lisensi yang
belum pernah didokumentasikan.
