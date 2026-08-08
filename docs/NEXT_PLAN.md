# Rencana Berikutnya — Kendali Penuh & Multi-Monitor

## AetherDesk

**Versi:** 1.0.0
**Tanggal:** 2026-08-08
**Status:** Menunggu keputusan
**Prasyarat:** Fase 0 selesai — signaling, TURN, Quick Connect, dan streaming
layar berbasis browser sudah berjalan di produksi

---

## 1. Yang diminta

Dua kemampuan, dan keduanya bermuara pada satu pekerjaan yang sama:

1. **Kendali** — menggerakkan mouse dan mengetik pada komputer yang diakses
2. **Seluruh layar, semua monitor** — bila mesin tujuan punya tiga layar,
   ketiganya dapat diakses dan dipindah-pindah dari viewer

---

## 2. Kenapa keduanya mustahil dengan agent berbasis browser

Ini bagian terpenting dokumen ini, karena menentukan seluruh sisanya.

### 2.1 Browser tidak dapat menyuntikkan input ke sistem operasi

Tidak ada API web untuk menggerakkan kursor atau menekan tombol di luar
halamannya sendiri. Ini bukan kekurangan yang belum diisi — ia **batas keamanan
yang disengaja**. Sebuah situs yang dapat mengetik di mesin pengunjungnya adalah
definisi dari sebuah eksploitasi.

Agent Fase 0 berjalan di dalam tab. Ia dapat *menampilkan* layar karena pengguna
memberi izin lewat `getDisplayMedia`, tetapi tidak akan pernah dapat
*mengendalikan* apa pun.

### 2.2 Browser tidak dapat menyebutkan atau memindah monitor

`getDisplayMedia` memunculkan pemilih milik browser. Penggunalah yang memilih
satu layar, dan halaman hanya menerima aliran video hasil pilihan itu. Halaman
tidak pernah tahu ada berapa monitor, berapa resolusinya, di mana posisinya,
apalagi dapat berpindah tanpa memunculkan pemilih lagi.

Bahkan bila pengguna memilih "Entire Screen" pada sistem tiga monitor, yang
diperoleh tetap **satu** layar.

### 2.3 Kesimpulan

Kedua permintaan memerlukan **agent native** — program yang dipasang dan
berjalan sebagai proses sistem operasi, bukan di dalam tab.

Kabar baiknya: seluruh tulang punggungnya sudah berdiri dan terbukti. Signaling,
TURN, Quick Connect, siklus hidup sesi, audit, dan presence sudah berjalan.
Agent native menggantikan satu ujung, bukan membangun ulang sistemnya.

---

## 3. Prasyarat yang belum ada

| Kebutuhan | Keadaan sekarang |
|---|---|
| Mesin build Windows | **Belum ada** |
| Mesin build macOS | Ada (mesin pengembangan Anda) |
| Sertifikat code signing Windows | Belum ada — lihat temuan T-18 |
| Apple Developer ID + notarisasi | Belum ada |
| Mesin uji dengan 3 monitor | **Milik Anda** — ini yang penting |

Agent native tidak dapat dikompilasi silang dengan mudah: capture layar dan
injeksi input memakai API sistem yang hanya ada di platformnya. Windows harus
dibangun dan diuji di Windows.

---

## 4. Arsitektur agent

Mengikuti ADR-010 (Windows) dan ADR-011 (macOS) yang sudah ditetapkan.

### 4.1 Windows — dua proses, bukan satu

```
  aetherdesk-service          LocalSystem, autostart
    ├── identitas perangkat (kunci Ed25519)
    ├── koneksi ke server + signaling
    ├── auto-update, watchdog
    └── memantau WTS_SESSION_CHANGE
             │
             │ CreateProcessAsUser + WTSQueryUserToken
             ▼
  aetherdesk-session          di dalam sesi interaktif
    ├── capture layar (DXGI / WGC)
    ├── injeksi input (SendInput)
    └── enumerasi monitor
```

Pemisahan ini bukan pilihan gaya. Proses di dalam sesi user tidak dapat
menangkap secure desktop — prompt UAC, Ctrl+Alt+Del, layar login. Service
LocalSystem yang meluncurkan ulang session agent saat sesi berpindah adalah
satu-satunya cara yang didukung Windows.

### 4.2 macOS

Daemon `launchd` dengan Hardened Runtime dan notarisasi. **Bukan** App Sandbox —
izin Screen Recording dan Accessibility tidak dapat diperoleh di dalamnya.

Untuk unattended, profil PPPC lewat MDM tetap menjadi prasyarat.

---

## 5. Multi-monitor

Bagian yang paling banyak jebakannya, dan sudah sebagian dirancang di
[MULTI_MONITOR.md](./MULTI_MONITOR.md).

### 5.1 Enumerasi

| Platform | API |
|---|---|
| Windows | `EnumDisplayMonitors` → koordinat virtual screen, bounding box, penanda primary |
| macOS | `CGGetActiveDisplayList` |

