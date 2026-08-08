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
mod identitas;
mod monitor;
mod signal;

use anyhow::{bail, Result};
use rdp_core::DeviceKeypair;

fn main() -> Result<()> {
    init_tracing();

    let argumen: Vec<String> = std::env::args().skip(1).collect();
    let perintah = argumen.first().map(String::as_str).unwrap_or("help");

    match perintah {
        "monitors" => cetak_monitor(),
        "enrol" | "enroll" => jalankan_async(perintah_enrol(&argumen[1..])),
        "connect" => jalankan_async(perintah_connect()),
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
  enrol --token <TOKEN>  Mendaftarkan mesin ini memakai token enrolment
  connect                Menyambung ke server dan tetap online
  status                 Menampilkan identitas perangkat yang tersimpan
  help                   Menampilkan bantuan ini

OPSI enrol
  --token <TOKEN>        Token enrolment dari dashboard (wajib)
  --server <URL>         Alamat server (baku: {baku})
  --alias <NAMA>         Nama yang ditampilkan di dashboard

LINGKUNGAN
  AETHERDESK_DIR         Direktori identitas, menimpa lokasi baku
  AETHERDESK_LOG         Filter log, mis. debug

BELUM TERSEDIA
  capture     Capture layar (M2)
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

async fn perintah_connect() -> Result<()> {
    let konfig = identitas::muat_konfig()?;
    let kunci = identitas::muat_kunci()?;

    tracing::info!(
        device_id = %konfig.device_id,
        server = %konfig.server,
        "agent mulai"
    );

    // Enumerasi sekali di awal. Belum dikirim ke mana pun — MONITOR_LAYOUT
    // baru ada di M3 — tetapi kegagalannya di sini jauh lebih mudah didiagnosis
    // daripada nanti di tengah sesi.
    match monitor::enumerasi() {
        Ok(m) => tracing::info!(jumlah = m.len(), "monitor terdeteksi"),
        Err(e) => tracing::warn!(error = %e, "enumerasi monitor gagal"),
    }

    signal::jalankan(konfig, kunci).await
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
