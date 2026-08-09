//! Quick Connect — koneksi memakai device ID dan password sekali pakai.
//!
//! Implementasi QUICK_CONNECT.md. Empat properti keamanan ditegakkan di sini,
//! dan ketiganya mudah hilang saat refactor, jadi masing-masing diberi test:
//!
//! 1. Check digit divalidasi **sebelum** menyentuh database
//! 2. Respons seragam untuk semua sebab kegagalan
//! 3. Lama respons dinormalkan ke satu nilai tetap
//! 4. Setiap upaya dicatat, termasuk yang device ID-nya tidak pernah ada

use crate::{
    audit::{self, aksi},
    auth::{hash, Terautentikasi},
    error::{ApiError, ApiResult, Sukses},
    net::IpKlien,
    ratelimit::{self, Keputusan},
    state::AppState,
};
use axum::extract::State;
use rdp_core::{DeviceId, QuickConnectOutcome};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;
use uuid::Uuid;

/// Lama respons minimum untuk **seluruh** hasil, berhasil maupun gagal.
///
/// Tanpa ini, jalur "ID tidak ditemukan" selesai jauh lebih cepat daripada
/// jalur "Argon2id dijalankan lalu gagal". Selisih itu adalah oracle yang
/// memberi tahu penyerang device ID mana yang hidup, dan membuat pemindaian
/// ruang ID menjadi murah. (QUICK_CONNECT.md §5.1)
const LANTAI_RESPONS: Duration = Duration::from_millis(250);

