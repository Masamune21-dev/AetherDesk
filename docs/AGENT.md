# Remote Command & Shell Specification

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Remote Shell & PTY Architecture

Platform memungkinkan teknisi membuka shell terminal secara remote tanpa harus membuka visual streaming desktop utama. Fitur ini dirancang menggunakan arsitektur PTY (Pseudo-Terminal) berlatensi rendah.

```
  Viewer Terminal (Xterm.js) ──► WebSocket / DataChannel ──► Agent (Rust)
                                                                 │
  Viewer Output  ◄─────────── WebSocket / DataChannel ◄────── PTY Spawn (conpty/fork)
```

- **Windows**: Menggunakan ConPTY (Windows Pseudo-Console) API untuk meluncurkan `powershell.exe` atau `cmd.exe` secara native.
- **macOS / Linux**: Menggunakan standard Unix `forkpty` API untuk men-spawn `/bin/zsh` atau `/bin/bash`.

---

## 2. Hardware & Software Inventory Collection

Agent memiliki modul berkala yang memindai inventaris sistem operasi dan mengirimkannya ke database:

### 2.1 Hardware Inventory (System Profiler)
Agent mengumpulkan data hardware:
- **CPU**: Model, arsitektur, jumlah core, clock speed (via `sysinfo` crate).
- **RAM**: Kapasitas total, sisa, clock speed (via WMI/sysctl).
- **Disk**: Daftar partisi, tipe media (SSD/HDD), sisa kapasitas.
- **GPU**: Model, kapasitas VRAM, versi driver GPU (via DXGI/Metal API).
- **Network**: MAC address, interfaces list, local IP addresses.

### 2.2 Software Inventory
- **Windows**: Membaca registry key uninstall hive:
  `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall` dan versi 32-bit di WOW6432Node.
- **macOS**: Memindai folder `/Applications/` dan query system profiler.

---

## 3. Remote Task Manager & System Controls

Teknisi dapat memantau dan memanipulasi resource target secara real-time via Web Dashboard:

- **Process Manager**: Mengirimkan live process list (CPU/RAM per PID). Teknisi dapat mengirim perintah kill process (Windows `TerminateProcess` / Unix `SIGKILL`).
- **Service Manager**: Melihat daftar service sistem operasi, mengubah start type (Automatic, Manual, Disabled), dan mengubah state (Start, Stop, Restart).
- **Registry Editor (Windows)**: CRUD registry key dan value untuk troubleshooting tingkat dalam secara remote tanpa memerlukan GUI desktop sharing.
