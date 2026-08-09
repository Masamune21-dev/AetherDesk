//! Capture layar.
//!
//! M2, langkah pertama. Sengaja dipisah dari encoder dan dari jaringan: sebuah
//! frame yang dapat dibuka dan dilihat membuktikan seluruh jalur capture sehat
//! tanpa satu pun variabel dari H.264 maupun WebRTC ikut bermain. Ketika nanti
//! gambar di viewer tampak rusak, langkah ini yang menentukan apakah sebabnya
//! ada di sini atau di hilir.
//!
//! ## Kenapa DXGI Desktop Duplication
//!
//! STREAMING.md §2.1 menempatkan DXGI untuk berbagi seluruh desktop dan WGC
//! untuk berbagi satu jendela. M2 mengerjakan yang pertama.
//!
//! ## Yang tidak dapat ditangkapnya
//!
//! Desktop Duplication berjalan di dalam sesi pengguna, sehingga **tidak** dapat
//! melihat secure desktop — prompt UAC, Ctrl+Alt+Del, dan layar masuk. Layar
//! akan tampak membeku di detik-detik itu. Bukan bug: itu batas keamanan
//! Windows, dan ADR-010 sudah merancang jalan keluarnya lewat pemisahan service
//! LocalSystem dan session agent.

/// Satu frame BGRA yang sudah berada di memori sistem.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// BGRA8, rapat — `width * 4` byte per baris, tanpa padding.
    pub data: Vec<u8>,
    /// Berapa frame desktop yang terakumulasi sejak pengambilan sebelumnya.
    ///
    /// Berguna untuk mendeteksi encoder yang tertinggal — nilai yang terus
    /// naik berarti frame dibuang sebelum sempat dikodekan. Belum ada yang
    /// membacanya.
    #[cfg_attr(not(test), allow(dead_code))]
    pub accumulated: u32,
}

impl Frame {
    pub fn bytes(&self) -> usize {
        self.data.len()
    }
}

/// Menulis frame sebagai BMP 32-bit.
///
/// BMP dipilih justru karena primitif: tidak ada pustaka, tidak ada kompresi,
/// tidak ada ruang untuk salah tafsir. Kalau berkasnya terbuka dan gambarnya
/// benar, maka byte yang keluar dari capture memang benar — dan itulah satu
/// hal yang perlu dibuktikan pada tahap ini.
pub fn tulis_bmp(frame: &Frame, path: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;

    let ukuran_piksel = frame.width * frame.height * 4;
    // 14 byte file header + 40 byte BITMAPINFOHEADER.
    let offset = 54u32;
    let ukuran_berkas = offset + ukuran_piksel;

    let mut keluar = Vec::with_capacity(ukuran_berkas as usize);
    keluar.extend_from_slice(b"BM");
    keluar.extend_from_slice(&ukuran_berkas.to_le_bytes());
    keluar.extend_from_slice(&0u32.to_le_bytes()); // dua field cadangan
    keluar.extend_from_slice(&offset.to_le_bytes());

    keluar.extend_from_slice(&40u32.to_le_bytes()); // ukuran header info
    keluar.extend_from_slice(&(frame.width as i32).to_le_bytes());
    // Tinggi negatif menandakan baris tersusun dari atas ke bawah. Tanpa ini
    // BMP menyimpan gambar terbalik, dan capture-nya akan terlihat gagal
    // padahal datanya benar.
    keluar.extend_from_slice(&(-(frame.height as i32)).to_le_bytes());
    keluar.extend_from_slice(&1u16.to_le_bytes()); // bidang
    keluar.extend_from_slice(&32u16.to_le_bytes()); // bit per piksel
    keluar.extend_from_slice(&0u32.to_le_bytes()); // tanpa kompresi
    keluar.extend_from_slice(&ukuran_piksel.to_le_bytes());
    keluar.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI horizontal
    keluar.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI vertikal
    keluar.extend_from_slice(&0u32.to_le_bytes()); // palet
    keluar.extend_from_slice(&0u32.to_le_bytes()); // warna penting

    keluar.extend_from_slice(&frame.data);

    let mut f = std::fs::File::create(path)?;
    f.write_all(&keluar)
}

