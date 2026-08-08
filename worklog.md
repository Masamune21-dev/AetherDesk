# Worklog — AetherDesk

Catatan kronologis pengerjaan. Entri terbaru di atas.
Format: tanggal, ringkasan, detail, dan keputusan yang menunggu jawaban.

---

## 2026-08-08 — Sesi 1: Review dokumentasi, survei server, inisialisasi repo

### Yang dikerjakan

**1. Review 25 dokumen spesifikasi (4.732 baris)**

Hasil: **53 temuan** — 8 Blocker, 21 Tinggi, 14 Sedang, 10 Rendah.
Laporan lengkap: <https://claude.ai/code/artifact/567d8bcf-d1a4-4f6c-9098-285312a0c398>

Delapan Blocker (harus punya jawaban tertulis sebelum kode ditulis):

| ID | Temuan |
|---|---|
| B-01 | Klaim E2E runtuh di lapisan signaling — SDP/fingerprint DTLS tidak ditandatangani device key |
| B-02 | Session recording server-side saling meniadakan dengan E2E |
| B-03 | Secure desktop & session-0 isolation Windows tidak dibahas (FR-INP-03, UC-02 tidak bisa dipenuhi) |
| B-04 | App Sandbox macOS tidak kompatibel dengan agent unattended (butuh TCC + PPPC via MDM) |
| B-05 | Target latency <16ms LAN mustahil secara fisik pada capture 60fps |
| B-06 | FR-SES-06 (koneksi via kode sekali pakai) berstatus P0 tapi tidak punya desain sama sekali |
| B-07 | Jalur frame decoder → canvas Tauri tidak didefinisikan; ARCHITECTURE §4.2 vs VIEWER.md bertentangan |
| B-08 | Reboot Safe Mode akan membuat mesin remote tidak terjangkau secara permanen |

**2. Survei server deploy (`root@192.168.99.63`)**

| Aspek | Kondisi |
|---|---|
| OS | Ubuntu 22.04.5 LTS, container di Proxmox (kernel PVE 6.14.8) |
| Resource | 4 vCPU, 8 GB RAM, 98 GB disk (81 GB kosong) |
| Web server | nginx aktif di :80 dan :443, **sudah dipakai produksi** |
| vhost aktif | `masamune` (default_server, proxy ke Next.js :3000), `vid` (PHP-FPM) |
| Sudah terpasang | nginx, PHP 8.3.31 + FPM, Node v26.3.0 (via nvm), pm2, fail2ban, ufw, git |
| **Belum** terpasang | Docker, PostgreSQL, Redis, NATS, Rust toolchain, certbot |
| Pola SSL | Cloudflare Origin Certificate di `/etc/ssl/cloudflare/<domain>.pem` + `.key` |
| Proteksi origin | Cloudflare Authenticated Origin Pull (`origin-pull-ca.pem` + client cert) |
| Real IP | Snippet `/etc/nginx/snippets/cloudflare-real-ip.conf` (router SNAT dari 192.168.99.1) |

**3. Repo lokal**

- `git init` di `/Users/admin/Documents/Antigravity/Projek/AetherDesk`, branch `main`
- Baseline commit `6595976` — 25 dokumen apa adanya, supaya perbaikan terlihat sebagai diff

### Temuan yang mengubah rencana

**IP origin 103.189.249.88 kemungkinan besar sudah usang.**

Komentar di `/etc/nginx/sites-enabled/vid` menyebut sendiri:
`"Migrated from the old origin (was 192.168.99.58 / public 103.189.249.88)"` —
artinya `.88` adalah IP origin **lama** sebelum `vid.masamune.my.id` dipindah ke server ini.

Bukti pendukung:

| Sumber | IP |
|---|---|
| IP egress server saat ini (`api.ipify.org`) | `103.189.249.193` |
| `server_name` di vhost `masamune` | `103.189.249.83` |
| `server_name` di vhost `vid` (sisa konfigurasi lama) | `103.189.249.88` |

Karena `vid.masamune.my.id` dan `masamune.my.id` keduanya di-proxy Cloudflare
(resolve ke `104.21.69.142` / `172.67.209.30`), IP origin sebenarnya hanya
terlihat di dashboard Cloudflare.

**Tindakan:** saat membuat A record `aetherdesk.masamune.my.id`, **salin IP origin
dari record `vid.masamune.my.id` yang sudah jalan**, jangan pakai `.88` dari ingatan.

### Keputusan yang diambil

