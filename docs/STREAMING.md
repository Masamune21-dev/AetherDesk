# Hardware Capabilities & GPU Acceleration Specification

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** Video/Streaming Engineer

---

## 1. Pipeline Input & Akselerasi GPU

```
 OS Capture Texture (VRAM) ──► GPU Color Convert (NV12) ──► HW Encode (VRAM NVENC)
                                                                    │
                                                                    ▼
 Display (Tauri Canvas)  ◄── GPU YUV->RGB Shader ◄── HW Decode ◄─ WebRTC UDP
```

Sistem meminimalkan memory transfers antara RAM (system memory) dan VRAM (graphics memory) untuk menjaga latency streaming tetap di bawah 16ms pada LAN.

---

## 2. Platform Capture Pipelines

### 2.1 Windows Graphics Capture (WGC) vs DXGI
Selain DXGI Desktop Duplication, Agent mendukung Windows Graphics Capture (WGC) API pada Windows 10/11:
- **WGC Advantage**: Mendukung pembatasan perekaman window tertentu (security-sensitive app masking) dan secara otomatis menyembunyikan cursor remote jika diinginkan.
- **DXGI Advantage**: Performa stabil pada multi-monitor layout berkinerja tinggi.
- WGC digunakan untuk per-window sharing, DXGI untuk full-desktop sharing.

### 2.2 macOS ScreenCaptureKit
- Menggunakan zero-copy pipeline: SCKit menghasilkan frame langsung di `CVPixelBuffer` berbasis Metal texture.
- Metal texture dilewatkan langsung ke API encoding **VideoToolbox** (`VTCompressionSession`), mempertahankan data frame seluruhnya di VRAM Apple Silicon unified memory.

---

## 3. Hardware Codec Integrations

Agent memuat library dynamic link API GPU saat startup untuk inisiasi hardware encoder:

```
                  ┌──────────────────────┐
                  │     Agent Startup    │
                  └──────────┬───────────┘
                             │
            ┌────────────────┼────────────────┐
            ▼ (Windows)      ▼ (macOS)        ▼ (Linux)
     [NVIDIA / AMD / Intel]  [Apple Silicon]  [VA-API / Intel]
            │                │                │
     ┌──────┴──────┐         ▼                ▼
     │ NVENC / AMF │   VideoToolbox       libva / VA-API
     │ / QuickSync │
     └──────┬──────┘
            │
            ▼
   GPU Pipeline Initialized
```

### 3.1 NVIDIA NVENC
- Menggunakan NVIDIA Video Codec SDK.
- Di-load dinamis via `nvEncodeAPI.dll` (Windows) atau `libnvidia-encode.so` (Linux).
- Preset: `LL_HQ` (Low Latency High Quality), rate control: `CBR` (Constant Bitrate) untuk stabilitas network.

### 3.2 Intel Quick Sync Video (QSV)
- Diintegrasikan melalui Intel OneVPL / Media SDK.
- Target pemakaian: Laptop tipis dan server headless berbasis prosesor Intel Xeon/Core dengan iGPU terintegrasi.

### 3.3 Apple VideoToolbox
- Menggunakan Swift/C interoperability di Rust untuk memanggil `VTCompressionSessionCreate`.
- Parameter `kVTCompressionPropertyKeyRealTime` diset `true` untuk mematikan frame buffering B-frame, menghasilkan zero-latency H.264/H.265 I-P frame sequence.
