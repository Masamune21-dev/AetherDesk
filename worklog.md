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


## 2026-08-09 — Sesi 2: Agent native terkompilasi di Windows

Sesi pertama yang dijalankan **dari PC Windows**, bukan dari macOS. Itu yang
menghilangkan satu-satunya penghalang M1: kode ber-`#[cfg(windows)]` akhirnya
bertemu compiler yang bisa memeriksanya.

### Yang dikerjakan

**28. Toolchain Windows terpasang**

| Komponen | Versi |
|---|---|
| Rust | 1.97.1 (MSVC) |
| Visual Studio Build Tools | 17.14.37516.0, workload Desktop C++ |
| Windows SDK | 10.0.26100.0 |
| OS | Windows 11 Pro 26200 |

Dipasang lewat `winget`. Prasyarat BUILD_WINDOWS.md §2 terpenuhi seluruhnya.

**29. `monitor.rs` dikompilasi untuk pertama kalinya — satu galat**

BUILD_WINDOWS.md memperingatkan bahwa berkas ini ditulis dari pengetahuan API,
bukan dari verifikasi compiler, dan memperkirakan "satu-dua ketidakcocokan tipe
atau nama item". Perkiraannya tepat, dan bahkan lebih ringan dari itu:

```
error[E0432]: unresolved import `windows::Win32::Graphics::Gdi::MONITORINFOF_PRIMARY`
```

`MONITORINFOF_PRIMARY` berada di `Win32::UI::WindowsAndMessaging`, bukan di
`Win32::Graphics::Gdi`. Satu baris import. Segala hal lain yang layak
dikhawatirkan — tata letak `MONITORINFOEXW`, `cbSize` yang harus menunjuk ke
struct luar, tanda tangan callback `EnumDisplayMonitors`, konversi `LPARAM`
ke pointer `Vec` — lolos apa adanya.

**30. Kesadaran DPI — bug yang compiler tidak akan pernah temukan**

Ini temuan sungguhan dari sesi ini, dan jauh lebih berbahaya daripada galat di
atas justru karena tidak menghasilkan pesan apa pun.

Proses yang tidak menyatakan dirinya sadar-DPI menerima koordinat **yang sudah
divirtualkan** Windows. Monitor 1920×1080 berskala 150% terbaca 1280×720.
Angkanya konsisten, wajar, dan salah. `scale_percent` sebelumnya dipatok 100
dengan komentar "menyusul", padahal fitur `Win32_UI_HiDpi` sudah lama ada di
`Cargo.toml` — niatnya sudah tercatat, implementasinya belum ditulis.

Tiga akibat yang menunggu di hilir bila dibiarkan:

| Tahap | Akibat |
|---|---|
| M4 | `ke_absolut()` menghasilkan piksel meleset, `SendInput` mengklik di tempat salah |
| M2 | Desktop Duplication menyerahkan frame beresolusi **fisik** yang tidak cocok dengan tata letak yang sudah dikirim |
| M3 | Bounding box virtual desktop menyusut, monitor bisa terhitung saling tindih |

Perbaikan: `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` dipanggil lewat
`Once` **dari dalam `enumerasi()`**, bukan dari `main()` — supaya pemanggil
tidak mungkin lupa. `scale_percent` kini diisi `GetDpiForMonitor` dengan
`MDT_EFFECTIVE_DPI`.

**31. Enumerasi terverifikasi silang**

Mesin uji punya dua monitor, yang sekunder tegak dan di kiri-atas:

```
ID   NAMA                         X       Y   LEBAR  TINGGI  SKALA  PRIMER
0    \\.\DISPLAY1                 0       0    1920    1080   100%  ya
1    \\.\DISPLAY2             -1080    -406    1080    1920   100%

Virtual desktop: 3000×1920 mulai dari (-1080, -406)
```

Susunan ini **lebih ketat daripada contoh di dokumen**: X dan Y dua-duanya
negatif, bukan hanya X. Persis bentuk yang dimaksud temuan T-16, dan kebetulan
sudah tersedia tanpa perlu menata ulang monitor.

Diverifikasi silang terhadap `System.Windows.Forms.Screen::AllScreens` dan
`SystemInformation::VirtualScreen` — jalur yang sepenuhnya berbeda dari
`EnumDisplayMonitors`. **Identik sampai angka terakhir**, termasuk bounding box
3000×1920 dari (−1080, −406).

**32. 111 unit test lulus di Windows**

| Crate | Test |
|---|---|
| `rdp-core` | 33 |
| `rdp-api` | 48 |
| `rdp-signal` | 18 + 2 |
| `rdp-agent` | 10 |

Sebelumnya angka ini hanya pernah dibuktikan di Linux. Tidak ada satu pun test
yang bergantung platform — hasilnya sama di kedua sisi.

**33. Working tree disambungkan ke remote**

Salinan di PC ini datang tanpa `.git`. Disambungkan ulang lewat `git init` +
`fetch` + `reset --mixed origin/main`, yang hanya memperbarui index dan tidak
menyentuh satu berkas pun — sehingga hasil kerja lokal langsung terlihat sebagai
diff terhadap `c4fe5a8`, bukan sebagai repo baru tanpa riwayat.

Akses server dari PC ini diuji dan **sehat**, berbeda dari blocker sesi 1:

| Uji | Hasil |
|---|---|
| ICMP ke `<HOST-LAN>` | hidup |
| TCP 22 | terbuka |
| `https://aetherdesk.masamune.my.id/api/health` | **200** |

### Temuan yang belum ditindaklanjuti

**`Cargo.lock` tidak pernah dikomit.** Untuk workspace yang menghasilkan biner
terpasang di mesin orang, ini gap yang nyata — dan baru saja terwujud secara
konkret: build Windows ini me-resolve 309 paket dari nol, tanpa jaminan
versinya sama dengan yang sedang berjalan di server produksi.

**`id` monitor berasal dari urutan enumerasi.** Urutan `EnumDisplayMonitors`
tidak dijanjikan stabil. Begitu M3 mengirim `MONITOR_SELECT`, mencabut satu
monitor akan menggeser id yang lain, dan viewer akan menunjuk layar yang salah.
Identitas yang stabil sebaiknya diturunkan dari nama perangkat, bukan dari
posisi dalam daftar.

**Skala ≠ 100% belum pernah diuji.** Kedua monitor mesin uji memakai 100%, jadi
jalur DPI benar secara konstruksi tetapi belum terbukti secara empiris.

**34. Identitas perangkat Ed25519 — M1 selesai**

