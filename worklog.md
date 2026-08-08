# Worklog — AetherDesk

Catatan kronologis pengerjaan. Entri terbaru di atas.
Format: tanggal, ringkasan, detail, dan keputusan yang menunggu jawaban.

---

> **Alamat disamarkan.** Seluruh IP nyata pada dokumen ini diganti placeholder
> agar tidak ikut tersimpan di repositori. Nilai sebenarnya ada di dashboard
> Cloudflare, pada `ip addr` di server, dan di `env/aetherdesk.env`.
>
> | Placeholder | Artinya |
> |---|---|
> | `<IP-ORIGIN>` | IP publik origin yang dipakai `vid` dan `aetherdesk` |
> | `<IP-ORIGIN-2>` | IP publik kedua pada mesin yang sama (`masamune`) |
> | `<IP-EGRESS>` | IP keluar server |
> | `<HOST-LAN>` | Alamat LAN server |
> | `<HOST-LAN-LAMA>` | Alamat LAN host sebelum migrasi |
> | `<GATEWAY-LAN>` | Gateway LAN server |
> | `<SUBNET-LAN>` | Subnet LAN tempat server berada |
> | `<MESIN-DEV>` / `<GATEWAY-DEV>` | Mesin pengembangan dan gateway-nya |
>
> Alamat `203.0.113.x`, `198.51.100.x`, dan `192.0.2.x` yang muncul di contoh
> adalah rentang dokumentasi RFC 5737 — memang bukan alamat siapa pun.

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

**2. Survei server deploy (`root@<HOST-LAN>`)**

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
| Real IP | Snippet `/etc/nginx/snippets/cloudflare-real-ip.conf` (router SNAT dari <GATEWAY-LAN>) |

**3. Repo lokal**

- `git init` di `/Users/admin/Documents/Antigravity/Projek/AetherDesk`, branch `main`
- Baseline commit `6595976` — 25 dokumen apa adanya, supaya perbaikan terlihat sebagai diff

### Temuan yang mengubah rencana

**IP origin <IP-ORIGIN> kemungkinan besar sudah usang.**

Komentar di `/etc/nginx/sites-enabled/vid` menyebut sendiri:
`"Migrated from the old origin (was <HOST-LAN-LAMA> / public <IP-ORIGIN>)"` —
artinya `.88` adalah IP origin **lama** sebelum `vid.masamune.my.id` dipindah ke server ini.

Bukti pendukung:

| Sumber | IP |
|---|---|
| IP egress server saat ini (`api.ipify.org`) | `<IP-EGRESS>` |
| `server_name` di vhost `masamune` | `<IP-ORIGIN-2>` |
| `server_name` di vhost `vid` (sisa konfigurasi lama) | `<IP-ORIGIN>` |

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

**6. Push ke GitHub — berhasil**

Deploy key ternyata sudah terdaftar di akun. Repo:
<https://github.com/Masamune21-dev/AetherDesk>, branch `main`.

**7. Koreksi temuan IP origin — `.88` ternyata masih benar**

Setelah melihat dashboard Cloudflare: record `vid.masamune.my.id` memang masih
memakai `<IP-ORIGIN>` dan situsnya jalan normal. Kesimpulannya router mem-forward
beberapa IP publik ke host internal yang sama:

| Domain | IP origin | Menuju |
|---|---|---|
| `masamune.my.id` | `<IP-ORIGIN-2>` | `<HOST-LAN>` |
| `vid.masamune.my.id` | `<IP-ORIGIN>` | `<HOST-LAN>` |
| `aetherdesk.masamune.my.id` | `<IP-ORIGIN>` | `<HOST-LAN>` |

Komentar "old origin" pada vhost `vid` merujuk pada perpindahan *host internal*
(`<HOST-LAN-LAMA>` → `.63`), bukan perubahan IP publik. Record `aetherdesk` sudah benar.

**8. Sertifikat SSL diverifikasi**

