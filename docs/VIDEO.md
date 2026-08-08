# Remote Video & Camera Sharing Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Arsitektur Video/Camera Sharing

Fitur Video Sharing memungkinkan kamera lokal (Viewer webcam) dibagikan ke remote agent (untuk video call/support) atau sebaliknya (kamera remote agent dibagikan ke viewer).

---

## 2. Pipeline Capture & Driver Virtual

```
 Local Camera ──► Capture (AVFoundation/MSMF) ──► H.264 Encoder ──► WebRTC Video Track
                                                                            │
 Remote Applications ◄── Virtual Camera Driver ◄── H.264 Decoder ◄──────────┘
```

1. **Capture**: Menangkap stream webcam menggunakan platform API:
   - Windows: Media Foundation (MSMF) atau DirectShow.
   - macOS: AVFoundation (AVCaptureSession).
2. **Virtual Camera Driver**:
   - Windows: Indirect Display Driver atau custom virtual camera driver (WDM/AVStream).
   - macOS: CoreMediaIO Camera Extension (modern extension model).
   - Driver virtual menerima stream video terkompresi dari viewer dan mempresentasikannya sebagai webcam fisik ke OS remote, memungkinkan aplikasi seperti Zoom, Teams, atau browser di remote desktop mengakses webcam lokal.

---

## 3. Bandwidth & Quality Optimization

Kamera dioptimalkan secara terpisah dari streaming layar utama:
- **Low-priority channel**: Kamera diberikan prioritas bandwidth lebih rendah dibanding frame interaksi layar desktop.
- **Dynamic Resolution Scaling**:
  - Jaringan Baik: 720p (1280x720) @ 30fps.
  - Jaringan Terbatas: 360p (640x360) @ 15fps.
  - Jaringan Buruk: Pause webcam stream (hanya kirim static frame terakhir).
- **H.264 / AV1 Encoding**: Menggunakan hardware acceleration jika tersedia di platform.
