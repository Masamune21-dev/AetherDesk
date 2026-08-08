---
name: aetherdesk-ui
description: Sistem desain antarmuka AetherDesk. Gunakan setiap kali membuat atau mengubah halaman web, komponen, warna, tipografi, atau tata letak di dalam web/ — termasuk dashboard, agent, viewer, dan halaman penyiapan. Memuat token warna, skala tipografi, pola komponen, aturan aksesibilitas, dan alasan di balik setiap keputusan.
---

# Sistem Desain AetherDesk

Panduan ini mengikat seluruh berkas di `web/`. Tujuannya bukan keseragaman demi
keseragaman, melainkan agar antarmuka ini terbaca sebagai **satu produk yang
dapat dipercaya** — karena yang diminta darinya adalah menyerahkan layar sendiri
kepada orang lain.

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
--bg          #0B1016   /* dasar, biru-hitam dalam — bukan hitam murni */
--surface     #131B24   /* kartu */
--surface-2   #1A2530   /* elemen terangkat di atas kartu */
--line        #223040   /* pembatas */
--line-strong #35485A   /* pembatas yang perlu terlihat */

--ink         #E6EDF3   /* teks utama */
--ink-2       #A3B4C4   /* teks sekunder */
--ink-3       #6E8296   /* label, keterangan */

--accent      #58B0E3   /* interaktif, tautan, fokus */
--accent-ink  #06121B   /* teks di atas accent */
--accent-dim  #16283A   /* latar lembut accent */

--ok          #4BC08A   /* tersambung, sehat */
--warn        #E0A85C   /* menunggu, perlu perhatian */
--bad         #E8837C   /* gagal, ditolak */
```

Warna semantik (`ok`/`warn`/`bad`) **terpisah** dari accent. Sebuah tombol
primer tidak pernah hijau hanya karena hasilnya bagus.

## 3. Tipografi

```css
--sans  ui-sans-serif, -apple-system, "Segoe UI", Roboto, sans-serif
--mono  ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace
```

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