// ── Windows ──────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use super::Frame;
    use anyhow::{bail, Context, Result};
    use windows::core::Interface;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
        D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
        D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION, DXGI_MODE_ROTATION_ROTATE180,
        DXGI_MODE_ROTATION_ROTATE270, DXGI_MODE_ROTATION_ROTATE90, DXGI_SAMPLE_DESC,
    };
    use windows::Win32::Graphics::Dxgi::{
        IDXGIAdapter, IDXGIDevice, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
        DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
    };

    /// Duplikasi satu output, beserta perangkat Direct3D yang memilikinya.
    pub struct Duplikasi {
        device: ID3D11Device,
        konteks: ID3D11DeviceContext,
        dup: IDXGIOutputDuplication,
        /// Nama perangkat GDI, mis. `\\.\DISPLAY1`. Disimpan supaya pemulihan
        /// setelah ACCESS_LOST kembali ke monitor yang sama, bukan ke monitor
        /// pertama yang kebetulan ditemukan.
        pub nama_output: String,
        /// Ukuran sebagaimana **dilihat pengguna**, mengikuti rotasi desktop.
        pub width: u32,
        pub height: u32,
        /// Ukuran permukaan yang benar-benar diserahkan DXGI.
        ///
        /// Pada monitor tegak keduanya **berbeda**: DXGI menyerahkan permukaan
        /// dalam orientasi panel aslinya — 1920×1080 — sementara desktopnya
        /// 1080×1920. Menyamakan keduanya adalah bug yang gejalanya paling
        /// menyesatkan yang saya temui di modul ini: `CopyResource` antara dua
        /// tekstur berbeda ukuran **tidak melapor gagal**, ia hanya tidak
        /// melakukan apa-apa, sehingga tekstur singgah tetap berisi nol dan
        /// hasilnya frame hitam sempurna dengan dimensi yang tampak benar.
        pub surface_width: u32,
        pub surface_height: u32,
        pub rotation: DXGI_MODE_ROTATION,
        staging: Option<ID3D11Texture2D>,
    }

    impl std::fmt::Debug for Duplikasi {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Duplikasi")
                .field("output", &self.nama_output)
                .field("width", &self.width)
                .field("height", &self.height)
                .field("rotasi", &self.rotation.0)
                .finish_non_exhaustive()
        }
    }

    /// Memutar buffer BGRA dari orientasi panel ke orientasi desktop.
    ///
    /// Monitor tegak adalah panel lanskap yang diputar, dan DXGI menyerahkan
    /// permukaannya apa adanya — tanpa diputar. Pemutaran ini yang membuat
    /// frame cocok dengan apa yang benar-benar dilihat pengguna, dan itu bukan
    /// sekadar soal enak dilihat: M4 memetakan koordinat mouse relatif terhadap
    /// **tampilan**, jadi frame yang orientasinya berbeda akan membuat setiap
    /// klik mendarat di tempat yang salah.
    ///
    /// Dikerjakan CPU untuk sekarang. Tempatnya kelak di GPU, bersama konversi
    /// ke NV12 yang dibutuhkan encoder (STREAMING.md §1).
    pub fn putar(src: &[u8], sw: usize, sh: usize, rot: DXGI_MODE_ROTATION) -> Vec<u8> {
        if rot != DXGI_MODE_ROTATION_ROTATE90
            && rot != DXGI_MODE_ROTATION_ROTATE180
            && rot != DXGI_MODE_ROTATION_ROTATE270
        {
            return src.to_vec();
        }

        let (dw, dh) = if rot == DXGI_MODE_ROTATION_ROTATE180 {
            (sw, sh)
        } else {
            (sh, sw)
        };
        let mut dst = vec![0u8; dw * dh * 4];

        for y in 0..dh {
            for x in 0..dw {
                let (sx, sy) = match rot {
                    // Desktop diputar 90° searah jarum jam terhadap panel.
                    DXGI_MODE_ROTATION_ROTATE90 => (y, sh - 1 - x),
                    DXGI_MODE_ROTATION_ROTATE180 => (sw - 1 - x, sh - 1 - y),
                    _ => (sw - 1 - y, x),
                };
                let s = (sy * sw + sx) * 4;
                let d = (y * dw + x) * 4;
                dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
            }
        }

        dst
    }

    /// Nama rotasi untuk ditampilkan.
    pub fn nama_rotasi(r: DXGI_MODE_ROTATION) -> &'static str {
        match r {
            DXGI_MODE_ROTATION_ROTATE90 => "90°",
            DXGI_MODE_ROTATION_ROTATE180 => "180°",
            DXGI_MODE_ROTATION_ROTATE270 => "270°",
            _ => "tegak lurus",
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_ROTATION_IDENTITY;

        /// Membangun gambar 2×3 tempat setiap piksel dapat dikenali dari nilainya.
        fn contoh() -> Vec<u8> {
            let mut v = Vec::new();
            for p in 0..6u8 {
                v.extend_from_slice(&[p, p, p, 255]);
            }
            v
        }

        /// Mengambil kanal biru satu piksel — penanda identitas piksel.
        fn di(buf: &[u8], w: usize, x: usize, y: usize) -> u8 {
            buf[(y * w + x) * 4]
        }

        #[test]
        fn tanpa_rotasi_menyalin_apa_adanya() {
            let src = contoh();
            let out = putar(&src, 2, 3, DXGI_MODE_ROTATION_IDENTITY);
            assert_eq!(out, src);
        }

        #[test]
        fn rotasi_180_membalik_kedua_sumbu() {
            let src = contoh();
            let out = putar(&src, 2, 3, DXGI_MODE_ROTATION_ROTATE180);
            assert_eq!(out.len(), src.len());
            // Piksel pertama menjadi piksel terakhir.
            assert_eq!(di(&out, 2, 0, 0), 5);
            assert_eq!(di(&out, 2, 1, 2), 0);
        }

        #[test]
        fn rotasi_90_menukar_dimensi() {
            // 2×3 diputar menjadi 3×2.
            let out = putar(&contoh(), 2, 3, DXGI_MODE_ROTATION_ROTATE90);
            assert_eq!(out.len(), 6 * 4);
            // Baris atas hasil berasal dari kolom kiri sumber, dibaca dari bawah.
            assert_eq!(di(&out, 3, 0, 0), 4);
            assert_eq!(di(&out, 3, 1, 0), 2);
            assert_eq!(di(&out, 3, 2, 0), 0);
        }

        #[test]
        fn rotasi_270_kebalikan_dari_90() {
            // Memutar 90° lalu 270° harus mengembalikan gambar semula.
            let src = contoh();
            let sekali = putar(&src, 2, 3, DXGI_MODE_ROTATION_ROTATE90);
            let kembali = putar(&sekali, 3, 2, DXGI_MODE_ROTATION_ROTATE270);
            assert_eq!(kembali, src, "90° lalu 270° tidak kembali ke asal");
        }

        #[test]
        fn rotasi_tidak_pernah_mengubah_jumlah_piksel() {
            for rot in [
                DXGI_MODE_ROTATION_ROTATE90,
                DXGI_MODE_ROTATION_ROTATE180,
                DXGI_MODE_ROTATION_ROTATE270,
            ] {
                let out = putar(&contoh(), 2, 3, rot);
                assert_eq!(out.len(), 6 * 4, "rotasi {} kehilangan piksel", rot.0);
            }
        }
    }

    fn buat_device() -> Result<(ID3D11Device, ID3D11DeviceContext, IDXGIAdapter)> {
        let tingkat = [D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1];
        let mut device = None;
        let mut konteks = None;

        unsafe {
            D3D11CreateDevice(
                // Adapter dibiarkan dipilih Direct3D. Pasangan ini mengikat:
                // `D3D_DRIVER_TYPE_UNKNOWN` hanya sah bila adapter diberikan
                // secara eksplisit, dan tanpa adapter tipenya wajib `HARDWARE`.
                // Melanggarnya menghasilkan E_INVALIDARG yang pesannya —
                // "The parameter is incorrect" — tidak menyebutkan parameter
                // mana yang dimaksud.
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                // Desktop Duplication menyerahkan tekstur BGRA; tanpa flag ini
                // sebagian driver menolak membuat perangkatnya.
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&tingkat),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut konteks),
            )
        }
        .context("gagal membuat perangkat Direct3D 11")?;

        let device = device.context("Direct3D tidak mengembalikan perangkat")?;
        let konteks = konteks.context("Direct3D tidak mengembalikan konteks")?;

        let dxgi: IDXGIDevice = device.cast().context("perangkat bukan IDXGIDevice")?;
        let adapter = unsafe { dxgi.GetAdapter() }.context("gagal memperoleh adapter DXGI")?;

        Ok((device, konteks, adapter))
    }

    /// Mencari output yang cocok dengan nama perangkat GDI.
    ///
    /// Mengembalikan output beserta namanya dan ukuran desktopnya, supaya
    /// pemanggil tidak perlu membaca deskripsi untuk kedua kalinya.
    fn cari_output(
        adapter: &IDXGIAdapter,
        nama: Option<&str>,
    ) -> Result<(IDXGIOutput1, String, u32, u32)> {
        let mut i = 0u32;
        let mut terlihat: Vec<String> = Vec::new();

        loop {
            // Daftar output berakhir dengan DXGI_ERROR_NOT_FOUND. Itu cara
            // normal iterasi selesai, bukan kegagalan.
            let Ok(output) = (unsafe { adapter.EnumOutputs(i) }) else {
                break;
            };
            i += 1;

            let desc = unsafe { output.GetDesc() }.context("gagal membaca deskripsi output")?;
            let nama_ini = String::from_utf16_lossy(&desc.DeviceName)
                .trim_end_matches('\0')
                .to_string();

            if !desc.AttachedToDesktop.as_bool() {
                continue;
            }

            let cocok = match nama {
                Some(n) => nama_ini == n,
                None => true,
            };

            if cocok {
                let r = desc.DesktopCoordinates;
                let o1: IDXGIOutput1 = output.cast().context("output bukan IDXGIOutput1")?;
                return Ok((
                    o1,
                    nama_ini,
                    (r.right - r.left) as u32,
                    (r.bottom - r.top) as u32,
                ));
            }

            terlihat.push(nama_ini);
        }

        match nama {
            Some(n) => bail!(
                "monitor {n} tidak ditemukan; yang terpasang: {}",
                terlihat.join(", ")
            ),
            None => bail!("tidak ada output yang terpasang ke desktop"),
        }
    }

    impl Duplikasi {
        /// Membuka duplikasi untuk sebuah monitor.
        ///
        /// `nama` adalah nama perangkat GDI seperti yang dilaporkan
        /// [`crate::monitor::enumerasi`], sehingga monitor di seluruh agent
        /// selalu dirujuk dengan sebutan yang sama.
        pub fn buka(nama: Option<&str>) -> Result<Self> {
            // Ukuran hanya benar bila proses sadar-DPI. Modul monitor sudah
            // menyatakannya lewat `Once`; memanggil ulang di sini menutup
            // kemungkinan capture dijalankan tanpa enumerasi lebih dulu.
            crate::monitor::siapkan_dpi();

            let (device, konteks, adapter) = buat_device()?;
            let (output, nama_output, width, height) = cari_output(&adapter, nama)?;

            let dup = unsafe { output.DuplicateOutput(&device) }.context(
                "gagal memulai Desktop Duplication — biasanya karena sudah ada aplikasi \
                 lain yang menduplikasi output ini, atau proses berjalan tanpa sesi desktop",
            )?;

            let desc = unsafe { dup.GetDesc() };

            Ok(Self {
                device,
                konteks,
                dup,
                nama_output,
                width,
                height,
                surface_width: desc.ModeDesc.Width,
                surface_height: desc.ModeDesc.Height,
                rotation: desc.Rotation,
                staging: None,
            })
        }

        /// Membangun ulang duplikasi setelah hilang.
        ///
        /// `DXGI_ERROR_ACCESS_LOST` bukan kejadian langka: ia muncul saat
        /// resolusi berubah, saat secure desktop mengambil alih layar, saat
        /// sesi berpindah, dan saat driver GPU di-reset. Agent yang menyerah
        /// pada kejadian pertama akan mati sendiri pada prompt UAC pertama.
        pub fn bangun_ulang(&mut self) -> Result<()> {
            let (device, konteks, adapter) = buat_device()?;
            let (output, _, width, height) = cari_output(&adapter, Some(&self.nama_output))?;

            self.dup = unsafe { output.DuplicateOutput(&device) }
                .context("gagal membangun ulang Desktop Duplication")?;

            let desc = unsafe { self.dup.GetDesc() };
            self.device = device;
            self.konteks = konteks;
            self.width = width;
            self.height = height;
            self.surface_width = desc.ModeDesc.Width;
            self.surface_height = desc.ModeDesc.Height;
            self.rotation = desc.Rotation;
            // Resolusi maupun rotasi mungkin berubah — dan berubahnya rotasi
            // justru salah satu sebab paling umum ACCESS_LOST — jadi tekstur
            // singgah lama tidak lagi sah.
            self.staging = None;

            Ok(())
        }

        /// Menyiapkan tekstur singgah seukuran tekstur sumber.
        ///
        /// Ukurannya diambil dari tekstur yang benar-benar diserahkan
        /// `AcquireNextFrame`, **bukan** dari `DXGI_OUTDUPL_DESC.ModeDesc`.
        /// Keduanya berbeda pada monitor tegak: ModeDesc melaporkan orientasi
        /// desktop, sementara teksturnya tetap dalam orientasi panel.
        ///
        /// Perbedaan itu mahal karena tidak bersuara. `CopyResource` antara dua
        /// tekstur berbeda ukuran **tidak melapor gagal** — ia hanya tidak
        /// melakukan apa-apa, sehingga tekstur singgah tetap berisi nol dan
        /// hasilnya frame hitam sempurna berdimensi yang tampak benar.
        fn staging(&mut self, w: u32, h: u32) -> Result<ID3D11Texture2D> {
            if let Some(t) = &self.staging {
                if (self.surface_width, self.surface_height) == (w, h) {
                    return Ok(t.clone());
                }
            }

            let desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut tex = None;
            unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut tex)) }
                .context("gagal membuat tekstur singgah")?;
            let tex = tex.context("Direct3D tidak mengembalikan tekstur")?;

            self.surface_width = w;
            self.surface_height = h;
            self.staging = Some(tex.clone());
            Ok(tex)
        }

        /// Mengambil satu frame.
        ///
        /// `Ok(None)` berarti tidak ada yang berubah selama `timeout_ms` — itu
        /// keadaan **normal** pada desktop yang diam, bukan galat. Desktop
        /// Duplication memang hanya menyerahkan frame ketika ada perubahan,
        /// dan sifat itulah yang membuatnya murah.
        pub fn ambil(&mut self, timeout_ms: u32) -> Result<Option<Frame>> {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut sumber: Option<IDXGIResource> = None;

            if let Err(e) = unsafe { self.dup.AcquireNextFrame(timeout_ms, &mut info, &mut sumber) }
            {
                if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                    return Ok(None);
                }
                if e.code() == DXGI_ERROR_ACCESS_LOST {
                    tracing::info!(output = %self.nama_output, "duplikasi hilang, membangun ulang");
                    self.bangun_ulang()?;
                    return Ok(None);
                }
                return Err(e).context("AcquireNextFrame gagal");
            }

            // Mulai dari sini frame wajib dilepas, apa pun yang terjadi —
            // termasuk saat penyalinan di bawah gagal. Duplikasi yang frame-nya
            // tidak pernah dilepas berhenti menyerahkan frame berikutnya, dan
            // gejalanya adalah layar yang membeku tanpa satu pun galat.
            let hasil = self.salin(&sumber, &info);
            let lepas = unsafe { self.dup.ReleaseFrame() };

            let frame = hasil?;
            lepas.context("ReleaseFrame gagal")?;
            Ok(frame)
        }

        fn salin(
            &mut self,
            sumber: &Option<IDXGIResource>,
            info: &DXGI_OUTDUPL_FRAME_INFO,
        ) -> Result<Option<Frame>> {
            // LastPresentTime nol berarti hanya kursor yang bergerak; gambar
            // desktopnya tidak berubah dan tidak perlu disalin dari VRAM.
            if info.LastPresentTime == 0 {
                return Ok(None);
            }

            let Some(sumber) = sumber else {
                return Ok(None);
            };
            let tekstur: ID3D11Texture2D =
                sumber.cast().context("sumber frame bukan ID3D11Texture2D")?;

            let mut asal = D3D11_TEXTURE2D_DESC::default();
            unsafe { tekstur.GetDesc(&mut asal) };

            let staging = self.staging(asal.Width, asal.Height)?;
            unsafe { self.konteks.CopyResource(&staging, &tekstur) };

            let mut peta = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                self.konteks
                    .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut peta))
            }
            .context("gagal memetakan tekstur singgah")?;

            let sw = self.surface_width as usize;
            let sh = self.surface_height as usize;
            let lebar_byte = sw * 4;
            let pitch = peta.RowPitch as usize;
            let mut permukaan = vec![0u8; lebar_byte * sh];

            // RowPitch hampir tidak pernah sama dengan lebar × 4: GPU
            // menyelaraskan setiap baris. Menyalin seluruh blok sekaligus
            // menghasilkan gambar yang makin miring ke bawah — bug capture
            // paling klasik, dan tampilannya cukup meyakinkan untuk membuat
            // orang mencari penyebabnya di encoder.
            unsafe {
                let dasar = peta.pData as *const u8;
                for y in 0..sh {
                    std::ptr::copy_nonoverlapping(
                        dasar.add(y * pitch),
                        permukaan.as_mut_ptr().add(y * lebar_byte),
                        lebar_byte,
                    );
                }
                self.konteks.Unmap(&staging, 0);
            }

            // Keputusan memutar diambil dari **ukuran yang benar-benar
            // diterima**, bukan semata dari klaim rotasi. Keduanya sudah
            // terbukti dapat berselisih di mesin ini, dan ukuran adalah fakta
            // sementara klaim rotasi adalah keterangan.
            let transpos = (sw, sh) == (self.height as usize, self.width as usize);
            let data = if transpos {
                putar(&permukaan, sw, sh, self.rotation)
            } else if self.rotation == DXGI_MODE_ROTATION_ROTATE180 {
                putar(&permukaan, sw, sh, self.rotation)
            } else {
                permukaan
            };

            // Kalau ukurannya tetap tidak cocok setelah semua itu, lebih baik
            // berhenti daripada menyerahkan frame yang akan ditafsirkan salah
            // oleh encoder dan viewer.
            let diharapkan = (self.width * self.height * 4) as usize;
            if data.len() != diharapkan {
                bail!(
                    "ukuran frame tidak konsisten: permukaan {}×{}, tampilan {}×{}, rotasi {}",
                    sw,
                    sh,
                    self.width,
                    self.height,
                    nama_rotasi(self.rotation)
                );
            }

            Ok(Some(Frame {
                width: self.width,
                height: self.height,
                data,
                accumulated: info.AccumulatedFrames,
            }))
        }
    }
}

