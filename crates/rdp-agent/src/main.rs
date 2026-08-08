//! Agent native AetherDesk.
//!
//! Keadaan sekarang: **M1 sebagian** — enumerasi monitor.
//!
//! Modul ini sengaja dikerjakan lebih dulu karena dapat dibuktikan tanpa
//! capture, tanpa encode, dan tanpa jaringan. Menjalankan `rdp-agent monitors`
//! di mesin bermonitor tiga, lalu melihat ketiganya terdaftar dengan koordinat
//! yang benar — termasuk koordinat negatif untuk monitor di sebelah kiri —
//! membuktikan fondasi multi-monitornya sehat sebelum satu piksel pun ditangkap.
//!
//! Peta jalan lengkap ada di `docs/NEXT_PLAN.md`.

mod monitor;

use anyhow::Result;

fn main() -> Result<()> {
    init_tracing();

    let perintah = std::env::args().nth(1).unwrap_or_else(|| "monitors".into());

    match perintah.as_str() {
        "monitors" => cetak_monitor(),
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

fn bantuan() {
    println!(
        "\
rdp-agent — agent native AetherDesk

PERINTAH
  monitors    Menyebutkan seluruh monitor beserta koordinat virtual desktop
  help        Menampilkan bantuan ini

BELUM TERSEDIA
  capture     Capture layar (M2)
  connect     Registrasi dan signaling (M1, sisanya)
  input       Injeksi mouse dan papan ketik (M4)

Lihat docs/NEXT_PLAN.md untuk urutan pengerjaannya."
    );
}

fn cetak_monitor() -> Result<()> {
    let monitors = monitor::enumerasi()?;

    println!("\n{} monitor terdeteksi\n", monitors.len());
    println!(
        "{:<4} {:<22} {:>7} {:>7} {:>7} {:>7}  {}",
        "ID", "NAMA", "X", "Y", "LEBAR", "TINGGI", "PRIMER"
    );
    println!("{}", "─".repeat(74));

    for m in &monitors {
        println!(
            "{:<4} {:<22} {:>7} {:>7} {:>7} {:>7}  {}",
            m.id,
            potong(&m.name, 22),
            m.x,
            m.y,
            m.width,
            m.height,
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
}
