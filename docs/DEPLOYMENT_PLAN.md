# Deployment Plan — Fase 0

## AetherDesk

**Versi:** 0.1.0
**Tanggal:** 2026-08-08
**Status:** Aktif
**Target:** `aetherdesk.masamune.my.id`

> Dokumen ini adalah rencana deployment **nyata** untuk Fase 0 di server yang sudah tersedia.
> Berbeda dari [DEPLOYMENT.md](./DEPLOYMENT.md) yang menggambarkan topologi Kubernetes
> multi-region untuk skala penuh. Fase 0 berjalan pada satu node tanpa Kubernetes.

---

## 1. Server Target

| Aspek | Nilai |
|---|---|
| Host | `root@192.168.99.63` (LAN) |
| Hostname | `masamune` |
| OS | Ubuntu 22.04.5 LTS (container Proxmox, kernel PVE 6.14.8) |
| Resource | 4 vCPU · 8 GB RAM · 98 GB disk (81 GB kosong) |
| Domain | `aetherdesk.masamune.my.id` |

### 1.1 Layanan yang sudah berjalan — jangan diganggu

Server ini **melayani trafik produksi**. Dua vhost sudah aktif:

| vhost | Domain | Backend |
|---|---|---|
| `masamune` | `masamune.my.id`, `www.masamune.my.id` | Next.js via pm2 di `127.0.0.1:3000` |
| `vid` | `vid.masamune.my.id` | PHP-FPM 8.3 (`/run/php/php8.3-fpm.sock`) |

Konsekuensi yang mengikat:

- `masamune` memegang `default_server` pada `:80` dan `:443`. vhost AetherDesk
  **tidak boleh** dideklarasikan sebagai `default_server`.
- Setiap perubahan nginx wajib lolos `nginx -t` sebelum `systemctl reload nginx`.
  Jangan pernah `restart` — `reload` tidak memutus koneksi berjalan.
- `fail2ban` dan `ufw` aktif. Port baru yang dibuka harus disengaja.

---

## 2. DNS dan IP Origin

### 2.1 Peringatan: `103.189.249.88` kemungkinan besar sudah usang

Komentar pada `/etc/nginx/sites-enabled/vid` menyatakan sendiri:

> `Migrated from the old origin (was 192.168.99.58 / public 103.189.249.88).`

Artinya `.88` adalah IP origin **sebelum** `vid.masamune.my.id` dipindahkan ke server ini.
Tiga nilai berbeda ditemukan di sistem:

| Sumber | IP |
|---|---|
| IP egress server saat ini | `103.189.249.193` |
| `server_name` di vhost `masamune` | `103.189.249.83` |
| `server_name` di vhost `vid` (sisa konfigurasi lama) | `103.189.249.88` |

Kedua domain di-proxy Cloudflare — keduanya resolve ke `104.21.69.142` dan
`172.67.209.30` — sehingga IP origin sesungguhnya hanya terlihat di dashboard Cloudflare.

### 2.2 Yang harus dilakukan

Buat A record berikut di Cloudflare:

| Type | Name | Content | Proxy |
|---|---|---|---|
| A | `aetherdesk` | **salin persis dari record `vid`** | Proxied (awan oranye) |

Jangan mengetik `103.189.249.88` dari ingatan. Buka record `vid.masamune.my.id`
yang sudah terbukti jalan, salin nilai `Content`-nya, pakai nilai itu.

### 2.3 Mode SSL Cloudflare

Set ke **Full (strict)**, sama seperti dua domain lain, karena origin menyajikan
Cloudflare Origin Certificate yang sah.

---

## 3. Struktur Direktori

```
/var/www/aetherdesk.masamune.my.id/
├── repo/           # git checkout — sumber kode, tempat build
├── bin/            # biner rilis hasil build (rdp-api, rdp-signal)
├── dashboard/      # hasil build SPA Vue 3 (statis, disajikan nginx)
├── env/
│   └── aetherdesk.env   # rahasia runtime — mode 0600, TIDAK PERNAH di-commit
└── log/            # log aplikasi (rotasi via logrotate)
```