#[cfg(windows)]
pub use win::{nama_rotasi, Duplikasi};

// ── Platform lain ────────────────────────────────────────────────────────────

#[cfg(not(windows))]
#[derive(Debug)]
pub struct Duplikasi;

#[cfg(not(windows))]
impl Duplikasi {
    pub fn buka(_nama: Option<&str>) -> anyhow::Result<Self> {
        // macOS memakai ScreenCaptureKit, Linux memakai PipeWire atau X11.
        // Keduanya menyusul; Windows dikerjakan lebih dulu sesuai keputusan di
        // NEXT_PLAN.md §10.
        anyhow::bail!("capture layar belum diimplementasikan pada platform ini")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_uji(w: u32, h: u32) -> Frame {
        Frame {
            width: w,
            height: h,
            data: vec![0u8; (w * h * 4) as usize],
            accumulated: 1,
        }
    }

    #[test]
    fn bmp_berukuran_tepat() {
        let f = frame_uji(4, 3);
        let p = std::env::temp_dir().join("aetherdesk_bmp_uji.bmp");
        tulis_bmp(&f, &p).unwrap();
        assert_eq!(std::fs::read(&p).unwrap().len(), 54 + 4 * 3 * 4);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn bmp_berawalan_tanda_tangan_dan_tinggi_negatif() {
        let f = frame_uji(2, 2);
        let p = std::env::temp_dir().join("aetherdesk_bmp_uji2.bmp");
        tulis_bmp(&f, &p).unwrap();
        let isi = std::fs::read(&p).unwrap();

        assert_eq!(&isi[0..2], b"BM");

        let tinggi = i32::from_le_bytes(isi[22..26].try_into().unwrap());
        // Tinggi negatif = baris atas-ke-bawah. Positif membuat gambar
        // tersimpan terbalik, dan capture-nya terlihat gagal padahal benar.
        assert_eq!(tinggi, -2, "BMP akan tersimpan terbalik");

        assert_eq!(u16::from_le_bytes(isi[28..30].try_into().unwrap()), 32);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn frame_melaporkan_ukuran_rapat() {
        // Tanpa padding: inilah kontrak yang akan diandalkan encoder di M2b.
        let f = frame_uji(1920, 1080);
        assert_eq!(f.bytes(), 1920 * 1080 * 4);
    }
}