```
SAN         DNS:aetherdesk.masamune.my.id
Issuer      CloudFlare Origin SSL Certificate Authority
Berlaku     8 Agu 2026 → 4 Agu 2041
Key match   cocok (MD5 pubkey cert == MD5 pubkey key)
```

**9. vhost nginx aktif — situs live**

File `/etc/nginx/sites-available/aetherdesk` → symlink ke `sites-enabled/`.
Urutan aman dipatuhi: tulis → symlink → `nginx -t` → baru `reload`.

| Rute | Tujuan |
|---|---|
| `/` | SPA statis dengan fallback `index.html` |
| `/api/` | `127.0.0.1:8080` |
| `/ws` | `127.0.0.1:8081`, header Upgrade, timeout 3600s |
| `/nginx-health` | 200 `nginx-ok`, tanpa access log |

Verifikasi:

| Uji | Hasil |
|---|---|
| `https://aetherdesk.masamune.my.id/` lewat Cloudflare | **200** (cf-ray edge SIN) |
| `/nginx-health` lewat Cloudflare | **200** `nginx-ok` |
| `/api/health` | 502 — wajar, `rdp-api` belum ada |
| **Regresi** `masamune.my.id` | **200** |
| **Regresi** `vid.masamune.my.id` | **200** |

Halaman status sementara terpasang di `dashboard/index.html` — memeriksa ketiga
komponen tiap 5 detik, jadi kemajuan deploy terlihat langsung dari browser.

**10. Dependensi terpasang**

| Komponen | Versi | Bind | Catatan |
|---|---|---|---|
| PostgreSQL | 16.14 (PGDG) | `127.0.0.1:5432` | Ubuntu 22.04 hanya menyediakan PG14, jadi repo PGDG ditambahkan |
| Redis | 6.0.16 (Ubuntu) | `127.0.0.1:6379` | **Menyimpang dari dokumen** yang menyebut Redis 7 — Fase 0 hanya memakai SET/GET/EXPIRE/pubsub, tidak ada fitur 7.x yang dibutuhkan |
| build-essential, pkg-config, libssl-dev | — | — | prasyarat kompilasi Rust |

Database `aetherdesk` dan role `aetherdesk` dibuat, Redis diberi `requirepass`.
Kredensial acak 32 karakter ditulis ke `env/aetherdesk.env` mode `0600`,
diblokir `.gitignore`. Keduanya diuji: `PostgreSQL 16.14` dan `PONG`.

**11. Workspace Rust — `rdp-core` dan `rdp-api` berjalan di produksi**

Struktur mengikuti ARCHITECTURE.md §11.1.

`rdp-core` — tanpa dependensi framework, database, maupun message bus.
Batas itu yang membuat ADR-005 dapat ditegakkan, bukan sekadar dijanjikan.

| Modul | Isi |
|---|---|
| `damm` | Check digit device ID, dengan test yang membuktikan **seluruh** kesalahan satu digit dan **seluruh** transposisi bersebelahan tertangkap |
| `ids` | Newtype `DeviceId`, `UserId`, `OrgId`, `SessionId`, `DeviceUuid` |
| `password` | Password sesi 8 karakter, alfabet 32 simbol, entropi 40 bit |
| `event` | `DomainEvent` + trait `EventBus` (ADR-013) dengan `InProcessBus` |
| `error` | `CoreError` — sengaja **tanpa** varian infrastruktur, agar `rdp-core` tidak menarik `sqlx`/`redis` |

`rdp-api` — Axum, kolam koneksi, endpoint kesehatan, shutdown rapi via SIGTERM.

**Tiga bug ditemukan oleh test, bukan oleh pengguna:**