Penghalangnya jelas sejak awal: `POST /api/v1/devices` mewajibkan JWT
**pengguna**, dan `Masuk::Auth` di `rdp-signal` juga menerima token pengguna.
Agent tanpa pengawasan yang menyimpan kredensial manusia berarti satu mesin
yang dibongkar membocorkan satu akun beserta seluruh armada organisasinya.

Alurnya sekarang tiga langkah, dan dua di antaranya sengaja tanpa sesi manusia:

| Langkah | Endpoint | Kredensial |
|---|---|---|
| Terbitkan token | `POST /devices/enrolment-tokens` | JWT pengguna |
| Tukar jadi perangkat | `POST /devices/enrol` | token enrolment sekali pakai |
| Buktikan diri | `POST /devices/token` | tanda tangan kunci perangkat |
| Detak | `POST /devices/heartbeat` | JWT perangkat |

Kunci privat tidak pernah meninggalkan mesin agent.

**35. Tabel yang hampir saya duplikasi**

Rancangan pertama menambahkan kolom `public_key` ke `devices`. Survei server
menunjukkan **`device_keys` sudah ada sejak migrasi 0001** — kosong, tetapi
dirancang persis untuk ini, lengkap dengan `is_active` untuk rotasi dan
`revoked_at` untuk pencabutan. Kolom baru di `devices` akan menduplikasinya
sekaligus membuang kemampuan itu.

Yang perlu disesuaikan hanya `certificate`, yang semula `NOT NULL`. Fase 0
belum punya CA, dan ADR-008 mensyaratkan tanda tangan kunci perangkat — bukan
rantai sertifikat. Memaksanya terisi hanya menghasilkan string kosong yang
berpura-pura menjadi sertifikat, jadi kolomnya dijadikan nullable.

Ikut ditutup: **`device_keys` tidak pernah diberi RLS** di migrasi 0001.
Selama tabelnya kosong itu tidak berakibat apa-apa; begitu enrolment mengisinya,
`aetherdesk_app` dapat membaca kunci milik seluruh organisasi. Diberi policy
sekarang, sebelum ada baris pertama.

**36. Empat keputusan keamanan yang layak dicatat**

**Tantangan diberi awalan domain** `aetherdesk-device-auth:v1`. ADR-008
mewajibkan kunci yang sama kelak menandatangani SDP. Tanpa pemisahan domain,
tanda tangan yang dikumpulkan dari satu alur dapat diputar ulang sebagai tanda
tangan sah di alur lain.

**Pembentukan tantangan hidup di `rdp-core`**, bukan diduplikasi di kedua sisi.
Penanda tangan dan pemverifikasi yang membangun byte berbeda menghasilkan bug
yang gejalanya hanya "tanda tangan ditolak", tanpa petunjuk sisi mana yang
keliru.

**`Terautentikasi` kini menolak token perangkat.** Keduanya ditandatangani
kunci yang sama dan sama-sama lolos verifikasi kriptografis. Tanpa pemeriksaan
jenis, agent mana pun dapat menerbitkan token enrolment dan membuka sesi ke
perangkat lain — satu mesin tersusupi menjadi pijakan ke seluruh organisasi.

**Nonce diklaim sebelum tanda tangan diverifikasi.** Yang dilindungi adalah
pemutaran ulang request yang tanda tangannya memang sah; mencatat nonce setelah
verifikasi berhasil membiarkan request tersadap dikirim ulang selama jendela
stempel waktu masih terbuka.

**37. Heartbeat sengaja tidak menyentuh `status`**

Kehadiran dimiliki Signal Server, yang menandai offline seketika saat WebSocket
putus (butir 20, temuan S-09). Bila heartbeat ikut menulis `'online'`, agent
yang koneksinya baru putus akan menghidupkan kembali statusnya pada detak
berikutnya — persis bug yang sudah diperbaiki, kembali lewat pintu lain.

Yang dicatat heartbeat adalah keterjangkauan dan metadata.

**38. Bug di migrasi, ditemukan uji fungsional**

Migrasi diuji terhadap **salinan skema produksi** di basis data terpisah,
bukan langsung di produksi. Uji pertama langsung gagal:

```
ERROR: new row for relation "device_enrolment_tokens" violates check
constraint "enrolment_terpakai_punya_perangkat"
CONTEXT: SQL function "claim_enrolment_token"
```

Constraint aslinya menuliskan kesetaraan `(used_at IS NULL) = (device_uuid IS
NULL)`, yang membuat `claim_enrolment_token` **mustahil dijalankan** — token
diklaim lebih dulu, perangkatnya baru dibuat sesudahnya. Saya menulis komentar
yang menjelaskan mengapa penautan harus terpisah, lalu memasang constraint yang
melarang persis keadaan antara itu.

Diperbaiki menjadi implikasi satu arah. Keadaan `used_at IS NOT NULL AND
device_uuid IS NULL` juga bukan sekadar transisi sesaat: bila pembuatan
perangkat gagal setelah token diklaim, itulah keadaan akhir yang benar — token
sudah habis dan tidak boleh dapat dipakai ulang, justru karena percobaannya
pernah terjadi.

Setelah perbaikan: **15 uji perilaku lulus**, mencakup sekali pakai, token
kedaluwarsa, tabrakan device ID, kunci tercabut, heartbeat lintas organisasi,
dan indeks unik kunci publik.

**39. Deploy ke produksi**

| Langkah | Hasil |
|---|---|
| Backup `pg_dump -Fc` + biner lama | 109 KB, tersimpan di `/root/backup-aetherdesk` |
| Migrasi 0005 ke produksi | OK — layanan **lama** tetap 200 selama migrasi |
| Build `rdp-api` + `rdp-signal` di server | 58 detik |
| Restart kedua layanan | aktif, `/api/health/ready` postgres + redis OK |

Migrasi aditif, jadi biner lama tetap berjalan di atas skema baru — itulah yang
membuat urutan "migrasi dulu, biner kemudian" aman.

**40. Uji ujung ke ujung — PC Windows ini terdaftar di produksi**

Bukan simulasi: agent sungguhan, lewat Cloudflare, ke server produksi.

```
Device ID        543 096 477
Kunci publik     4LFG2NAQnFUHVdxuzYcEDZUaJ8gGaoStLE2trAq-w9s
Hostname         DESKTOP-9VLA031
```

| Uji | Hasil |
|---|---|
| Ketiga endpoint baru tanpa kredensial | **401**, pesan seragam |
| `enrol` lewat `https://` | perangkat + kunci terbuat |
| Token enrolment dipakai kedua kali | ditolak |
| `connect` lewat `wss://` | terautentikasi sebagai agent |
| Status perangkat | **online** |
| Heartbeat | tepat 60 detik, `client_version 0.1.0` |
| `certificate` | NULL, sesuai rancangan |
| Audit `device.enrol` | `user_id` **NULL** — tidak ada manusia di baliknya |
| Agent dihentikan | **offline seketika**, bukan menunggu TTL |

