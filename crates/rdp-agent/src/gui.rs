//! Jendela aplikasi.
//!
//! Yang mengubah agent dari perkakas terminal menjadi sesuatu yang dapat
//! dipasang di komputer orang lain: nomor dan kata sandinya terpampang, dapat
//! disalin, dan dapat diganti tanpa menyentuh baris perintah.
//!
//! ## Dua dunia yang harus tetap terpisah
//!
//! Agent hidup di runtime async; jendela hidup di event loop-nya sendiri, di
//! thread utama, dan tidak boleh diblokir sedetik pun. Keduanya hanya
//! berbicara lewat channel dan satu mutex bersama — tidak ada `await` di dalam
//! `update`, dan tidak ada penggambaran di dalam task async.
//!
//! ## Kotak persetujuan tinggal di sini
//!
//! Sejak M5b, persetujuan memakai `MessageBoxW` yang tombolnya tidak dapat
//! diberi label sendiri — "Ya/Tidak/Batal" harus dijelaskan lewat isi pesan.
//! Di dalam jendela, ketiga pilihan itu akhirnya dapat berbunyi seperti apa
//! yang sebenarnya mereka lakukan.

use crate::{
    api::{Diri, Klien},
    dipercaya::Daftar,
    identitas::Konfigurasi,
    persetujuan::{Keputusan, Permintaan},
};
use eframe::egui;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// Token warna, disalin dari sistem desain web supaya jendela dan halaman
// terbaca sebagai satu produk. Bukan tema gelap bawaan egui.
const VOID: egui::Color32 = egui::Color32::from_rgb(0x07, 0x08, 0x0D);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(0x10, 0x13, 0x1C);
const SURFACE_2: egui::Color32 = egui::Color32::from_rgb(0x16, 0x1A, 0x26);
const LINE: egui::Color32 = egui::Color32::from_rgb(0x1E, 0x23, 0x31);
const LINE_2: egui::Color32 = egui::Color32::from_rgb(0x2B, 0x31, 0x45);
const INK: egui::Color32 = egui::Color32::from_rgb(0xEE, 0xF1, 0xF8);
const INK_2: egui::Color32 = egui::Color32::from_rgb(0x9A, 0xA3, 0xBD);
const INK_3: egui::Color32 = egui::Color32::from_rgb(0x62, 0x6C, 0x88);
const SIGNAL_B: egui::Color32 = egui::Color32::from_rgb(0x4C, 0xC9, 0xF0);
const OK: egui::Color32 = egui::Color32::from_rgb(0x3D, 0xDC, 0x97);
const WARN: egui::Color32 = egui::Color32::from_rgb(0xF4, 0xA2, 0x61);
const BAD: egui::Color32 = egui::Color32::from_rgb(0xFF, 0x6B, 0x6B);

const RUANG_BAGIAN: f32 = 16.0;

/// Menerapkan token warna dan ruang ke seluruh jendela.
pub fn atur_gaya(ctx: &egui::Context) {
    let mut gaya = (*ctx.style()).clone();

    let v = &mut gaya.visuals;
    v.dark_mode = true;
    v.panel_fill = VOID;
    v.window_fill = VOID;
    v.extreme_bg_color = VOID;
    v.faint_bg_color = SURFACE;
    v.override_text_color = Some(INK_2);
    v.selection.bg_fill = SIGNAL_B.linear_multiply(0.35);
    v.hyperlink_color = SIGNAL_B;

    // Sudut 4px, bukan membulat besar — sama seperti kartu di web.
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = egui::Rounding::same(4.0);
    }

    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LINE);
    v.widgets.inactive.bg_fill = SURFACE_2;
    v.widgets.inactive.weak_bg_fill = SURFACE_2;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, LINE_2);
    v.widgets.hovered.bg_fill = SURFACE_2;
    v.widgets.hovered.weak_bg_fill = SURFACE_2;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, SIGNAL_B);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, SIGNAL_B);

    gaya.spacing.item_spacing = egui::vec2(8.0, 6.0);
    gaya.spacing.button_padding = egui::vec2(10.0, 6.0);
    gaya.spacing.interact_size.y = 28.0;

    ctx.set_style(gaya);
}