Pemilik: user sistem khusus `aetherdesk` (tanpa shell login), **bukan** root.
Direktori `env/` mode `0700`, file di dalamnya `0600`.

---

## 4. Komponen yang Dipasang

Semua **native systemd**, tanpa Docker. Alasannya: server ini sudah dikelola dengan
pola systemd + pm2 + nginx, RAM 8 GB dipakai bersama layanan produksi, dan Docker
menambahkan satu lapisan yang tidak memberi manfaat pada deployment satu node.

| Komponen | Versi | Sumber | Bind |
|---|---|---|---|
| PostgreSQL | 16 | PGDG apt repo | `127.0.0.1:5432` |
| Redis | 7.x | apt Ubuntu | `127.0.0.1:6379` |
| Rust toolchain | stable | rustup (user `aetherdesk`) | — |
| `rdp-api` | 0.1.0 | build dari repo | `127.0.0.1:8080` |
| `rdp-signal` | 0.1.0 | build dari repo | `127.0.0.1:8081` |
| Dashboard SPA | 0.1.0 | build Vite → statis | disajikan nginx |

**Seluruh port aplikasi bind ke `127.0.0.1` saja.** Tidak ada port baru yang
terekspos ke LAN maupun internet; nginx satu-satunya pintu masuk.

### 4.1 Yang sengaja belum dipasang di Fase 0

| Komponen | Alasan penundaan |
|---|---|
| NATS JetStream | Satu node, satu proses — event bus in-process sudah cukup. Dipasang saat modul pertama diekstrak. |
| Kubernetes | Tidak proporsional untuk satu node. |
| coturn (TURN) | Butuh port forwarding UDP di router — lihat §7. |
| Laravel / PHP BFF | Dihapus dari arsitektur, lihat ADR-007. |

---

## 5. Konfigurasi nginx

File: `/etc/nginx/sites-available/aetherdesk` → symlink ke `sites-enabled/`.

Mengikuti pola vhost `vid` yang sudah terbukti:

- Cloudflare Origin Certificate di `/etc/ssl/cloudflare/aetherdesk.masamune.my.id.{pem,key}`
- `include /etc/nginx/snippets/cloudflare-real-ip.conf;` untuk memulihkan IP pengunjung
- Authenticated Origin Pull memakai `origin-pull-ca.pem` yang sudah ada
- **Tanpa** `default_server`

Peta rute:

| Path | Tujuan | Catatan |
|---|---|---|
| `/` | `dashboard/` (statis) | SPA fallback ke `index.html` |
| `/api/` | `127.0.0.1:8080` | REST API |
| `/ws` | `127.0.0.1:8081` | WebSocket, perlu header `Upgrade` |
| `/health` | `127.0.0.1:8080/health` | Tanpa access log |

Catatan WebSocket: Cloudflare mendukung WebSocket pada plan gratis, tetapi
memutus koneksi idle sekitar 100 detik. Signal server mengirim ping setiap
25 detik (sesuai ARCHITECTURE.md §8.4), jadi aman.

---

## 6. Sertifikat SSL

Anda menyiapkan sertifikatnya. Saya membuat file kosong dengan permission yang benar
supaya Anda tinggal menempelkan isinya:

| File | Mode | Isi |
|---|---|---|
| `/etc/ssl/cloudflare/aetherdesk.masamune.my.id.pem` | `0644` | Origin Certificate (bagian `CERTIFICATE`) |
| `/etc/ssl/cloudflare/aetherdesk.masamune.my.id.key` | `0600` | Private Key |

Cara membuatnya: Cloudflare → SSL/TLS → Origin Server → Create Certificate,
dengan hostname `aetherdesk.masamune.my.id`. Masa berlaku 15 tahun.

Sampai kedua file terisi, vhost **tidak akan diaktifkan** — nginx menolak
`ssl_certificate` yang kosong dan itu akan menjatuhkan dua situs produksi.
Urutannya: isi sertifikat → `nginx -t` → baru symlink → `reload`.

