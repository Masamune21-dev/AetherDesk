//! Password sesi sekali pakai untuk alur Quick Connect.
//!
//! Device ID adalah alamat; **password inilah satu-satunya rahasia**.
//! Parameter di sini mengikuti QUICK_CONNECT.md §3.

use rand::Rng;

/// Alfabet 32 simbol.
///
/// Membuang empat karakter yang paling sering tertukar secara visual:
/// `0`/`O` dan `1`/`I`. Huruf `L` **dipertahankan** — kerancuan `l`/`1` hanya
/// muncul pada huruf kecil, sementara password ini selalu ditampilkan dan
/// dinormalkan ke huruf besar.
///
/// Membuang kelimanya (`0`, `1`, `I`, `O`, `L`) akan menyisakan 31 simbol dan
/// entropinya menjadi 39,6 bit — angka ganjil yang tidak sepadan dengan
/// manfaatnya. Dengan 32 simbol, entropi tepat 40 bit.
pub const ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Panjang password sesi.
pub const LEN: usize = 8;

/// Entropi dalam bit: `log2(32^8)` = 40.
pub const ENTROPY_BITS: u32 = 40;

/// Membangkitkan password sesi baru.
///
/// Sampling dilakukan lewat `gen_range` pada panjang alfabet, bukan
/// `byte % 32`. Kebetulan 32 membagi 256 dengan rapi sehingga modulo tidak
/// akan bias di sini, tetapi menuliskannya secara eksplisit membuat kode
/// tetap benar bila panjang alfabet berubah suatu saat.
pub fn generate() -> String {
    let mut rng = rand::thread_rng();
    (0..LEN)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Menormalkan input pengguna sebelum verifikasi.
///
/// Pengguna mengetik apa yang mereka dengar, jadi huruf kecil dan spasi
/// harus dimaafkan. Karakter yang tidak ada di alfabet dibiarkan lewat
/// supaya verifikasi tetap gagal, bukan diam-diam "diperbaiki" menjadi
/// password lain.
pub fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn alfabet_berukuran_32_dan_tanpa_karakter_ambigu() {
        assert_eq!(ALPHABET.len(), 32, "32 simbol memberi entropi tepat 40 bit");
        for c in b"01OI" {
            assert!(!ALPHABET.contains(c), "karakter ambigu {} ada", *c as char);
        }
        // L sengaja dipertahankan; lihat dokumentasi ALPHABET.
        assert!(ALPHABET.contains(&b'L'));
    }

    #[test]
    fn alfabet_hanya_huruf_besar_dan_digit() {
        assert!(ALPHABET
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()));
    }

    #[test]
    fn alfabet_tanpa_duplikat() {
        let unik: HashSet<_> = ALPHABET.iter().collect();
        assert_eq!(unik.len(), ALPHABET.len());
    }

    #[test]
    fn generate_panjang_dan_alfabet_benar() {
        for _ in 0..1000 {
            let p = generate();
            assert_eq!(p.len(), LEN);
            assert!(p.bytes().all(|b| ALPHABET.contains(&b)), "keluar alfabet: {p}");
        }
    }

    #[test]
    fn generate_tidak_berulang_dalam_sampel_wajar() {
        // 2^40 ruang; 500 sampel seharusnya seluruhnya unik.
        let set: HashSet<String> = (0..500).map(|_| generate()).collect();
        assert_eq!(set.len(), 500, "ada tabrakan pada sampel kecil");
    }

    #[test]
    fn seluruh_alfabet_terpakai_pada_sampel_besar() {
        let mut terlihat = HashSet::new();
        for _ in 0..3000 {
            terlihat.extend(generate().bytes());
        }
        assert_eq!(
            terlihat.len(),
            ALPHABET.len(),
            "ada simbol yang tidak pernah muncul — indikasi sampling bias"
        );
    }

    #[test]
    fn normalize_memaafkan_format_ketikan() {
        assert_eq!(normalize("a2b3 c4d5"), "A2B3C4D5");
        assert_eq!(normalize("A2B3-C4D5"), "A2B3C4D5");
    }

    #[test]
    fn normalize_tidak_memperbaiki_karakter_ambigu() {
        // '0' tidak boleh diam-diam menjadi 'O' — verifikasi harus gagal.
        assert_eq!(normalize("0OIL1"), "0OIL1");
    }

    #[test]
    fn entropi_konsisten_dengan_parameter() {
        let hitung = (ALPHABET.len() as f64).log2() * LEN as f64;
        assert_eq!(hitung as u32, ENTROPY_BITS);
    }
}