/// Membuat ikon baki sistem: satu titik sumber dengan muka gelombang yang
/// memancar — tanda yang sama dengan yang dipakai di web.
///
/// Digambar dalam kode alih-alih dimuat dari berkas `.ico`. Ikon yang hidup
/// sebagai berkas terpisah adalah satu hal lagi yang dapat hilang saat program
/// disalin ke mesin lain, dan aplikasi ini memang dirancang untuk disalin.
#[cfg(windows)]
fn ikon_baki() -> Option<tray_icon::Icon> {
    const N: i32 = 32;
    let mut piksel = vec![0u8; (N * N * 4) as usize];

    // Titik sumber di pojok kiri-bawah, tiga busur sepusat yang meredup.
    let sumber = (6.0_f32, 26.0_f32);
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = (x as f32 - sumber.0, y as f32 - sumber.1);
            let jarak = (dx * dx + dy * dy).sqrt();

            // Hanya kuadran kanan-atas: gelombang merambat menjauhi sudutnya.
            let (nyala, alfa) = if jarak < 2.6 {
                (true, 1.0)
            } else if dx >= -1.0 && dy <= 1.0 {
                let dekat = [9.0_f32, 16.0, 23.0]
                    .iter()
                    .map(|r| ((jarak - r).abs(), *r))
                    .fold((f32::MAX, 0.0), |a, b| if b.0 < a.0 { b } else { a });
                // Busur setebal ~1,6 piksel, memudar makin jauh dari sumber.
                (dekat.0 < 1.6, (1.0 - dekat.1 / 30.0).clamp(0.25, 1.0))
            } else {
                (false, 0.0)
            };

            if nyala {
                // Gradien sinyal: ungu di dekat sumber, kuning di ujung.
                let t = (jarak / 26.0).clamp(0.0, 1.0);
                let (r, g, b) = if t < 0.5 {
                    let u = t * 2.0;
                    (
                        0x8b as f32 + (0x4c as f32 - 0x8b as f32) * u,
                        0x7b as f32 + (0xc9 as f32 - 0x7b as f32) * u,
                        0xf7 as f32 + (0xf0 as f32 - 0xf7 as f32) * u,
                    )
                } else {
                    let u = (t - 0.5) * 2.0;
                    (
                        0x4c as f32 + (0xf4 as f32 - 0x4c as f32) * u,
                        0xc9 as f32 + (0xa2 as f32 - 0xc9 as f32) * u,
                        0xf0 as f32 + (0x61 as f32 - 0xf0 as f32) * u,
                    )
                };
                let i = ((y * N + x) * 4) as usize;
                piksel[i] = r as u8;
                piksel[i + 1] = g as u8;
                piksel[i + 2] = b as u8;
                piksel[i + 3] = (alfa * 255.0) as u8;
            }
        }
    }

    tray_icon::Icon::from_rgba(piksel, N as u32, N as u32).ok()
}

/// Tombol mata untuk menampilkan atau menyembunyikan kata sandi.
///
/// Digambar, bukan diambil dari font. Emoji tidak selalu tersedia pada font
/// bawaan, dan ikon yang kadang berubah menjadi kotak kosong lebih buruk
/// daripada tidak ada ikon sama sekali.
fn tombol_mata(ui: &mut egui::Ui, terlihat: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 28.0), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let warna = if resp.hovered() { INK } else { INK_3 };
        let garis = egui::Stroke::new(1.3, warna);
        let p = ui.painter();
        let c = rect.center();
        let (w, h) = (8.5, 4.8);

        // Kelopak atas dan bawah, masing-masing satu busur parabola.
        let busur = |arah: f32| -> Vec<egui::Pos2> {
            (0..=14)
                .map(|i| {
                    let t = i as f32 / 14.0;
                    let x = -w + 2.0 * w * t;
                    let y = arah * h * (1.0 - (x / w).powi(2));
                    c + egui::vec2(x, y)
                })
                .collect()
        };
        p.add(egui::Shape::line(busur(-1.0), garis));
        p.add(egui::Shape::line(busur(1.0), garis));
        p.circle_stroke(c, 2.1, garis);

        if !terlihat {
            p.line_segment(
                [c + egui::vec2(-8.5, 6.0), c + egui::vec2(8.5, -6.0)],
                garis,
            );
        }
    }

    resp.on_hover_text(if terlihat { "Sembunyikan" } else { "Tampilkan" })
}