| Bug | Detail |
|---|---|
| Fixture Damm salah | `942716385` tidak lolos Damm; check digit yang benar `2`. QUICK_CONNECT.md ikut dikoreksi. |
| Alfabet password kontradiktif | Dokumen menyatakan membuang `0 O 1 I L`, tetapi kelimanya hanya menyisakan **31** simbol — bukan 32 seperti yang diklaim. Diputuskan `L` dipertahankan (kerancuan `l`/`1` hanya ada pada huruf kecil) sehingga entropi tepat 40 bit. Dokumen dan kode kini sepakat. |
| Prefiks path | nginx meneruskan URI apa adanya, jadi route harus hidup di bawah `/api`. Sekaligus menuntaskan **R-05** — satu bentuk path untuk seluruh sistem. |

Hasil akhir: **37 test lulus, 0 gagal.**

**12. Migrasi database — `migrations/0001_initial.sql`**

Perbaikan diterapkan sejak migrasi pertama, dan masing-masing **dibuktikan**, bukan
diasumsikan:

| Temuan | Perbaikan | Bukti |
|---|---|---|
| T-05 | `UNIQUE (organization_id, email)` | Email sama di dua org berhasil; duplikat dalam satu org ditolak |
| T-06 | `PRIMARY KEY (id, created_at)` pada tabel terpartisi | Terverifikasi pada ketiga tabel |
| T-06 | `ON DELETE SET NULL` + snapshot identitas pada `sessions` | Organisasi dengan sesi historis kini bisa dihapus |
| T-07 | Tabel `groups` didefinisikan, FK `devices.group_id` ditambahkan | — |
| T-07 | Kolom `version` + trigger OCC | Naik otomatis, tidak bergantung disiplin pemanggil |
| T-07 | Policy RLS pada 6 tabel, `FORCE ROW LEVEL SECURITY` | Tenant Alpha hanya melihat 1 dari 2 pengguna |
| T-08 | Trigger append-only pada `audit_logs` | `UPDATE` dan `DELETE` keduanya ditolak |
| T-01 | Kolom `mac_address MACADDR` | Wake-on-LAN kini mungkin dibentuk |
| R-08 | `ip_address INET` menggantikan `VARCHAR(45)` | — |
| — | Partisi `DEFAULT` pada ketiga tabel | Audit trail tidak berhenti diam-diam bila cron partisi terlewat |

**13. Layanan berjalan**

`aetherdesk-api.service` — systemd dengan hardening penuh: `ProtectSystem=strict`,
`MemoryDenyWriteExecute`, `RestrictAddressFamilies`, `NoNewPrivileges`.

```
$ curl https://aetherdesk.masamune.my.id/api/health
{"status":"ok","service":"rdp-api","version":"0.1.0"}

$ curl https://aetherdesk.masamune.my.id/api/health/ready
{"status":"ready","checks":[{"name":"postgres","ok":true,"latency_ms":0},
                            {"name":"redis","ok":true,"latency_ms":0}]}
```

Regresi diperiksa ulang setelah setiap perubahan nginx: `masamune.my.id` **200**,
`vid.masamune.my.id` **200**.

**14. Modul auth, device, dan Quick Connect — ditulis, belum terverifikasi build**

Ada di branch `feat/auth-quickconnect`, **bukan** `main`. Alasannya di bagian
berikutnya. `main` sengaja dipertahankan hanya berisi commit yang sudah terbukti
hijau.

| Berkas | Isi |
|---|---|
| `migrations/0002_lookup_functions.sql` | Empat fungsi `SECURITY DEFINER` untuk lookup lintas-tenant |
| `auth/hash.rs` | Argon2id, parameter OWASP 2024 (19 MiB, t=2, p=1) |
| `auth/jwt.rs` | JWT EdDSA sesuai ADR-008, algoritma dikunci saat verifikasi |
| `auth/mod.rs` | Ekstraktor `Terautentikasi` |
| `net.rs` | Ekstraktor `IpKlien` dari `X-Real-IP` |
| `ratelimit.rs` | Batas per device ID, bukan per IP |
| `db.rs` | Transaksi bercakupan tenant lewat `set_config` |
| `error.rs` | Amplop respons API.md §3, error infrastruktur tidak bocor |
| `routes/auth.rs` | bootstrap, login, me |
| `routes/devices.rs` | daftar, daftar semua, rotasi password |
| `routes/connect.rs` | Quick Connect |

