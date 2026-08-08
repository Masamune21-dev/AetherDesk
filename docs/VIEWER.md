# UI Component Spec & Layout Guidelines

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Multi-Window & Floating Toolbar UI

Tauri Viewer dirancang dengan tata letak minimalis dan fungsional untuk mengoptimalkan area visual remote desktop.

```
┌────────────────────────────────────────────────────────┐
│ [Tab 1: Server-01] [Tab 2: Win-Client-03] [+] [ _ [] X]│
├────────────────────────────────────────────────────────┤
│                                                        │
│                  Live Canvas Render Area               │
│                                                        │
│                 ┌───────────────────────┐              │
│                 │ ≡   ⚙   📁   🎤   ▶   X  │              │  ◄── Floating Toolbar
│                 └───────────────────────┘              │
│                                                        │
│                                                        │
└────────────────────────────────────────────────────────┘
```

### 1.1 Floating Toolbar Actions
Toolbar melayang di area tengah atas layar, dapat di-hide, pin, atau di-drag ke sisi lain:
- **Menu (≡)**: Screen scaling options, monitor switcher, view statistics.
- **Settings (⚙)**: Keyboard shortcuts configuration, audio setup, session parameters.
- **File Manager (📁)**: Buka dual-panel interface transfer berkas.
- **Intercom (🎤)**: Aktifkan microphone passthrough & voice call.
- **Record (▶)**: Start/stop session recording lokal.
- **Disconnect (X)**: Tutup sesi remote saat ini.

---

## 2. Advanced Performance Overlays

Untuk IT administrator dan gamers, statistik performa ditampilkan sebagai HUD overlay semi-transparan di pojok kanan atas layar:

```
┌──────────────────────────────────────────┐
│ SESSION STATISTICS                       │
├──────────────────────────────────────────┤
│ Connection Type: WebRTC P2P (UDP)        │
│ Latency: 12 ms      Packet Loss: 0.0%    │
│ Bitrate: 4.2 Mbps   Codec: AV1           │
│ FPS: 60 / 60        Resolution: 1080p    │
│ CPU Agent: 4%       CPU Viewer: 3%       │
└──────────────────────────────────────────┘
```

Overlay ini mengambil data internal WebRTC stats API (`getStats()`) setiap 1 detik.
