---
name: aetherdesk-ui
description: Sistem desain antarmuka AetherDesk. Gunakan setiap kali membuat atau mengubah halaman web, komponen, warna, tipografi, atau tata letak di dalam web/ — termasuk dashboard, agent, viewer, dan halaman penyiapan. Memuat token warna, skala tipografi, pola komponen, aturan aksesibilitas, dan alasan di balik setiap keputusan.
---

# Sistem Desain AetherDesk

Panduan ini mengikat seluruh berkas di `web/`. Tujuannya bukan keseragaman demi
keseragaman, melainkan agar antarmuka ini terbaca sebagai **satu produk yang
dapat dipercaya** — karena yang diminta darinya adalah menyerahkan layar sendiri
kepada orang lain.

## 0. Arah: sinyal yang merambat melintasi medium

Aether adalah medium klasik tempat cahaya dipercaya merambat — zat tak
kasatmata yang mengisi ruang. Nama produk ini secara harfiah berarti *medium
transmisi*, dan itulah yang dipinjam seluruh antarmukanya.

Konsekuensinya konkret, bukan puitis:

- **Latar adalah ruang gelap**, bukan sekadar tema gelap. Nyaris tanpa cahaya
  sendiri; yang menyala hanyalah sinyal yang menyeberanginya.
- **Gradien sinyal** ungu → sian → kuning mewakili dua ujung koneksi beserta
  lintasan di antaranya. Ia hanya muncul pada elemen yang memang berbicara
  tentang koneksi: tanda, garis di bawah header, tepi kredensial, tepi atas
  panggung video, dan tombol primer. **Tidak pernah** sebagai latar besar.
- **Monospace adalah suara utama**, bukan sekadar untuk kode. Ini alat
  instrumentasi; angka-angkanya dibacakan lewat telepon dan dibaca dari HUD.
  Sans hanya dipakai untuk prosa.
- **Medan aether** — muka gelombang sepusat yang merambat pelan dari titik di
  luar tepi kiri-atas. Satu-satunya gerakan di seluruh antarmuka. Bukan hujan
  partikel: yang digambar adalah hal yang benar-benar dilakukan produk ini.

## 1. Sikap desain

AetherDesk adalah alat teknis yang dipakai pada saat orang sedang panik: layar
rusak, server bermasalah, tenggat mendesak. Karena itu:

- **Tenang, bukan meriah.** Tidak ada gradien mencolok, animasi berlebihan, atau
  emoji sebagai penanda bagian. Yang bergerak hanya yang memang berubah.
- **Angka adalah isi.** Device ID, kata sandi, latensi, FPS — semuanya monospace
  dan diberi ruang. Ini produk tempat orang mendiktekan angka lewat telepon.
- **Keadaan selalu terlihat.** Setiap komponen jaringan punya status eksplisit.
  Diam berarti tidak diketahui, dan itu harus terlihat berbeda dari sehat.
- **Gelap sebagai pilihan, bukan tren.** Isi layar remote adalah hal paling
  terang di halaman; kroma di sekitarnya sengaja diredam agar tidak bersaing.

## 2. Token warna

Seluruh warna berasal dari token. Jangan pernah menulis nilai heks langsung di
komponen.

```css
/* Medium — ruang gelap tempat sinyal merambat */
--void       #07080D
--bg         #0A0C13
--surface    #10131C
--surface-2  #161A26
--surface-3  #1D2231
--line       #1E2331
--line-2     #2B3145
--line-lit   #3D4560

/* Cahaya */
--ink        #EEF1F8
--ink-2      #9AA3BD
--ink-3      #626C88

/* Sinyal — dua ujung koneksi dan lintasannya */
--signal-a   #8B7BF7   /* ungu — sumber */
--signal-b   #4CC9F0   /* sian — lintasan, sekaligus warna interaktif */
--signal-c   #F4A261   /* kuning — tujuan */

--ok         #3DDC97
--warn       #F4A261
--bad        #FF6B6B
```

Warna semantik (`ok`/`warn`/`bad`) **terpisah** dari gradien sinyal. Sebuah
tombol primer tidak pernah hijau hanya karena hasilnya bagus.

Kedalaman dibangun dari nilai permukaan dan sorot setipis rambut di tepi atas
panel — seperti cahaya yang tersangkut di sisi sebuah lempeng. **Bukan** dari
bayangan yang di-blur.

## 3. Tipografi

