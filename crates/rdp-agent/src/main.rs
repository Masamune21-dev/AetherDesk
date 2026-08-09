//! Agent native AetherDesk.
//!
//! Keadaan sekarang: **M1 selesai** — enumerasi monitor, identitas perangkat
//! Ed25519, enrolment, heartbeat, dan koneksi signaling.
//!
//! Enumerasi monitor sengaja dikerjakan lebih dulu karena dapat dibuktikan
//! tanpa capture, tanpa encode, dan tanpa jaringan. Menjalankan
//! `rdp-agent monitors` di mesin bermonitor tiga, lalu melihat ketiganya
//! terdaftar dengan koordinat yang benar — termasuk koordinat negatif untuk
//! monitor di sebelah kiri — membuktikan fondasi multi-monitornya sehat
//! sebelum satu piksel pun ditangkap.
//!
//! Yang **belum** ada adalah capture layar (M2) dan injeksi input (M4).
//! Peta jalan lengkap ada di `docs/NEXT_PLAN.md`.

mod api;
mod capture;
mod encode;
mod identitas;
mod input;
mod monitor;
mod rtc;
mod signal;

use anyhow::{bail, Result};
use rdp_core::DeviceKeypair;

fn main() -> Result<()> {
    init_tracing();

    let argumen: Vec<String> = std::env::args().skip(1).collect();
    let perintah = argumen.first().map(String::as_str).unwrap_or("help");

    match perintah {
        "monitors" => cetak_monitor(),
        "capture" => perintah_capture(&argumen[1..]),
        "encode" => perintah_encode(&argumen[1..]),
        "enrol" | "enroll" => jalankan_async(perintah_enrol(&argumen[1..])),
        "connect" => jalankan_async(perintah_connect(&argumen[1..])),
        "status" => perintah_status(),
        "--help" | "-h" | "help" => {
            bantuan();
            Ok(())
        }
        lain => {
            eprintln!("perintah tidak dikenal: {lain}\n");
            bantuan();
            std::process::exit(2);
        }
    }
}

/// Runtime dibangun hanya untuk perintah yang memerlukannya.
///
/// `monitors` dan `status` murni lokal dan sinkron; memaksa seluruh biner
/// melalui `#[tokio::main]` akan menyalakan kolam thread untuk pekerjaan yang
/// tidak pernah menyentuh jaringan.
fn jalankan_async<F: std::future::Future<Output = Result<()>>>(f: F) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(f)
}

fn bantuan() {
    println!(
        "\
rdp-agent — agent native AetherDesk

PERINTAH
  monitors               Menyebutkan monitor beserta koordinat virtual desktop
  capture                Menangkap layar dan melaporkan hasilnya
  enrol --token <TOKEN>  Mendaftarkan mesin ini memakai token enrolment
  connect                Menyambung ke server dan tetap online
  status                 Menampilkan identitas perangkat yang tersimpan
  help                   Menampilkan bantuan ini

OPSI enrol
  --token <TOKEN>        Token enrolment dari dashboard (wajib)
  --server <URL>         Alamat server (baku: {baku})
  --alias <NAMA>         Nama yang ditampilkan di dashboard

OPSI connect
  --monitor <NAMA>       Monitor yang dibagikan (baku: primer)
  --fps <N>              Sasaran laju frame (baku: 30; 60 terbukti sanggup)
  --mbps <N>             Sasaran bitrate (baku: 4)
  --izinkan-kendali      Izinkan viewer menggerakkan mouse dan mengetik.
                         BAKU MATI. Lihat NEXT_PLAN.md §7 sebelum memakainya.

OPSI capture
  --monitor <NAMA>       Nama perangkat, mis. \\\\.\\DISPLAY2 (baku: primer)
  --detik <N>            Lama pengambilan (baku: 3)
  --simpan <BERKAS.bmp>  Menyimpan frame terakhir untuk diperiksa mata

LINGKUNGAN
  AETHERDESK_DIR         Direktori identitas, menimpa lokasi baku
  AETHERDESK_LOG         Filter log, mis. debug

BELUM TERSEDIA
  input       Injeksi mouse dan papan ketik (M4)

Lihat docs/NEXT_PLAN.md untuk urutan pengerjaannya.",
        baku = identitas::SERVER_BAKU
    );
}

// ── monitors ─────────────────────────────────────────────────────────────────