**41. M2a — capture DXGI berjalan di kedua monitor**

Langkah pertama M2, sengaja dipisah dari encoder dan jaringan: frame yang dapat
dibuka dan dilihat membuktikan jalur capture sehat tanpa satu pun variabel dari
H.264 maupun WebRTC ikut bermain.

| Monitor | Hasil |
|---|---|
| `\\.\DISPLAY1` 1920×1080 | 94 fps, gambar benar |
| `\\.\DISPLAY2` 1080×1920 tegak | 100 fps, gambar benar |
| Monitor tak dikenal | ditolak, menyebutkan yang tersedia |

Diverifikasi dengan mata — BMP dikonversi lalu dilihat — bukan hanya dari
ukuran berkasnya.

**42. Frame hitam sempurna di monitor tegak**

Bug paling menyesatkan sejauh ini. Capture DISPLAY2 menghasilkan berkas
berdimensi benar, berukuran tepat, **dan hitam seluruhnya**.

Yang memastikan ini bug dan bukan monitor yang memang gelap: area layar yang
sama diambil lewat GDI `CopyFromScreen` — jalur yang sepenuhnya berbeda — dan
menghasilkan 20.632 piksel non-hitam, sementara DXGI menghasilkan nol.

Sebabnya: pada monitor tegak, DXGI menyerahkan tekstur dalam orientasi **panel**
(1920×1080), sementara tekstur singgah dibuat seukuran **desktop** (1080×1920).
`CopyResource` antara dua tekstur berbeda ukuran **tidak melapor gagal** — ia
diam saja dan tidak melakukan apa pun, sehingga tekstur singgah tetap berisi
nol.

Percobaan pertama memperbaikinya lewat `DXGI_OUTDUPL_DESC.ModeDesc`, dan itu
keliru: ModeDesc justru melaporkan orientasi desktop (1080×1920), bukan
orientasi tekstur. Yang benar adalah membaca `GetDesc` dari **tekstur yang
diserahkan `AcquireNextFrame`** — satu-satunya sumber yang tidak menafsirkan
apa pun.

Keputusan memutar pun kini diambil dari ukuran yang benar-benar diterima, bukan
dari klaim rotasi: ukuran adalah fakta, rotasi hanya keterangan, dan keduanya
sudah terbukti dapat berselisih di mesin ini. Ada pemeriksaan penutup yang
menolak menyerahkan frame bila panjangnya tidak sesuai dimensi yang dijanjikan.

Ini bukan bug pinggiran. Orientasi frame yang keliru akan membuat **setiap klik
mendarat di tempat yang salah** begitu M4 memetakan koordinat mouse relatif
terhadap tampilan.

**43. Kasus tepi yang ikut ditangani**

| Kasus | Perlakuan |
|---|---|
| `DXGI_ERROR_WAIT_TIMEOUT` | normal — desktop diam, bukan galat |
| `DXGI_ERROR_ACCESS_LOST` | duplikasi dibangun ulang ke monitor yang sama |
| `LastPresentTime == 0` | hanya kursor bergerak, VRAM tidak perlu disalin |
| `RowPitch != lebar × 4` | disalin per baris |
| `ReleaseFrame` | selalu dipanggil, termasuk saat penyalinan gagal |

`ACCESS_LOST` muncul saat resolusi berubah, saat secure desktop mengambil alih,
saat sesi berpindah, dan saat driver GPU di-reset. Agent yang menyerah pada
kejadian pertama akan mati sendiri pada prompt UAC pertama.

Satu galat kompilasi yang layak dicatat karena pesannya tidak menolong:
`D3D_DRIVER_TYPE_UNKNOWN` hanya sah bila adapter diberikan eksplisit; tanpa
adapter tipenya wajib `HARDWARE`. Melanggarnya menghasilkan
"The parameter is incorrect" tanpa menyebut parameter mana.

**44. M2b — encode H.264 lewat Media Foundation**

Media Foundation dipilih karena ia pintu Windows menuju seluruh encoder
perangkat keras yang diminta STREAMING.md §3 — NVENC, QuickSync, AMF semuanya
mendaftarkan diri sebagai MFT — dan karena ia tidak menambah satu pun
dependensi: seluruh API-nya sudah ada di crate `windows` yang dipakai capture.

| Monitor | Keluaran |
|---|---|
| 1920×1080 | 30,1 fps, **1,71 Mbps**, rasio kompresi **1703×** |
| 1080×1920 tegak | 30 fps, bitstream lengkap, dimensi cocok |

Encoder yang terpilih: `H264 Encoder MFT` — encoder **perangkat lunak** bawaan
Windows. Enumerasi saat ini meminta MFT sinkron, dan encoder perangkat keras
hampir selalu asinkron: ia menuntut protokol berbasis peristiwa
(`METransformNeedInput` / `METransformHaveOutput`) yang bentuknya cukup berbeda
untuk pantas dikerjakan terpisah, dengan jalur sinkron yang sudah terbukti
sebagai pembanding. Nama encoder yang terpilih selalu dilaporkan, jadi tidak
akan pernah ada keraguan mana yang sedang berjalan.

Frame terakhir dipertahankan dan dikirim ulang saat layar diam. Tanpa itu
aliran berhenti setiap kali tidak ada yang bergerak, dan penerima tidak dapat
membedakannya dari koneksi yang putus.

**45. Membuktikan bitstream-nya benar tanpa ffmpeg**

Mesin ini tidak punya ffmpeg maupun VLC, jadi "berkasnya jadi" bukan bukti apa
pun — sekumpulan byte acak juga punya ukuran dan laju bit. Tiga lapis
pemeriksaan menggantikannya:

**Pemisah NAL dan pembaca SPS ditulis sendiri.** Keduanya bukan perkakas uji
sekali pakai: paketisasi RTP di M2c bekerja pada NAL satuan, bukan unit akses
utuh. Pembaca SPS memakai Exp-Golomb lengkap, termasuk membuang emulation
prevention byte.

Hasilnya: SPS asli dari encoder Microsoft dibaca parser saya sendiri dan
menghasilkan **1920×1080** — dan **1080×1920** pada monitor tegak. Dua
implementasi berbeda menyepakati angka yang sama. Kedua vektor SPS itu kini
menjadi test, disalin byte demi byte dari keluaran sungguhan, bukan karangan.