---

## 7. TURN / Relay — kenapa ditunda

Fase 0 memakai STUN publik saja. Konsekuensinya: koneksi P2P gagal bagi pengguna
di belakang Symmetric NAT (secara industri sekitar 10-20% kasus).

Memasang TURN sendiri di server ini menghadapi dua kendala yang perlu keputusan Anda:

1. **Port forwarding.** TURN butuh UDP `3478` plus rentang media `49152-65535`
   diteruskan dari router ke `192.168.99.63`. Ini konfigurasi di router, bukan di server.
2. **Cloudflare tidak mem-proxy UDP.** TURN harus diakses langsung ke IP origin,
   misalnya lewat subdomain `turn.masamune.my.id` dengan awan abu-abu — yang berarti
   **IP origin Anda menjadi publik**, dan itu menghapus sebagian perlindungan
   Cloudflare untuk seluruh server, termasuk dua situs produksi.

Alternatif yang lebih aman: sewa TURN terkelola (Cloudflare Calls, Twilio NTS,
Metered) atau tempatkan coturn di VPS terpisah yang memang IP-nya boleh publik.
Keputusan ini diambil sebelum Fase 1.

---

## 8. Rahasia Runtime

Dibangkitkan di server, disimpan di `env/aetherdesk.env` mode `0600`, tidak pernah masuk git.

| Variabel | Cara dibangkitkan |
|---|---|
| `AETHERDESK_DB_URL` | password acak 32 byte base64 |
| `AETHERDESK_REDIS_URL` | Redis bind localhost, dengan `requirepass` acak |
| `AETHERDESK_JWT_PRIVATE_KEY` | keypair Ed25519 (ADR-008 — asimetris, bukan HMAC) |
| `AETHERDESK_DEVICE_CA_KEY` | keypair CA Ed25519 untuk sertifikat device |

`.gitignore` memblokir `*.env`, `*.key`, `*.pem` sejak commit pertama.

---

## 9. Urutan Eksekusi

| # | Langkah | Prasyarat | Status |
|---|---|---|---|
| 1 | Buat user sistem `aetherdesk` + struktur direktori | — | menunggu |
| 2 | Pasang PostgreSQL 16 dan Redis 7, bind localhost | — | menunggu |
| 3 | Pasang Rust toolchain untuk user `aetherdesk` | — | menunggu |
| 4 | Buat file SSL kosong dengan permission benar | — | menunggu |
| 5 | **Anda:** tempel Origin Certificate + key | langkah 4 | menunggu |
| 6 | **Anda:** buat A record `aetherdesk` di Cloudflare (§2.2) | — | menunggu |
| 7 | Build `rdp-api`, `rdp-signal`, dashboard | langkah 3 | menunggu |
| 8 | Pasang unit systemd, jalankan, verifikasi di localhost | langkah 7 | menunggu |
| 9 | Aktifkan vhost nginx, `nginx -t`, `reload` | langkah 5, 8 | menunggu |
| 10 | Verifikasi end-to-end lewat domain | langkah 6, 9 | menunggu |

Langkah 5 dan 6 milik Anda. Sisanya saya kerjakan.

---

## 10. Rollback

Setiap langkah dapat dibatalkan tanpa menyentuh layanan produksi:

| Kondisi | Tindakan pemulihan |
|---|---|
| nginx gagal `-t` | Symlink belum dibuat — tidak ada dampak. Perbaiki file, uji lagi. |
| Layanan AetherDesk crash | `systemctl stop aetherdesk-*`. nginx mengembalikan 502 hanya pada subdomain itu. |
| Perlu bersih total | `rm -rf /var/www/aetherdesk.masamune.my.id`, hapus unit systemd, hapus symlink vhost, `dropdb`. Dua situs lain tidak tersentuh. |

Tidak ada langkah dalam rencana ini yang mengubah vhost `masamune` maupun `vid`.