fn cetak_monitor() -> Result<()> {
    let monitors = monitor::enumerasi()?;

    println!("\n{} monitor terdeteksi\n", monitors.len());
    println!(
        "{:<4} {:<22} {:>7} {:>7} {:>7} {:>7} {:>6}  {}",
        "ID", "NAMA", "X", "Y", "LEBAR", "TINGGI", "SKALA", "PRIMER"
    );
    println!("{}", "─".repeat(81));

    for m in &monitors {
        println!(
            "{:<4} {:<22} {:>7} {:>7} {:>7} {:>7} {:>5}%  {}",
            m.id,
            potong(&m.name, 22),
            m.x,
            m.y,
            m.width,
            m.height,
            m.scale_percent,
            if m.is_primary { "ya" } else { "" }
        );
    }

    if let Some((x, y, w, h)) = monitor::bounding_box(&monitors) {
        println!("\nVirtual desktop: {w}×{h} mulai dari ({x}, {y})");
    }

    // Justru inilah yang paling perlu terlihat. Susunan dengan monitor di
    // sebelah kiri menghasilkan koordinat negatif, dan itulah bentuk yang
    // membongkar implementasi yang memakai tipe tak bertanda. (Temuan T-16)
    let negatif: Vec<_> = monitors.iter().filter(|m| m.x < 0 || m.y < 0).collect();
    if negatif.is_empty() {
        println!(
            "\nSeluruh monitor berkoordinat non-negatif. Untuk menguji jalur\n\
             yang paling rawan, pindahkan satu monitor ke sebelah KIRI monitor\n\
             primer lewat pengaturan tampilan, lalu jalankan ulang."
        );
    } else {
        println!("\n{} monitor berkoordinat negatif:", negatif.len());
        for m in negatif {
            println!("  {} pada ({}, {})", m.name, m.x, m.y);
        }
        println!("Inilah susunan yang wajib ikut diuji sebelum injeksi input dibuat.");
    }

    Ok(())
}

// ── capture ──────────────────────────────────────────────────────────────────

fn perintah_capture(argumen: &[String]) -> Result<()> {
    let nama = opsi(argumen, "--monitor");
    let detik: u64 = opsi(argumen, "--detik")
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let simpan = opsi(argumen, "--simpan");

    let mut dup = capture::Duplikasi::buka(nama.as_deref())?;
    println!("\nMenangkap {}", dup.nama_output);
    println!(
        "  Tampilan   {}×{}  (rotasi {})\n",
        dup.width,
        dup.height,
        capture::nama_rotasi(dup.rotation)
    );

    let mulai = std::time::Instant::now();
    let batas = std::time::Duration::from_secs(detik);
    let mut frame_terakhir = None;
    let mut jumlah = 0u32;
    let mut kosong = 0u32;
    let mut total_byte = 0usize;

    while mulai.elapsed() < batas {
        match dup.ambil(100)? {
            Some(f) => {
                jumlah += 1;
                total_byte += f.bytes();
                frame_terakhir = Some(f);
            }
            None => kosong += 1,
        }
    }

    let berlalu = mulai.elapsed().as_secs_f64();

    // Ukuran permukaan sesungguhnya baru diketahui setelah frame pertama:
    // ia dibaca dari tekstur yang diserahkan DXGI, bukan dari deskripsi mode.
    if jumlah > 0 && (dup.surface_width, dup.surface_height) != (dup.width, dup.height) {
        println!(
            "{:<26} {}×{} → diputar ke {}×{}",
            "Permukaan DXGI", dup.surface_width, dup.surface_height, dup.width, dup.height
        );
    }
    println!("{:<26} {}", "Frame berubah", jumlah);
    println!("{:<26} {}", "Polling tanpa perubahan", kosong);
    println!("{:<26} {:.1}", "Frame per detik", jumlah as f64 / berlalu);
    println!(
        "{:<26} {:.1} MB/dtk mentah",
        "Laju data BGRA",
        total_byte as f64 / berlalu / 1_048_576.0
    );

    if jumlah == 0 {
        println!(
            "\nTidak ada satu pun frame berubah. Desktop Duplication hanya\n\
             menyerahkan frame ketika ada yang bergerak — gerakkan jendela atau\n\
             putar video di monitor tersebut, lalu jalankan lagi."
        );
        return Ok(());
    }

    if let (Some(f), Some(path)) = (&frame_terakhir, &simpan) {
        let p = std::path::Path::new(path);
        capture::tulis_bmp(f, p)?;
        println!("\nFrame terakhir disimpan: {}", p.display());
        println!("Buka berkas itu — bila gambarnya benar, seluruh jalur capture sehat.");
    } else if simpan.is_none() {
        println!("\nTambahkan --simpan layar.bmp untuk memeriksa hasilnya dengan mata.");
    }

    Ok(())
}