**Uji pulang-pergi NV12.** Ini menutup celah yang paling berbahaya: bitstream
H.264 dapat tersusun sepenuhnya sah sambil berisi gambar sampah, karena salah
tata letak bidang kroma lolos setiap pemeriksaan struktural. Gambar uji berpola
blok 2×2 — satuan subsampling kroma — dikonversi ke NV12 lalu kembali, dan
setiap blok wajib tetap warnanya sendiri, bukan warna tetangganya.

**Matriks warna dipatok tepat.** Konversi memakai BT.709 rentang terbatas,
bukan BT.601, karena materinya beresolusi tinggi. Test-nya memeriksa nilai
persis: U untuk merah murni adalah **102** pada BT.709, sementara pada BT.601
angkanya 90. Selisih itulah yang membuat matriks keliru lolos dari pemeriksaan
longgar semacam "U harus di bawah 100", lalu muncul belakangan sebagai rona
kulit yang meleset — kesalahan yang bertahan lama justru karena tidak pernah
cukup mengganggu untuk diselidiki.

Ambang di test itu sempat salah, dan encoder-nya yang benar.

**46. Satu galat runtime yang pesannya tidak menolong**

`MFT_MESSAGE_COMMAND_FLUSH` sebelum streaming dimulai ditolak encoder bawaan
Windows dengan `E_FAIL` telanjang — masuk akal, karena belum ada apa pun untuk
dibuang, tetapi pesannya tidak mengatakan itu. Dihapus.

**47. M2c — WebRTC di sisi agent**

Signaling, TURN, persetujuan, dan siklus hidup sesi di server tidak disentuh
sama sekali. Agent native menggantikan **satu ujung** dari koneksi yang sudah
bekerja, persis seperti yang dijanjikan NEXT_PLAN.md §11 — dan viewer browser
yang sudah berjalan di produksi tidak diubah satu baris pun.

**Versi crate: 0.14, bukan 0.20.** Rilis 0.20 adalah penulisan ulang di atas
inti Sans-I/O dengan API yang jauh lebih rendah tingkatnya —
`TrackLocalStaticSample` di sana menuntut `MediaStreamTrack` yang SSRC dan
codec-nya disusun sendiri. Rancangannya menarik, tetapi mempelajarinya sambil
mengejar frame pertama menambah satu variabel yang tidak perlu. Dengan 0.14,
kode yang sudah ditulis kompilasi bersih pada percobaan pertama.

**Capture dan encode menempati thread OS sendiri.** Objek Direct3D dan Media
Foundation terikat pada thread tempat mereka dibuat dan bukan `Send`, sehingga
tidak dapat hidup di dalam task async yang bebas berpindah thread. Hasilnya
diserahkan lewat channel — yang juga kebetulan bentuk yang benar untuk
pekerjaan yang memblokir. `blocking_send` menahan thread capture saat penerima
tertinggal, dan itu perilaku yang diinginkan: menumpuk frame di memori hanya
menambah latensi tanpa menambah satu pun frame yang benar-benar sampai.

**Trickle ICE, bukan menunggu pengumpulan selesai.** Menunggu berarti beberapa
detik sunyi sebelum gambar pertama, dan pada jaringan yang harus jatuh ke TURN
penantian itu paling terasa.

**Kredensial TURN diambil saat agent menyala**, bukan saat sesi pertama
diminta. Relay yang tidak terkonfigurasi adalah kesalahan penyiapan, dan
menemukannya saat start jauh lebih baik daripada menemukannya ketika seseorang
sedang menunggu layar muncul.

**48. Celah yang saya buat sendiri, dan tutup**

`GET /api/v1/turn-credentials` memakai ekstraktor `Terautentikasi`, yang sejak
butir 36 **menolak token perangkat**. Artinya agent — pihak yang justru paling
membutuhkan relay — tidak bisa memperolehnya.

Ditutup dengan ekstraktor `SubjekTerautentikasi` yang menerima keduanya.
Sengaja langka: sebagian besar endpoint memang milik salah satu pihak saja, dan
pemisahan itulah yang menahan perangkat tersusupi agar tidak menjadi pijakan ke
seluruh organisasi. Kredensial TURN adalah pengecualian yang sah karena sebuah
sesi punya dua ujung. Nama pengguna TURN memuat id subjek, jadi pemakaian relay
tetap dapat ditelusuri ke perangkat tertentu.

Terverifikasi di produksi: `kredensial TURN diperoleh, relay tersedia`.

**49. Agent native tidak meminta persetujuan — dan itu masalah**

QUICK_CONNECT.md §4.1 mewajibkan prompt persetujuan yang menyebutkan siapa yang
meminta dan apa yang dapat ia lakukan. Agent berbasis browser memenuhinya
lengkap dengan tombol Tolak yang ber-`autofocus` dan Izinkan yang terkunci tiga
detik.

Agent native **menerima setiap permintaan secara otomatis**. Ia tidak punya
antarmuka untuk bertanya, dan tidak ada seorang pun yang perlu menyetujui.

Untuk mesin tanpa pengawasan itu memang perilaku yang dimaksudkan. Tetapi
konsekuensinya harus tertulis, bukan tersirat: **siapa pun yang memegang device
ID dan password sesi memperoleh layar mesin ini tanpa ada yang menyadarinya.**
Password sekali pakai dan rotasi setelah sesi adalah satu-satunya yang
membatasinya sekarang.

Ini pekerjaan M5, dan sudah dirancang di NEXT_PLAN.md §7.2 — indikator yang
selalu tampil, tingkat izin terpisah, jeda otomatis saat pengguna lokal
bergerak. Sampai itu ada, agent native ini pantas dijalankan hanya pada mesin
milik sendiri.

**50. Sesi pertama berhasil — dan mati setelah 185 detik**

Uji manusia pertama tersambung. Bukti dari kedua sisi:

```
ICE connection state    connected
peer connection state   connected
capture                 \\.\DISPLAY1 1920×1080, H264 Encoder MFT
sesi 751a2cb5…          active
viewer                  masamunekazuto21@gmail.com
```

Lalu, pada detik ke-185:

```
ERROR rdp_agent::rtc: encode gagal error=Not enough memory resources
                      are available to complete this operation. (0x8007000E)
INFO  rdp_agent::rtc: capture berhenti
```

Galatnya menuding encoder. Sebabnya ada di kode saya.

`ProcessOutput` menerima `MFT_OUTPUT_DATA_BUFFER` yang medan sampelnya
dibungkus `ManuallyDrop`. Versi pertama mengambil hasilnya dengan
`ManuallyDrop::into_inner(buf[0].pSample.clone())` — dan `clone` pada
`Option<IMFSample>` **menaikkan refcount COM**. Klon itu kemudian dilepas,
tetapi nilai asli yang masih duduk di dalam `ManuallyDrop` tidak pernah
dilepas sama sekali.

