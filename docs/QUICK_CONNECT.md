# Quick Connect — Device ID & Sesi Sekali Pakai

## AetherDesk

**Versi:** 1.0.0
**Tanggal:** 2026-08-08
**Status:** Draft
**Pemilik:** Security Architect / Product

> Dokumen ini menutup temuan **B-06**. FR-SES-06 ("Temporary session — one-time code")
> berstatus **P0** dan muncul di UC-01, tetapi sebelumnya tidak punya desain sama sekali.
> Satu-satunya jejaknya adalah komentar SQL `eg: 123-456-789` pada `devices.device_id`.

---

## 1. Kenapa dokumen ini penting

Ini adalah alur yang paling sering dipakai pengguna dan paling sering diserang.
Pola "sebutkan ID dan password lewat telepon" adalah antarmuka utama produk sekelas
TeamViewer dan AnyDesk — sekaligus permukaan yang paling banyak disalahgunakan
untuk penipuan dukungan teknis.

Dua ancaman yang harus ditangani sejak desain:

| Ancaman | Konsekuensi bila gagal |
|---|---|
| Penebakan ID massal | Penyerang memindai ruang ID untuk menemukan perangkat hidup |
| Rekayasa sosial | Korban dibujuk menyebutkan ID + password kepada penipu yang menyamar sebagai IT |

Ancaman pertama diselesaikan secara kriptografi. Ancaman kedua tidak bisa —
yang bisa dilakukan adalah membatasi kerusakan dan membuat jejaknya terlihat (§7).

---

## 2. Format Device ID

### 2.1 Struktur

```
   9 4 2   7 1 6   3 8 5
   └─────┴─────┴─────┘
   9 digit, ditampilkan dalam 3 kelompok
```

| Aspek | Nilai |
|---|---|
| Panjang | 9 digit desimal |
| Digit 1-8 | Diambil acak dari CSPRNG |
| Digit 9 | Check digit, algoritma Damm |
| Ruang valid | 10⁸ = 100.000.000 |
| Tampilan | Dikelompokkan `NNN NNN NNN`, spasi hanya untuk dibaca |
| Penyimpanan | Sembilan digit tanpa pemisah |

### 2.2 Kenapa ada check digit

Check digit Damm menangkap seluruh kesalahan satu digit dan seluruh transposisi
dua digit bersebelahan — dua kesalahan paling umum saat seseorang mendiktekan
angka lewat telepon. Efeknya di sisi keamanan: 90% dari string sembilan digit
adalah ID yang tidak valid secara struktur dan ditolak **sebelum** menyentuh
database, sehingga pemindaian buta menjadi sepuluh kali lebih mahal dan
sepuluh kali lebih mudah terlihat.

### 2.3 Alokasi

ID **tidak berurutan**. Alokasi berurutan akan membocorkan usia perangkat dan
membuat pemindaian ruang ID menjadi sepele.

Prosedur alokasi saat registrasi:

1. Bangkitkan 8 digit dari CSPRNG, hitung check digit
2. Coba `INSERT`; bila melanggar constraint unik, ulangi
3. Setelah 5 kali gagal, catat peringatan — kepadatan ruang ID sudah tinggi

Dengan 10 juta perangkat pada ruang 100 juta, kepadatan 10% dan tabrakan alokasi
tetap jarang. Ambang perluasan ruang ID ditetapkan pada **kepadatan 25%**;
setelah itu ID baru diterbitkan dengan 12 digit sementara ID lama tetap berlaku.

---

## 3. Password Sesi

Device ID adalah **alamat, bukan rahasia**. Seluruh kekuatan autentikasi ada di
password sesi.

| Aspek | Nilai |
|---|---|
| Panjang | 8 karakter |
| Alfabet | `23456789ABCDEFGHJKLMNPQRSTUVWXYZ` (32 simbol) |
| Entropi | 32⁸ = 2⁴⁰ ≈ 1,1 triliun kemungkinan |
| Sumber | CSPRNG, tidak pernah diturunkan dari device ID atau waktu |
| Penyimpanan | Argon2id, **tidak pernah** disimpan dalam bentuk asli |

Alfabet sengaja membuang `0`, `O`, `1`, `I`, dan `L` — karakter yang paling sering
tertukar saat dibacakan lewat telepon.

### 3.1 Siklus hidup

