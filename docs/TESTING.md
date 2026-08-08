# Testing Strategy Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Piramida Pengujian (Testing Pyramid)

```
        ▲     ┌───────────┐
       / \    │   E2E     │  < 10% (Manual & Webdriver)
      /   \   ├───────────┤
     /     \  │Integration│  ~ 30% (API, DB, WebRTC connection)
    /       \ ├───────────┤
   /  Unit   \│   Unit    │  ~ 60% (Rust modules, Cargo tests)
  /───────────\───────────┘
```

---

## 2. Metodologi Unit Testing (Rust)

- Setiap file modul Rust wajib memiliki modul pengujian internal:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      #[test]
      fn test_protocol_parsing() {
          // assertions here
      }
  }
  ```
- Target Cakupan Kode (Code Coverage): **Minimum 80%** untuk domain logic (`crates/rdp-core` dan `crates/rdp-api`). Coverage dipantau via CI menggunakan `tarpaulin`.

---

## 3. Integration Testing (Signaling & WebRTC)

Pengujian integrasi otomatis mensimulasikan siklus koneksi lengkap:

1. **Mock Signal Server**: Menjalankan mock WebSocket server di background.
2. **Virtual Clients**: Men-spawn dua thread/proses biner agent dan viewer tiruan.
3. **ICE Negotiation**: Memaksa negosiasi ICE terjadi di interface lokal (localhost).
4. **Data Transmission**: Mengirim data input mouse/keyboard palsu melalui WebRTC DataChannel dan memverifikasi data yang diterima di sisi agent sama persis.
5. **WebRTC State Assertions**: Memastikan status WebRTC peer connection bertransisi dengan benar: `new` → `connecting` → `connected` → `completed`.

---

## 4. Performance & Load Testing

- **API Load Testing**: Menggunakan **k6** untuk mensimulasikan 50,000 HTTP/gRPC request per detik ke API cluster.
- **WebSocket Connection Scaling**: Menggunakan script Node.js/Rust khusus untuk menjaga 1,000,000 koneksi WebSocket konkuren tetap terbuka pada Signal Server cluster guna mengukur penggunaan memori per node (target: < 20KB per WebSocket connection).
- **Relay Load Testing**: Menyebarkan load generator UDP yang membanjiri TURN server dengan paket media terenkripsi 50 Mbps per session untuk menentukan batas kejenuhan CPU relay (throughput benchmark).
- **Latency Benchmarking**: Menggunakan software loopback delay generator untuk menguji responsivitas visual adaptif pada kondisi simulasi latency tinggi (100ms, 200ms, 300ms) dan packet loss (1% s/d 20%).