Satu `IMFSample` bocor setiap frame. Pada 1080p30 dengan sampel keluaran
sekitar 2 MB, itu **sekitar 60 MB per detik** — dan 185 detik adalah tepat
berapa lama mesin ini bertahan sebelum menyerah.

Diperbaiki dengan `ManuallyDrop::take`, yang memindahkan nilainya keluar
alih-alih menyalinnya. Terverifikasi: `encode` selama 45 detik menahan memori
**rata di 371 MB**, bergeser 3,6 MB. Sebelumnya rentang yang sama akan
menambah sekitar 2,7 GB.

Kebocoran ini juga sempat menjatuhkan `cargo build` — rustc mati dengan
"The paging file is too small" karena agent yang bocor masih berjalan.

**51. Gambar beku adalah kegagalan berbentuk paling buruk**

Ketika capture mati, penyuap track ikut berhenti tetapi peer connection tetap
terbuka. Viewer menerima frame terakhir lalu tidak menerima apa pun lagi —
gambarnya membeku, dan itu **terlihat seperti jaringan lambat**. Orang menunggu
alih-alih melapor.

Sekarang berhentinya aliran tanpa permintaan menutup peer connection, sehingga
viewer melihat sesi berakhir. Kegagalan yang kentara jauh lebih baik daripada
kegagalan yang menyamar.

**52. Dua batasan ICE yang terlihat di log**

`Unable to handle URL in gather_candidates_relay turn:...?transport=tcp` —
webrtc-rs 0.14 tidak mengumpulkan kandidat relay lewat TCP. Agent hanya
memakai relay UDP.

Akibatnya lebih sempit daripada kelihatannya: viewer berbasis browser tetap
dapat memakai TURN TCP, dan relay menjembatani keduanya. Yang benar-benar
gagal hanyalah keadaan ketika **jaringan agent sendiri** memblokir UDP
sepenuhnya.

Selain itu ada 10 peringatan `No available ipv6 IP address found` dan
`could not listen udp 169.254.x.x` — alamat link-local dan mDNS yang tidak
berguna bagi agent. Tidak merusak apa pun, tetapi mengotori log yang kelak
dipakai mendiagnosis kegagalan sungguhan.

**53. M3a — perpindahan monitor lewat DataChannel**

`MONITOR_LAYOUT` dan `MONITOR_SELECT` berjalan di atas **DataChannel WebRTC**,
bukan lewat Signal Server — NEXT_PLAN.md §8. Server tidak perlu tahu monitor
mana yang sedang dilihat, dan menaruhnya di sana berarti setiap perpindahan
menempuh perjalanan pulang-pergi lewat internet untuk keputusan yang sepenuhnya
lokal bagi kedua ujung.

**Monitor dirujuk dengan nama perangkat, bukan indeks.** Ini menutup catatan
terbuka dari sesi 2: urutan `EnumDisplayMonitors` tidak dijanjikan stabil,
sehingga mencabut satu layar menggeser indeks yang lain — dan viewer yang
menyimpan indeks akan menunjuk layar yang salah tanpa satu pun galat muncul.
Nama perangkat pun bukan jaminan mutlak, tetapi ia bertahan melewati kejadian
yang lazim: layar dimatikan, kabel dicabut sementara.

**Encoder dibuat ulang setiap perpindahan**, bukan dipakai lagi. Resolusi dan
orientasi ikut berubah, dan encoder H.264 tidak dapat mengganti ukuran frame di
tengah jalan. Membuatnya baru juga menghasilkan SPS, PPS, dan keyframe segar —
persis yang dibutuhkan dekoder di seberang untuk menyusun ulang dirinya.

**Perpindahan yang gagal tidak mematikan sesi.** Monitor lama tetap dipakai dan
peringatan dicatat. Layar yang diminta mungkin baru saja dicabut, dan sesi yang
sedang berjalan tidak pantas mati karenanya.

Di sisi viewer, pemilih monitor **tersembunyi bila hanya ada satu layar** — satu
tombol untuk satu-satunya pilihan hanya menambah bising. Tombolnya menampilkan
ukuran, bukan `\\.\DISPLAY1`, karena nama perangkat Windows tidak berarti apa
pun bagi pengguna.

Agent berbasis browser tidak membuat kanal kendali sama sekali, sehingga
pemilih monitor tetap tersembunyi di sana — dan itu memang benar, satu tab
hanya punya satu aliran layar.

**Belum dikerjakan dari M3:** track thumbnail beresolusi rendah untuk monitor
yang tidak aktif (NEXT_PLAN.md §5.4). Tanpa itu perpindahan tetap bekerja,
hanya saja pengguna memilih berdasarkan ukuran dan nomor, bukan berdasarkan
gambar kecil yang hidup.

**Terverifikasi dari viewer sungguhan.** Lima perpindahan bolak-balik,
1920×1080 ↔ 1080×1920, seluruhnya berhasil:

```
viewer meminta pindah monitor  monitor="\\.\DISPLAY2"
monitor berpindah  dari=\\.\DISPLAY1 ke=\\.\DISPLAY2 ukuran=1080×1920
monitor berpindah  dari=\\.\DISPLAY2 ke=\\.\DISPLAY1 ukuran=1920×1080
…
```

Sesi yang sama juga membuktikan perbaikan kebocoran per-frame: ia melewati
**297 detik**, jauh di atas 185 detik tempat sesi sebelumnya mati.

**54. Kebocoran kedua — satu encoder utuh per perpindahan**

Sesi yang sama menunjukkan memori agent duduk di **2.262 MB**, padahal saat
idle hanya 15 MB. Pengambilan sampel selama 30 detik menunjukkan angkanya
**mendatar**, tidak lagi tumbuh — jadi bukan kebocoran per frame.

Aritmetikanya langsung menunjuk sebabnya: satu pasang capture dan encoder
memakai sekitar 371 MB, sesi itu berpindah monitor lima kali, dan
6 × 371 = 2.226 MB. **Setiap perpindahan membocorkan encoder lamanya utuh.**

Sebabnya `IMFActivate`. Ia menyimpan rujukan internal ke objek yang dibuatnya,
dan rujukan itu **tidak ikut lepas** saat `IMFActivate`-nya dilepas —
satu-satunya yang melepasnya adalah `ShutdownObject`. Tanpa itu setiap encoder
yang pernah dibuat hidup selamanya beserta seluruh kolam buffernya.

