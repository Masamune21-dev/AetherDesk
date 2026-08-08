# Remote Audio Design Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Arsitektur Pipeline Audio

```
 Capturer (WASAPI loopback) ──► Opus Encoder ──► RTP Packetizer ──► WebRTC SRTP
                                                                         │
 Playback (cpal/OS output) ◄── Opus Decoder ◄── Jitter Buffer ◄──────────┘
```

---

## 2. Capture & Playback Interface

### 2.1 Windows (WASAPI Loopback)
- Menggunakan `IAudioClient` dalam mode loopback (`AUDCLNT_STREAMFLAGS_LOOPBACK`).
- Menangkap audio output dari default playback device.
- Format default: PCM 32-bit float (IEEE) atau 16-bit PCM, 48kHz, stereo.

### 2.2 macOS (CoreAudio / Audio Unit)
- Menggunakan virtual audio driver (CoreAudio HAL plug-in) atau `SCStreamConfiguration.capturesAudio` pada macOS 13+ (ScreenCaptureKit).
- Mengarahkan audio output sistem ke virtual input driver untuk ditangkap oleh agent.

---

## 3. Komparasi Audio Codec

| Fitur | Opus (Rekomendasi) | AAC | PCM (Raw) |
|---|---|---|---|
| **Latency** | Sangat Rendah (5 - 20ms) | Menengah (50 - 100ms) | Sangat Rendah (< 1ms) |
| **Bitrate** | 32 - 128 kbps (adaptive) | 96 - 256 kbps | 1.54 Mbps (uncompressed) |
| **Kualitas Suara**| Sangat Baik (untuk musik & vokal) | Sangat Baik (musik) | Lossless |
| **CPU Usage** | Rendah | Sedang | Sangat Rendah |
| **Kompresi** | Sangat Tinggi | Tinggi | Tidak ada |

Sistem menetapkan **Opus** sebagai codec utama untuk seluruh komunikasi media audio.

---

## 4. Microphone Passthrough & Echo Cancellation

Fitur interkom/voice call antara teknisi dan pengguna akhir:
- **Microphone Passthrough**: Mengirim audio mikrofon Viewer ke virtual microphone driver di Agent untuk digunakan pada remote apps.
- **AEC (Acoustic Echo Cancellation)** dan **NS (Noise Suppression)** diimplementasikan menggunakan pustaka **WebRTC Audio Processing (webrtc-audio-processing)** untuk menghilangkan gaung dan suara latar bising.