Tiga keputusan yang muncul saat menulis, dan alasannya:

**Login sekarang wajib menyertakan `org_slug`.** Ini konsekuensi langsung T-05.
Begitu email hanya unik per organisasi, `email + password` tidak lagi menunjuk ke
satu orang — dua organisasi boleh punya `erik@msp.id` yang berbeda. API.md perlu
diperbarui mengikuti ini.

**Empat fungsi `SECURITY DEFINER` ditambahkan.** T-07 mengaktifkan `FORCE RLS`,
sehingga setiap query harus tahu tenant lebih dulu — padahal saat login dan saat
Quick Connect, tenant justru **belum** diketahui. Fungsi-fungsi ini sangat sempit:
masing-masing hanya mengembalikan kolom minimum untuk menentukan tenant.

**`periksa()` dipisah dari `catat_kegagalan()`.** Kalau digabung, percobaan yang
sudah dijeda akan memperpanjang jedanya sendiri, dan penyerang dapat mengunci
pemilik perangkat selamanya — pembatasan laju berubah menjadi denial of service.

---

**15. Blocker jaringan teratasi — akses lewat IP publik**

Solusinya sederhana: SSH langsung ke `root@<IP-ORIGIN>`.

Penemuan yang menjelaskan banyak hal sebelumnya: `hostname -I` menunjukkan box
ini punya **tiga alamat sekaligus** — `<HOST-LAN>`, `<IP-ORIGIN-2>`, dan
`<IP-ORIGIN>`. Bukan NAT, melainkan IP publik yang terikat langsung ke host.
Itulah sebabnya `.83` dan `.88` sama-sama bekerja, dan kenapa kekhawatiran awal
tentang `.88` yang "usang" memang tidak berdasar.

Catatan sampingan: `load average` sempat terbaca 5,26 pada box 4-core, tetapi
CPU justru 85,7% idle dengan RAM 1,8 GB bebas. Itu artefak LXC — `/proc/loadavg`
di dalam container menampilkan beban **host Proxmox**, bukan container. Bukan
masalah.

**16. Tiga bug ditemukan uji end-to-end**

| # | Bug | Sebab |
|---|---|---|
| 1 | Build gagal: `IpAddr` tidak dapat di-bind | `sqlx` tidak memetakan `IpAddr` ke `INET` tanpa fitur `ipnetwork`. Diperbaiki dengan cast `$2::inet` di SQL — lebih ringan daripada menambah dependensi. |
| 2 | Test gagal kompilasi: `start_paused` tidak dikenal | Fitur `test-util` tokio tidak termasuk dalam `full`. Ditambahkan sebagai dev-dependency. |
| 3 | **Login selalu 401 meski kredensial benar** | Lihat di bawah — ini yang paling penting. |

**Bug ketiga layak dicatat khusus.** `FORCE ROW LEVEL SECURITY` berlaku pada
pemilik tabel **termasuk di dalam fungsi `SECURITY DEFINER` yang dimiliki role
yang sama**. `resolve_login` berjalan sebagai `aetherdesk`, tetap terkena RLS,
dan karena tenant memang belum diketahui saat login, policy menyaring seluruh
baris. Fungsi mengembalikan nol baris dan pemanggil menyimpulkan password salah.

Pelajarannya: **`SECURITY DEFINER` bukan mekanisme bypass RLS.** Yang mem-bypass
RLS adalah atribut `BYPASSRLS` pada role, atau status superuser.

`migrations/0003_lookup_bypass_role.sql` memperbaikinya dengan role khusus
`aetherdesk_lookup` (`NOLOGIN BYPASSRLS`) sebagai pemilik keempat fungsi.
Sengaja **bukan** `postgres`: menjadikan superuser pemilik fungsi
`SECURITY DEFINER` berarti setiap cacat di dalamnya berakibat kompromi total.