Diperbaiki dengan menyimpan `IMFActivate` di dalam `H264` dan memanggil
`ShutdownObject` pada `Drop`.

**55. Uji rendam, dan bug yang ia temukan sendiri**

Kebocoran per-perpindahan tidak akan pernah terlihat pada uji frame tunggal —
uji 45 detik sebelumnya rata di 371 MB justru karena hanya ada satu encoder.
Karena itu `encode` diberi opsi `--ganti-tiap`, yang membangun ulang capture dan
encoder secara berkala di dalam satu proses.

Percobaan pertamanya langsung gagal, dan itu berguna:

```
Error: gagal memulai Desktop Duplication
Caused by: The parameter is incorrect. (0x80070057)
```

Desktop Duplication hanya mengizinkan **satu duplikasi per output**, dan
duplikasi lama baru dilepas setelah yang baru berhasil dibuat — jadi membuka
ulang output yang sama pasti ditolak. Jalur produksi tidak terkena karena
permintaan pindah ke monitor yang sedang aktif memang diabaikan lebih dulu,
tetapi urutannya kini tertulis, bukan kebetulan.

Uji rendam diubah menjadi bergantian antar monitor — yang juga persis meniru
apa yang dilakukan viewer. Hasilnya, dengan enam pembangunan ulang dalam 60
detik:

```
t= 6s  371 MB      t=36s  374 MB
t=18s  371 MB      t=48s  373 MB
t=30s  373 MB      t=60s  380 MB
```

Selisih 9 MB. Sebelum perbaikan, siklus yang sama menghasilkan sekitar 2,2 GB.

Ikut diperbaiki: pemeriksa bitstream sempat melaporkan "TIDAK cocok" pada
berkas uji rendam, karena ia membaca SPS pertama lalu membandingkannya dengan
monitor terakhir. Sekarang ia melaporkan seluruh resolusi yang muncul —
`1920×1080, 1080×1920 — aliran berpindah resolusi` — sehingga berkas yang sehat
tidak lagi terlihat salah.

**56. M4 — injeksi mouse dan papan ketik**

Ini modul yang mengubah sifat produk: sampai sekarang agent hanya *menunjukkan*
layar, dan mulai di sini ia menyerahkan kendali atasnya.

**Kendali tidak menyala dengan sendirinya.** Seluruh jalur mati kecuali agent
dijalankan dengan `--izinkan-kendali`. NEXT_PLAN.md §7.1 mewajibkan izin
diminta per tingkat, dan agent native tidak punya antarmuka untuk bertanya di
tengah sesi. Yang tersisa sebagai persetujuan yang jujur adalah keputusan orang
yang menyalakan agent, diambil sebelum siapa pun tersambung — dan peringatannya
dicetak menonjol di terminal, bukan disembunyikan di log.

Viewer diberi tahu tingkatnya lewat `CONTROL_LEVEL` saat kanal terbuka,
sehingga ia tidak menangkap papan ketik ketika tidak berhak. Viewer yang
menangkapnya tanpa hak hanya akan mencuri pintasan browser pengguna tanpa
menghasilkan apa pun.

**Koordinat relatif, bukan piksel.** Viewer mengirim 0,0–1,0 terhadap monitor
yang sedang dilihat; agent yang menerjemahkannya. Ini menghapus seluruh kelas
bug yang berasal dari perbedaan resolusi, penskalaan DPI, dan ukuran jendela
viewer.

Penerjemahannya bukan hal sepele pada susunan mesin ini. `SendInput` dengan
`MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK` tidak menerima piksel,
melainkan pecahan dari **seluruh virtual desktop**. Menghitungnya terhadap satu
monitor saja membuat setiap klik meleset — dan pada susunan berkoordinat
negatif, meleset ke arah yang berlawanan. Fungsi `ke_absolut` dan
`bounding_box` yang ditulis di sesi 2 dan sejak itu menganggur akhirnya
terpakai persis untuk ini.

**Scancode, bukan virtual key.** Mengirim virtual key membuat huruf yang
diketik bergantung pada tata letak yang aktif di mesin tujuan, sehingga viewer
ber-QWERTY yang mengakses mesin ber-AZERTY menghasilkan huruf yang salah.
`KeyboardEvent.code` di browser sudah menyatakan posisi fisik tombol, bukan
huruf yang tercetak padanya — jadi ia memang pasangan yang tepat untuk
scancode PS/2.

**57. Pengguna lokal selalu menang**

NEXT_PLAN.md §7.2: "orang yang merebut kembali kendali mesinnya sendiri harus
selalu menang". Agent membandingkan posisi kursor yang sebenarnya dengan posisi
terakhir yang **ia sendiri** tempatkan; menyimpang lebih dari 8 piksel berarti
ada tangan lain di mesin itu, dan injeksi dijeda tiga detik.

Selama jeda, input jarak jauh **dibuang**, bukan diantre. Input yang tertunda
lalu diputar sekaligus jauh lebih berbahaya daripada input yang hilang.

Viewer diberi tahu lewat `INPUT_PAUSED` dan menyebutkan sebabnya, supaya
pengguna tidak menyimpulkan koneksinya yang bermasalah.

Ini heuristik, bukan mekanisme yang pasti. Cara yang benar-benar membedakan
input fisik dari input suntikan adalah low-level hook dengan bendera
`LLMHF_INJECTED`, dan itu menuntut message loop tersendiri. Heuristik ini
menangkap kasus yang sebenarnya penting tanpa menambah thread yang harus
dijaga hidupnya.

**58. Satu jebakan di sisi viewer**

Koordinat dihitung terhadap **gambar**, bukan terhadap elemen video.
`object-fit: contain` menyisakan pita kosong di sisi yang tidak terisi;
menghitung terhadap seluruh elemen menggeser seluruh koordinat, dan gesernya
berubah setiap kali jendela diubah ukurannya. Pada monitor tegak yang
ditampilkan di jendela lanskap, pita itu lebar sekali.

Gerakan dibatasi sekitar 120 pesan per detik. Tanpa itu satu gerakan cepat
mengirim ratusan pesan yang seluruhnya menggambarkan jalur yang sama.

**Belum ada:** `Ctrl+Alt+Del` — tidak dapat disuntikkan `SendInput` dan
memerlukan service LocalSystem (NEXT_PLAN.md §6.3), serta Keyboard Lock API
untuk menahan pintasan yang ditelan browser (§6.4).

**Belum terverifikasi:** injeksi belum pernah dicoba dari viewer sungguhan.

**59. Delay 400 ms, dan gejalanya yang menyesatkan**

Laporan dari uji: kendali bekerja, "tapi agak delay — mouse realtime, ketika
pencet dan scroll delay, ketik juga".

