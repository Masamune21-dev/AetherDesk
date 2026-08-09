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

    alias_diketik: String,
    sandi_tetap_diketik: String,
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
            alias_diketik: String::new(),
            sandi_tetap_diketik: String::new(),
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
        // koneksi dan permintaan persetujuan datang dari thread lain.
        ctx.request_repaint_after(std::time::Duration::from_millis(400));

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

        egui::CentralPanel::default().show(ctx, |ui| self.gambar_utama(ui));
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

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("AetherDesk");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (warna, teks) = match (&sesi, tersambung) {
                    (Some(_), _) => (egui::Color32::from_rgb(90, 190, 120), "sedang dilihat"),
                    (None, true) => (egui::Color32::from_rgb(90, 160, 220), "siap"),
                    (None, false) => (egui::Color32::from_rgb(200, 100, 100), "terputus"),
                };
                ui.colored_label(warna, format!("● {teks}"));
            });
        });
        ui.separator();
        ui.add_space(6.0);

        let Some((nomor, handle, org, punya_tetap)) = diri else {
            ui.label("Memuat identitas dari server…");
            if let Some(g) = &self.galat {
                ui.colored_label(egui::Color32::from_rgb(220, 120, 120), g);
            }
            return;
        };

        // ── Identitas ───────────────────────────────────────────────────────
        ui.label(egui::RichText::new("Nomor perangkat").small());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&nomor).size(26.0).monospace().strong());
            if ui.button("Salin").clicked() {
                let bersih: String = nomor.chars().filter(|c| c.is_ascii_digit()).collect();
                self.salin(&bersih);
            }
        });
        ui.small(format!("Organisasi {org}"));
        ui.add_space(10.0);

        // ── Alias ───────────────────────────────────────────────────────────
        ui.label(egui::RichText::new("Alias").small());
        ui.horizontal(|ui| {
            if self.alias_diketik.is_empty() {
                if let Some(h) = &handle {
                    self.alias_diketik = h.clone();
                }
            }
            ui.text_edit_singleline(&mut self.alias_diketik);
            if ui.button("Simpan").clicked() {
                let _ = self
                    .perintah
                    .send(Perintah::SetAlias(self.alias_diketik.trim().to_string()));
            }
        });
        ui.small("Huruf kecil, angka, tanda hubung. Dapat dipakai menggantikan nomor.");
        ui.add_space(10.0);

        // ── Kata sandi ──────────────────────────────────────────────────────
        ui.label(egui::RichText::new("Kata sandi sesi").small());
        ui.horizontal(|ui| {
            match &sandi_baru {
                Some(p) => {
                    ui.label(egui::RichText::new(p).size(20.0).monospace().strong());
                    let p = p.clone();
                    if ui.button("Salin").clicked() {
                        self.salin(&p);
                    }
                }
                None => {
                    ui.label(egui::RichText::new("••••••••").size(20.0).monospace());
                }
            }
            if ui.button("Ganti").clicked() {
                let _ = self.perintah.send(Perintah::RotasiSandiSesi);
            }
        });
        ui.small("Berotasi setiap kali diganti; hanya terlihat sekali.");
        ui.add_space(10.0);

        ui.label(egui::RichText::new("Kata sandi tetap").small());
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.sandi_tetap_diketik)
                    .password(true)
                    .hint_text("minimal 10 karakter"),
            );
            if ui.button("Pasang").clicked() {
                let s = self.sandi_tetap_diketik.clone();
                self.sandi_tetap_diketik.clear();
                let _ = self.perintah.send(Perintah::SetSandiTetap(s));
            }
            if punya_tetap && ui.button("Hapus").clicked() {
                let _ = self.perintah.send(Perintah::HapusSandiTetap);
            }
        });
        ui.small(if punya_tetap {
            "Aktif. Tidak berotasi — siapa pun yang tahu dapat masuk sampai diganti."
        } else {
            "Belum aktif. Hanya kata sandi sesi yang berlaku."
        });

        // ── Daftar kepercayaan ──────────────────────────────────────────────
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Diizinkan mengakses mesin ini").small());

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
            ui.small("Belum ada. Setiap permintaan akan ditanyakan lebih dulu.");
        } else {
            for (id, email, terakhir) in entri {
                ui.horizontal(|ui| {
                    ui.label(&email);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Cabut").clicked() {
                            if let Ok(mut d) = self.daftar.lock() {
                                d.cabut(id);
                                let _ = d.simpan();
                            }
                        }
                        ui.small(&terakhir);
                    });
                });
            }
        }

        if let Some(g) = &self.galat {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::from_rgb(220, 120, 120), g);
        }
        if let Some(p) = self.bersama.lock().ok().and_then(|b| b.pesan.clone()) {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::from_rgb(150, 190, 150), p);
        }

        ui.add_space(10.0);
        ui.small(format!("Server {}", self.konfig.server));
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
