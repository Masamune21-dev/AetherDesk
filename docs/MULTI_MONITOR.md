# Multi-Monitor Management Design

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Monitor Detection & Topology

Agent mendeteksi topologi monitor remote menggunakan API native:
- **Windows**: `EnumDisplayMonitors` untuk mendapatkan koordinat virtual screen space, bounding boxes, dan status primary monitor.
- **macOS**: `CGGetActiveDisplayList` untuk mendeteksi layar fisik aktif.

Data topologi dikirim ke Viewer sebagai `MONITOR_LAYOUT` packet:
```json
{
  "monitors": [
    {
      "id": 0,
      "name": "Display 1 (Primary)",
      "x": 0,
      "y": 0,
      "width": 1920,
      "height": 1080,
      "is_primary": true
    },
    {
      "id": 1,
      "name": "Display 2",
      "x": 1920,
      "y": 0,
      "width": 1440,
      "height": 900,
      "is_primary": false
    }
  ]
}
```

---

## 2. Rendering & Windowing Modes (Viewer)

Viewer mendukung tiga mode tampilan multi-monitor:

### 2.1 Tabbed View
User dapat memilih satu monitor dari dropdown di toolbar. Monitor dirender penuh di canvas. Monitor lain ditampilkan sebagai thumbnail live kecil di panel bawah.

### 2.2 Combined View (Span Mode)
Satu canvas besar yang menggabungkan seluruh monitor berdasarkan layout koordinat aslinya. Berguna jika viewer memiliki monitor ultra-wide.

### 2.3 Independent Windows (Multi-Window Mode)
Tauri viewer men-spawn window OS baru per monitor remote:
- Sesi WebRTC yang sama membagi multi-stream video tracks.
- Setiap track dirender ke window Tauri terpisah.
- Drag-and-drop file/input dipetakan secara akurat berdasarkan bounding box monitor aktif.

---

## 3. Monitor Hot-Plug Handling

- **Detection**: Saat monitor remote ditambahkan atau dilepas, OS me-trigger event (`WM_DISPLAYCHANGE` di Windows atau `CGDisplayRegisterReconfigurationCallback` di macOS).
- **Sync**: Agent mem-push `MONITOR_LAYOUT` terbaru ke Viewer.
- **Tauri Adjustment**: Viewer menutup window monitor yang dilepas atau membuka window baru jika monitor ditambahkan, menyesuaikan layout canvas rendering secara dinamis.
