//! Encode H.264.
//!
//! M2, langkah kedua. Capture menghasilkan BGRA mentah pada laju ~790 MB/detik;
//! angka itu sendiri yang menjelaskan kenapa modul ini harus ada sebelum satu
//! byte pun dikirim ke jaringan.
//!
//! ## Kenapa Media Foundation
//!
//! STREAMING.md §3 menghendaki encoder perangkat keras — NVENC, QuickSync,
//! VideoToolbox. Media Foundation adalah pintu Windows menuju semuanya: encoder
//! perangkat keras dari NVIDIA, Intel, dan AMD mendaftarkan diri sebagai MFT,
//! sehingga jalur ini tidak menutup pintu ke NVENC melainkan justru merupakan
//! cara resmi mencapainya.
//!
//! Ia juga tidak menambah satu pun dependensi: seluruh API-nya sudah ada di
//! crate `windows` yang dipakai capture.
//!
//! ## Yang dipakai sekarang, dan apa yang belum
//!
//! Enumerasi saat ini meminta MFT **sinkron**, yang dalam praktiknya berarti
//! encoder perangkat lunak bawaan Windows. Encoder perangkat keras hampir
//! selalu **asinkron**: ia menuntut protokol berbasis peristiwa
//! (`METransformNeedInput` / `METransformHaveOutput`) yang bentuknya cukup
//! berbeda untuk pantas dikerjakan terpisah, dengan jalur sinkron yang sudah
//! terbukti sebagai pembanding.
//!
//! Nama encoder yang benar-benar terpilih selalu dilaporkan, supaya tidak
//! pernah ada keraguan mana yang sedang dipakai.

/// Satu unit akses H.264 — satu frame terkode, dalam format Annex-B.
pub type UnitAkses = Vec<u8>;

// ── Konversi warna ───────────────────────────────────────────────────────────