/// Keadaan yang dibagi antara agent dan jendela.
#[derive(Debug, Default)]
pub struct Bersama {
    pub tersambung: bool,
    pub sesi_aktif: Option<String>,
    pub pesan: Option<String>,
    /// Terisi setelah panggilan pertama ke server berhasil.
    pub diri: Option<Diri>,
    /// Hanya terisi tepat setelah dirotasi — kata sandi sesi tidak pernah
    /// dapat dibaca ulang dari server.
    pub sandi_sesi_baru: Option<String>,
}

pub struct Aplikasi {
    bersama: Arc<Mutex<Bersama>>,
    daftar: Arc<Mutex<Daftar>>,
    /// Permintaan persetujuan yang sedang menunggu jawaban.
    menunggu: Option<Permintaan>,
    terima_izin: tokio::sync::mpsc::UnboundedReceiver<Permintaan>,
    perintah: tokio::sync::mpsc::UnboundedSender<Perintah>,
    konfig: Konfigurasi,

    /// Ikon baki. Dipegang hanya agar tetap hidup — melepasnya menghapus
    /// ikonnya dari baki sistem.
    #[cfg(windows)]
    _baki: Option<tray_icon::TrayIcon>,
    /// Jendela tersembunyi ke baki, bukan tertutup.
    tersembunyi: bool,
    /// Benar-benar keluar, bukan sekadar menyembunyikan.
    keluar: bool,

    alias_diketik: String,
    sandi_tetap_diketik: String,
    /// Kata sandi tetap terlihat atau tersamar.
    ///
    /// Kata sandi yang mesin yang pilih dapat disembunyikan tanpa biaya — ia
    /// akan disalin, bukan diketik ulang. Yang **diketik manusia** justru
    /// harus dapat dilihat: mengetik sepuluh karakter tanpa umpan balik lalu
    /// mendapati diri terkunci adalah kegagalan yang sepenuhnya dapat dihindari.
    tetap_terlihat: bool,
    sesi_terlihat: bool,
    galat: Option<String>,
}

/// Pekerjaan yang harus dijalankan di runtime async atas permintaan jendela.
#[derive(Debug)]
pub enum Perintah {
    MuatDiri,
    SetAlias(String),
    RotasiSandiSesi,
    SetSandiTetap(String),
    HapusSandiTetap,
}

impl Aplikasi {
    pub fn baru(
        bersama: Arc<Mutex<Bersama>>,
        daftar: Arc<Mutex<Daftar>>,
        terima_izin: tokio::sync::mpsc::UnboundedReceiver<Permintaan>,
        perintah: tokio::sync::mpsc::UnboundedSender<Perintah>,
        konfig: Konfigurasi,
    ) -> Self {
        let _ = perintah.send(Perintah::MuatDiri);
        Self {
            bersama,
            daftar,
            menunggu: None,
            terima_izin,
            perintah,
            konfig,
            #[cfg(windows)]
            _baki: ikon_baki().and_then(|i| {
                tray_icon::TrayIconBuilder::new()
                    .with_icon(i)
                    .with_tooltip("AetherDesk")
                    .build()
                    .ok()
            }),
            tersembunyi: false,
            keluar: false,

            alias_diketik: String::new(),
            sandi_tetap_diketik: String::new(),
            tetap_terlihat: false,
            sesi_terlihat: false,
            galat: None,
        }
    }