| Peristiwa | Perilaku |
|---|---|
| Agent start | Password baru dibangkitkan |
| Sesi berakhir | Password langsung dibangkitkan ulang |
| Idle 30 menit tanpa sesi | Password dibangkitkan ulang |
| User klik "Ganti password" | Segera dibangkitkan ulang |

Sifat sekali pakai inilah yang membedakan alur ini dari unattended access.
Password yang bocor lewat rekaman layar, tangkapan foto, atau riwayat chat
tidak bernilai setelah sesi selesai.

### 3.2 Hubungannya dengan unattended access

Keduanya adalah jalur yang terpisah dan tidak boleh tercampur:

| | Quick Connect | Unattended (FR-SES-07) |
|---|---|---|
| Kredensial | Password sekali pakai, berotasi | Password permanen atau device certificate |
| Persetujuan | **Selalu** perlu klik dari sisi remote | Tidak perlu |
| Aktif | Hanya saat agent berjalan di foreground | Selalu, sebagai service |
| MFA | Tidak berlaku | Wajib (PRD §12.2) |

---

## 4. Alur Koneksi

```
  Viewer                    API Server                      Agent
    │                            │                            │
    │  1. POST /sessions/connect │                            │
    │     { device_id, password }│                            │
    │───────────────────────────►│                            │
    │                            │  2. Validasi check digit   │
    │                            │     (tolak dini bila salah)│
    │                            │                            │
    │                            │  3. Cek rate limit per-ID  │
    │                            │     (§5)                   │
    │                            │                            │
    │                            │  4. Argon2id verify        │
    │                            │                            │
    │                            │  5. CONNECT_REQUEST        │
    │                            │     { viewer_name,         │
    │                            │       viewer_org, ip_geo } │
    │                            │───────────────────────────►│
    │                            │                            │
    │                            │        6. Prompt persetujuan
    │                            │           ditampilkan ke user
    │                            │           (wajib, tidak bisa
    │                            │            dilewati)       │
    │                            │                            │
    │                            │  7. ACCEPT / REJECT        │
    │                            │◄───────────────────────────│
    │  8. { session_id, sdp }    │                            │
    │◄───────────────────────────│                            │
    │                            │                            │
    │  9. Negosiasi WebRTC dengan SDP bertanda tangan (ADR-008)
    │◄═══════════════════════════════════════════════════════►│
```

Langkah 6 tidak dapat dinonaktifkan pada alur Quick Connect. Password yang benar
memberi hak **meminta** koneksi, bukan hak mendapatkannya.

### 4.1 Isi prompt persetujuan

Prompt harus menjawab pertanyaan yang sebenarnya ada di kepala pengguna —
"siapa ini dan apa yang bisa dia lakukan?" — bukan sekadar meminta konfirmasi:

```
┌────────────────────────────────────────────────┐
│  Permintaan akses jarak jauh                   │
│                                                │
│  Nama       Budi Santoso                       │
│  Organisasi PT Contoh Teknologi                │
│  Lokasi     Jakarta, Indonesia                 │
│  Waktu      08 Agu 2026, 14:32                 │
│                                                │
│  Bila diizinkan, orang ini dapat melihat       │
│  layar Anda dan mengendalikan mouse serta      │
│  keyboard Anda.                                │
│                                                │
│  Anda dapat mengakhiri sesi kapan saja.        │
│                                                │
│         [ Tolak ]        [ Izinkan ]           │
└────────────────────────────────────────────────┘
```

Aturan yang mengikat:

- Tombol **Tolak** menjadi fokus awal. Menekan Enter tanpa membaca berarti menolak.
- Tombol **Izinkan** tidak aktif selama 2 detik pertama, untuk mencegah klik refleks
  dan clickjacking.
- Dialog tidak pernah menampilkan teks yang dikirim viewer secara mentah;
  nama dan organisasi diambil dari akun terverifikasi di server, bukan dari input.

---

## 5. Rate Limiting dan Lockout

Pembatasan diterapkan **per device ID**, bukan per IP penyerang. Membatasi per IP
saja tidak berguna karena penyerang dapat berpindah IP dengan mudah.

| Lapisan | Batas | Tindakan saat terlampaui |
|---|---|---|
| Check digit tidak valid | — | Ditolak sebelum query database, tidak dihitung |
| Percobaan gagal per ID | 5 dalam 10 menit | Jeda 15 menit untuk ID tersebut |
| Percobaan gagal per ID | 20 dalam 24 jam | Password diputar, pemilik diberi tahu |
| ID tidak dikenal per IP | 10 dalam 1 jam | IP diblokir 24 jam |
| Global | > 1000 ID tidak dikenal/menit | Alarm — indikasi pemindaian ruang ID |

