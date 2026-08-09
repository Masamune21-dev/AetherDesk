<#
.SYNOPSIS
    Memasang agent AetherDesk pada Windows.

.DESCRIPTION
    Menyalin rdp-agent.exe ke direktori aplikasi pengguna, membuat pintasan di
    menu Start, dan — bila diminta — mendaftarkannya agar berjalan saat masuk.

    Dipasang **per pengguna**, tanpa hak administrator. Itu bukan penyederhanaan
    melainkan keputusan: agent menangkap layar sesi interaktif seseorang, jadi
    ia hanya pantas hidup di sesi orang yang memang memasangnya. Pemasangan
    per-mesin akan menjalankannya untuk setiap orang yang masuk ke komputer itu,
    termasuk yang tidak pernah menyetujui apa pun.

    Peringatan SmartScreen wajar muncul: biner belum ditandatangani. Sejak Juni
    2023 sertifikat code signing mewajibkan kunci privat berada di perangkat
    keras bersertifikasi FIPS, dan itu belum diurus. Lihat temuan T-18.

.PARAMETER Sumber
    Lokasi rdp-agent.exe. Baku: berkas di sebelah skrip ini.

.PARAMETER Otomatis
    Daftarkan agar berjalan saat masuk ke Windows.

.PARAMETER IzinkanKendali
    Izinkan viewer menggerakkan mouse dan mengetik. Baku mati.

.PARAMETER Hapus
    Copot pemasangan.

.EXAMPLE
    .\pasang-windows.ps1 -Otomatis -IzinkanKendali
#>

[CmdletBinding()]
param(
    [string]$Sumber,
    [switch]$Otomatis,
    [switch]$IzinkanKendali,
    [switch]$Hapus
)

$ErrorActionPreference = 'Stop'

$Tujuan   = Join-Path $env:LOCALAPPDATA 'AetherDesk'
$Exe      = Join-Path $Tujuan 'rdp-agent.exe'
$Pintasan = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\AetherDesk.lnk'

function Tulis($teks, $warna = 'Gray') { Write-Host "  $teks" -ForegroundColor $warna }

# ── Copot ────────────────────────────────────────────────────────────────────
if ($Hapus) {
    Write-Host "`nMencopot AetherDesk`n" -ForegroundColor Cyan

    if (Test-Path $Exe) {
        # Agent yang sedang berjalan mengunci binernya sendiri; menghentikannya
        # lebih dulu adalah satu-satunya cara penghapusan berhasil.
        Get-Process -Name 'rdp-agent' -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $Exe } |
            ForEach-Object { Tulis "menghentikan agent yang berjalan"; Stop-Process -Id $_.Id -Force }
        Start-Sleep -Milliseconds 800
        & $Exe autostart --hapus 2>$null | Out-Null
    }

    Remove-Item $Pintasan -ErrorAction SilentlyContinue
    Remove-Item $Tujuan -Recurse -Force -ErrorAction SilentlyContinue
    Tulis "program dihapus" 'Green'

    # Identitas perangkat dan daftar kepercayaan sengaja **tidak** dihapus.
    # Memasang ulang lalu mendapati mesin ini masih dikenal jauh lebih berguna
    # daripada harus mendaftar ulang; yang ingin benar-benar bersih dapat
    # menghapusnya sendiri, dan lokasinya disebutkan.
    $identitas = Join-Path $env:APPDATA 'masamune\aetherdesk\config'
    if (Test-Path $identitas) {
        Write-Host ""
        Tulis "Identitas perangkat dibiarkan di:" 'Yellow'
        Tulis "  $identitas" 'Yellow'
        Tulis "Hapus sendiri bila memang ingin mendaftarkan ulang dari nol." 'Yellow'
    }
    Write-Host ""
    exit 0
}

# ── Pasang ───────────────────────────────────────────────────────────────────
Write-Host "`nMemasang AetherDesk`n" -ForegroundColor Cyan

if (-not $Sumber) { $Sumber = Join-Path $PSScriptRoot 'rdp-agent.exe' }
if (-not (Test-Path $Sumber)) {
    Write-Host "  rdp-agent.exe tidak ditemukan di:" -ForegroundColor Red
    Write-Host "    $Sumber" -ForegroundColor Red
    Write-Host "`n  Sebutkan lokasinya dengan -Sumber, atau letakkan berkasnya" -ForegroundColor Red
    Write-Host "  di sebelah skrip ini.`n" -ForegroundColor Red
    exit 1
}

Get-Process -Name 'rdp-agent' -ErrorAction SilentlyContinue |
    ForEach-Object { Tulis "menghentikan agent yang sedang berjalan"; Stop-Process -Id $_.Id -Force }
Start-Sleep -Milliseconds 800

New-Item -ItemType Directory -Force -Path $Tujuan | Out-Null
Copy-Item $Sumber $Exe -Force
Tulis "program disalin ke $Tujuan" 'Green'

$argumen = @('gui')
if ($IzinkanKendali) { $argumen += '--izinkan-kendali' }

$shell = New-Object -ComObject WScript.Shell
$lnk = $shell.CreateShortcut($Pintasan)
$lnk.TargetPath       = $Exe
$lnk.Arguments        = ($argumen | Select-Object -Skip 1) -join ' '
$lnk.WorkingDirectory = $Tujuan
$lnk.Description      = 'AetherDesk — remote desktop'
$lnk.Save()
Tulis "pintasan menu Start dibuat" 'Green'

if ($Otomatis) {
    $opsi = @('--pasang')
    if ($IzinkanKendali) { $opsi += '--izinkan-kendali' }
    & $Exe autostart @opsi | Out-Null
    Tulis "akan berjalan otomatis saat Anda masuk" 'Green'
}

Write-Host ""
if ($IzinkanKendali) {
    Write-Host "  KENDALI PENUH AKTIF" -ForegroundColor Yellow
    Write-Host "  Siapa pun yang memegang Device ID dan kata sandi dapat" -ForegroundColor Yellow
    Write-Host "  menggerakkan mouse dan mengetik di mesin ini." -ForegroundColor Yellow
    Write-Host ""
}

Write-Host "  Berikutnya:" -ForegroundColor Cyan
Write-Host "    1. Terbitkan token enrolment dari dashboard"
Write-Host "    2. `"$Exe`" enrol --token <TOKEN>"
Write-Host "    3. Jalankan AetherDesk dari menu Start"
Write-Host ""
