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

> ✅ **Sudah terkompilasi dan berjalan di Windows** — 2026-08-09, Windows 11 Pro
> 26200, Rust 1.97.1 MSVC, Build Tools 17.14, Windows SDK 10.0.26100.
>
> Peringatan sebelumnya di tempat ini menyebut `monitor.rs` belum pernah
> disentuh compiler. Perkiraannya tepat sasaran: **satu** galat, dan memang
> perbaikannya satu baris — `MONITORINFOF_PRIMARY` berada di
> `Win32::UI::WindowsAndMessaging`, bukan di `Win32::Graphics::Gdi` seperti yang
> diasumsikan. Seluruh sisanya — `MONITORINFOEXW`, tanda tangan callback
> `EnumDisplayMonitors`, konversi `LPARAM` — lolos apa adanya.
>
> Seluruh workspace lulus **111 unit test** di Windows, sama seperti di Linux.

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

Keluaran sungguhan dari mesin uji pertama — dua monitor, yang sekunder
diputar tegak dan diletakkan di kiri-atas:

```
2 monitor terdeteksi

ID   NAMA                         X       Y   LEBAR  TINGGI  SKALA  PRIMER
─────────────────────────────────────────────────────────────────────────────────
0    \\.\DISPLAY1                 0       0    1920    1080   100%  ya
1    \\.\DISPLAY2             -1080    -406    1080    1920   100%

Virtual desktop: 3000×1920 mulai dari (-1080, -406)

1 monitor berkoordinat negatif:
  \\.\DISPLAY2 pada (-1080, -406)
```

### 5.1 Yang paling perlu Anda perhatikan

Perhatikan `DISPLAY2` berkoordinat **X = −1080 dan Y = −406**. Monitor yang
diletakkan di sebelah **kiri** monitor primer memang berkoordinat negatif, dan
susunan itu sangat umum. Ketika ia juga tidak sejajar di bagian atas — dan itu
hampir selalu terjadi pada monitor tegak yang lebih tinggi — sumbu Y ikut
negatif.

Agent akan memberi tahu Anda bila belum ada monitor berkoordinat negatif, dan
meminta Anda memindahkan satu monitor ke kiri lewat pengaturan tampilan lalu
menjalankannya ulang. Ini bukan basa-basi: implementasi yang memakai tipe tak
bertanda akan tampak sempurna pada susunan kiri-ke-kanan yang rapi, lalu rusak
diam-diam bagi pengguna yang menyusun monitornya berbeda. Temuan **T-16**.

Cara memverifikasinya tanpa mempercayai agent sendiri — Windows dimintai
jawaban yang sama lewat jalur yang sama sekali berbeda:

```powershell
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Screen]::AllScreens |
    ForEach-Object { "{0} {1} Primary={2}" -f $_.DeviceName, $_.Bounds, $_.Primary }
[System.Windows.Forms.SystemInformation]::VirtualScreen
```

Keluaran keduanya harus identik sampai ke angka terakhir. Pada mesin uji
pertama memang demikian.

### 5.2 Kesadaran DPI

Kolom `SKALA` berasal dari `GetDpiForMonitor`, dan agent menyatakan dirinya
`PER_MONITOR_AWARE_V2` sebelum membaca koordinat mana pun.

Tanpa pernyataan itu Windows memvirtualkan seluruh angka yang dilaporkannya:
monitor 1920×1080 berskala 150% akan terbaca 1280×720. Angkanya konsisten dan
tampak masuk akal, sehingga kesalahannya baru terlihat jauh di hilir — kursor
yang meleset saat injeksi input (M4), dan frame Desktop Duplication beresolusi
fisik yang tidak cocok dengan tata letak yang sudah terlanjur dikirim (M2).

**Belum terverifikasi:** mesin uji pertama memakai 100% pada kedua monitornya,
jadi jalur ini benar secara konstruksi tetapi belum pernah dibuktikan pada
skala ≠ 100%. Bila Anda punya monitor berskala 125% atau 150%, jalankan
`monitors` di sana — nilai `LEBAR`/`TINGGI` harus tetap resolusi **fisik**
panel, bukan angka yang sudah diperkecil.

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

## 7. Mendaftarkan mesin ini ke server

Agent memerlukan identitasnya sendiri. Ia **tidak** memakai email dan password
Anda: mesin tanpa pengawasan yang menyimpan kredensial manusia berarti satu
mesin yang dibongkar membocorkan seluruh akun.

### 7.1 Terbitkan token enrolment

Dari dashboard, atau langsung lewat API memakai token pengguna Anda:

```powershell
curl -X POST https://aetherdesk.masamune.my.id/api/v1/devices/enrolment-tokens `
     -H "Authorization: Bearer <TOKEN-PENGGUNA>" `
     -H "Content-Type: application/json" `
     -d '{\"alias\":\"PC Kantor\"}'
```

Token berlaku satu jam dan **sekali pakai**.

### 7.2 Daftarkan

```powershell
.\target\release\rdp-agent.exe enrol --token <TOKEN> --alias "PC Kantor"
```

Keluarannya memuat device ID dan password sesi. **Password sesi hanya
ditampilkan sekali** — setelah itu hanya hash-nya yang tersimpan.

Identitas disimpan di `%APPDATA%\masamune\aetherdesk\config`:

| Berkas | Isi |
|---|---|
| `device.json` | UUID, device ID, alamat server |
| `device.key` | seed privat Ed25519 — **rahasia** |

Kunci privat tidak pernah dikirim ke server. Server hanya menyimpan kunci
publiknya.

> ⚠ Pada Windows, `device.key` mengandalkan ACL bawaan direktori profil
> pengguna. Itu membatasi akses ke pemilik dan administrator, tetapi **bukan**
> padanan `chmod 600`. Pengetatan yang sesungguhnya menyusul bersama service
> Windows (ADR-010), saat kunci pindah ke penyimpanan milik LocalSystem.

### 7.3 Jalankan

```powershell
.\target\release\rdp-agent.exe connect
```

Perangkat akan tampil **online** di dashboard. Periksa identitas kapan saja
dengan `rdp-agent status`.

Permintaan sesi yang masuk saat ini ditolak dengan alasan tertulis — capture
layar baru ada di M2.

---

## 8. Berikutnya

Sesuai `docs/NEXT_PLAN.md`:

| Tahap | Isi | Keadaan |
|---|---|---|
| **M1** | Enumerasi monitor, identitas Ed25519, enrolment, heartbeat, signaling | **selesai** |
| **M2** | Capture DXGI Desktop Duplication, encode H.264 | berikutnya |
| **M3** | `MONITOR_LAYOUT`, perpindahan monitor, thumbnail | |
| **M4** | Injeksi input `SendInput` dengan scancode | |
| **M5** | Pengerasan: tingkat izin, indikator, pintasan putus | |

Yang belum ada dan akan menghambat di M6: sertifikat code signing. Sejak
Juni 2023 kunci privatnya wajib berada di perangkat keras bersertifikasi FIPS,
bukan berkas PFX — lihat temuan **T-18**.
