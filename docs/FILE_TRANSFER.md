# File Transfer Protocol Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Arsitektur File Transfer

File transfer berjalan di atas **WebRTC SCTP DataChannels** untuk pengiriman data berkecepatan tinggi, aman, dan berorientasi pada integritas berkas.

---

## 2. Chunking & Serialization

1. Berkas dipecah menjadi chunks berukuran tetap: **256 KB** (optimal untuk bandwidth/memory ratio).
2. Setiap chunk diserialisasi ke format biner:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      File Index (32)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Chunk Index (32)                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Total Chunks (32)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Compressed Size (32)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Uncompressed Size (32)                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      SHA-256 (256 bit)                         |
|                                                                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Data Payload (variable)                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

---

## 3. Kompresi & Enkripsi

- **Kompresi**: Chunks dikompresi secara real-time menggunakan algoritma **Zstd** (level 1-3) atau **LZ4** untuk file besar.
- **Enkripsi**: Setiap chunk dienkripsi secara independen menggunakan **AES-256-GCM** dengan session key ephemeral.

---

## 4. Transfer Lifecycle

```
 Viewer                                               Agent
   │                                                    │
   │──► FILE_TRANSFER_INIT {files, size, checksums} ───►│
   │◄─  FILE_TRANSFER_ACCEPT {transfer_id} ─────────────│
   │                                                    │
   │──► FILE_CHUNK {file_idx, chunk_idx, data} ────────►│
   │◄─  FILE_CHUNK_ACK {chunk_idx} ─────────────────────│ (repeated)
   │                                                    │
   │──► FILE_COMPLETE {file_idx} ──────────────────────►│
   │◄─  FILE_VERIFY_OK {sha256_match: true} ────────────│
```

---

## 5. Fitur Lanjutan

- **Parallel Uploads**: Sistem dapat mengunggah hingga 4 file secara bersamaan pada data channel terpisah.
- **Resume & Pause**: Jika koneksi terputus, metadata transfer di-cache. Setelah reconnect, transfer dilanjutkan dari `chunk_idx` terakhir yang sukses dikirim.
- **Conflict Resolution**:
  - **Overwrite**: Timpa file tujuan.
  - **Rename**: Simpan sebagai `file (1).ext`.
  - **Skip**: Lewati jika checksum file tujuan sama.
- **Drag-and-Drop**: Integrasi dengan UI Tauri viewer untuk drag file dari local file explorer langsung ke remote desktop canvas.