/// Mengubah BGRA menjadi NV12.
///
/// Encoder H.264 tidak menerima RGB. NV12 adalah format yang diminta hampir
/// setiap encoder Windows: satu bidang Y beresolusi penuh, disusul satu bidang
/// UV berselang-seling pada seperempat resolusi.
///
/// Koefisiennya **BT.709 rentang terbatas**, bukan BT.601. Untuk materi
/// beresolusi tinggi BT.709 adalah yang benar, dan memilih yang keliru tidak
/// menghasilkan kegagalan yang kentara — hanya warna yang meleset sedikit,
/// paling terlihat pada rona kulit dan gradasi. Kelas kesalahan yang bertahan
/// lama justru karena tidak pernah cukup mengganggu untuk diselidiki.
pub fn bgra_ke_nv12(bgra: &[u8], w: usize, h: usize, keluar: &mut Vec<u8>) {
    debug_assert_eq!(bgra.len(), w * h * 4);
    debug_assert_eq!(w % 2, 0, "lebar ganjil tidak dapat disubsampel 4:2:0");
    debug_assert_eq!(h % 2, 0, "tinggi ganjil tidak dapat disubsampel 4:2:0");

    keluar.clear();
    keluar.resize(w * h * 3 / 2, 0);
    let (y_plane, uv_plane) = keluar.split_at_mut(w * h);

    for y in 0..h {
        for x in 0..w {
            let p = (y * w + x) * 4;
            let (b, g, r) = (bgra[p] as i32, bgra[p + 1] as i32, bgra[p + 2] as i32);
            y_plane[y * w + x] = (((47 * r + 157 * g + 16 * b + 128) >> 8) + 16) as u8;
        }
    }

    // Kroma diambil dari rata-rata blok 2×2, bukan dari satu piksel pojok.
    // Mencuplik satu piksel lebih murah tetapi menghasilkan tepi warna yang
    // bergerigi pada teks berwarna — dan teks adalah isi utama layar kerja.
    for by in (0..h).step_by(2) {
        for bx in (0..w).step_by(2) {
            let mut sr = 0i32;
            let mut sg = 0i32;
            let mut sb = 0i32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let p = ((by + dy) * w + bx + dx) * 4;
                    sb += bgra[p] as i32;
                    sg += bgra[p + 1] as i32;
                    sr += bgra[p + 2] as i32;
                }
            }
            let (r, g, b) = (sr / 4, sg / 4, sb / 4);
            let u = (((-26 * r - 87 * g + 112 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
            let v = (((112 * r - 102 * g - 10 * b + 128) >> 8) + 128).clamp(0, 255) as u8;

            let i = (by / 2) * w + bx;
            uv_plane[i] = u;
            uv_plane[i + 1] = v;
        }
    }
}

/// Mengubah NV12 kembali menjadi BGRA.
///
/// Kebalikan [`bgra_ke_nv12`], dan ada terutama untuk membuktikannya. Sebuah
/// bitstream H.264 dapat tersusun sepenuhnya sah sambil berisi gambar sampah:
/// salah tata letak bidang kroma menghasilkan berkas yang lolos setiap
/// pemeriksaan struktural, dan baru ketahuan saat ada mata yang melihatnya.
/// Perjalanan pulang-pergi menutup celah itu tanpa perlu dekoder.
pub fn nv12_ke_bgra(nv12: &[u8], w: usize, h: usize, keluar: &mut Vec<u8>) {
    keluar.clear();
    keluar.resize(w * h * 4, 255);
    let (y_plane, uv_plane) = nv12.split_at(w * h);

    for y in 0..h {
        for x in 0..w {
            let yy = (y_plane[y * w + x] as i32 - 16) * 298;
            let i = (y / 2) * w + (x & !1);
            let u = uv_plane[i] as i32 - 128;
            let v = uv_plane[i + 1] as i32 - 128;

            // Kebalikan BT.709 rentang terbatas.
            let r = (yy + 459 * v + 128) >> 8;
            let g = (yy - 55 * u - 136 * v + 128) >> 8;
            let b = (yy + 541 * u + 128) >> 8;

            let p = (y * w + x) * 4;
            keluar[p] = b.clamp(0, 255) as u8;
            keluar[p + 1] = g.clamp(0, 255) as u8;
            keluar[p + 2] = r.clamp(0, 255) as u8;
        }
    }
}

// ── Pembacaan bitstream ──────────────────────────────────────────────────────

/// Memisahkan unit akses Annex-B menjadi NAL satuan, tanpa kode awal.
///
/// Dipakai untuk memeriksa keluaran encoder, dan akan dipakai lagi di M2c:
/// paketisasi RTP bekerja pada NAL satuan, bukan pada unit akses utuh.
pub fn pisah_nal(au: &[u8]) -> Vec<&[u8]> {
    let mut hasil = Vec::new();
    let mut mulai: Option<usize> = None;
    let mut i = 0;

    while i + 2 < au.len() {
        // Kode awal boleh tiga byte maupun empat.
        let tiga = au[i] == 0 && au[i + 1] == 0 && au[i + 2] == 1;
        if tiga {
            if let Some(m) = mulai {
                hasil.push(&au[m..i]);
            }
            i += 3;
            mulai = Some(i);
            continue;
        }
        i += 1;
    }

    if let Some(m) = mulai {
        if m < au.len() {
            hasil.push(&au[m..]);
        }
    }

    hasil
}

/// Tipe NAL, yaitu lima bit terendah dari byte pertama.
pub fn tipe_nal(nal: &[u8]) -> u8 {
    nal.first().map(|b| b & 0x1F).unwrap_or(0)
}

pub const NAL_IDR: u8 = 5;
pub const NAL_SPS: u8 = 7;
pub const NAL_PPS: u8 = 8;

/// Pembaca bit dengan Exp-Golomb, secukupnya untuk membaca SPS.
struct Bit<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bit<'a> {
    fn baru(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn bit(&mut self) -> u32 {
        let byte = self.pos / 8;
        if byte >= self.data.len() {
            return 0;
        }
        let b = (self.data[byte] >> (7 - (self.pos % 8))) & 1;
        self.pos += 1;
        b as u32
    }

    fn bits(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |a, _| (a << 1) | self.bit())
    }

    /// Exp-Golomb tanpa tanda.
    fn ue(&mut self) -> u32 {
        let mut nol = 0;
        while self.bit() == 0 && nol < 32 && self.pos < self.data.len() * 8 {
            nol += 1;
        }
        if nol == 0 {
            return 0;
        }
        ((1u32 << nol) - 1) + self.bits(nol)
    }

    /// Exp-Golomb bertanda.
    fn se(&mut self) -> i32 {
        let k = self.ue();
        let v = ((k + 1) / 2) as i32;
        if k % 2 == 0 {
            -v
        } else {
            v
        }
    }
}