```css
--mono  ui-monospace, "SF Mono", SFMono-Regular, "JetBrains Mono", Menlo, monospace
--sans  ui-sans-serif, -apple-system, "Segoe UI", Roboto, sans-serif
```

**Monospace adalah suara utama.** Judul, label, tombol, lencana, dan seluruh
angka memakainya. Sans hanya untuk prosa yang benar-benar dibaca sebagai
kalimat. Ini membalik kebiasaan umum, dan disengaja: produk ini adalah alat
instrumentasi, bukan halaman pemasaran.

Skala tetap. Jangan menyisipkan ukuran di antaranya:

| Token | Ukuran | Pemakaian |
|---|---|---|
| `--t-xs` | 12px | label huruf besar, keterangan |
| `--t-sm` | 13px | teks sekunder, HUD |
| `--t-base` | 15px | isi |
| `--t-lg` | 18px | judul kartu |
| `--t-xl` | 24px | judul halaman |
| `--t-2xl` | 32px | judul utama |
| `--t-hero` | 44px | kredensial yang dibacakan |

Aturan:

- Judul memakai `letter-spacing: -0.015em` dan `text-wrap: balance`
- Label huruf besar memakai `letter-spacing: 0.12em`, ukuran `--t-xs`
- Angka yang berbaris dalam kolom memakai `font-variant-numeric: tabular-nums`
- Baris teks tidak melebihi 68 karakter

## 4. Ruang

Kelipatan 4px, diakses lewat token `--s1` (4px) sampai `--s10` (64px). Jarak
antar-saudara diatur `gap` pada flex atau grid, **bukan** margin per elemen —
margin yang saling tumpuk adalah sumber cacat spasi paling umum.

## 5. Pola komponen

### 5.1 Cangkang aplikasi

Setiap halaman memakai header yang sama: wordmark di kiri yang menautkan ke
beranda, navigasi di kanan. Ini sekaligus memenuhi kebutuhan "kembali ke
halaman awal" tanpa menempelkan tombol lepas di tiap halaman.

Tautan halaman aktif ditandai `aria-current="page"`.

### 5.2 Kartu

Sudut 4px — bukan `rounded-lg`. Latar `--surface`, garis 1px `--line`, tanpa
bayangan. Kedalaman dibangun dari nilai permukaan, bukan dari blur.

### 5.3 Kredensial

Device ID dan kata sandi sesi adalah **momen utama** halaman agent. Keduanya
tampil pada ukuran `--t-hero`, monospace, dengan jarak antar-karakter longgar,
dan tombol salin. Device ID dikelompokkan tiga-tiga (`942 716 382`) karena
memang dibacakan lewat telepon.

### 5.4 Lencana status

Teks monospace huruf besar, ukuran `--t-xs`, garis 1px `currentColor`, dan
sebuah titik berwarna. Warna saja tidak cukup — bentuk dan teksnya juga harus
membedakan, agar terbaca oleh mata yang tidak membedakan warna.

### 5.5 Dialog persetujuan

Lihat `QUICK_CONNECT.md` §4.1. Aturannya mengikat:

- Tombol **Tolak** diletakkan lebih dulu dan menerima `autofocus`
- Tombol **Izinkan** terkunci tiga detik dengan hitung mundur terlihat
- Identitas peminta diambil dari klaim token, tidak pernah dari input

## 6. Aksesibilitas

- Kontras teks utama minimal 4.5:1, teks besar minimal 3:1
- Setiap elemen fokusable punya `:focus-visible` yang terlihat jelas
- Sasaran sentuh minimal 44×44px
- `prefers-reduced-motion` mematikan seluruh transisi
- Status tidak pernah disampaikan hanya lewat warna

## 7. Yang harus dihindari

Pola berikut membuat halaman terlihat seperti hasil generator, dan tidak dipakai
di proyek ini:

- Gradien ungu-ke-biru pada header
- Sudut membulat besar di semua elemen
- Emoji sebagai penanda bagian
- Bayangan bertumpuk untuk mensimulasikan kedalaman
- Animasi masuk pada setiap elemen saat halaman dimuat
- Penomoran hias (01 / 02 / 03) untuk hal yang bukan urutan

## 8. Berkas

```
web/style.css   token, cangkang, seluruh komponen
web/app.js      klien API, signaling, WebRTC, utilitas tampilan
web/*.html      halaman; skrip inline hanya berisi logika halaman itu
```

Ubah `style.css` untuk hal yang dipakai lebih dari satu halaman. Skrip inline
hanya boleh memuat logika yang benar-benar khas halaman tersebut.