Pemisahan role akhirnya menjadi:

| Role | Login | BYPASSRLS | Peran |
|---|---|---|---|
| `aetherdesk` | ya | **tidak** | runtime aplikasi, tunduk pada RLS |
| `aetherdesk_app` | tidak | tidak | disiapkan untuk pemisahan lebih lanjut |
| `aetherdesk_lookup` | tidak | **ya** | hanya memiliki empat fungsi lookup |

**17. Uji end-to-end — 26 dari 26 lulus**

`scripts/e2e.sh` menjalankan alur penuh sekaligus memverifikasi properti keamanan
yang mudah hilang saat refactor:

```
1. Kesehatan          liveness, readiness
2. Bootstrap          organisasi pertama, slug tidak valid ditolak
3. Login              password salah, org tidak dikenal, login berhasil
4. Autentikasi        tanpa token, token sampah, token sah
5. Perangkat          device ID 9 digit, password 8 karakter dari alfabet benar
6. Quick Connect      check digit, respons seragam, lantai waktu 304 ms,
                      kredensial benar, normalisasi huruf kecil
7. Pembatasan laju    5 kegagalan menjeda, kredensial benar pun ditolak,
                      jeda tidak merembet ke perangkat lain
```

Pembatasan laju diverifikasi secara perilaku, bukan sekadar "request terkirim":
setelah lima password salah, **password yang benar pun ditolak**, dan `throttled`
tercatat di `quick_connect_attempts`.

Satu subtlety yang ditemukan dan diterima: lantai waktu respons hanya berlaku
pada badan handler, bukan pada penolakan di ekstraktor autentikasi. Ini tidak
merugikan — request tanpa token tidak pernah menyentuh data perangkat, jadi tidak
ada yang bisa dibocorkan lewat selisih waktunya.

**18. Header JWT terverifikasi**

```json
{"typ":"JWT","alg":"EdDSA"}
```

ADR-008 terpenuhi di tingkat wire, bukan hanya di dokumen.

**19. `rdp-signal` — Signal Server berjalan**

Crate ketiga. Meneruskan SDP dan kandidat ICE antara viewer dan agent, serta
memelihara kehadiran perangkat.

| Modul | Isi |
|---|---|
| `protocol.rs` | Enum pesan masuk/keluar sesuai amplop API.md §9 |
| `registry.rs` | Registri koneksi in-memory + peta sesi (ADR-013) |
| `presence.rs` | Kehadiran perangkat — **menutup temuan S-09** |
| `auth.rs` | Verifikasi JWT saja, tanpa kemampuan menerbitkan |
| `main.rs` | Handler WebSocket, ping 25 detik, routing pesan |

Tiga keputusan yang layak dicatat:

**Server tidak pernah membaca isi SDP.** Ia hanya merutekan byte apa adanya.
Begitu server ikut menormalkan atau memformat ulang SDP, tanda tangan device key
yang diwajibkan ADR-008 langsung batal. Ada test khusus yang menjaga properti ini.

**Setiap pesan bersesi diperiksa keanggotaannya.** Tanpa itu, siapa pun yang
terautentikasi dapat menyuntikkan SDP atau kandidat ICE ke sesi orang lain hanya
dengan menebak `session_id` — pembajakan sesi tanpa perlu menembus kripto apa pun.

**Verifikasi JWT saja, tanpa kunci privat.** Inilah nilai praktis ADR-008 yang
terasa langsung: dengan HMAC, Signal Server mau tidak mau ikut memegang kemampuan
menerbitkan token. Dengan Ed25519, cukup kunci publik yang dipasang di sini.

**20. S-09 ditutup — offline seketika**

ARCHITECTURE.md §8.4 mengandalkan TTL Redis 90 detik, sehingga mesin yang mati
mendadak tetap tampil online sampai satu setengah menit. Itu bertabrakan dengan
FR-DEV-06 yang menjanjikan status real-time.