/// Membuang emulation prevention byte (`00 00 03` → `00 00`).
fn buang_emulasi(nal: &[u8]) -> Vec<u8> {
    let mut keluar = Vec::with_capacity(nal.len());
    let mut i = 0;
    while i < nal.len() {
        if i + 2 < nal.len() && nal[i] == 0 && nal[i + 1] == 0 && nal[i + 2] == 3 {
            keluar.push(0);
            keluar.push(0);
            i += 3;
        } else {
            keluar.push(nal[i]);
            i += 1;
        }
    }
    keluar
}

/// Membaca lebar dan tinggi dari SPS.
///
/// Ini pemeriksaan yang membedakan "berkas berisi sesuatu" dari "berkas berisi
/// H.264 yang benar": bila SPS terbaca dan menyebutkan dimensi yang sama dengan
/// yang diminta, maka bitstream-nya memang tersusun sah — bukan sekadar
/// sekumpulan byte yang kebetulan berawalan kode awal.
pub fn baca_sps(sps: &[u8]) -> Option<(u32, u32)> {
    if tipe_nal(sps) != NAL_SPS {
        return None;
    }
    let bersih = buang_emulasi(sps);
    let mut b = Bit::baru(&bersih[1..]); // lewati header NAL

    let profile = b.bits(8);
    b.bits(8); // constraint flags + reserved
    b.bits(8); // level
    b.ue(); // seq_parameter_set_id

    if matches!(profile, 100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135)
    {
        let chroma = b.ue();
        if chroma == 3 {
            b.bit(); // separate_colour_plane_flag
        }
        b.ue(); // bit_depth_luma_minus8
        b.ue(); // bit_depth_chroma_minus8
        b.bit(); // qpprime_y_zero_transform_bypass_flag
        if b.bit() == 1 {
            // seq_scaling_matrix_present_flag
            let n = if chroma != 3 { 8 } else { 12 };
            for i in 0..n {
                if b.bit() == 1 {
                    let ukuran = if i < 6 { 16 } else { 64 };
                    let mut next = 8i32;
                    let mut last = 8i32;
                    for _ in 0..ukuran {
                        if next != 0 {
                            next = (last + b.se() + 256) % 256;
                        }
                        last = if next == 0 { last } else { next };
                    }
                }
            }
        }
    }

    b.ue(); // log2_max_frame_num_minus4
    let pic_order = b.ue();
    if pic_order == 0 {
        b.ue(); // log2_max_pic_order_cnt_lsb_minus4
    } else if pic_order == 1 {
        b.bit();
        b.se();
        b.se();
        let n = b.ue();
        for _ in 0..n {
            b.se();
        }
    }

    b.ue(); // max_num_ref_frames
    b.bit(); // gaps_in_frame_num_value_allowed_flag

    let lebar_mb = b.ue() + 1;
    let tinggi_map = b.ue() + 1;
    let frame_mbs_only = b.bit();
    if frame_mbs_only == 0 {
        b.bit(); // mb_adaptive_frame_field_flag
    }
    b.bit(); // direct_8x8_inference_flag

    let (mut kiri, mut kanan, mut atas, mut bawah) = (0u32, 0u32, 0u32, 0u32);
    if b.bit() == 1 {
        // frame_cropping_flag
        kiri = b.ue();
        kanan = b.ue();
        atas = b.ue();
        bawah = b.ue();
    }

    let lebar = lebar_mb * 16;
    let tinggi = tinggi_map * 16 * (2 - frame_mbs_only);

    // Pemotongan dinyatakan dalam satuan sampel kroma: 2 piksel per satuan
    // secara horizontal pada 4:2:0.
    let potong_x = (kiri + kanan) * 2;
    let potong_y = (atas + bawah) * 2 * (2 - frame_mbs_only);

    Some((
        lebar.saturating_sub(potong_x),
        tinggi.saturating_sub(potong_y),
    ))
}

