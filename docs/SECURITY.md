# Zero Trust Security Specification

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** Security Architect

---

## 1. Zero Trust Access Control

Sistem menerapkan model Zero Trust di mana setiap request diverifikasi secara eksplisit.

```
       Request (Any Network)
                 │
                 ▼
      [Device Identity Check] ──► Verified by Ed25519 Device Certificate (mTLS)
                 │
                 ▼
      [User Identity Check]   ──► Verified by Argon2id + TOTP/WebAuthn (SSO JWT)
                 │
                 ▼
      [Policy Engine Checks]  ──► RBAC Matrix + ABAC Context Rules
                 │
                 ▼
            Access Granted
```

---

## 2. Dynamic Policy Engine (ABAC)

Selain RBAC (Role-Based Access Control) standar, platform menggunakan ABAC (Attribute-Based Access Control) untuk mengevaluasi kebijakan secara dinamis saat sesi remote diinisiasi:

### Variabel Kebijakan ABAC
- **Context.Time**: Izinkan koneksi hanya pada jam kerja (misal: 08:00 - 17:00).
- **Context.Location**: Izinkan akses hanya dari alamat IP dalam negeri atau subnet VPN korporat.
- **Device.Compliance**: Viewer wajib memiliki OS terupdate, antivirus aktif, dan disk terenkripsi (BitLocker/FileVault).
- **Session.Sensitivity**: Mematikan clipboard dan file transfer jika masuk ke server kritis (tag: "production-core").

---

## 3. Sandboxing & Tamper Detection (Agent Security)

### 3.1 Linux & macOS Sandbox
- Agent berjalan dengan hak akses terbatas. Pada macOS, fitur dibatasi oleh *App Sandbox* entitlement.
- Pada Linux, systemd service dikonfigurasi dengan opsi isolasi:
  ```ini
  ProtectSystem=strict
  ProtectHome=true
  PrivateTmp=true
  NoNewPrivileges=true
  ```

### 3.2 Anti-Tamper & Memory Integrity (Windows)
- Biner Agent Windows dilindungi oleh integritas kontrol tanda tangan digital. Jika biner dimodifikasi di disk, service manager akan menolak menjalankannya.
- Proteksi eksploitasi memori diaktifkan saat kompilasi Rust:
  - **Stack Clashes Protection**: `-C force-stack-check`
  - **Control Flow Guard (CFG)**: Diaktifkan pada Windows build target untuk mencegah pembajakan alur eksekusi memori.
  - **ASLR & DEP**: Wajib aktif secara native pada compiler toolchain.