| # | Pertanyaan | Keputusan |
|---|---|---|
| 1 | Fokus Fase 0 | **Control plane + viewer browser.** API Rust + signaling + dashboard + agent/viewer berbasis browser. Agent native menyusul saat ada mesin build Windows/macOS. |
| 2 | Stack dashboard | **Vue 3 SPA langsung ke API Rust.** Laravel dihapus dari arsitektur — dicatat sebagai ADR-007. |
| 3 | Auth GitHub | **Deploy key** memakai SSH key yang sudah ada di mesin lokal. |

**4. Perbaikan dokumen — 6 dari 8 Blocker ditutup** (commit `0e3ffb2`)

| Temuan | Penyelesaian |
|---|---|
| B-01, T-11 | ADR-008 — SDP ditandatangani device key Ed25519, SAS untuk sesi attended, JWT pindah ke EdDSA |
| B-02 | ADR-009 — recording dienkripsi di klien, kunci dibungkus kunci publik escrow organisasi |
| B-03 | ADR-010 — agent Windows dipecah jadi service LocalSystem + session agent via `WTSQueryUserToken` |
| B-04 | ADR-011 — macOS pakai hardened runtime + profil PPPC via MDM, bukan App Sandbox |
| B-05 | PRD §6.1 — latency didefinisikan ulang sebagai *added latency*, ditambah baris glass-to-glass |
| B-06 | `QUICK_CONNECT.md` baru — device ID 9 digit + check digit Damm, password sekali pakai 2⁴⁰, rate limit per-ID, mitigasi penipuan |
| B-07 | ADR-012 — viewer merender ke native surface `wgpu` di bawah webview, bukan ke canvas |
| B-08 | SYNC.md §2.1 — prasyarat pendaftaran SafeBoot registry + watchdog pemulihan 15 menit |

Ikut tertutup: S-02 (ADR-007), R-01 dan R-04 (README dengan indeks 25 dokumen).
Ditambahkan: ADR-013 (Fase 0 tanpa NATS/K8s, trait `EventBus` sejak commit pertama),
`.gitignore` yang memblokir `*.env`, `*.key`, `*.pem`.

**5. Persiapan server** — langkah 1 dan 4 dari DEPLOYMENT_PLAN.md §9

```
user sistem   aetherdesk (nologin, home /home/aetherdesk)
direktori     /var/www/aetherdesk.masamune.my.id/{repo,bin,dashboard,env,log}
              env/ mode 0700, sisanya 0755, pemilik aetherdesk
SSL kosong    /etc/ssl/cloudflare/aetherdesk.masamune.my.id.pem  (0644 root)
              /etc/ssl/cloudflare/aetherdesk.masamune.my.id.key  (0600 root)
```

Belum ada satu pun perubahan pada nginx. Dua situs produksi tidak tersentuh.

### Menunggu Anda

| # | Tindakan | Kenapa memblokir |
|---|---|---|
| 1 | Tambahkan deploy key ke repo GitHub (Settings → Deploy keys, centang *Allow write access*) | Tanpa ini push tidak bisa dilakukan |
| 2 | Tempel Cloudflare Origin Certificate ke dua file SSL yang sudah disiapkan | nginx menolak `ssl_certificate` kosong; mengaktifkan vhost sebelum ini terisi akan menjatuhkan dua situs produksi |
| 3 | Buat A record `aetherdesk` di Cloudflare — **salin IP origin dari record `vid`**, jangan pakai `.88` | Lihat temuan IP di bawah |

Kunci untuk langkah 1:

```
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINg0C3lMaddFcO44fuYHE8i0aheYGNcaheI3eecnPPvV masamunekazuto21@gmail.com
```

### Berikutnya dari saya

1. Pasang PostgreSQL 16, Redis 7, dan Rust toolchain di server
2. Scaffold workspace Rust: `rdp-core`, `rdp-api`, `rdp-signal`
3. Skema database awal dengan perbaikan T-05 s/d T-08 sudah diterapkan sejak migrasi pertama
4. Lanjutkan perbaikan 21 Tinggi + 14 Sedang + sisa Rendah

### Catatan operasional

- vhost baru **tidak boleh** `default_server` — slot itu milik `masamune`
- Node/npm ada di `/root/.nvm/versions/node/v26.3.0/bin`, tidak masuk PATH shell non-interaktif
- Server sedang melayani trafik produksi; setiap perubahan nginx wajib `nginx -t` sebelum reload