Sekarang transisi offline terjadi langsung saat WebSocket putus. TTL tetap ada
tetapi turun perannya menjadi jaring pengaman untuk kasus tanpa event putus —
proses dibunuh paksa, node signal mati, atau jaringan hilang tanpa FIN.

Terverifikasi: **offline dalam < 1 detik.**

**21. Bug ditemukan uji signaling**

Pesan `ERROR` untuk token palsu tidak pernah sampai ke klien. Penyebabnya
`penulis.abort()` membunuh task penulis **sebelum** pesan sempat mengalir ke
socket. Klien hanya melihat koneksi tertutup tanpa alasan.

Diperbaiki dengan `tutup_setelah_terkuras()`: seluruh pengirim dilepas sehingga
channel tertutup, task penulis menguras antrean lalu berhenti sendiri, dengan
batas waktu 2 detik agar klien yang berhenti membaca tidak menahannya selamanya.

**22. Uji signaling — 16/16 lulus, termasuk lewat Cloudflare**

```
1. Persiapan          bootstrap, token, daftar perangkat
2. Autentikasi WS     token palsu ditolak, agent, viewer
3. Presence           online saat agent terhubung
4. Alur sesi          tawaran, persetujuan, SDP byte-per-byte,
                      SDP answer, kandidat ICE utuh
5. Isolasi sesi       pihak luar ditolak menyuntik SDP maupun
                      mengakhiri sesi orang lain
6. Akhiri sesi        agent diberi tahu
7. Offline seketika   < 1 detik, bukan TTL 90 detik
```

Dijalankan dua kali: lokal (`ws://127.0.0.1:8081`) dan lewat internet
(`wss://aetherdesk.masamune.my.id/ws`). Keduanya 16/16.

Total pengujian: **84 unit test + 26 uji API + 16 uji signaling.**

**23. Halaman status diperbaiki**

Probe `rdp-signal` sebelumnya memakai `fetch` ke `/ws`, yang selalu menghasilkan
400 karena bukan permintaan upgrade — dan itu bukan indikasi layanannya mati.
Sekarang memakai WebSocket sungguhan, plus menampilkan latensi PostgreSQL dan
Redis dari `/api/health/ready`.

**24. Agent dan viewer berbasis browser**

Empat halaman di `web/`, tanpa framework dan tanpa langkah build. Ini cikal
bakal "Zero-Install Viewer" di PRD §17.2 — semakin sedikit yang harus diunduh
sebelum layar muncul, semakin baik.

| Halaman | Isi |
|---|---|
| `/` | Status layanan + navigasi |
| `/setup` | Membuat organisasi pertama |
| `/agent` | Berbagi layar via `getDisplayMedia`, prompt persetujuan |
| `/viewer` | Quick Connect, video, HUD statistik |

**Prompt persetujuan diwujudkan persis seperti QUICK_CONNECT.md §4.1:**

- Tombol **Tolak** diletakkan lebih dulu dan menerima `autofocus` — menekan
  Enter tanpa membaca berarti menolak, bukan mengizinkan
- Tombol **Izinkan** terkunci tiga detik dengan hitung mundur terlihat,
  mencegah klik refleks dan clickjacking
- Nama dan email diambil dari klaim token yang sudah terverifikasi server,
  bukan dari input yang dikirim viewer

**T-10 diselesaikan di kode.** Dokumen menjawab dua kali secara berlawanan soal
siapa pengirim SDP offer: ARCHITECTURE.md §7.1 dan API.md §9 menempatkan agent,
sementara §8.1 menempatkan viewer. Implementasi memakai **agent sebagai offerer**
— agent yang memiliki media, jadi ia yang menawarkan.

HUD viewer menampilkan latensi, FPS, resolusi, dan bitrate dari `getStats()`,
sesuai VIEWER.md §2.

**25. Verifikasi**

| Uji | Hasil |
|---|---|
| Seluruh halaman lewat Cloudflare | **200** (`/`, `/setup`, `/agent`, `/viewer`, `/app.js`, `/style.css`) |
| Sintaks `app.js` dan skrip inline ketiga halaman | bersih |
| Regresi dua situs produksi | **200** |