    fn salin(&mut self, teks: &str) {
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(teks.to_string())) {
            Ok(()) => self.galat = None,
            Err(e) => self.galat = Some(format!("gagal menyalin: {e}")),
        }
    }
}

impl eframe::App for Aplikasi {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        // Jendela harus tetap hidup meski tidak ada peristiwa masukan: status
        // koneksi dan permintaan persetujuan datang dari thread lain. Saat
        // tersembunyi, denyutnya diperlambat — tidak ada yang melihatnya, dan
        // yang perlu dijaga hanyalah kemampuan menerima klik dari baki.
        ctx.request_repaint_after(std::time::Duration::from_millis(
            if self.tersembunyi { 250 } else { 400 },
        ));

        // ── Baki sistem ─────────────────────────────────────────────────────
        #[cfg(windows)]
        while let Ok(peristiwa) = tray_icon::TrayIconEvent::receiver().try_recv() {
            if matches!(
                peristiwa,
                tray_icon::TrayIconEvent::Click { .. } | tray_icon::TrayIconEvent::DoubleClick { .. }
            ) {
                self.tersembunyi = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }

        // Menutup jendela menyembunyikannya, bukan mematikan agent. Aplikasi
        // remote desktop yang berhenti ketika jendelanya ditutup tidak dapat
        // diandalkan untuk hal yang justru menjadi tugasnya — dan pengguna
        // menutup jendela karena selesai membacanya, bukan karena ingin
        // memutus akses.
        if ctx.input(|i| i.viewport().close_requested()) && !self.keluar {
            self.tersembunyi = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        if self.menunggu.is_none() {
            if let Ok(p) = self.terima_izin.try_recv() {
                // Permintaan yang muncul saat jendela terminimalkan tidak akan
                // pernah terlihat. Memintanya tampil di depan adalah bagian
                // dari persetujuan itu sendiri, bukan kenyamanan.
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                self.menunggu = Some(p);
            }
        }

        if self.menunggu.is_some() {
            self.gambar_persetujuan(ctx);
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.gambar_utama(ui));
        });
    }
}