// ── encode ───────────────────────────────────────────────────────────────────

fn perintah_encode(argumen: &[String]) -> Result<()> {
    let nama = opsi(argumen, "--monitor");
    let detik: u64 = opsi(argumen, "--detik")
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let fps: u32 = opsi(argumen, "--fps")
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let mbps: f64 = opsi(argumen, "--mbps")
        .and_then(|v| v.parse().ok())
        .unwrap_or(8.0);
    let keluar = opsi(argumen, "--keluar").unwrap_or_else(|| "layar.h264".into());

    // Uji rendam: membangun ulang capture dan encoder secara berkala, meniru
    // apa yang terjadi setiap kali viewer berpindah monitor. Jalur itu pernah
    // membocorkan satu encoder utuh per perpindahan, dan kebocoran seperti itu
    // hanya terlihat bila siklusnya diulang di dalam satu proses.
    let ganti_tiap: Option<u64> = opsi(argumen, "--ganti-tiap").and_then(|v| v.parse().ok());

    let mut dup = capture::Duplikasi::buka(nama.as_deref())?;
    let mut enc = encode::H264::baru(dup.width, dup.height, fps, (mbps * 1_000_000.0) as u32)?;

    println!("\nMenangkap {} — {}×{}", dup.nama_output, dup.width, dup.height);
    println!("Encoder    {}", enc.nama);
    println!("Sasaran    {fps} fps, {mbps} Mbps\n");

    let mut berkas = std::fs::File::create(&keluar)?;
    let mulai = std::time::Instant::now();
    let batas = std::time::Duration::from_secs(detik);
    let jarak = std::time::Duration::from_micros(1_000_000 / fps.max(1) as u64);

    let mut ditangkap = 0u32;
    let mut dikode = 0u32;
    let mut byte_keluar = 0usize;
    // Berapa frame yang sudah masuk encoder tetapi belum keluar. Inilah
    // kedalaman pipa encoder, dan pada 30 fps setiap frame yang tertahan
    // berarti 33 ms tambahan sebelum gambar sampai ke mata.
    let mut tertahan = 0i64;
    let mut tertahan_maks = 0i64;
    let mut terakhir: Option<capture::Frame> = None;
    let mut berikutnya = std::time::Instant::now();

    // Berputar antar monitor, bukan membuka ulang yang sama. Desktop
    // Duplication hanya mengizinkan **satu** duplikasi per output, dan yang
    // lama baru dilepas setelah yang baru berhasil dibuat — jadi membuka ulang
    // output yang sama pasti ditolak. Bergantian antar monitor juga persis
    // meniru apa yang dilakukan viewer.
    let daftar_monitor: Vec<String> = monitor::enumerasi()
        .map(|m| m.into_iter().map(|x| x.name).collect())
        .unwrap_or_default();
    let mut ganti_berikutnya = ganti_tiap.map(|d| mulai + std::time::Duration::from_secs(d));
    let mut siklus = 0usize;

    while mulai.elapsed() < batas {
        if let (Some(waktu), Some(d)) = (ganti_berikutnya, ganti_tiap) {
            if std::time::Instant::now() >= waktu && daftar_monitor.len() > 1 {
                siklus += 1;
                let target = &daftar_monitor[siklus % daftar_monitor.len()];
                dup = capture::Duplikasi::buka(Some(target))?;
                enc = encode::H264::baru(
                    dup.width,
                    dup.height,
                    fps,
                    (mbps * 1_000_000.0) as u32,
                )?;
                terakhir = None;
                ganti_berikutnya =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(d));
                println!("  siklus {siklus}: pindah ke {target} ({}×{})", dup.width, dup.height);
            }
        }

        // Frame terakhir dipertahankan dan dikirim ulang saat layar diam.
        // Tanpa itu aliran akan berhenti setiap kali tidak ada yang bergerak,
        // dan penerima tidak dapat membedakannya dari koneksi yang putus.
        if let Some(f) = dup.ambil(5)? {
            terakhir = Some(f);
            ditangkap += 1;
        }

        if std::time::Instant::now() < berikutnya {
            continue;
        }
        berikutnya += jarak;

        let Some(f) = &terakhir else { continue };
        let waktu = mulai.elapsed().as_nanos() as i64 / 100;
        let unit = enc.encode(&f.data, waktu)?;
        tertahan += 1;
        tertahan -= unit.len() as i64;
        tertahan_maks = tertahan_maks.max(tertahan);
        for au in unit {
            use std::io::Write;
            byte_keluar += au.len();
            berkas.write_all(&au)?;
            dikode += 1;
        }
    }

    for au in enc.kuras()? {
        use std::io::Write;
        byte_keluar += au.len();
        berkas.write_all(&au)?;
        dikode += 1;
    }
    drop(berkas);

    let berlalu = mulai.elapsed().as_secs_f64();
    let mentah = ditangkap as f64 * (dup.width * dup.height * 4) as f64;

    println!("{:<24} {}", "Frame ditangkap", ditangkap);
    println!("{:<24} {}", "Frame dikode", dikode);
    println!(
        "{:<24} {} frame  (~{:.0} ms pada {fps} fps)",
        "Tertahan di encoder",
        tertahan_maks,
        tertahan_maks as f64 * 1000.0 / fps.max(1) as f64
    );
    println!("{:<24} {:.1}", "Frame per detik", dikode as f64 / berlalu);
    println!(
        "{:<24} {:.2} Mbps",
        "Laju keluar",
        byte_keluar as f64 * 8.0 / berlalu / 1e6
    );
    if mentah > 0.0 {
        println!(
            "{:<24} {:.0}×",
            "Rasio kompresi",
            mentah / byte_keluar.max(1) as f64
        );
    }

    periksa_bitstream(&keluar, dup.width, dup.height)?;
    Ok(())
}

