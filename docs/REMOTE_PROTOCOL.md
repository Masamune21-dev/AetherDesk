# Remote Protocol Specification

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** Principal Software Architect

---

## 1. Ikhtisar Protokol

Remote Desktop Platform menggunakan protokol biner custom yang dirancang untuk efisiensi maksimal. Protokol ini berjalan di atas **WebRTC DataChannels** (untuk data kontrol) dan **SRTP** (untuk media streaming).

### Prinsip Desain Protokol
- **Binary-first**: Semua packet menggunakan format biner (bukan JSON/XML) untuk meminimalkan overhead parsing dan ukuran payload.
- **Little-endian byte order**: Konsisten dengan arsitektur x86/ARM modern.
- **Zero-copy parsing**: Struktur packet dirancang agar bisa di-parse tanpa alokasi memori tambahan menggunakan Rust `zerocopy` crate.

---

## 2. Header Packet Universal

Setiap packet diawali dengan header 12-byte berikut:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Version (4)  |  Type (8)     |  Flags (8)    |  Reserved (12) |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Sequence Number (32)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Payload Length (32)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Payload (variable)                        |
|                          ...                                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Field | Ukuran | Deskripsi |
|---|---|---|
| Version | 4 bit | Versi protokol (saat ini: `1`) |
| Type | 8 bit | Tipe packet (lihat Tabel Tipe Packet) |
| Flags | 8 bit | Bit flags (encrypted, compressed, fragmented, ack_required) |
| Reserved | 12 bit | Dicadangkan untuk penggunaan mendatang (harus `0`) |
| Sequence Number | 32 bit | Nomor urut packet per channel, untuk deteksi replay & ordering |
| Payload Length | 32 bit | Panjang payload dalam byte |
| Payload | variable | Data spesifik per tipe packet |

---

## 3. Tabel Tipe Packet

| Kode (Hex) | Nama | Arah | Deskripsi |
|---|---|---|---|
| `0x01` | HELLO | Viewer → Agent | Inisiasi koneksi, negosiasi versi |
| `0x02` | AUTH | Bidirectional | Challenge-response authentication |
| `0x03` | SESSION | Bidirectional | Session management (create/resume/end) |
| `0x10` | PING | Bidirectional | Latency probe |
| `0x11` | PONG | Bidirectional | Latency response |
| `0x12` | KEEPALIVE | Bidirectional | Keep connection alive |
| `0x13` | HEARTBEAT | Agent → Server | Device health status |
| `0x20` | SCREEN | Agent → Viewer | Encoded screen frame |
| `0x21` | KEYBOARD | Viewer → Agent | Keyboard input event |
| `0x22` | MOUSE | Viewer → Agent | Mouse input event |
| `0x23` | CLIPBOARD | Bidirectional | Clipboard sync data |
| `0x30` | FILE | Bidirectional | File transfer chunk |
| `0x31` | AUDIO | Agent → Viewer | Audio stream packet |
| `0x32` | CHAT | Bidirectional | Text chat message |
| `0x40` | ERROR | Bidirectional | Error notification |
| `0x41` | UPDATE | Server → Agent | Update notification |
| `0x42` | CANCEL | Bidirectional | Cancel ongoing operation |
| `0x43` | RESUME | Bidirectional | Resume interrupted operation |

---

## 4. Definisi Payload Per Tipe Packet

### 4.1 HELLO (0x01)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Protocol Version Min (16)    |  Protocol Version Max (16)    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Device ID (128 bit UUID)                  |
|                                                                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Supported Codecs Bitmask (16)|  Capabilities Bitmask (16)    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Supported Codecs Bitmask:**
- Bit 0: H.264
- Bit 1: H.265
- Bit 2: AV1
- Bit 3: Opus Audio
- Bit 4: AAC Audio

**Capabilities Bitmask:**
- Bit 0: File Transfer
- Bit 1: Clipboard
- Bit 2: Audio
- Bit 3: Multi-Monitor
- Bit 4: Session Recording
- Bit 5: Remote Terminal

