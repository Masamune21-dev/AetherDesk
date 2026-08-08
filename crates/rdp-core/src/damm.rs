//! Check digit Damm untuk device ID.
//!
//! Dipilih di [`QUICK_CONNECT.md`] karena menangkap **seluruh** kesalahan satu
//! digit dan **seluruh** transposisi dua digit bersebelahan — dua kesalahan
//! paling umum saat seseorang mendiktekan angka lewat telepon.
//!
//! Efek sampingnya di sisi keamanan sama pentingnya: 90% string sembilan digit
//! adalah ID yang tidak valid secara struktur, sehingga dapat ditolak sebelum
//! menyentuh database. Pemindaian buta ruang ID menjadi sepuluh kali lebih mahal
//! dan sepuluh kali lebih mudah terlihat.
//!
//! [`QUICK_CONNECT.md`]: ../../../docs/QUICK_CONNECT.md

/// Quasigroup anti-simetrik total berordo 10.
///
/// Baris = nilai interim berjalan, kolom = digit yang sedang diproses.
/// Diagonalnya nol, yang membuat digit terulang tidak mengubah state —
/// properti inilah yang memberi jaminan deteksi transposisi.
const TABLE: [[u8; 10]; 10] = [
    [0, 3, 1, 7, 5, 9, 8, 6, 4, 2],
    [7, 0, 9, 2, 1, 5, 4, 8, 6, 3],
    [4, 2, 0, 6, 8, 7, 1, 3, 5, 9],
    [1, 7, 5, 0, 9, 8, 3, 4, 2, 6],
    [6, 1, 2, 3, 0, 4, 5, 9, 7, 8],
    [3, 6, 7, 4, 2, 0, 9, 5, 8, 1],
    [5, 8, 6, 9, 7, 2, 0, 1, 3, 4],
    [8, 9, 4, 5, 3, 6, 2, 0, 1, 7],
    [9, 4, 3, 8, 6, 1, 7, 2, 0, 5],
    [2, 5, 8, 1, 4, 3, 6, 7, 9, 0],
];

/// Menjalankan digit melalui quasigroup dan mengembalikan interim akhir.
///
/// Mengembalikan `None` bila ada byte yang bukan digit ASCII.
fn interim(digits: &[u8]) -> Option<u8> {
    let mut acc = 0u8;
    for &b in digits {
        let d = (b as char).to_digit(10)? as usize;
        acc = TABLE[acc as usize][d];
    }
    Some(acc)
}

/// Menghitung check digit untuk sebuah payload.
///
/// ```
/// use rdp_core::damm;
/// assert_eq!(damm::check_digit(b"572"), Some(4));
/// ```
pub fn check_digit(payload: &[u8]) -> Option<u8> {
    interim(payload)
}

/// Memvalidasi string lengkap yang check digit-nya ada di posisi terakhir.
///
/// ```
/// use rdp_core::damm;
/// assert!(damm::is_valid(b"5724"));
/// assert!(!damm::is_valid(b"5721"));
/// ```
pub fn is_valid(full: &[u8]) -> bool {
    interim(full) == Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ID sembilan digit yang valid. Check digit `2` adalah hasil perhitungan
    /// Damm atas payload `94271638` — dihitung manual supaya test tidak
    /// bergantung pada fungsi yang sedang diuji.
    const FIXTURE: &[u8; 9] = b"942716382";

    #[test]
    fn fixture_valid() {
        assert!(is_valid(FIXTURE), "fixture harus valid lebih dulu");
        assert_eq!(check_digit(&FIXTURE[..8]), Some(2));
    }

    #[test]
    fn check_digit_menutup_string() {
        for payload in [b"0".as_slice(), b"9", b"572", b"12345678", b"00000000"] {
            let cd = check_digit(payload).unwrap();
            let mut full = payload.to_vec();
            full.push(b'0' + cd);
            assert!(is_valid(&full), "gagal untuk payload {payload:?}");
        }
    }

    #[test]
    fn menangkap_semua_kesalahan_satu_digit() {
        // Untuk setiap posisi dan setiap penggantian digit, hasilnya harus invalid.
        let base = FIXTURE;
        assert!(is_valid(base), "fixture harus valid lebih dulu");

        for pos in 0..base.len() {
            for d in b'0'..=b'9' {
                if d == base[pos] {
                    continue;
                }
                let mut m = base.to_vec();
                m[pos] = d;
                assert!(!is_valid(&m), "kesalahan satu digit lolos: {m:?}");
            }
        }
    }

    #[test]
    fn menangkap_semua_transposisi_bersebelahan() {
        let base = FIXTURE;
        for pos in 0..base.len() - 1 {
            if base[pos] == base[pos + 1] {
                continue; // menukar digit identik bukan kesalahan
            }
            let mut m = base.to_vec();
            m.swap(pos, pos + 1);
            assert!(!is_valid(&m), "transposisi lolos di posisi {pos}: {m:?}");
        }
    }

    #[test]
    fn menolak_karakter_bukan_digit() {
        assert_eq!(check_digit(b"12a4"), None);
        assert!(!is_valid(b"12a4"));
    }

    #[test]
    fn diagonal_tabel_nol() {
        // Properti yang menjamin deteksi transposisi.
        for i in 0..10 {
            assert_eq!(TABLE[i][i], 0, "diagonal tidak nol di baris {i}");
        }
    }

    #[test]
    fn tabel_adalah_latin_square() {
        for i in 0..10 {
            let mut baris = [false; 10];
            let mut kolom = [false; 10];
            for j in 0..10 {
                baris[TABLE[i][j] as usize] = true;
                kolom[TABLE[j][i] as usize] = true;
            }
            assert!(baris.iter().all(|&x| x), "baris {i} bukan permutasi");
            assert!(kolom.iter().all(|&x| x), "kolom {i} bukan permutasi");
        }
    }
}