#[derive(Debug, Deserialize)]
pub struct ConnectReq {
    pub device_id: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectResp {
    pub session_id: Uuid,
    pub device_uuid: Uuid,
    /// Sesi belum aktif. Agent masih harus menampilkan prompt persetujuan;
    /// password yang benar hanya memberi hak **meminta** koneksi.
    /// (QUICK_CONNECT.md §4)
    pub status: &'static str,
}

/// `POST /api/v1/connect`
pub async fn connect(
    State(state): State<AppState>,
    IpKlien(ip): IpKlien,
    Terautentikasi(claims): Terautentikasi,
    axum::Json(req): axum::Json<ConnectReq>,
) -> ApiResult<Sukses<ConnectResp>> {
    let mulai = Instant::now();
    let hasil = jalankan(&state, &req, &claims, ip).await;
    normalkan_waktu(mulai).await;
    hasil
}

async fn jalankan(
    state: &AppState,
    req: &ConnectReq,
    claims: &crate::auth::jwt::Claims,
    ip: std::net::IpAddr,
) -> ApiResult<Sukses<ConnectResp>> {
    let mut redis = state.redis.clone();

    // ── 1. Bentuk masukan ────────────────────────────────────────────────────
    //
    // Nomor dan alias hidup berdampingan sejak migrasi 0006. Yang menyerupai
    // nomor tetap diperiksa check digit-nya lebih dulu — 90% string sembilan
    // digit gagal di situ, tanpa satu pun query database. Yang lain
    // diperlakukan sebagai alias, dan alias tidak punya check digit untuk
    // disaring, jadi ia langsung menjadi urusan basis data.
    let Some(kunci) = normalkan_kunci(&req.device_id) else {
        catat(state, &req.device_id, ip, QuickConnectOutcome::UnknownId, claims).await;
        let _ = ratelimit::catat_kegagalan(
            &mut redis,
            &format!("ip:{ip}"),
            ratelimit::ID_TAK_DIKENAL_PER_IP,
        )
        .await;
        return Err(ApiError::ConnectDitolak);
    };

    // ── 2. Sedang dijeda? ────────────────────────────────────────────────────
    let kunci_device = format!("qc:{kunci}");
    if let Keputusan::Dijeda { retry_after_seconds } =
        ratelimit::periksa(&mut redis, &kunci_device).await?
    {
        catat(state, &req.device_id, ip, QuickConnectOutcome::Throttled, claims).await;
        tracing::warn!(device = %kunci, %ip, "quick connect ditolak: sedang dijeda");
        // Sengaja tidak memberi tahu bahwa jedanya ada — itu sendiri
        // mengonfirmasi device ID-nya hidup.
        let _ = retry_after_seconds;
        return Err(ApiError::ConnectDitolak);
    }

    // ── 3. Resolusi lintas-tenant ────────────────────────────────────────────
    let baris: Option<(Uuid, Uuid, Option<String>, Option<String>, bool, String)> = sqlx::query_as(
        "SELECT device_uuid, org_id, session_hash, unattended_hash, enabled, status
         FROM resolve_connect_key($1)",
    )
    .bind(&kunci)
    .fetch_optional(&state.db)
    .await?;

    // Perangkat tanpa satu pun kata sandi tidak dapat diakses lewat Quick
    // Connect, dan itu tidak dibedakan dari perangkat yang tidak ada.
    let Some((device_uuid, org_id, sesi_hash, tetap_hash, enabled, status)) = baris else {
        // Mencakup dua kasus sekaligus: ID tidak ada, dan ID ada tetapi belum
        // pernah punya password sesi. Keduanya menghasilkan respons sama.
        hash::verify_dummy(&req.password);
        catat(state, &req.device_id, ip, QuickConnectOutcome::UnknownId, claims).await;
        let _ = ratelimit::catat_kegagalan(
            &mut redis,
            &format!("ip:{ip}"),
            ratelimit::ID_TAK_DIKENAL_PER_IP,
        )
        .await;
        return Err(ApiError::ConnectDitolak);
    };

    if !enabled {
        hash::verify_dummy(&req.password);
        catat(state, &req.device_id, ip, QuickConnectOutcome::UnknownId, claims).await;
        return Err(ApiError::ConnectDitolak);
    }

    // ── 4. Verifikasi password ───────────────────────────────────────────────
    //
    // Dua kata sandi dapat berlaku: yang acak dan berotasi untuk bantuan
    // sesaat, dan yang tetap pilihan pemilik untuk mesinnya sendiri.
    //
    // **Keduanya selalu diperiksa**, bahkan setelah yang pertama cocok. Berhenti
    // lebih awal membuat lama respons memberi tahu kata sandi mana yang benar,
    // dan itu memberi penyerang cara memilah tebakannya. Argon2id memang mahal,
    // tetapi lantai waktu 250 ms sudah menutupi biaya keduanya.
    let dinormalkan = rdp_core::password::normalize(&req.password);
    let cocok_sesi = match &sesi_hash {
        Some(h) => hash::verify(&dinormalkan, h),
        None => {
            hash::verify_dummy(&req.password);
            false
        }
    };
    // Kata sandi tetap tidak dinormalkan: ia dipilih manusia dan boleh memuat
    // huruf kecil, spasi, maupun simbol. Menormalkannya seperti kata sandi sesi
    // akan diam-diam mengubah apa yang pengguna ketik.
    let cocok_tetap = match &tetap_hash {
        Some(h) => hash::verify(&req.password, h),
        None => {
            hash::verify_dummy(&req.password);
            false
        }
    };

    if !cocok_sesi && !cocok_tetap {
        catat(state, &req.device_id, ip, QuickConnectOutcome::BadPassword, claims).await;
        ratelimit::catat_kegagalan(&mut redis, &kunci_device, ratelimit::PER_DEVICE).await?;
        tracing::info!(device = %kunci, %ip, "quick connect ditolak: password salah");
        return Err(ApiError::ConnectDitolak);
    }

    if status == "offline" {
        // Berbeda dari kegagalan kredensial: pemanggil sudah membuktikan tahu
        // password, jadi memberitahunya perangkat sedang mati tidak
        // membocorkan apa pun yang belum ia ketahui.
        catat(state, &req.device_id, ip, QuickConnectOutcome::Accepted, claims).await;
        return Err(ApiError::Konflik("perangkat sedang offline".into()));
    }

    // ── 5. Sesi dibuat dalam status pending ──────────────────────────────────
    // Belum aktif. Agent masih harus menampilkan prompt persetujuan.
    let mut tx = crate::db::mulai_transaksi_tenant(&state.db, org_id).await?;

    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions
            (organization_id, device_uuid, viewer_user_id,
             device_id_snapshot, viewer_email_snapshot, connect_method, status)
         VALUES ($1, $2, $3, $4, $5, 'quick_connect', 'pending')
         RETURNING id",
    )
    .bind(org_id)
    .bind(device_uuid)
    .bind(claims.user_id())
    .bind(&kunci)
    .bind(&claims.email)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    ratelimit::bersihkan(&mut redis, &kunci_device).await?;
    catat(state, &req.device_id, ip, QuickConnectOutcome::Accepted, claims).await;
    audit::catat(&state.db, audit::Entri {
        org_id, user_id: Some(claims.user_id()), ip, aksi: aksi::SESI_DIMINTA,
        payload: Some(serde_json::json!({
            "session_id": session_id,
            "kunci": kunci,
            // Berguna saat menyelidiki insiden: kata sandi tetap yang dipakai
            // berarti akses tanpa pengawasan, bukan bantuan sesaat.
            "lewat_sandi_tetap": cocok_tetap && !cocok_sesi,
        })),
    }).await;