impl Aplikasi {
    fn gambar_persetujuan(&mut self, ctx: &egui::Context) {
        let p = self.menunggu.as_ref().map(|x| x.peminta.clone());
        let Some(peminta) = p else { return };

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("Permintaan akses jarak jauh");
            ui.add_space(10.0);

            egui::Grid::new("peminta").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Email");
                ui.strong(&peminta.email);
                ui.end_row();
                ui.label("Dari");
                ui.label(if peminta.ip.is_empty() { "tidak diketahui" } else { &peminta.ip });
                ui.end_row();
                ui.label("Waktu");
                ui.label(chrono::Local::now().format("%H:%M:%S").to_string());
                ui.end_row();
            });

            ui.add_space(10.0);
            ui.colored_label(
                egui::Color32::from_rgb(220, 160, 60),
                "Bila diizinkan, orang ini dapat melihat layar mesin ini — dan \
                 menggerakkan mouse serta mengetik bila kendali diaktifkan.",
            );
            ui.add_space(4.0);
            ui.small("Bila Anda tidak sedang meminta bantuan siapa pun, tolak.");
            ui.add_space(16.0);

            // Tombol menolak diletakkan lebih dulu dan diberi fokus, mengikuti
            // QUICK_CONNECT.md §4.1: menekan Enter tanpa membaca berarti
            // menolak, bukan mengizinkan.
            ui.horizontal(|ui| {
                let tolak = ui.button("  Tolak  ");
                if tolak.clicked() {
                    self.jawab(Keputusan::Tolak);
                }
                tolak.request_focus();

                if ui.button("Izinkan sekali").clicked() {
                    self.jawab(Keputusan::IzinkanSekali);
                }
                if ui.button("Izinkan dan ingat").clicked() {
                    self.jawab(Keputusan::IzinkanSelalu);
                }
            });
        });
    }

    fn jawab(&mut self, k: Keputusan) {
        if let Some(p) = self.menunggu.take() {
            let _ = p.jawab.send(k);
        }
    }

    /// Judul bagian: huruf kecil, renggang, redup — sama seperti di web.
    fn label_bagian(ui: &mut egui::Ui, teks: &str) {
        ui.add_space(RUANG_BAGIAN);
        ui.label(
            egui::RichText::new(teks.to_uppercase())
                .size(10.0)
                .monospace()
                .color(INK_3),
        );
        ui.add_space(4.0);
    }

    fn keterangan(ui: &mut egui::Ui, teks: &str) {
        ui.add_space(3.0);
        ui.label(egui::RichText::new(teks).size(11.0).color(INK_3));
    }

    fn gambar_utama(&mut self, ui: &mut egui::Ui) {
        let (tersambung, sesi, diri, sandi_baru) = {
            let b = self.bersama.lock().ok();
            match b {
                Some(g) => (
                    g.tersambung,
                    g.sesi_aktif.clone(),
                    g.diri.as_ref().map(|d| {
                        (
                            d.device_id_tampil.clone(),
                            d.handle.clone(),
                            d.org_name.clone(),
                            d.punya_sandi_tetap,
                        )
                    }),
                    g.sandi_sesi_baru.clone(),
                ),
                None => (false, None, None, None),
            }
        };

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("AetherDesk")
                    .size(19.0)
                    .monospace()
                    .strong()
                    .color(INK),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Warna saja tidak cukup: bentuk titiknya ikut membedakan,
                // sehingga terbaca oleh mata yang tidak membedakan warna.
                let (warna, teks) = match (&sesi, tersambung) {
                    (Some(_), _) => (WARN, "● SEDANG DILIHAT"),
                    (None, true) => (OK, "● SIAP"),
                    (None, false) => (BAD, "○ TERPUTUS"),
                };
                ui.label(egui::RichText::new(teks).size(10.0).monospace().color(warna));
            });
        });
        if let Some(e) = &sesi {
            ui.label(egui::RichText::new(format!("Dilihat oleh {e}")).size(11.0).color(WARN));
        }
        ui.add_space(8.0);
        ui.separator();

        let Some((nomor, handle, org, punya_tetap)) = diri else {
            ui.add_space(RUANG_BAGIAN);
            ui.label("Memuat identitas dari server…");
            if let Some(g) = &self.galat {
                ui.colored_label(BAD, g);
            }
            return;
        };

        // ── Nomor perangkat ─────────────────────────────────────────────────
        Self::label_bagian(ui, "Nomor perangkat");
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&nomor)
                    .size(30.0)
                    .monospace()
                    .strong()
                    .color(INK),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Salin").clicked() {
                    let bersih: String = nomor.chars().filter(|c| c.is_ascii_digit()).collect();
                    self.salin(&bersih);
                }
            });
        });
        Self::keterangan(ui, &format!("Organisasi {org}"));

        // ── Alias ───────────────────────────────────────────────────────────
        Self::label_bagian(ui, "Alias");
        if self.alias_diketik.is_empty() {
            if let Some(h) = &handle {
                self.alias_diketik = h.clone();
            }
        }
        ui.horizontal(|ui| {
            let sisa = ui.available_width() - 78.0;
            ui.add_sized(
                [sisa.max(120.0), 28.0],
                egui::TextEdit::singleline(&mut self.alias_diketik)
                    .hint_text("mis. pc-kantor"),
            );
            if ui.button("Simpan").clicked() {
                let _ = self
                    .perintah
                    .send(Perintah::SetAlias(self.alias_diketik.trim().to_string()));
            }
        });
        Self::keterangan(ui, "Huruf kecil, angka, tanda hubung. Menggantikan nomor.");

        // ── Kata sandi sesi ─────────────────────────────────────────────────
        Self::label_bagian(ui, "Kata sandi sesi");
        ui.horizontal(|ui| {
            match &sandi_baru {
                Some(p) => {
                    let tampil = if self.sesi_terlihat {
                        p.clone()
                    } else {
                        "•".repeat(p.chars().count())
                    };
                    ui.label(
                        egui::RichText::new(tampil)
                            .size(21.0)
                            .monospace()
                            .strong()
                            .color(INK),
                    );
                    if tombol_mata(ui, self.sesi_terlihat).clicked() {
                        self.sesi_terlihat = !self.sesi_terlihat;
                    }
                    let p = p.clone();
                    if ui.button("Salin").clicked() {
                        self.salin(&p);
                    }
                }
                None => {
                    ui.label(
                        egui::RichText::new("— belum dibangkitkan")
                            .size(13.0)
                            .color(INK_3),
                    );
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Ganti").clicked() {
                    self.sesi_terlihat = true;
                    let _ = self.perintah.send(Perintah::RotasiSandiSesi);
                }
            });
        });
        Self::keterangan(ui, "Berotasi setiap diganti, dan hanya terlihat sekali.");

        // ── Kata sandi tetap ────────────────────────────────────────────────
        Self::label_bagian(ui, "Kata sandi tetap");
        ui.horizontal(|ui| {
            let sisa = ui.available_width() - (if punya_tetap { 152.0 } else { 108.0 });
            ui.add_sized(
                [sisa.max(110.0), 28.0],
                egui::TextEdit::singleline(&mut self.sandi_tetap_diketik)
                    // Yang diketik manusia harus dapat dilihat. Mengetik
                    // sepuluh karakter tanpa umpan balik lalu mendapati diri
                    // terkunci adalah kegagalan yang dapat dihindari.
                    .password(!self.tetap_terlihat)
                    .hint_text("minimal 10 karakter"),
            );
            if tombol_mata(ui, self.tetap_terlihat).clicked() {
                self.tetap_terlihat = !self.tetap_terlihat;
            }
            if ui.button("Pasang").clicked() {
                let s = self.sandi_tetap_diketik.clone();
                self.sandi_tetap_diketik.clear();
                self.tetap_terlihat = false;
                let _ = self.perintah.send(Perintah::SetSandiTetap(s));
            }
            if punya_tetap && ui.button("Hapus").clicked() {
                let _ = self.perintah.send(Perintah::HapusSandiTetap);
            }
        });
        Self::keterangan(
            ui,
            if punya_tetap {
                "Aktif. Tidak berotasi — siapa pun yang tahu dapat masuk sampai diganti."
            } else {
                "Belum aktif. Hanya kata sandi sesi yang berlaku."
            },
        );

        // ── Daftar kepercayaan ──────────────────────────────────────────────
        ui.add_space(RUANG_BAGIAN);
        ui.separator();
        Self::label_bagian(ui, "Diizinkan mengakses mesin ini");

        let entri: Vec<(Uuid, String, String)> = self
            .daftar
            .lock()
            .map(|d| {
                d.entri
                    .iter()
                    .map(|e| {
                        (
                            e.user_id,
                            e.email.clone(),
                            e.terakhir_dipakai
                                .map(|t| t.format("%d %b %H:%M").to_string())
                                .unwrap_or_else(|| "belum pernah".into()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        if entri.is_empty() {
            Self::keterangan(ui, "Belum ada. Setiap permintaan akan ditanyakan lebih dulu.");
        } else {
            for (id, email, terakhir) in entri {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&email).size(12.0).color(INK_2));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Cabut").clicked() {
                            if let Ok(mut d) = self.daftar.lock() {
                                d.cabut(id);
                                let _ = d.simpan();
                            }
                        }
                        ui.label(egui::RichText::new(&terakhir).size(10.0).color(INK_3));
                    });
                });
            }
        }

        if let Some(g) = &self.galat {
            ui.add_space(8.0);
            ui.colored_label(BAD, egui::RichText::new(g).size(11.0));
        }
        if let Some(p) = self.bersama.lock().ok().and_then(|b| b.pesan.clone()) {
            ui.add_space(8.0);
            ui.colored_label(OK, egui::RichText::new(p).size(11.0));
        }

        // ── Kaki ────────────────────────────────────────────────────────────
        ui.add_space(RUANG_BAGIAN);
        ui.separator();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Server {}", self.konfig.server))
                    .size(10.0)
                    .color(INK_3),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Satu-satunya jalan benar-benar berhenti. Tanpa tombol ini,
                // menutup jendela hanya menyembunyikannya dan tidak ada cara
                // mematikan agent selain lewat pengelola tugas.
                if ui.button("Keluar").on_hover_text(
                    "Menghentikan agent. Perangkat menjadi offline dan tidak dapat diakses.",
                ).clicked()
                {
                    self.keluar = true;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "Menutup jendela hanya menyembunyikannya ke baki. Agent tetap berjalan.",
            )
            .size(10.0)
            .color(INK_3),
        );
    }
}