// ── Encoder ──────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use super::UnitAkses;
    use anyhow::{bail, Context, Result};
    use std::sync::Once;
    use windows::core::PWSTR;
    use windows::Win32::Media::MediaFoundation::*;

    static MULAI: Once = Once::new();

    fn mf_startup() -> Result<()> {
        let mut hasil = Ok(());
        MULAI.call_once(|| {
            // MFSTARTUP_NOSOCKET: agent tidak memakai jaringan Media Foundation,
            // dan menyalakannya hanya menambah permukaan yang tidak dipakai.
            hasil = unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }
                .context("MFStartup gagal");
        });
        hasil
    }

    pub struct H264 {
        transform: IMFTransform,
        /// Objek aktivasi yang melahirkan `transform`, disimpan **hanya** untuk
        /// dapat mematikannya.
        ///
        /// `IMFActivate` menyimpan rujukan internal ke objek yang dibuatnya, dan
        /// rujukan itu tidak ikut lepas saat `IMFActivate`-nya dilepas —
        /// satu-satunya yang melepasnya adalah `ShutdownObject`. Tanpa itu
        /// setiap encoder yang pernah dibuat hidup selamanya beserta seluruh
        /// kolam buffernya.
        ///
        /// Ini ketahuan dari sesi produksi: setiap perpindahan monitor membuat
        /// encoder baru, dan memori agent naik sekitar 370 MB per perpindahan
        /// lalu mendatar — bukan bocor per frame, melainkan bocor satu encoder
        /// utuh setiap kali.
        aktivasi: IMFActivate,
        pub nama: String,
        pub width: u32,
        pub height: u32,
        durasi: i64,
        nv12: Vec<u8>,
        /// Encoder mengalokasikan sampel keluarannya sendiri atau kita yang harus.
        sampel_dari_mft: bool,
        ukuran_keluaran: u32,
    }

    impl std::fmt::Debug for H264 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("H264")
                .field("encoder", &self.nama)
                .field("width", &self.width)
                .field("height", &self.height)
                .finish_non_exhaustive()
        }
    }

    impl Drop for H264 {
        fn drop(&mut self) {
            unsafe {
                let _ = self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
                // Inilah yang benar-benar membebaskan encoder. Melepas
                // `IMFTransform` saja tidak cukup; lihat catatan pada
                // `aktivasi`.
                let _ = self.aktivasi.ShutdownObject();
            }
        }
    }

    /// Mencari MFT encoder H.264 dan menyalakannya.
    fn cari_encoder() -> Result<(IMFTransform, IMFActivate, String)> {
        let masuk = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_NV12,
        };
        let keluar = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };

        let mut daftar: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut jumlah = 0u32;

        unsafe {
            MFTEnumEx(
                MFT_CATEGORY_VIDEO_ENCODER,
                // SORTANDFILTER membuang MFT yang tidak dapat dipakai; SYNCMFT
                // membatasi ke encoder sinkron. Lihat catatan modul soal
                // encoder perangkat keras yang asinkron.
                MFT_ENUM_FLAG(MFT_ENUM_FLAG_SYNCMFT.0 | MFT_ENUM_FLAG_SORTANDFILTER.0),
                Some(&masuk),
                Some(&keluar),
                &mut daftar,
                &mut jumlah,
            )
        }
        .context("MFTEnumEx gagal")?;

        if jumlah == 0 || daftar.is_null() {
            bail!("tidak ada encoder H.264 yang terdaftar di Media Foundation");
        }

        let potongan = unsafe { std::slice::from_raw_parts(daftar, jumlah as usize) };
        let aktivasi = potongan[0]
            .clone()
            .context("entri encoder pertama kosong")?;

        let nama = unsafe {
            let mut buf = PWSTR::null();
            let mut panjang = 0u32;
            match aktivasi.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut buf, &mut panjang) {
                Ok(()) => {
                    let s = buf.to_string().unwrap_or_default();
                    windows::Win32::System::Com::CoTaskMemFree(Some(buf.0 as *const _));
                    s
                }
                Err(_) => "encoder tanpa nama".to_string(),
            }
        };

        let transform: IMFTransform = unsafe { aktivasi.ActivateObject() }
            .context("gagal menyalakan encoder")?;

        // Seluruh entri sisanya, beserta arraynya, milik pemanggil.
        unsafe {
            for e in potongan.iter().skip(1) {
                drop(e.clone());
            }
            windows::Win32::System::Com::CoTaskMemFree(Some(daftar as *const _));
        }

        Ok((transform, aktivasi, nama))
    }

    /// Menyusun dua u32 menjadi satu atribut UINT64, sesuai kebiasaan
    /// Media Foundation untuk ukuran dan rasio.
    fn pasangan(tinggi: u32, rendah: u32) -> u64 {
        ((tinggi as u64) << 32) | rendah as u64
    }

    impl H264 {
        pub fn baru(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<Self> {
            mf_startup()?;
            let (transform, aktivasi, nama) = cari_encoder()?;

            // Tipe keluaran wajib ditetapkan lebih dulu. Media Foundation
            // menolak tipe masukan sebelum tahu hendak menghasilkan apa, dan
            // galatnya tidak menjelaskan urutan itu.
            let keluar = unsafe { MFCreateMediaType() }?;
            unsafe {
                keluar.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                keluar.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
                keluar.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
                keluar.SetUINT64(&MF_MT_FRAME_SIZE, pasangan(width, height))?;
                keluar.SetUINT64(&MF_MT_FRAME_RATE, pasangan(fps, 1))?;
                keluar.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pasangan(1, 1))?;
                keluar.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
                // Baseline: tanpa B-frame, sehingga tidak ada penyusunan ulang
                // yang menambah latensi, dan diterima setiap browser.
                keluar.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base.0 as u32)?;
                transform.SetOutputType(0, &keluar, 0)?;
            }

            let masuk = unsafe { MFCreateMediaType() }?;
            unsafe {
                masuk.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                masuk.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
                masuk.SetUINT64(&MF_MT_FRAME_SIZE, pasangan(width, height))?;
                masuk.SetUINT64(&MF_MT_FRAME_RATE, pasangan(fps, 1))?;
                masuk.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pasangan(1, 1))?;
                masuk.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
                // Menyatakan matriks yang benar-benar dipakai konversi warna.
                // Tanpa ini pemutar menebak, dan tebakannya berbeda-beda.
                masuk.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
                transform.SetInputType(0, &masuk, 0)?;
            }

            let info = unsafe { transform.GetOutputStreamInfo(0) }?;
            let sampel_dari_mft = info.dwFlags
                & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0
                    | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0) as u32
                != 0;

            // Tanpa FLUSH di sini. Encoder bawaan Windows menolaknya dengan
            // E_FAIL sebelum streaming dimulai — masuk akal, karena belum ada
            // apa pun untuk dibuang, tetapi pesan galatnya tidak mengatakan itu.
            unsafe {
                transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
                transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            }

            Ok(Self {
                transform,
                aktivasi,
                nama,
                width,
                height,
                durasi: 10_000_000 / fps.max(1) as i64,
                nv12: Vec::new(),
                sampel_dari_mft,
                ukuran_keluaran: info.cbSize.max(width * height),
            })
        }

        /// Mengkodekan satu frame BGRA. Mengembalikan nol atau lebih unit akses.
        ///
        /// Nol adalah keluaran yang sah: encoder boleh menahan frame sebelum
        /// menghasilkan apa pun.
        pub fn encode(&mut self, bgra: &[u8], waktu_100ns: i64) -> Result<Vec<UnitAkses>> {
            super::bgra_ke_nv12(bgra, self.width as usize, self.height as usize, &mut self.nv12);

            let buffer = unsafe { MFCreateMemoryBuffer(self.nv12.len() as u32) }?;
            unsafe {
                let mut tujuan = std::ptr::null_mut();
                let mut maks = 0u32;
                buffer.Lock(&mut tujuan, Some(&mut maks), None)?;
                std::ptr::copy_nonoverlapping(self.nv12.as_ptr(), tujuan, self.nv12.len());
                buffer.Unlock()?;
                buffer.SetCurrentLength(self.nv12.len() as u32)?;
            }

            let sampel = unsafe { MFCreateSample() }?;
            unsafe {
                sampel.AddBuffer(&buffer)?;
                sampel.SetSampleTime(waktu_100ns)?;
                sampel.SetSampleDuration(self.durasi)?;
                self.transform.ProcessInput(0, &sampel, 0)?;
            }

            self.kumpulkan()
        }

        /// Mengambil seluruh keluaran yang siap.
        fn kumpulkan(&mut self) -> Result<Vec<UnitAkses>> {
            let mut hasil = Vec::new();

            loop {
                let sampel_keluar = if self.sampel_dari_mft {
                    None
                } else {
                    let s = unsafe { MFCreateSample() }?;
                    let b = unsafe { MFCreateMemoryBuffer(self.ukuran_keluaran) }?;
                    unsafe { s.AddBuffer(&b)? };
                    Some(s)
                };

                let mut buf = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: std::mem::ManuallyDrop::new(sampel_keluar),
                    dwStatus: 0,
                    pEvents: std::mem::ManuallyDrop::new(None),
                }];
                let mut status = 0u32;

                let hasil_proses =
                    unsafe { self.transform.ProcessOutput(0, &mut buf, &mut status) };

                // `take`, bukan `into_inner(clone())`.
                //
                // Versi pertama memakai clone, dan itu **membocorkan satu
                // IMFSample setiap frame**. Mengkloning `Option<IMFSample>`
                // menaikkan refcount COM; klon itu kemudian dilepas, tetapi
                // nilai asli yang masih duduk di dalam `ManuallyDrop` tidak
                // pernah dilepas sama sekali.
                //
                // Pada 1080p30 setiap sampel keluaran sekitar 2 MB, jadi
                // kebocorannya sekitar 60 MB per detik. Sesi pertama di
                // produksi berjalan 185 detik lalu mati dengan E_OUTOFMEMORY —
                // "Not enough memory resources are available" — yang menuding
                // encoder, padahal sebabnya ada di sini.
                //
                // `take` memindahkan nilainya keluar dan meninggalkan `buf`
                // dalam keadaan tidak dipakai lagi, sehingga tidak ada yang
                // bocor dan tidak ada yang dilepas dua kali.
                let keluaran = unsafe { std::mem::ManuallyDrop::take(&mut buf[0].pSample) };
                let _ = unsafe { std::mem::ManuallyDrop::take(&mut buf[0].pEvents) };

                if let Err(e) = hasil_proses {
                    // Bukan galat: encoder sekadar belum punya apa-apa untuk
                    // diserahkan. Inilah cara normal perulangan berakhir.
                    if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT {
                        break;
                    }
                    // Tipe keluaran berubah di tengah jalan — encoder berhak
                    // melakukannya, dan menolaknya berarti berhenti bekerja.
                    if e.code() == MF_E_TRANSFORM_STREAM_CHANGE {
                        continue;
                    }
                    return Err(e).context("ProcessOutput gagal");
                }

                let Some(sampel) = keluaran else { break };
                hasil.push(unsafe { baca_sampel(&sampel) }?);
            }

            Ok(hasil)
        }

        /// Menguras encoder di akhir aliran.
        pub fn kuras(&mut self) -> Result<Vec<UnitAkses>> {
            unsafe {
                self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0)?;
                self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0)?;
            }
            self.kumpulkan()
        }
    }

    unsafe fn baca_sampel(sampel: &IMFSample) -> Result<UnitAkses> {
        let buffer = sampel.ConvertToContiguousBuffer()?;
        let mut data = std::ptr::null_mut();
        let mut panjang = 0u32;
        buffer.Lock(&mut data, None, Some(&mut panjang))?;
        let keluar = std::slice::from_raw_parts(data, panjang as usize).to_vec();
        buffer.Unlock()?;
        Ok(keluar)
    }
}