URL bersih tanpa `.html` lewat `try_files $uri $uri.html $uri/ /index.html`.

**Yang belum bisa saya verifikasi sendiri:** jalur media WebRTC dari ujung ke
ujung. Signaling sudah terbukti 16/16 termasuk lewat `wss://`, tetapi negosiasi
media memerlukan dua browser sungguhan. Itu perlu diuji manusia — buka `/agent`
di satu tab dan `/viewer` di tab lain.

Batasan yang sudah diketahui: Fase 0 hanya memakai STUN publik, jadi koneksi
akan gagal bagi jaringan di belakang Symmetric NAT (secara industri 10-20%
kasus). Viewer menampilkan pesan yang menjelaskan hal ini alih-alih sekadar
"gagal". Lihat DEPLOYMENT_PLAN.md §7 untuk keputusan TURN yang tertunda.

**26. Empat bug ditemukan dari pengujian Anda**

Urutannya menarik: setiap perbaikan membuka lapisan berikutnya.

| # | Gejala | Sebab sebenarnya |
|---|---|---|
| 1 | Koneksi `failed` meski signaling bersih | Kandidat ICE dari agent tiba sebelum viewer membuat peer connection, lalu dibuang. Bug trickle-ICE klasik. |
| 2 | Tombol Masuk mati total | `expires 7d` pada `app.js`. Browser menahan versi lama sementara HTML sudah baru; impor gagal dan seluruh skrip berhenti. |
| 3 | "tidak terautentikasi" padahal kredensial benar | `/auth/refresh` yang dijanjikan ARCHITECTURE.md §6.2 tidak pernah diimplementasikan. Sesi mati tiap 15 menit tanpa jalan kembali ke form masuk. |
| 4 | P2P gagal lintas jaringan | Ini yang **asli** — memang butuh TURN. |

Gejala 1 sempat menampilkan pesan "Symmetric NAT", padahal itu tebakan default
yang kebetulan salah sasaran. Diagnosanya kini berbasis bukti: nol kandidat
berarti WebRTC diblokir, hanya `host` berarti STUN tidak terjangkau, dan
Symmetric NAT baru disebut bila `srflx` ada tetapi `relay` tidak.

**27. TURN terpasang — coturn di server sendiri**

Temuan yang menyederhanakan rencana: `ip addr` menunjukkan `<IP-ORIGIN-2>/32`
dan `<IP-ORIGIN>/32` **terikat langsung ke `eth0`**, bukan hasil NAT.
DEPLOYMENT_PLAN.md §7 sebelumnya menyatakan perlu port forwarding di router —
itu keliru dan sudah dikoreksi. Cukup membuka ufw.

| Aspek | Nilai |
|---|---|
| Alamat | `<IP-ORIGIN>:3478` UDP dan TCP |
| Rentang relay | `49160-49260` UDP |
| Autentikasi | HMAC berumur pendek, TTL 6 jam |
| Endpoint | `GET /api/v1/turn-credentials`, wajib terautentikasi |

**Pengerasan adalah bagian terpentingnya.** Server berada di `<SUBNET-LAN>`
bersama host Proxmox dan mesin lain. TURN tanpa pembatasan dapat dipakai siapa
pun yang memperoleh kredensial untuk meneruskan paket ke seluruh LAN itu —
berubah menjadi pintu masuk jaringan. Konfigurasi memuat **13 aturan
`denied-peer-ip`** yang menutup rentang privat, loopback, link-local, CGNAT,
multicast, beserta padanan IPv6-nya, ditambah `no-multicast-peers` dan kuota.

Verifikasi:

| Uji | Hasil |
|---|---|
| Alokasi relay dengan kredensial HMAC | berhasil, 0 paket hilang |
| STUN Binding dari internet | balas **6 ms**, Binding Success |
| TCP 3478 dari internet | terbuka |
| 96 unit test | seluruhnya lulus |