Agent mengirim `MONITOR_LAYOUT` saat sesi dimulai dan setiap kali topologi
berubah (`WM_DISPLAYCHANGE` / `CGDisplayRegisterReconfigurationCallback`).

### 5.2 Koordinat negatif — jebakan yang sudah ditandai

Temuan **T-16** berlaku persis di sini. Monitor sekunder yang diletakkan di
sebelah **kiri** monitor primer memiliki koordinat X **negatif** pada virtual
desktop. Ini konfigurasi yang sangat umum.

Konsekuensinya mengikat:

- Koordinat mouse pada protokol wajib **signed** (`i32`), bukan unsigned
- Dirty rect pada packet SCREEN juga signed
- Pengujian wajib menyertakan tata letak dengan monitor di kiri, bukan hanya
  susunan kiri-ke-kanan yang rapi

Melewatkan ini menghasilkan bug yang hanya muncul pada sebagian pengguna, dan
selalu sulit direproduksi oleh pengembang yang monitornya kebetulan tersusun
rapi.

### 5.3 Mode tampilan viewer

Tiga mode, sesuai MULTI_MONITOR.md §2:

| Mode | Perilaku |
|---|---|
| **Satu monitor** | Satu layar penuh di canvas; monitor lain sebagai thumbnail kecil |
| **Gabungan** | Seluruh monitor dalam satu canvas mengikuti koordinat aslinya |
| **Jendela terpisah** | Satu jendela per monitor (hanya viewer native, bukan browser) |

Untuk web viewer, dua mode pertama yang realistis.

### 5.4 Berapa aliran video

Dua pendekatan, dan pilihannya berdampak besar pada bandwidth:

| Pendekatan | Kelebihan | Kekurangan |
|---|---|---|
| **Satu track, monitor dipilih** | Bandwidth minimal; hanya yang dilihat yang dikirim | Perpindahan monitor perlu negosiasi ulang, ada jeda |
| **Track per monitor** | Perpindahan seketika, thumbnail hidup | Bandwidth berlipat; 3 monitor 1080p bisa >15 Mbps |

**Rekomendasi:** satu track resolusi penuh untuk monitor aktif, ditambah track
thumbnail beresolusi sangat rendah (160×90, 2 fps) untuk monitor lain.
Perpindahan terasa seketika karena thumbnail-nya sudah hidup, sementara
tambahan bandwidth-nya dapat diabaikan.

---

## 6. Injeksi input

### 6.1 API

| Platform | Mouse | Keyboard |
|---|---|---|
| Windows | `SendInput` dengan `MOUSEEVENTF_ABSOLUTE` | `SendInput` dengan scancode |
| macOS | `CGEventCreateMouseEvent` | `CGEventCreateKeyboardEvent` |

Windows memakai **scancode**, bukan virtual key code. Alasannya tata letak
papan ketik: mengirim virtual key membuat huruf yang diketik bergantung pada
tata letak yang aktif di mesin tujuan, sehingga viewer ber-QWERTY yang mengakses
mesin ber-AZERTY menghasilkan huruf yang salah.

### 6.2 Pemetaan koordinat

Viewer mengirim koordinat **relatif terhadap monitor yang sedang dilihat**
(0.0–1.0), bukan piksel. Agent yang menerjemahkannya ke koordinat virtual
desktop absolut.

Ini menghindari seluruh kelas bug yang berasal dari perbedaan resolusi, DPI
scaling, dan ukuran jendela viewer. Viewer tidak perlu tahu apa pun tentang
tata letak fisik mesin tujuan.

### 6.3 Tombol khusus

| Tombol | Catatan |
|---|---|
| `Ctrl+Alt+Del` | Tidak dapat disuntikkan `SendInput`. Perlu `SAS` lewat service LocalSystem |
| Tombol Windows | Perlu penanganan khusus agar tidak memicu menu Start lokal |
| `PrintScreen` | Harus ditangkap viewer, bukan diteruskan browser |

### 6.4 Yang harus dicegat di sisi viewer

Browser menahan sebagian pintasan untuk dirinya sendiri (`Ctrl+W`, `Ctrl+T`,
`F11`, `Cmd+Q`). Viewer web perlu mode "tangkap papan ketik" memakai
Keyboard Lock API, yang hanya bekerja pada mode layar penuh dan hanya di
sebagian browser. Ini salah satu alasan viewer native akhirnya tetap dibutuhkan.

---

## 7. Konsekuensi keamanan — bagian yang tidak boleh dilewat

Menambahkan kendali **mengubah sifat produk**, bukan sekadar menambah fitur.

### 7.1 Dialog persetujuan harus berubah

Teks sekarang berbunyi *"orang ini dapat melihat layar Anda"*. Begitu kendali
ada, kalimat itu menjadi tidak jujur. QUICK_CONNECT.md §4.1 wajib diperbarui,
dan izinnya dipisah:

| Tingkat | Arti |
|---|---|
| **Lihat saja** | Hanya melihat layar. Perilaku Fase 0. |
| **Kendali penuh** | Mouse, papan ketik, clipboard |