/// Menjalankan perintah dari jendela di runtime async.
pub async fn layani_perintah(
    klien: Klien,
    konfig: Konfigurasi,
    kunci: rdp_core::DeviceKeypair,
    bersama: Arc<Mutex<Bersama>>,
    mut terima: tokio::sync::mpsc::UnboundedReceiver<Perintah>,
) {
    while let Some(p) = terima.recv().await {
        // Token diambil segar setiap perintah. Menyimpannya berarti menangani
        // kedaluwarsa di sini juga, untuk operasi yang toh jarang dilakukan.
        let token = match klien.token_perangkat(konfig.device_uuid, &kunci).await {
            Ok(t) => t.access_token,
            Err(e) => {
                lapor(&bersama, format!("gagal memperoleh token: {e}"));
                continue;
            }
        };

        let hasil: anyhow::Result<String> = match p {
            Perintah::MuatDiri => klien.diri(&token).await.map(|d| {
                if let Ok(mut b) = bersama.lock() {
                    b.diri = Some(d);
                }
                String::new()
            }),
            Perintah::SetAlias(a) => {
                let kosong = a.is_empty();
                klien
                    .set_alias(&token, (!kosong).then_some(a.as_str()))
                    .await
                    .map(|_| {
                        if kosong { "alias dihapus".into() } else { format!("alias: {a}") }
                    })
            }
            Perintah::RotasiSandiSesi => klien.set_sandi(&token, true, None).await.map(|r| {
                if let Ok(mut b) = bersama.lock() {
                    b.sandi_sesi_baru = r.session_password.clone();
                }
                "kata sandi sesi diganti".into()
            }),
            Perintah::SetSandiTetap(s) => klien
                .set_sandi(&token, false, Some(&s))
                .await
                .map(|_| "kata sandi tetap dipasang".into()),
            Perintah::HapusSandiTetap => klien
                .set_sandi(&token, false, Some(""))
                .await
                .map(|_| "kata sandi tetap dihapus".into()),
        };

        match hasil {
            Ok(pesan) => {
                if !pesan.is_empty() {
                    lapor(&bersama, pesan);
                }
                // Apa pun yang berubah, ringkasan disegarkan supaya jendela
                // tidak pernah menampilkan keadaan yang sudah usang.
                if let Ok(d) = klien.diri(&token).await {
                    if let Ok(mut b) = bersama.lock() {
                        b.diri = Some(d);
                    }
                }
            }
            Err(e) => lapor(&bersama, format!("{e}")),
        }
    }
}

fn lapor(bersama: &Arc<Mutex<Bersama>>, pesan: String) {
    if let Ok(mut b) = bersama.lock() {
        b.pesan = Some(pesan);
    }
}
