# Build di Windows

## AetherDesk — agent native

**Versi:** 1.0.0
**Tanggal:** 2026-08-08
**Untuk:** membangun `rdp-agent` di PC Windows

---

## 1. Apa yang dibangun di sini

Hanya **`rdp-agent`**. Tiga crate lain adalah komponen server dan tetap berjalan
di Linux:

| Crate | Tempat |
|---|---|
| `rdp-core` | pustaka bersama, ikut terkompilasi |
| `rdp-api` | server — Linux |
| `rdp-signal` | server — Linux |
| **`rdp-agent`** | **PC Windows Anda** |

Anda tidak perlu PostgreSQL, Redis, maupun coturn di PC ini. Agent berbicara ke
server yang sudah berjalan.

---

> ⚠ **Kode Windows di `monitor.rs` belum pernah dikompilasi.** Ia ditulis dari
> pengetahuan tentang API `EnumDisplayMonitors` dan crate `windows` 0.58, bukan
> hasil verifikasi compiler — mesin pengembangan yang dipakai menulisnya adalah
> macOS, dan bagian ber-`#[cfg(windows)]` tidak ikut diperiksa di sana.
>
> Kemungkinan besar ada satu-dua ketidakcocokan tipe atau nama item yang
> berubah antar versi crate `windows`. Kalau `cargo build` gagal, **kirimkan
> pesan galatnya apa adanya** — biasanya perbaikannya satu baris, dan jauh lebih
> cepat daripada menebak-nebak sendiri.
>
> Sisanya (`rdp-core`, `rdp-api`, `rdp-signal`) sudah terbukti kompilasi dan
> lulus 101 unit test di Linux.

---

## 2. Prasyarat

### 2.1 Visual Studio Build Tools

Rust pada Windows memakai linker MSVC. Unduh **Build Tools for Visual Studio**,
lalu pada penginstal centang:

- **Desktop development with C++**
- **Windows 11 SDK** (atau Windows 10 SDK)

Ini bagian terbesar unduhannya (sekitar 2-4 GB) dan paling sering terlewat —
tanpa linker, `cargo build` gagal pada tahap terakhir dengan pesan `link.exe not
found` yang membingungkan karena kompilasinya sendiri sudah berhasil.

### 2.2 Rust

Unduh `rustup-init.exe` dari <https://rustup.rs>, jalankan, pilih instalasi
baku. Toolchain `stable-x86_64-pc-windows-msvc` sudah tepat.

Verifikasi di PowerShell **baru** (PATH perlu dimuat ulang):

```powershell
rustc --version
cargo --version
```

### 2.3 Git — opsional

Hanya diperlukan bila Anda ingin melanjutkan riwayat versi dari PC ini.

---

## 3. Menyiapkan sumber

Ekstrak arsip ke lokasi tanpa spasi maupun karakter non-ASCII pada path.
`C:\dev\aetherdesk` aman; `C:\Users\Nama Anda\Desktop\proyek baru` mengundang
masalah pada sebagian build script.

```powershell
cd C:\dev\aetherdesk
```

---

## 4. Build

```powershell
cargo build --release -p rdp-agent
```

Kompilasi pertama mengunduh seluruh dependensi dan memakan beberapa menit.
Hasilnya:

```
target\release\rdp-agent.exe
```

Untuk membangun seluruh workspace — termasuk komponen server, yang berguna
sebagai pemeriksaan bahwa semuanya masih rapi:

```powershell
cargo build --release
cargo test --workspace
```

---

## 5. Uji pertama: enumerasi monitor

Inilah yang membuat perjalanan ini bermakna. Jalankan:

```powershell
.\target\release\rdp-agent.exe monitors
```

Keluaran yang diharapkan pada mesin bermonitor tiga:

```
3 monitor terdeteksi

ID   NAMA                         X       Y   LEBAR  TINGGI  PRIMER
──────────────────────────────────────────────────────────────────────────
0    \\.\DISPLAY1                 0       0    1920    1080  ya
1    \\.\DISPLAY2             -1920       0    1920    1080
2    \\.\DISPLAY3              1920    -200    2560    1440

Virtual desktop: 6400×1640 mulai dari (-1920, -200)
```

### 5.1 Yang paling perlu Anda perhatikan

Perhatikan `DISPLAY2` berkoordinat **X = −1920**. Monitor yang diletakkan di
sebelah **kiri** monitor primer memang berkoordinat negatif, dan susunan itu
sangat umum.

Agent akan memberi tahu Anda bila belum ada monitor berkoordinat negatif, dan
meminta Anda memindahkan satu monitor ke kiri lewat pengaturan tampilan lalu
menjalankannya ulang. Ini bukan basa-basi: implementasi yang memakai tipe tak
bertanda akan tampak sempurna pada susunan kiri-ke-kanan yang rapi, lalu rusak
diam-diam bagi pengguna yang menyusun monitornya berbeda. Temuan **T-16**.

Jalankan sekali pada susunan rapi dan sekali dengan monitor di kiri. Kirimkan
kedua keluarannya — itu yang saya pakai untuk memastikan pemetaan koordinatnya
benar sebelum injeksi input dibuat.

---

## 6. Bila gagal

| Pesan | Sebab | Perbaikan |
|---|---|---|
| `link.exe not found` | Build Tools C++ belum terpasang | §2.1 |
| `error: linker 'link.exe' not found` | Sama | §2.1 |
| `cargo` tidak dikenali | PATH belum dimuat ulang | Buka PowerShell baru |
| `failed to run custom build command for ring` | SDK Windows belum tercentang | §2.1 |
| `tidak ada monitor terdeteksi` | Dijalankan tanpa sesi desktop | Jalankan langsung, bukan lewat SSH atau service |

---

## 7. Berikutnya setelah ini berhasil

Sesuai `docs/NEXT_PLAN.md`:

| Tahap | Isi |
|---|---|
| **M1 sisanya** | Identitas perangkat Ed25519, registrasi, heartbeat, signaling |
| **M2** | Capture DXGI Desktop Duplication, encode H.264 |
| **M3** | `MONITOR_LAYOUT`, perpindahan monitor, thumbnail |
| **M4** | Injeksi input `SendInput` dengan scancode |
| **M5** | Pengerasan: tingkat izin, indikator, pintasan putus |

Yang belum ada dan akan menghambat di M6: sertifikat code signing. Sejak
Juni 2023 kunci privatnya wajib berada di perangkat keras bersertifikasi FIPS,
bukan berkas PFX — lihat temuan **T-18**.