#[cfg(windows)]
pub use win::H264;

#[cfg(not(windows))]
#[derive(Debug)]
pub struct H264;

#[cfg(not(windows))]
impl H264 {
    pub fn baru(_w: u32, _h: u32, _fps: u32, _bitrate: u32) -> anyhow::Result<Self> {
        anyhow::bail!("encoder H.264 belum diimplementasikan pada platform ini")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nv12_berukuran_satu_setengah_kali_luas() {
        let mut out = Vec::new();
        bgra_ke_nv12(&vec![0u8; 4 * 4 * 4], 4, 4, &mut out);
        assert_eq!(out.len(), 4 * 4 * 3 / 2);
    }

    #[test]
    fn hitam_dan_putih_jatuh_pada_rentang_terbatas() {
        let mut out = Vec::new();

        // BT.709 rentang terbatas: hitam = 16, putih = 235.
        bgra_ke_nv12(&vec![0u8; 2 * 2 * 4], 2, 2, &mut out);
        assert_eq!(out[0], 16, "hitam harus 16, bukan 0");

        bgra_ke_nv12(&vec![255u8; 2 * 2 * 4], 2, 2, &mut out);
        assert!(
            (233..=236).contains(&out[0]),
            "putih harus mendekati 235, dapat {}",
            out[0]
        );
    }

    #[test]
    fn abu_abu_netral_menghasilkan_kroma_tengah() {
        // Abu-abu tidak punya warna: kedua kanal kroma harus duduk di 128.
        let mut out = Vec::new();
        bgra_ke_nv12(&vec![128u8; 2 * 2 * 4], 2, 2, &mut out);
        let uv = &out[4..];
        assert!((127..=129).contains(&uv[0]), "U meleset: {}", uv[0]);
        assert!((127..=129).contains(&uv[1]), "V meleset: {}", uv[1]);
    }

    #[test]
    fn merah_dan_biru_menghasilkan_kroma_khas_bt709() {
        // Nilai-nilai di bawah ini adalah BT.709, dan sengaja dipatok tepat.
        //
        // Perhatikan U untuk merah: 102, hanya 26 di bawah netral. Pada BT.601
        // angkanya 90 — jauh lebih rendah. Selisih itulah yang membuat matriks
        // yang keliru lolos dari pemeriksaan longgar semacam "U harus di bawah
        // 100", lalu muncul belakangan sebagai rona kulit yang meleset.
        let dekat = |dapat: u8, harap: i32, nama: &str| {
            assert!(
                (dapat as i32 - harap).abs() <= 2,
                "{nama}: dapat {dapat}, harap sekitar {harap}"
            );
        };

        let mut out = Vec::new();

        let merah: Vec<u8> = [0u8, 0, 255, 255].repeat(4);
        bgra_ke_nv12(&merah, 2, 2, &mut out);
        dekat(out[4], 102, "U merah");
        dekat(out[5], 240, "V merah");

        let biru: Vec<u8> = [255u8, 0, 0, 255].repeat(4);
        bgra_ke_nv12(&biru, 2, 2, &mut out);
        dekat(out[4], 240, "U biru");
        dekat(out[5], 118, "V biru");
    }

    /// Gambar uji dengan blok warna 2×2 yang berbeda-beda.
    ///
    /// Blok 2×2 penting: itulah satuan subsampling kroma, sehingga pola ini
    /// membuktikan penempatan bidang UV — bukan sekadar nilai satu piksel.
    fn pola(w: usize, h: usize) -> Vec<u8> {
        let warna = [
            [0u8, 0, 255],     // merah
            [0, 255, 0],       // hijau
            [255, 0, 0],       // biru
            [255, 255, 255],   // putih
            [0, 0, 0],         // hitam
            [128, 128, 128],   // abu-abu
        ];
        let mut v = vec![255u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let c = warna[((y / 2) * (w / 2) + (x / 2)) % warna.len()];
                let p = (y * w + x) * 4;
                v[p] = c[0];
                v[p + 1] = c[1];
                v[p + 2] = c[2];
            }
        }
        v
    }