Pola itu justru yang memberi jawabannya. Gerakan mouse terasa seketika karena
kursor yang terlihat adalah **kursor browser pengguna sendiri** — DXGI tidak
menyertakan kursor dalam gambar desktop, jadi tidak ada perjalanan jaringan
sama sekali di sana. Klik, gulir, dan ketikan baru terlihat hasilnya ketika
**frame video menyusul**. Yang lambat videonya, bukan inputnya.

Pengukuran menemukan tempatnya:

```
Tertahan di encoder      12 frame  (~400 ms pada 30 fps)
```

Encoder Media Foundation menahan frame untuk lookahead karena
`CODECAPI_AVLowLatencyMode` tidak pernah dinyalakan. Pada video biasa itu
wajar; pada remote desktop ia mendarat persis pada hal yang paling penting.

Setelah dinyalakan: **0 frame tertahan.**

Ditetapkan sebelum tipe media, karena sebagian encoder mengunci konfigurasi
pipanya begitu tipe keluaran diterima.

**60. Pertukaran yang muncul bersamanya**

Laju keluar melonjak dari 1,71 menjadi 8,2 Mbps untuk isi layar yang sama.
Dugaan pertama menyalahkan CBR yang ikut dipasang — dan itu keliru: melepasnya
tidak mengubah apa pun. Sebabnya mode latensi rendah itu sendiri. Tanpa
lookahead, encoder kehilangan kemampuan menabung bit pada bagian yang mudah,
sehingga lajunya menjadi hampir rata.

Jadi pertukarannya nyata dan tidak dapat dihindari di encoder ini: **400 ms
pada 1,7 Mbps, atau nol pada laju rata**. Untuk remote desktop latensi menang.

Yang berubah sebagai gantinya adalah arti angka bitrate: ia berhenti menjadi
atap yang jarang tersentuh dan menjadi laju yang benar-benar dipakai. Karena
itu bakunya diturunkan dari 8 ke 4 Mbps. Terukur presisi: setelan 2 memberi
2,20 Mbps, setelan 4 memberi 4,40 Mbps.

**61. 60 fps sanggup**

| fps | Tercapai | Laju | Tertahan |
|---|---|---|---|
| 30 | 30,0 | 4,4 Mbps | 0 frame |
| 60 | 60,0 | 4,4 Mbps | 0 frame |

Encoder **perangkat lunak** menahan 1080p60 tanpa tertinggal. Bakunya tetap 30
karena mesin agent tidak selalu sekuat ini, tetapi `--fps 60` terbukti.

Dua penyangga lain ikut dipendekkan: antrean unit akses dari 8 slot menjadi 2 —
delapan slot berarti seperempat detik gambar basi yang tetap harus dikirim
sebelum yang terbaru sampai — dan viewer meminta `playoutDelayHint = 0`, karena
baku browser menukar latensi dengan kemulusan, pertukaran yang terbalik pada
remote desktop.

**63. M5a — alias perangkat dan kata sandi tetap**

Permintaan berikutnya: aplikasi Windows yang dipasang di mesin lain,
menampilkan ID dan kata sandinya sendiri, dapat menggantinya, ikut organisasi
yang terdaftar, dan meminta izin **sekali** lalu mengingat siapa yang boleh
masuk — kebiasaan yang sudah dikenal orang dari UltraViewer dan AnyDesk.

Dua bagian dari permintaan itu berbenturan dengan rancangan yang ada, dan
keduanya diselesaikan ke arah yang sama dengan pilihan AnyDesk sendiri.

**Nomor perangkat tetap tidak dapat diubah; yang ditambahkan adalah alias.**
Nomor sembilan digit membawa check digit Damm, `sessions.device_id_snapshot`
menyimpannya apa adanya supaya riwayat bertahan setelah perangkat dihapus, dan
nomor yang dapat dipilih mengundang penyamaran — mengambil nomor mirip milik
mesin lain agar orang salah menyambung. Alias menutup kebutuhan yang sebenarnya
("ID yang mudah diingat") tanpa satu pun biaya itu.

Aliasnya dibatasi ketat, dan setiap batasan punya sebabnya: huruf kecil saja
supaya `PC-Kantor` dan `pc-kantor` tidak pernah menjadi dua perangkat, tanpa
spasi karena alias dibacakan lewat telepon sama seperti nomor, dan **tidak
boleh berupa sembilan digit** — tanpa larangan itu seseorang dapat mengambil
alias yang persis sama dengan nomor perangkat orang lain, dan Quick Connect
tidak akan pernah dapat memutuskan mana yang dimaksud.

**Dua kata sandi, bukan satu.** Yang acak 40 bit dan berotasi setiap sesi tetap
ada untuk bantuan sesaat yang dibacakan lewat telepon. Yang tetap dipilih
manusia — dan manusia memilih buruk, jadi ia dijaga minimal 10 karakter,
menolak daftar yang paling umum, dan menolak yang hanya berisi tiga karakter
berbeda.

Keduanya **selalu diperiksa**, bahkan setelah yang pertama cocok. Berhenti
lebih awal membuat lama respons memberi tahu kata sandi mana yang benar, dan
itu memberi penyerang cara memilah tebakannya. Lantai waktu 250 ms sudah
menutupi biaya keduanya.

Satu perbedaan halus yang disengaja: kata sandi sesi dinormalkan (huruf besar,
tanpa spasi) karena ia dibacakan; kata sandi tetap **tidak**, karena ia dipilih
manusia dan boleh memuat apa saja. Menormalkannya akan diam-diam mengubah apa
yang pengguna ketik.

**64. Endpoint swalayan**

Tiga endpoint baru yang dipanggil dengan **token perangkat**, bukan sesi
pengguna — inilah yang membuat aplikasi Windows dapat menampilkan dan mengubah
identitasnya tanpa pemiliknya membuka dashboard.

| Endpoint | Isi |
|---|---|
| `GET /devices/self` | nomor, alias, organisasi, status, apakah sandi tetap aktif |
| `PUT /devices/self/handle` | memasang atau menghapus alias |
| `PUT /devices/self/passwords` | merotasi sandi sesi, memasang atau menghapus sandi tetap |

Perangkat hanya pernah dapat menyentuh dirinya sendiri: UUID diambil dari klaim
token yang sudah diverifikasi, bukan dari badan permintaan, dan tenant selalu
ikut menjadi syarat di sisi SQL.

