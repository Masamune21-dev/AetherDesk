<#
.SYNOPSIS
    Memercayai sertifikat penerbit AetherDesk pada komputer ini.

.DESCRIPTION
    Menjalankan skrip ini berarti menyatakan: **apa pun** yang ditandatangani
    sertifikat ini akan dijalankan komputer ini tanpa pertanyaan, sekarang dan
    seterusnya.

    Itu keputusan yang serius, dan bukan sesuatu yang pantas dilakukan atas
    berkas yang datang dari orang yang tidak Anda kenal. Jalankan hanya bila
    sertifikatnya memang milik Anda sendiri, atau milik organisasi Anda.

    Sertifikatnya dipasang di dua tempat, dan keduanya diperlukan:

      Root             — menjadikan sertifikat swa-tanda ini sebagai akar
                         tepercaya, sehingga rantainya dapat diverifikasi
      TrustedPublisher — menghentikan peringatan SmartScreen dan dialog
                         "Open File - Security Warning"

    Memerlukan hak administrator karena keduanya milik mesin, bukan pengguna.

.PARAMETER Sertifikat
    Berkas .cer yang dihasilkan tanda-tangani.ps1.

.PARAMETER Hapus
    Cabut kepercayaan.

.EXAMPLE
    .\percayai-sertifikat.ps1 -Sertifikat .\aetherdesk-penerbit.cer
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Sertifikat,
    [switch]$Hapus
)

$ErrorActionPreference = 'Stop'
function Tulis($t, $w = 'Gray') { Write-Host "  $t" -ForegroundColor $w }

$admin = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $admin) {
    Write-Host "`n  Perlu dijalankan sebagai administrator." -ForegroundColor Red
    Write-Host "  Klik kanan PowerShell, pilih 'Run as administrator'.`n" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path $Sertifikat)) {
    Write-Host "`n  Tidak ditemukan: $Sertifikat`n" -ForegroundColor Red
    exit 1
}

$c = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2 `
    ((Resolve-Path $Sertifikat).Path)

$toko = @('Root', 'TrustedPublisher')

if ($Hapus) {
    Write-Host "`nMencabut kepercayaan`n" -ForegroundColor Cyan
    foreach ($t in $toko) {
        Get-ChildItem "Cert:\LocalMachine\$t" |
            Where-Object { $_.Thumbprint -eq $c.Thumbprint } |
            ForEach-Object { Remove-Item $_.PSPath -Force; Tulis "dicabut dari $t" 'Green' }
    }
    Write-Host ""
    exit 0
}

Write-Host "`nMemercayai penerbit`n" -ForegroundColor Cyan
Tulis "Penerbit  $($c.Subject)"
Tulis "Berlaku   sampai $($c.NotAfter.ToString('d MMMM yyyy'))"
Tulis "Sidik     $($c.Thumbprint)"
Write-Host ""

# Sidik jari ditampilkan supaya dapat dicocokkan dengan yang disebutkan
# penerbitnya lewat jalur lain. Sertifikat yang datang bersama berkas yang
# ditandatanganinya tidak membuktikan apa pun — penyerang yang mengganti
# binernya akan mengganti sertifikatnya sekalian.
Tulis "Cocokkan sidik jari itu dengan yang diberikan penerbitnya" 'Yellow'
Tulis "lewat jalur lain sebelum melanjutkan." 'Yellow'
Write-Host ""

$jawab = Read-Host "  Percayai sertifikat ini? (ketik: ya)"
if ($jawab -ne 'ya') {
    Write-Host "`n  Dibatalkan. Tidak ada yang diubah.`n" -ForegroundColor Yellow
    exit 0
}

foreach ($t in $toko) {
    $s = New-Object System.Security.Cryptography.X509Certificates.X509Store($t, 'LocalMachine')
    $s.Open('ReadWrite')
    $s.Add($c)
    $s.Close()
    Tulis "dipasang di $t" 'Green'
}

Write-Host ""
Write-Host "  Selesai. Berkas yang ditandatangani sertifikat ini kini berjalan" -ForegroundColor Gray
Write-Host "  tanpa peringatan di komputer ini." -ForegroundColor Gray
Write-Host ""
Write-Host "  Cabut kapan saja:" -ForegroundColor Gray
Write-Host "    .\percayai-sertifikat.ps1 -Sertifikat $Sertifikat -Hapus" -ForegroundColor White
Write-Host ""