    #[test]
    fn pulang_pergi_nv12_mempertahankan_gambar() {
        // Inilah yang membuktikan tata letak bidang NV12 benar. Kroma yang
        // salah tempat menghasilkan gambar yang tetap "valid" tetapi warnanya
        // berpindah antar blok — dan H.264 di atasnya akan tetap lolos setiap
        // pemeriksaan struktural.
        let (w, h) = (16usize, 12usize);
        let asal = pola(w, h);

        let mut nv12 = Vec::new();
        bgra_ke_nv12(&asal, w, h, &mut nv12);
        let mut kembali = Vec::new();
        nv12_ke_bgra(&nv12, w, h, &mut kembali);

        let mut terburuk = 0i32;
        for i in 0..w * h {
            for k in 0..3 {
                let d = (asal[i * 4 + k] as i32 - kembali[i * 4 + k] as i32).abs();
                terburuk = terburuk.max(d);
            }
        }

        // 4:2:0 membuang tiga perempat informasi kroma, jadi warna primer jenuh
        // tidak akan kembali persis. Yang dijaga di sini adalah bahwa setiap
        // blok tetap warnanya sendiri, bukan warna tetangganya.
        assert!(
            terburuk <= 24,
            "selisih terbesar {terburuk} — kroma kemungkinan salah tempat"
        );
    }