### 5.1 Respons harus seragam

Server mengembalikan galat yang **identik** untuk ID tidak ada, ID ada dengan
password salah, dan ID sedang dijeda:

```json
{ "error": { "code": "CONNECT_REJECTED",
             "message": "Device ID atau password salah" } }
```

Waktu respons dinormalkan ke nilai tetap. Tanpa ini, selisih waktu antara
"ID tidak ditemukan" dan "Argon2id dijalankan lalu gagal" menjadi oracle yang
memberi tahu penyerang ID mana yang hidup — dan pemindaian ruang ID menjadi murah.

---

## 6. Skema Database

```sql
ALTER TABLE devices
    ADD COLUMN quick_connect_enabled  BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN session_password_hash  VARCHAR(255),
    ADD COLUMN session_password_set_at TIMESTAMP WITH TIME ZONE;

CREATE TABLE quick_connect_attempts (
    id              UUID        NOT NULL DEFAULT gen_random_uuid(),
    device_id_input VARCHAR(9)  NOT NULL,
    source_ip       INET        NOT NULL,
    outcome         VARCHAR(20) NOT NULL,  -- accepted, bad_password,
                                           -- unknown_id, throttled, rejected_by_user
    created_at      TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id, created_at)           -- kolom partisi wajib masuk PK
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_qc_attempts_device ON quick_connect_attempts(device_id_input, created_at DESC);
CREATE INDEX idx_qc_attempts_ip     ON quick_connect_attempts(source_ip, created_at DESC);
```

Catatan: `device_id_input` menyimpan apa yang **diketik**, bukan foreign key —
mayoritas baris pada tabel ini justru merujuk ID yang tidak pernah ada, dan
justru baris itulah sinyal pemindaian yang perlu dianalisis.

Retensi 90 hari, dipartisi bulanan mengikuti pola `connection_logs`.

---

## 7. Mitigasi Penipuan Dukungan Teknis

Rekayasa sosial tidak dapat dicegah secara teknis. Yang bisa dilakukan adalah
membuat serangan lebih mahal dan jejaknya terlihat.

| Kendali | Perilaku |
|---|---|
| Spanduk sesi pertama | Akun yang belum pernah terhubung ke perangkat ini menampilkan peringatan menonjol pada prompt |
| Reputasi akun | Akun berumur < 7 hari ditandai "Akun baru" di dalam prompt |
| Batas laju akun | Akun baru dibatasi 3 perangkat berbeda per 24 jam |
| Pengingat berjalan | Indikator "Sesi remote aktif" selalu tampil di atas jendela lain dan tidak bisa disembunyikan viewer |
| Tombol akhiri | Pintasan global `Ctrl+Alt+Shift+X` memutus sesi seketika, ditangani agent dan tidak dapat dicegat viewer |
| Jeda operasi sensitif | Perintah pertama yang menyentuh perbankan atau kredensial memunculkan konfirmasi ulang |
| Blokir setelah penolakan | Menolak permintaan dari suatu akun memblokir akun itu selama 24 jam |

Kendali "Pengingat berjalan" dan "Tombol akhiri" secara langsung melayani persona
Citra di PRD §4.3, yang pain point utamanya adalah takut kehilangan kendali.

---

## 8. Yang Dicatat ke Audit Log

Setiap upaya, berhasil maupun gagal, menghasilkan entri audit:

| Field | Contoh |
|---|---|
| `action` | `quick_connect.attempt` |
| `outcome` | `accepted` / `bad_password` / `unknown_id` / `throttled` / `rejected_by_user` |
| `device_id_input` | `942716385` |
| `source_ip` | `203.0.113.195` |
| `viewer_user_id` | `usr_7812` bila terautentikasi |
| `latency_ms` | durasi penanganan |

Entri `rejected_by_user` sangat bernilai untuk deteksi penipuan: lonjakan penolakan
pada satu akun viewer adalah sinyal paling awal dan paling jelas dari kampanye
rekayasa sosial yang sedang berjalan.

---

## 9. Requirement yang Ditutup

| ID | Requirement |
|---|---|
| FR-SES-06 | Temporary session (one-time code) — **P0** |
| FR-CON-01 | Alur inisiasi koneksi P2P |
| FR-DEV-01 | Penerbitan device ID saat registrasi |
| UC-01 | Remote IT Support (Attended) |