/// Memeriksa bahwa berkas keluaran benar-benar H.264 yang tersusun sah.
///
/// Ukuran berkas dan laju bit tidak membuktikan apa pun — sekumpulan byte acak
/// juga punya keduanya. Yang membuktikan adalah SPS yang dapat dibaca dan
/// menyebutkan dimensi yang sama dengan yang diminta.
fn periksa_bitstream(path: &str, w: u32, h: u32) -> Result<()> {
    let isi = std::fs::read(path)?;
    let nal = encode::pisah_nal(&isi);

    let mut sps = 0;
    let mut pps = 0;
    let mut idr = 0;
    let mut lain = 0;
    // Seluruh dimensi yang muncul, bukan hanya yang pertama. Aliran yang
    // berpindah monitor memuat lebih dari satu resolusi, dan melaporkan yang
    // pertama saja membuat berkas yang sehat terlihat salah.
    let mut dimensi: Vec<(u32, u32)> = Vec::new();

    for n in &nal {
        match encode::tipe_nal(n) {
            encode::NAL_SPS => {
                sps += 1;
                if let Some(d) = encode::baca_sps(n) {
                    if !dimensi.contains(&d) {
                        dimensi.push(d);
                    }
                }
            }
            encode::NAL_PPS => pps += 1,
            encode::NAL_IDR => idr += 1,
            _ => lain += 1,
        }
    }

    println!("\nPemeriksaan bitstream — {path}");
    println!("  {:<22} {} byte", "Ukuran", isi.len());
    println!("  {:<22} {}", "NAL", nal.len());
    println!("  {:<22} SPS {sps}, PPS {pps}, IDR {idr}, lain {lain}", "Jenis");

    match dimensi.as_slice() {
        [] => println!("  {:<22} tidak terbaca", "Dimensi dari SPS"),
        [(dw, dh)] if (*dw, *dh) == (w, h) => {
            println!("  {:<22} {dw}×{dh} — cocok", "Dimensi dari SPS");
        }
        [(dw, dh)] => {
            println!(
                "  {:<22} {dw}×{dh} — TIDAK cocok, diminta {w}×{h}",
                "Dimensi dari SPS"
            );
        }
        banyak => {
            let daftar: Vec<String> = banyak.iter().map(|(a, b)| format!("{a}×{b}")).collect();
            println!(
                "  {:<22} {} — aliran berpindah resolusi",
                "Dimensi dari SPS",
                daftar.join(", ")
            );
        }
    }

    if sps == 0 || pps == 0 || idr == 0 {
        println!("\n  Bitstream tidak lengkap — tanpa SPS, PPS, atau keyframe,");
        println!("  tidak ada dekoder yang dapat memulainya.");
    } else {
        println!("\n  Bitstream lengkap. Berkas ini dapat diputar VLC apa adanya.");
    }

    Ok(())
}