    #[test]
    fn blok_warna_tidak_bertukar_tempat() {
        // Pemeriksaan yang lebih tajam daripada selisih rata-rata: pojok
        // kiri-atas harus tetap merah, dan tetangga kanannya tetap hijau.
        let (w, h) = (8usize, 4usize);
        let asal = pola(w, h);
        let mut nv12 = Vec::new();
        bgra_ke_nv12(&asal, w, h, &mut nv12);
        let mut kembali = Vec::new();
        nv12_ke_bgra(&nv12, w, h, &mut kembali);

        // Merah: kanal R jauh di atas B.
        assert!(kembali[2] > 180 && kembali[0] < 70, "blok pertama bukan merah");
        // Hijau ada di blok berikutnya, mulai kolom 2.
        let p = 2 * 4;
        assert!(kembali[p + 1] > 180, "blok kedua bukan hijau");
    }

    #[test]
    fn pisah_nal_menemukan_kode_awal_tiga_dan_empat_byte() {
        let au = [
            0, 0, 0, 1, 0x67, 0xAA, // SPS berkode awal 4 byte
            0, 0, 1, 0x68, 0xBB, // PPS berkode awal 3 byte
            0, 0, 0, 1, 0x65, 0xCC, 0xDD,
        ];
        let nal = pisah_nal(&au);
        assert_eq!(nal.len(), 3);
        assert_eq!(tipe_nal(nal[0]), NAL_SPS);
        assert_eq!(tipe_nal(nal[1]), NAL_PPS);
        assert_eq!(tipe_nal(nal[2]), NAL_IDR);
    }