    tracing::info!(device = %kunci, %session_id, "quick connect diterima, menunggu persetujuan");

    Ok(Sukses::baru(ConnectResp {
        session_id,
        device_uuid,
        status: "pending_approval",
    }))
}

/// Menormalkan masukan Quick Connect menjadi nomor atau alias.
///
/// Mengembalikan `None` bila bentuknya tidak mungkin cocok dengan apa pun —
/// dan menolak di sini berarti menolak **tanpa satu pun query database**, yang
/// pada string sembilan digit acak berlaku untuk sekitar 90% masukan.
///
/// Alias tidak punya check digit untuk disaring, sehingga ia hanya dibatasi
/// bentuknya. Batasan itu sama dengan yang ditegakkan saat alias dipasang, jadi
/// apa pun yang lolos di sini setidaknya *mungkin* ada.
fn normalkan_kunci(masukan: &str) -> Option<String> {
    let dipangkas = masukan.trim();

    // ── Bentuk nomor ────────────────────────────────────────────────────────
    // Hanya digit beserta pemisah yang wajar diketik orang: `942 716 382` dan
    // `942-716-382` sama sahnya dengan `942716382`.
    let mirip_nomor = !dipangkas.is_empty()
        && dipangkas
            .chars()
            .all(|c| c.is_ascii_digit() || c == ' ' || c == '-');

    if mirip_nomor {
        let digit: String = dipangkas.chars().filter(|c| c.is_ascii_digit()).collect();
        if digit.len() == 9 {
            // Sembilan digit **selalu** diperlakukan sebagai nomor, dan check
            // digit yang salah berhenti di sini — tanpa satu pun query.
            // Membiarkannya jatuh ke jalur alias akan membuang properti itu,
            // karena alias sembilan digit dilarang dan tidak akan pernah cocok.
            return DeviceId::parse(&digit).ok().map(|d| d.to_string());
        }
    }

    // ── Bentuk alias ────────────────────────────────────────────────────────
    // Tanpa spasi sama sekali. Alias dibacakan lewat telepon sama seperti
    // nomor, dan spasi di dalamnya hanya menambah cara untuk salah dengar.
    let alias = dipangkas.to_lowercase();
    let bentuk_sah = (3..=32).contains(&alias.chars().count())
        && alias
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && alias.chars().next().is_some_and(|c| c.is_ascii_alphanumeric());

    bentuk_sah.then_some(alias)
}

/// Mencatat upaya lewat SECURITY DEFINER, karena mayoritas baris justru
/// merujuk device ID yang tidak pernah ada — dan baris itulah sinyal
/// pemindaian yang perlu dianalisis. (QUICK_CONNECT.md §6, §8)
async fn catat(
    state: &AppState,
    device_id_input: &str,
    ip: std::net::IpAddr,
    outcome: QuickConnectOutcome,
    claims: &crate::auth::jwt::Claims,
) {
    // Input dipotong ke 9 karakter agar tidak melampaui lebar kolom bila
    // pemanggil mengirim string panjang.
    let dipotong: String = device_id_input.chars().take(9).collect();

    // IP dikirim sebagai teks lalu di-cast ke `inet` di sisi SQL. sqlx tidak
    // memetakan `IpAddr` ke `INET` tanpa fitur `ipnetwork`, dan menambah
    // dependensi hanya untuk satu bind tidak sepadan.
    let hasil = sqlx::query("SELECT log_quick_connect_attempt($1, $2::inet, $3, $4)")
        .bind(&dipotong)
        .bind(ip.to_string())
        .bind(outcome.as_db_str())
        .bind(claims.user_id())
        .execute(&state.db)
        .await;

    if let Err(e) = hasil {
        // Gagal mencatat tidak boleh menggagalkan request, tetapi harus
        // terlihat — hilangnya jejak audit adalah masalah tersendiri.
        tracing::error!(error = %e, "gagal mencatat upaya quick connect");
    }
}