// ── enrol ────────────────────────────────────────────────────────────────────

async fn perintah_enrol(argumen: &[String]) -> Result<()> {
    let Some(token) = opsi(argumen, "--token") else {
        bail!("--token wajib diisi. Terbitkan token enrolment dari dashboard.");
    };
    let server = opsi(argumen, "--server").unwrap_or_else(|| identitas::SERVER_BAKU.to_string());
    let alias = opsi(argumen, "--alias");

    // Enrolment ulang akan meninggalkan baris perangkat lama yang tidak pernah
    // lagi terhubung, dan token sekali pakai sudah terlanjur habis. Lebih baik
    // berhenti dan meminta keputusan eksplisit.
    if identitas::sudah_enrol() {
        bail!(
            "mesin ini sudah ter-enrol ({}). Hapus {} lebih dulu bila memang \
             ingin mendaftar ulang.",
            identitas::muat_konfig()
                .map(|k| k.device_id)
                .unwrap_or_else(|_| "identitas rusak".into()),
            identitas::direktori()?.display()
        );
    }

    let kunci = DeviceKeypair::generate();
    let klien = api::Klien::baru(format!("{}/api/v1", server.trim_end_matches('/')))?;
    let hostname = api::hostname();

    let hasil = klien
        .enrol(
            &token,
            &kunci.public_key(),
            alias.as_deref(),
            hostname.as_deref(),
        )
        .await?;

    let konfig = identitas::Konfigurasi {
        device_uuid: hasil.device_uuid,
        device_id: hasil.device_id.clone(),
        server,
    };
    identitas::simpan(&konfig, &kunci)?;

    println!("\nPerangkat terdaftar.\n");
    println!("  Device ID        {}", hasil.device_id_tampil);
    println!("  Password sesi    {}", hasil.session_password);
    println!("  Identitas        {}", identitas::direktori()?.display());
    println!(
        "\nPassword sesi hanya ditampilkan sekali. Catat sekarang — setelah ini\n\
         hanya hash-nya yang tersimpan, dan rotasinya lewat dashboard.\n\n\
         Jalankan `rdp-agent connect` supaya perangkat tampil online."
    );

    Ok(())
}

// ── connect ──────────────────────────────────────────────────────────────────