    #[test]
    fn pisah_nal_aman_untuk_masukan_cacat() {
        for buruk in [vec![], vec![0], vec![0, 0], vec![0, 0, 1], vec![1, 2, 3, 4]] {
            let _ = pisah_nal(&buruk);
        }
    }

    #[test]
    fn baca_sps_hanya_menerima_sps() {
        assert_eq!(baca_sps(&[0x68, 0xCE]), None, "PPS bukan SPS");
        assert_eq!(baca_sps(&[0x65, 0x88]), None, "IDR bukan SPS");
        assert_eq!(baca_sps(&[]), None);
    }

    #[test]
    fn baca_sps_tidak_panik_pada_masukan_terpotong() {
        // Bitstream cacat datang dari jaringan di M2c. Pembaca yang panik pada
        // SPS terpotong akan menjatuhkan agent, bukan sekadar menolak frame.
        let utuh = [
            0x67, 0x42, 0xC0, 0x28, 0xDA, 0x01, 0xE0, 0x08, 0x9F, 0x97, 0x01, 0x10,
        ];
        for n in 0..utuh.len() {
            let _ = baca_sps(&utuh[..n]);
        }
    }

    #[test]
    fn sps_1920x1080_terbaca() {
        // SPS asli yang dihasilkan "H264 Encoder MFT" pada 1920×1080, disalin
        // byte demi byte dari keluaran `rdp-agent encode`. Bukan vektor
        // karangan: inilah yang benar-benar dikirim ke penerima.
        //
        // Perhatikan `00 00 03 00` di dalamnya — emulation prevention byte yang
        // wajib dibuang sebelum bit dibaca. SPS ini sekaligus menjaga jalur itu
        // tetap benar.
        let sps = [
            0x67, 0x42, 0xC0, 0x28, 0x95, 0xB0, 0x1E, 0x00, 0x89, 0xF9, 0x70, 0x11, 0x00, 0x00,
            0x03, 0x00, 0x01, 0x00, 0x00, 0x03, 0x00, 0x3C, 0x0D, 0xA0, 0x88, 0x46, 0xE0,
        ];
        assert_eq!(
            baca_sps(&sps),
            Some((1920, 1080)),
            "dimensi tidak terbaca dari SPS encoder sungguhan"
        );
    }

    #[test]
    fn sps_1080x1920_tegak_terbaca() {
        // Monitor tegak. Dimensi yang tertukar akan lolos pemeriksaan mana pun
        // yang hanya melihat luas, jadi orientasinya diuji terpisah.
        let sps = [
            0x67, 0x42, 0xC0, 0x28, 0x95, 0xB0, 0x11, 0x00, 0xF1, 0xE5, 0xF0, 0x11, 0x00, 0x00,
            0x03, 0x00, 0x01, 0x00, 0x00, 0x03, 0x00, 0x3C, 0x0D, 0xA0, 0x88, 0x46, 0xE0,
        ];
        assert_eq!(
            baca_sps(&sps),
            Some((1080, 1920)),
            "orientasi tegak terbaca terbalik"
        );
    }

    #[test]
    fn buang_emulasi_mengembalikan_nol_ganda() {
        assert_eq!(buang_emulasi(&[0, 0, 3, 1]), vec![0, 0, 1]);
        assert_eq!(buang_emulasi(&[1, 2, 3]), vec![1, 2, 3]);
    }
}
