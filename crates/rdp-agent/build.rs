//! Menyematkan metadata versi dan manifest ke dalam biner Windows.
//!
//! Tanpa ini, `rdp-agent.exe` muncul di Properties tanpa nama produk, tanpa
//! versi, dan tanpa penerbit — dan dialog SmartScreen menyebutnya "Unknown
//! publisher" atas berkas yang bahkan tidak punya deskripsi.
//!
//! Metadata tidak menghilangkan peringatan SmartScreen; hanya tanda tangan
//! kode yang dapat melakukannya. Yang ia ubah adalah apa yang dibaca orang
//! ketika peringatan itu muncul: program yang menyebutkan dirinya, bukan
//! berkas anonim.

fn main() {
    // Menjalankan ulang hanya bila yang disematkan benar-benar berubah.
    println!("cargo:rerun-if-changed=aetherdesk.manifest");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    sematkan();
}

#[cfg(windows)]
fn sematkan() {
    let versi = env!("CARGO_PKG_VERSION");

    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "AetherDesk")
        .set("FileDescription", "AetherDesk — agent remote desktop")
        .set("CompanyName", "Masamune")
        .set("LegalCopyright", "© 2026 Masamune")
        .set("OriginalFilename", "rdp-agent.exe")
        .set("InternalName", "rdp-agent")
        .set("ProductVersion", versi)
        .set("FileVersion", versi);

    res.set_manifest_file("aetherdesk.manifest");

    // Kegagalan menyematkan tidak boleh menggagalkan build.
    //
    // `rc.exe` datang bersama Windows SDK, dan bukan mustahil seseorang
    // membangun crate ini pada mesin yang hanya punya toolchain Rust. Biner
    // tanpa metadata tetap berjalan sepenuhnya benar — yang hilang hanya
    // keterangan di Properties.
    if let Err(e) = res.compile() {
        println!("cargo:warning=metadata versi tidak tersematkan: {e}");
    }
}