async fn perintah_connect(argumen: &[String]) -> Result<()> {
    let konfig = identitas::muat_konfig()?;
    let kunci = identitas::muat_kunci()?;

    let fps: u32 = opsi(argumen, "--fps")
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let atur = rtc::Pengaturan {
        monitor: opsi(argumen, "--monitor"),
        fps,
        // Bitrate baku mengikuti laju frame, bukan angka tetap.
        //
        // Bitrate adalah anggaran per **detik**; yang menentukan ketajaman
        // adalah anggaran per **frame**. Angka tetap membuat `--fps 60`
        // diam-diam memotong separuh jatah tiap frame, dan gejalanya muncul
        // persis saat seluruh layar berubah — scroll cepat menjadi buram
        // seolah resolusinya turun.
        //
        // Sekitar 133 kbit per frame: 4 Mbps pada 30 fps, 8 Mbps pada 60.
        bitrate: opsi(argumen, "--mbps")
            .and_then(|v| v.parse::<f64>().ok())
            .map(|m| (m * 1_000_000.0) as u32)
            .unwrap_or(fps.clamp(1, 120) * 133_333),
        izinkan_kendali: argumen.iter().any(|a| a == "--izinkan-kendali"),
    };

    tracing::info!(
        device_id = %konfig.device_id,
        server = %konfig.server,
        fps = atur.fps,
        "agent mulai"
    );

    // Dicetak menonjol, bukan disembunyikan di log. Orang yang menyalakan agent
    // ini perlu tahu persis apa yang baru saja ia izinkan.
    if atur.izinkan_kendali {
        println!(
            "\n  ⚠ KENDALI PENUH AKTIF\n\n  \
             Siapa pun yang memegang Device ID dan kata sandi sesi dapat\n  \
             menggerakkan mouse dan mengetik di mesin ini. Agent native belum\n  \
             menampilkan prompt persetujuan, sehingga tidak ada yang perlu\n  \
             menyetujui saat sesi dimulai.\n\n  \
             Menggerakkan mouse fisik akan menjeda input jarak jauh selama\n  \
             {} detik.\n",
            input::JEDA_LOKAL.as_secs()
        );
    } else {
        println!("\n  Mode lihat-saja. Tambahkan --izinkan-kendali untuk memberi kendali.\n");
    }

    // Enumerasi sekali di awal. Belum dikirim ke mana pun — MONITOR_LAYOUT
    // baru ada di M3 — tetapi kegagalannya di sini jauh lebih mudah didiagnosis
    // daripada nanti di tengah sesi.
    match monitor::enumerasi() {
        Ok(m) => {
            tracing::info!(jumlah = m.len(), "monitor terdeteksi");
            if atur.monitor.is_none() {
                if let Some(p) = m.iter().find(|x| x.is_primary) {
                    tracing::info!(monitor = %p.name, "akan membagikan monitor primer");
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "enumerasi monitor gagal"),
    }

    signal::jalankan(konfig, kunci, atur).await
}

// ── status ───────────────────────────────────────────────────────────────────

fn perintah_status() -> Result<()> {
    if !identitas::sudah_enrol() {
        println!(
            "\nBelum ter-enrol.\n\n\
             Terbitkan token enrolment dari dashboard, lalu jalankan:\n  \
             rdp-agent enrol --token <TOKEN>"
        );
        return Ok(());
    }

    let konfig = identitas::muat_konfig()?;
    let kunci = identitas::muat_kunci()?;

    println!("\nIdentitas perangkat\n");
    println!("  Device ID     {}", konfig.device_id);
    println!("  UUID          {}", konfig.device_uuid);
    println!("  Server        {}", konfig.server);
    println!("  Signaling     {}", konfig.ws_url());
    println!("  Kunci publik  {}", kunci.public_key_base64());
    println!("  Berkas        {}", identitas::direktori()?.display());

    Ok(())
}

// ── utilitas ─────────────────────────────────────────────────────────────────

/// Mengambil nilai sebuah opsi `--nama nilai`.
fn opsi(argumen: &[String], nama: &str) -> Option<String> {
    let i = argumen.iter().position(|a| a == nama)?;
    argumen.get(i + 1).filter(|v| !v.starts_with("--")).cloned()
}

fn potong(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter =
        EnvFilter::try_from_env("AETHERDESK_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().without_time())
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potong_menjaga_batas_lebar() {
        assert_eq!(potong("pendek", 22), "pendek");
        assert_eq!(potong(&"x".repeat(30), 10).chars().count(), 10);
    }

    #[test]
    fn potong_aman_untuk_karakter_multibyte() {
        // Nama perangkat Windows dapat memuat karakter non-ASCII; memotong
        // per byte akan menghasilkan string tidak valid.
        let s = "モニターディスプレイ装置";
        assert!(potong(s, 5).chars().count() <= 5);
    }

    fn arg(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn opsi_terbaca() {
        let a = arg(&["--token", "abc", "--alias", "PC Kantor"]);
        assert_eq!(opsi(&a, "--token").as_deref(), Some("abc"));
        assert_eq!(opsi(&a, "--alias").as_deref(), Some("PC Kantor"));
        assert_eq!(opsi(&a, "--server"), None);
    }

    #[test]
    fn opsi_tanpa_nilai_tidak_menelan_opsi_berikutnya() {
        // `--token --alias X` berarti token-nya lupa diisi. Mengembalikan
        // "--alias" sebagai token akan menghasilkan penolakan server yang
        // membingungkan, jauh dari sebab sebenarnya.
        let a = arg(&["--token", "--alias", "X"]);
        assert_eq!(opsi(&a, "--token"), None);
    }

    #[test]
    fn opsi_di_ujung_tanpa_nilai() {
        assert_eq!(opsi(&arg(&["--token"]), "--token"), None);
    }
}