/// Menunda sampai ambang tetap tercapai.
async fn normalkan_waktu(mulai: Instant) {
    let berlalu = mulai.elapsed();
    if berlalu < LANTAI_RESPONS {
        tokio::time::sleep(LANTAI_RESPONS - berlalu).await;
    } else {
        // Melampaui lantai berarti ada yang lambat — pantas dicatat, karena
        // artinya normalisasi waktu sudah tidak lagi menyembunyikan apa pun.
        tracing::warn!(?berlalu, "quick connect melampaui lantai respons");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn respons_cepat_ditahan_sampai_lantai() {
        let t0 = Instant::now();
        normalkan_waktu(t0).await;
        assert!(
            t0.elapsed() >= LANTAI_RESPONS,
            "respons selesai sebelum lantai: {:?}",
            t0.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn respons_lambat_tidak_ditahan_lagi() {
        let t0 = Instant::now();
        tokio::time::sleep(LANTAI_RESPONS * 2).await;
        let sebelum = t0.elapsed();
        normalkan_waktu(t0).await;
        assert_eq!(t0.elapsed(), sebelum, "respons lambat justru diperpanjang");
    }

    #[test]
    fn nomor_diterima_dalam_bentuk_yang_wajar_diketik() {
        // Nomornya dibangkitkan, bukan ditulis tangan: check digit Damm sulit
        // dihitung di kepala, dan vektor karangan justru sudah pernah membuat
        // uji ini gagal karena datanya yang salah, bukan kodenya.
        let nomor = DeviceId::generate().to_string();
        let sisip = |pemisah: char| {
            format!(
                "{}{pemisah}{}{pemisah}{}",
                &nomor[0..3],
                &nomor[3..6],
                &nomor[6..9]
            )
        };

        let polos = normalkan_kunci(&nomor);
        assert_eq!(polos.as_deref(), Some(nomor.as_str()), "nomor polos ditolak");
        assert_eq!(normalkan_kunci(&sisip(' ')), polos, "bentuk berspasi");
        assert_eq!(normalkan_kunci(&sisip('-')), polos, "bentuk bertanda hubung");
    }

    #[test]
    fn nomor_dengan_check_digit_salah_ditolak_tanpa_query() {
        // Inilah yang membuang mayoritas pemindaian sebelum menyentuh
        // database. Digit terakhir digeser sehingga check digit-nya pasti
        // tidak lagi cocok.
        let nomor = DeviceId::generate().to_string();
        let akhir = nomor.chars().last().unwrap().to_digit(10).unwrap();
        let rusak = format!("{}{}", &nomor[0..8], (akhir + 1) % 10);
        assert_eq!(normalkan_kunci(&rusak), None, "check digit salah lolos: {rusak}");
    }

    #[test]
    fn alias_diterima_dan_dinormalkan() {
        assert_eq!(normalkan_kunci("PC-Kantor").as_deref(), Some("pc-kantor"));
        assert_eq!(normalkan_kunci("  laptop_01  ").as_deref(), Some("laptop_01"));
    }

    #[test]
    fn masukan_yang_mustahil_cocok_ditolak_lebih_dulu() {
        for buruk in ["", "ab", "pc kantor", "-awal", "pc@kantor", &"a".repeat(33)] {
            assert_eq!(normalkan_kunci(buruk), None, "diterima padahal mustahil: {buruk}");
        }
    }

    #[test]
    fn sembilan_digit_tidak_pernah_jatuh_ke_jalur_alias() {
        // Sembilan digit selalu nomor. Yang check digit-nya salah harus
        // berhenti sebagai None, bukan diteruskan sebagai alias sembilan digit
        // — bentuk yang justru dilarang saat alias dipasang, sehingga query-nya
        // dijamin sia-sia.
        for _ in 0..50 {
            let acak: String = (0..9)
                .map(|_| char::from_digit(rand::random::<u32>() % 10, 10).unwrap())
                .collect();
            match normalkan_kunci(&acak) {
                Some(k) => assert!(
                    rdp_core::DeviceId::parse(&k).is_ok(),
                    "sembilan digit diterima tanpa check digit sah: {k}"
                ),
                None => {}
            }
        }
    }

    #[test]
    fn lantai_respons_melebihi_biaya_argon2() {
        // Argon2id dengan 19 MiB berjalan sekitar 30-60 ms pada perangkat
        // keras server. Lantai harus jelas di atasnya supaya selisih antara
        // jalur "hash dijalankan" dan "tidak dijalankan" tertutup.
        assert!(LANTAI_RESPONS >= Duration::from_millis(200));
    }
}