Sudah dapat dipakai lewat CLI sebelum GUI ada: `rdp-agent alias pc-kantor`,
`rdp-agent sandi --tetap <SANDI>`, dan `rdp-agent status` yang kini menampilkan
keadaan lokal **dan** keadaan menurut server. Bagian server sengaja tidak
menggagalkan perintah saat jaringan mati — identitas lokal tetap berguna
dilihat pada mesin yang sedang tidak terhubung.

Terverifikasi di produksi: alias `pc-masamune` dan nomor `543 096 477`
menunjuk perangkat yang sama, alias berspasi ditolak dengan pesan yang
menjelaskan, kata sandi enam karakter ditolak, dan perubahannya tercatat di
audit sebagai `device.set_handle`.

**65. Dua bug uji, bukan bug kode**

Uji perilaku migrasi melaporkan "hapus sandi tetap: GAGAL". Diperiksa terpisah,
penghapusannya benar — `hash=NULL set_at=NULL` dan sandi sesi tidak tersentuh.
Yang salah ujinya: SQL tidak menjamin urutan evaluasi antara pemanggilan fungsi
dan subquery di sisi lain `AND`, sehingga subquery membaca keadaan sebelum
fungsi berjalan.

Uji unit `normalkan_kunci` gagal pada nomor `123456782` yang saya tulis tangan —
check digit Damm sulit dihitung di kepala, dan vektor karangan itu memang tidak
sah. Ujinya kini membangkitkan nomor lewat `DeviceId::generate()` alih-alih
mengarangnya.

**67. M5b — persetujuan dan daftar kepercayaan**

Menutup lubang yang ditandai sejak butir 49: agent native **menerima setiap
permintaan sesi secara otomatis**, karena ia tidak punya antarmuka untuk
bertanya. QUICK_CONNECT.md §4.1 mewajibkan prompt yang menyebutkan siapa yang
meminta; agent berbasis browser memenuhinya sejak awal, agent native tidak
pernah bisa.

Sekarang bisa, dan sekaligus mewujudkan yang diminta: **izinkan sekali,
berikutnya tidak usah**.

| Mode | Perilaku |
|---|---|
| `--izinkan-semua` | menerima siapa pun — hanya untuk mesin sendiri |
| `--tanpa-dialog` | hanya yang sudah ada di daftar |
| baku | memunculkan kotak persetujuan, lalu mengingat jawabannya |

**Daftar kepercayaan hidup di mesin agent, bukan di server.** Itu bukan detail
penyimpanan. Persetujuan adalah milik orang yang duduk di depan mesin itu;
menaruh daftarnya di server berarti siapa pun yang menguasai server dapat
menambahkan dirinya sendiri ke dalamnya. Server tidak pernah tahu isinya dan
tidak pernah dapat mengubahnya.

Konsekuensi yang diterima: memasang ulang agent menghapus seluruh kepercayaan.
Itu justru benar — mesin yang baru dipasang belum pernah menyetujui siapa pun.

**Dipercaya berdasarkan `user_id`, bukan email.** Email dapat berpindah
pemilik, dan daftar yang menunjuk email lama akan diam-diam mengizinkan orang
yang salah. Karena itu `SessionOffer` di protokol signaling diberi
`viewer_user_id`, diambil dari klaim token yang sudah diverifikasi — tidak
pernah dari apa pun yang dikirim viewer. Prompt yang menampilkan nama pilihan
penyerang sendiri justru membantu penipuan.

Tiga keadaan yang semuanya berakhir sebagai penolakan, dan masing-masing
disengaja: tidak dijawab dalam 45 detik, jendela ditutup tanpa memilih, dan
daftar kepercayaan yang gagal dibaca. **Diam bukan persetujuan**, dan berkas
rusak tidak boleh berubah menjadi izin.

Persetujuan diminta **sebelum** capture disiapkan. Membuka Desktop Duplication
lebih dulu berarti mesin mulai menangkap layarnya untuk permintaan yang mungkin
ditolak.

**68. Kotak dialog dulu, jendela menyusul**

Rencana awalnya langsung ke jendela egui. Tetapi M5b tanpa antarmuka membuat
agent menolak **semuanya** — daftar masih kosong dan tidak ada yang bisa
ditanya — sehingga merilisnya sendirian justru membuat keadaan lebih buruk
daripada sebelumnya.

Karena itu persetujuan memakai `MessageBoxW` lebih dulu: sudah ada di crate
`windows` yang dipakai capture, jadi tanpa satu pun dependensi baru. Bentuknya
sederhana dan tombolnya mengikuti bahasa sistem, tetapi ia muncul di depan
(`MB_SYSTEMMODAL`), memaksa jawaban, dan menyebutkan siapa yang meminta beserta
apa yang dapat ia lakukan.

Pemetaan tombolnya ditulis di dalam pesannya sendiri, karena label Yes/No/Cancel
tidak dapat diubah: Ya berarti izinkan dan ingat, Tidak berarti izinkan sekali
saja, Batal berarti tolak.

Jendela egui yang sesungguhnya — dengan ID, alias, kata sandi, dan daftar
tepercaya yang dapat dicabut — tetap menjadi M5c.

**69. Keadaan M2**

| Bagian | Keadaan |
|---|---|
| Capture DXGI | **selesai** |
| Encode H.264 | **selesai** (perangkat lunak; perangkat keras menyusul) |
| Kirim lewat WebRTC | **selesai dan terbukti di produksi** |

**M2 selesai.** Layar mesin Windows tampil di browser lewat Quick Connect,
menggantikan agent berbasis tab — persis sasaran yang ditetapkan NEXT_PLAN.md
untuk tahap ini.

Biner agent 11,8 MB — masih di bawah batas NFR-PER-06 (20 MB); WebRTC menambah
sekitar 7 MB.

Yang belum diuji: sesi lintas jaringan yang benar-benar memaksa relay. Sesi
pertama tersambung cepat, yang menunjukkan jalur langsung, jadi TURN belum
pernah benar-benar membawa media.

Pemutaran dan konversi warna masih di CPU. Tempatnya kelak di GPU, sesuai
STREAMING.md §1.

### Keadaan M1

| Bagian | Keadaan |
|---|---|
| Enumerasi monitor | **selesai dan terverifikasi** |
| Identitas perangkat Ed25519 | **selesai** |
| Enrolment dan registrasi | **selesai** |
| Heartbeat | **selesai** |
| Koneksi signaling | **selesai** |

M1 selesai. Berikutnya M2 — capture DXGI Desktop Duplication.

Permintaan sesi yang masuk sekarang **ditolak dengan alasan tertulis**
("belum mendukung berbagi layar (M2)"), bukan didiamkan. Viewer yang menunggu
tanpa jawaban jauh lebih membingungkan daripada penolakan yang menyebut
sebabnya.

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
