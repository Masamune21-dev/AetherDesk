<#
.SYNOPSIS
    Menandatangani rdp-agent.exe dengan sertifikat swa-tanda.

.DESCRIPTION
    Sertifikat swa-tanda **tidak** menghilangkan peringatan SmartScreen bagi
    orang asing — hanya sertifikat dari CA tepercaya yang dapat melakukannya.
    Yang ia berikan ada tiga, dan ketiganya nyata:

      1. Pada komputer yang memercayai sertifikat ini, peringatan hilang
         sepenuhnya. Cocok untuk armada mesin milik sendiri atau satu
         organisasi.
      2. Nama penerbit muncul menggantikan "Unknown publisher".
      3. Berkas menjadi anti-rusak: satu byte berubah membuat tanda tangannya
         batal, sehingga penerima dapat memastikan yang ia jalankan persis
         yang Anda kirim.

    Kuncinya tinggal di penyimpanan sertifikat pengguna, tidak pernah menjadi
    berkas di disk. Yang diekspor hanya bagian publiknya.

.PARAMETER Exe
    Berkas yang ditandatangani. Baku: target\release\rdp-agent.exe.

.PARAMETER Nama
    Nama penerbit yang akan terlihat pengguna.

.PARAMETER Tahun
    Masa berlaku sertifikat.

.PARAMETER Baru
    Buat sertifikat baru meskipun sudah ada.

.EXAMPLE
    .\tanda-tangani.ps1
#>

[CmdletBinding()]
param(
    [string]$Exe,
    [string]$Nama = 'Masamune',
    [int]$Tahun = 5,
    [switch]$Baru
)

$ErrorActionPreference = 'Stop'
function Tulis($t, $w = 'Gray') { Write-Host "  $t" -ForegroundColor $w }

if (-not $Exe) {
    $Exe = Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\rdp-agent.exe'
}
if (-not (Test-Path $Exe)) {
    Write-Host "`n  Tidak ditemukan: $Exe`n" -ForegroundColor Red
    exit 1
}

$subjek = "CN=$Nama, O=$Nama, C=ID"
Write-Host "`nMenandatangani AetherDesk`n" -ForegroundColor Cyan

# ── Sertifikat ───────────────────────────────────────────────────────────────
# Dicari lebih dulu, bukan langsung dibuat. Membuat sertifikat baru setiap kali
# berarti setiap rilis ditandatangani penerbit yang berbeda di mata Windows,
# dan mesin yang sudah memercayai yang lama harus memercayai lagi yang baru.
$sertifikat = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq $subjek -and $_.NotAfter -gt (Get-Date).AddDays(30) } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1

if ($Baru -or -not $sertifikat) {
    Tulis "membuat sertifikat penandatanganan baru"
    $sertifikat = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $subjek `
        -KeyUsage DigitalSignature `
        -KeyExportPolicy NonExportable `
        -KeyAlgorithm RSA `
        -KeyLength 3072 `
        -HashAlgorithm SHA256 `
        -CertStoreLocation Cert:\CurrentUser\My `
        -NotAfter (Get-Date).AddYears($Tahun)
    Tulis "sidik jari $($sertifikat.Thumbprint)" 'Green'
} else {
    Tulis "memakai sertifikat yang sudah ada"
    Tulis "sidik jari $($sertifikat.Thumbprint)" 'Green'
}

# ── Menandatangani ───────────────────────────────────────────────────────────
# Stempel waktu membuat tanda tangan tetap sah setelah sertifikatnya
# kedaluwarsa — tanpa itu, berkas yang ditandatangani hari ini menjadi "tidak
# sah" begitu masa berlakunya habis, meskipun berkasnya tidak berubah.
$stempel = 'http://timestamp.digicert.com'
$hasil = $null
try {
    $hasil = Set-AuthenticodeSignature -FilePath $Exe -Certificate $sertifikat `
        -HashAlgorithm SHA256 -TimestampServer $stempel -ErrorAction Stop
} catch {
    Tulis "server stempel waktu tidak terjangkau, menandatangani tanpa stempel" 'Yellow'
    $hasil = Set-AuthenticodeSignature -FilePath $Exe -Certificate $sertifikat -HashAlgorithm SHA256
}

# Status `UnknownError` dengan pesan "root certificate which is not trusted"
# **bukan** kegagalan. Tanda tangannya sudah terpasang; yang gagal adalah
# memverifikasinya, karena akar swa-tanda ini memang belum dipercaya mesin
# mana pun — termasuk mesin yang baru saja membuatnya. Itu persis keadaan yang
# diperbaiki percayai-sertifikat.ps1.
#
# Memperlakukannya sebagai galat akan membuat skrip ini selalu gagal pada
# pemakaian pertama, padahal hasilnya benar.
$belum_dipercaya = $hasil.StatusMessage -match 'not trusted'

if ($hasil.Status -ne 'Valid' -and -not $belum_dipercaya) {
    Write-Host "`n  Gagal: $($hasil.StatusMessage)`n" -ForegroundColor Red
    exit 1
}

$tt = Get-AuthenticodeSignature -FilePath $Exe
if (-not $tt.SignerCertificate) {
    Write-Host "`n  Tanda tangan tidak terpasang.`n" -ForegroundColor Red
    exit 1
}

Tulis "berkas ditandatangani" 'Green'
if ($tt.TimeStamperCertificate) {
    Tulis "stempel waktu terpasang — tanda tangan tetap sah setelah sertifikat kedaluwarsa" 'Green'
}

# ── Mengekspor bagian publik ─────────────────────────────────────────────────
$cer = Join-Path (Split-Path $Exe -Parent) 'aetherdesk-penerbit.cer'
Export-Certificate -Cert $sertifikat -FilePath $cer -Type CERT | Out-Null
Tulis "sertifikat publik: $cer" 'Green'

Write-Host ""
Write-Host "  Berikutnya" -ForegroundColor Cyan
Write-Host "    Kirim $($cer | Split-Path -Leaf) bersama rdp-agent.exe, lalu di komputer"
Write-Host "    tujuan jalankan sekali sebagai administrator:"
Write-Host ""
Write-Host "      .\percayai-sertifikat.ps1 -Sertifikat .\aetherdesk-penerbit.cer" -ForegroundColor White
Write-Host ""
Write-Host "  Setelah itu Windows berhenti memperingatkan di mesin tersebut." -ForegroundColor Gray
Write-Host "  Pada komputer yang tidak memercayai sertifikat ini, peringatan" -ForegroundColor Gray
Write-Host "  tetap muncul sekali — hanya CA tepercaya yang dapat menghapusnya." -ForegroundColor Gray
Write-Host ""