### 4.2 SCREEN (0x20)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Monitor Index (8) | Codec (8) |  Frame Type (8) | Flags (8)  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Timestamp (64 bit, microseconds)          |
|                                                                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Width (16)         |  Height (16)                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Dirty Rect X (16)  |  Dirty Rect Y (16)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Dirty Rect W (16)  |  Dirty Rect H (16)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                   Encoded Frame Data (variable)                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Frame Type:** `0x00` = Keyframe (IDR), `0x01` = Delta (P-frame), `0x02` = Bidirectional (B-frame)

### 4.3 KEYBOARD (0x21)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Event Type (8)     |  Modifiers (8)     |  Scancode (16)     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Timestamp (64 bit)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Event Type:** `0x00` = Key Down, `0x01` = Key Up
**Modifiers Bitmask:** Bit 0: Ctrl, Bit 1: Shift, Bit 2: Alt, Bit 3: Meta/Win

### 4.4 MOUSE (0x22)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Event Type (8)     |  Button (8)        |  Modifiers (8)     | Pad(8) |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  X Position (16)    |  Y Position (16)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Scroll Delta X (16)|  Scroll Delta Y (16)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Timestamp (64 bit)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Event Type:** `0x00` = Move, `0x01` = Button Down, `0x02` = Button Up, `0x03` = Scroll

### 4.5 FILE (0x30)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Sub-Type (8)       |  Transfer ID (24)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      File Index (32)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Chunk Index (32)                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Total Chunks (32)                         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Chunk Data (variable)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Sub-Type:** `0x00` = Init, `0x01` = Data, `0x02` = Ack, `0x03` = Complete, `0x04` = Cancel, `0x05` = Resume

### 4.6 CLIPBOARD (0x23)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Content Type (8)   |  Encoding (8)      |  Flags (16)        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Data Length (32)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Data (variable)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Content Type:** `0x00` = Text (UTF-8), `0x01` = Image (PNG), `0x02` = HTML, `0x03` = Files

### 4.7 AUDIO (0x31)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Codec (8)          |  Channels (8)      |  Sample Rate (16)  |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Timestamp (64 bit)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Audio Data (variable)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### 4.8 PING/PONG (0x10/0x11)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Timestamp Sent (64 bit)                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

PONG mengembalikan timestamp yang sama sehingga RTT dapat dihitung: `RTT = now() - timestamp_sent`.

### 4.9 ERROR (0x40)

```
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Error Code (16)    |  Severity (8)      |  Pad (8)           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Message Length (16)|  Message (UTF-8, variable)               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Severity:** `0x00` = Info, `0x01` = Warning, `0x02` = Error, `0x03` = Fatal (session terminated)

---

## 5. Flags Field

| Bit | Nama | Deskripsi |
|---|---|---|
| 0 | ENCRYPTED | Payload terenkripsi (AES-256-GCM) |
| 1 | COMPRESSED | Payload terkompresi (Zstd) |
| 2 | FRAGMENTED | Packet ini adalah bagian dari packet besar yang terfragmentasi |
| 3 | ACK_REQUIRED | Pengirim mengharapkan ACK untuk packet ini |
| 4 | LAST_FRAGMENT | Fragment terakhir dari packet terfragmentasi |
| 5-7 | Reserved | Dicadangkan |

---

## 6. Keamanan Protokol

### 6.1 Replay Protection
Setiap packet memiliki `Sequence Number` monotonically increasing. Penerima melacak window 256 sequence number terakhir. Packet dengan sequence number di luar window atau sudah pernah diterima akan di-drop.

### 6.2 Encryption
Semua payload (kecuali HELLO) dienkripsi menggunakan AES-256-GCM dengan session key yang dinegosiasikan melalui DTLS handshake. Nonce 12-byte terdiri dari 4-byte sender ID + 8-byte sequence counter.

---

*Spesifikasi protokol ini dirancang untuk kecepatan parsing (~100ns per packet), ukuran overhead minimal (12 byte header), dan keamanan kriptografi tingkat militer.*
