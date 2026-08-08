# Advanced Enterprise Feature Integrations

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Wake-on-LAN (WoL) & Power Management

WoL memungkinkan administrator menyalakan komputer remote yang dalam kondisi mati atau sleep (S3/S4/S5 state).

```
   Viewer (Dashboard)
           │
           ▼
     [WoL Request] ──► NATS JetStream ──► Signal Server
                                                │
                                                ▼
                                         Proxy Agent Pod
                                                │
                                                ▼ (Subnet Broadcast)
                                         [Magic Packet]
                                                │ (UDP Port 9 / MAC Address)
                                                ▼
                                        Target Device Boot
```

### Magic Packet Structure
Paket UDP broadcast berisi 6-byte sync stream (`0xFF` sebanyak 6 kali) diikuti oleh MAC address target device yang diulang sebanyak 16 kali:
`[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, MAC*16]`.

---

## 2. Remote Power Operations & Safe Mode

### 2.1 Remote Reboot & Safe Mode (Windows)
Agent dapat me-reboot OS remote ke berbagai state:
- **Normal Reboot**: Menggunakan API `ExitWindowsEx(EWX_REBOOT, ...)` atau executing command `shutdown /r /t 0`.
- **Safe Mode Reboot**: Agent memodifikasi konfigurasi BCD (Boot Configuration Data) sebelum reboot agar sistem boot ke Safe Mode with Networking:
  ```cmd
  bcdedit /set {current} safeboot network
  ```
- **Revert to Normal**: Setelah pemeliharaan selesai, Agent membersihkan flag safeboot untuk boot normal selanjutnya:
  ```cmd
  bcdedit /deletevalue {current} safeboot
  ```

---

## 3. Remote Printing (Virtual Print Driver)

Memungkinkan pencetakan dokumen dari remote machine langsung ke printer lokal milik Viewer.

1. **Virtual Printer Driver**: Agent memasang virtual printer driver (e.g., PostScript printer emulator) di sistem operasi remote.
2. **Print Job Capture**: Saat pengguna mencetak dokumen di remote desktop, print job ditangkap oleh virtual printer sebagai file PDF/PostScript.
3. **Data Streaming**: Dokumen yang ter-render dikompresi (Zstd), dienkripsi, dan dikirim via WebRTC DataChannel (reliable channel) ke Viewer.
4. **Local Spooling**: Viewer menerima file, lalu mengirimkannya ke printer lokal fisik menggunakan OS print manager (Windows Spooler / CUPS di macOS).
