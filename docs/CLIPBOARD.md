# Clipboard Synchronization Design Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Arsitektur Clipboard Sync

Clipboard synchronization menyelaraskan papan klip (clipboard) antara sistem operasi lokal (Viewer) dan remote (Agent) secara transparan.

---

## 2. Format Data & Dukungan Media

| Tipe Data | Format Biner | Kompresi | Batas Ukuran Maksimal |
|---|---|---|---|
| **Plain Text** | UTF-8 String | LZ4 (jika > 10KB) | 10 MB |
| **Rich Text (HTML)**| UTF-8 HTML String | LZ4 | 10 MB |
| **Image** | PNG / BMP | Zstd | 50 MB |
| **Files** | File path list (binary serialized) | N/A (transfer via file module) | N/A |

---

## 3. Alur Data Clipboard

```
 OS Copy Event (Lokal)
        │
        ▼
 Read OS Clipboard ──► Serialize & Encrypt ──► WebRTC DataChannel
                                                      │
                                                      ▼
 Write OS Clipboard ◄─ Decrypt & Deserialize ◄── Receive Packet (Remote)
```

1. **Listener**: Viewer/Agent mendeteksi perubahan clipboard melalui OS hook (e.g., Win32 `WM_CLIPBOARDUPDATE` atau macOS `NSPasteboard` polling).
2. **Serialization**: Data clipboard diubah menjadi binary payload berformat `CLIPBOARD_SYNC` packet.
3. **Transport**: Dikirim via WebRTC DataChannel (reliable channel).
4. **Integration**: Penerima menulis ulang data tersebut ke Clipboard Manager OS native setempat.

---

## 4. Penanganan Ukuran Besar (Streaming)

Untuk clipboard berukuran besar (> 1MB, seperti gambar resolusi tinggi):
1. Pengirim mengirimkan header `CLIPBOARD_INIT { size, type }`.
2. Penerima mengalokasikan buffer memori sementara.
3. Data dikirim dalam bentuk chunking 64KB secara berurutan.
4. Setelah chunk terakhir diterima, buffer didekripsi, didekompresi, dan ditulis ke OS clipboard.

---

## 5. Security & Policy Settings

Kebijakan keamanan clipboard dikonfigurasi melalui Web Dashboard dan diterapkan pada tingkat organisasi atau sesi:
- **Clipboard Mode**: `Bidirectional` (dua arah), `LocalToRemoteOnly` (hanya lokal ke remote), `RemoteToLocalOnly` (hanya remote ke lokal), atau `Disabled` (dinonaktifkan).
- **Format Filtering**: Membatasi copy-paste file atau gambar, hanya mengizinkan plain text untuk mencegah kebocoran data.