Konsekuensi yang diterima dan dicatat: IP origin kini diketahui publik. Yang
tersisa sebagai perlindungan adalah ufw yang menolak 80/443 dari luar rentang
Cloudflare, sehingga mengetahui IP tidak memberi akses ke layanan web. Sisa
risiko nyata: DDoS volumetrik langsung ke IP.

`infra/turn/turnserver.conf` dan `infra/nginx/aetherdesk.conf` disalin ke repo
supaya konfigurasi yang berjalan terlacak, bukan hanya hidup di mesin itu.

---

## ~~Blocker~~ — rute jaringan ke server putus (SELESAI)

> Teratasi pada butir 15. Dipertahankan sebagai catatan diagnosis.

Terjadi di tengah pengerjaan, setelah commit `7693636` berhasil di-deploy.

### Yang tidak terpengaruh

Seluruh layanan **tetap berjalan normal**:

| Endpoint | Status |
|---|---|
| `https://aetherdesk.masamune.my.id/api/health` | **200** |
| `https://masamune.my.id/` | **200** |
| `https://vid.masamune.my.id/` | **200** |

`/api/health` yang menjawab 200 membuktikan `rdp-api`, PostgreSQL, dan Redis
semuanya masih hidup. Tidak ada yang rusak, dan tidak ada data yang hilang.

### Yang terpengaruh

Hanya jalur SSH dari mesin pengembangan ke `<HOST-LAN>`.

### Diagnosis

| Uji | Hasil |
|---|---|
| SSH `:22` | timeout (3 percobaan) |
| ICMP ke `<HOST-LAN>` | 100% packet loss |
| TCP `:80` dan `:443` dari LAN | tidak merespons |
| Gateway lokal `<GATEWAY-DEV>` | **hidup**, 2/2 ping |
| `netstat -rn \| grep 192.168.99` | **kosong — tidak ada rute** |
| Interface `utun0`–`utun3` | up, tetapi tidak membawa rute tersebut |

**Bukan** fail2ban: kalau itu penyebabnya, hanya port 22 yang terblokir, sementara
ICMP dan port 80/443 juga mati. **Bukan** server bermasalah: ketiga situs tetap
melayani trafik lewat internet.

Kesimpulan: rute `<SUBNET-LAN>` hilang dari tabel routing mesin pengembangan.
Mesin ini berada di `<MESIN-DEV>` — subnet berbeda — sehingga aksesnya selalu
bergantung pada rute yang kini tidak ada.

### Yang perlu Anda lakukan

Aktifkan kembali tunnel atau rute yang menyediakan akses ke `<SUBNET-LAN>`.
Setelah itu cukup bilang "sudah", dan saya lanjutkan.

### Status yang belum diketahui

Perintah pembangkitan keypair JWT terputus saat timeout, jadi belum dipastikan
apakah `env/jwt_ed25519.pem` sempat terbentuk. Skripnya idempoten (`if [ ! -s ]`),
jadi menjalankannya ulang aman apa pun kondisinya.

### Berikutnya setelah akses pulih

1. Bangkitkan keypair JWT, build branch `feat/auth-quickconnect`, jalankan test
2. Terapkan migrasi 0002, uji alur end-to-end: bootstrap → login → daftar
   perangkat → Quick Connect
3. Merge ke `main` setelah hijau
4. `rdp-signal`: WebSocket signaling
5. Dashboard Vue 3 + agent/viewer berbasis browser
6. Lanjutkan perbaikan 21 Tinggi + 14 Sedang + sisa Rendah

### Catatan operasional

- vhost baru **tidak boleh** `default_server` — slot itu milik `masamune`
- Node/npm ada di `/root/.nvm/versions/node/v26.3.0/bin`, tidak masuk PATH shell non-interaktif
- Server sedang melayani trafik produksi; setiap perubahan nginx wajib `nginx -t` sebelum reload