Persetujuan diminta **per tingkat**, dan menaikkan tingkat di tengah sesi
memerlukan persetujuan baru.

### 7.2 Yang harus ada sebelum kendali diaktifkan

- **Indikator selalu tampil** — penanda "sedang dikendalikan" yang tidak dapat
  disembunyikan viewer
- **Pintasan putus** yang ditangani agent, bukan viewer, sehingga tidak dapat
  dicegat
- **Jeda otomatis** saat pengguna lokal menggerakkan mouse fisiknya — orang yang
  merebut kembali kendali mesinnya sendiri harus selalu menang
- **Audit per aksi** untuk operasi bernilai tinggi
- Seluruh input dicatat pada `connection_logs` untuk penyelidikan insiden

### 7.3 Ancaman yang membesar

Kendali penuh menjadikan produk ini alat penipuan dukungan teknis yang sempurna.
Mitigasi di QUICK_CONNECT.md §7 dirancang untuk itu dan **wajib** sudah aktif
sebelum kendali dirilis, bukan sesudah.

---

## 8. Tambahan protokol

Sudah sebagian didefinisikan di [REMOTE_PROTOCOL.md](./REMOTE_PROTOCOL.md);
yang belum ada ditandai.

| Packet | Arah | Keadaan |
|---|---|---|
| `MOUSE` (0x22) | Viewer → Agent | Sudah didefinisikan; koordinat perlu diubah menjadi signed (T-16) |
| `KEYBOARD` (0x21) | Viewer → Agent | Sudah didefinisikan |
| `MONITOR_LAYOUT` | Agent → Viewer | Ada di MULTI_MONITOR.md, **belum** di protokol |
| `MONITOR_SELECT` | Viewer → Agent | **Baru** |
| `CONTROL_LEVEL` | Bidirectional | **Baru** — negosiasi lihat-saja vs kendali |
| `INPUT_PAUSED` | Agent → Viewer | **Baru** — pengguna lokal mengambil alih |

Seluruhnya lewat WebRTC DataChannel yang sudah berjalan, bukan lewat server.

---

## 9. Milestone

Setiap tahap menghasilkan sesuatu yang dapat dicoba, bukan hanya kode.

### M1 — Kerangka agent native

Registrasi perangkat memakai kunci Ed25519, heartbeat, koneksi signaling.
Belum ada capture.

**Dapat dicoba:** perangkat muncul online di dashboard, dan Quick Connect
menemukannya.

### M2 — Capture satu monitor

DXGI Desktop Duplication, encode H.264, kirim lewat WebRTC ke viewer web yang
sudah ada.

**Dapat dicoba:** layar mesin Windows tampil di browser, menggantikan agent tab.

### M3 — Enumerasi dan perpindahan monitor

`MONITOR_LAYOUT`, `MONITOR_SELECT`, track thumbnail beresolusi rendah,
pemilih monitor di viewer.

**Dapat dicoba:** ketiga layar terlihat sebagai thumbnail, diklik untuk beralih.

### M4 — Injeksi input

Mouse dan papan ketik, koordinat relatif, scancode, jeda otomatis saat pengguna
lokal bergerak.

**Dapat dicoba:** mesin benar-benar dapat dikendalikan.

### M5 — Pengerasan

Tingkat izin, indikator selalu tampil, pintasan putus, audit per aksi,
`Ctrl+Alt+Del` lewat service.

**Dapat dicoba:** siap dipakai orang lain, bukan hanya Anda sendiri.

### M6 — Distribusi

Installer, code signing, auto-update. Terganjal T-18: sertifikat code signing
kini mewajibkan kunci berada di perangkat keras FIPS, bukan berkas PFX.

---

## 10. Keputusan yang menunggu Anda

| # | Pertanyaan | Kenapa penting |
|---|---|---|
| 1 | **Platform mana lebih dulu — Windows atau macOS?** | Menentukan seluruh isi M2–M4. Tiga monitor lebih lazim pada desktop Windows, tetapi mesin pengembangan Anda macOS |
| 2 | **Ada mesin Windows untuk build dan uji?** | Tanpa ini, jalur Windows berhenti di M1 |
| 3 | **Viewer tetap di browser, atau mulai viewer native (Tauri)?** | Browser tidak dapat menangkap seluruh pintasan papan ketik, dan tidak dapat membuka jendela per monitor |
| 4 | **Kendali dirilis bersama pengerasan M5, atau lebih dulu untuk pemakaian pribadi?** | Merilis kendali tanpa pengerasan dapat diterima bila hanya Anda yang memakai; tidak dapat diterima begitu ada orang lain |

---

## 11. Yang tidak berubah

Perlu ditegaskan agar cakupannya tidak melebar: seluruh Fase 0 tetap dipakai apa
adanya. Signaling, TURN, Quick Connect, siklus hidup sesi, audit, presence,
autentikasi, dan basis datanya tidak perlu disentuh.

Agent native menggantikan **satu ujung** dari koneksi yang sudah bekerja.
