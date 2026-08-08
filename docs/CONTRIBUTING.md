# Contributing Guide

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Persyaratan Lingkungan (Prerequisites)

Untuk berkontribusi pada pengembangan Remote Desktop Platform, Anda memerlukan peralatan berikut terinstall di sistem lokal:

- **Rust toolchain** (stable channel, 1.80+)
- **Node.js** (LTS, v20+) dan **pnpm**
- **PHP** (8.4+) dan **Composer** (untuk Laravel Web Dashboard)
- **Docker** dan **Docker Compose**
- **PostgreSQL CLI** (`psql`)
- **NATS CLI**

---

## 2. Setup Lingkungan Pengembangan Lokal

1. **Clone repositori**:
   ```bash
   git clone https://github.com/org/remote-desktop-platform.git
   cd remote-desktop-platform
   ```
2. **Jalankan infrastruktur pendukung**:
   ```bash
   docker-compose -f infra/docker/docker-compose.local.yml up -d
   # Ini akan menyalakan PostgreSQL, Redis, dan NATS
   ```
3. **Setup backend Rust**:
   ```bash
   cargo build
   # Kompilasi seluruh crates workspace
   ```
4. **Setup Web Dashboard**:
   ```bash
   cd web
   composer install
   cp .env.example .env
   php artisan key:generate
   php artisan migrate --seed
   pnpm install
   pnpm dev
   ```

---

## 3. Alur Kontribusi

```
 Fork / Branch ──► Make Changes ──► Local Tests ──► Pull Request
                                                          │
                                                    [Code Review]
                                                    [CI Verification]
                                                          │
                                                          ▼
                                                     Squash Merge
```

### 3.1 Langkah demi Langkah
1. Buat branch baru dari `develop`: `git checkout -b feature/nama-fitur`.
2. Implementasikan perubahan Anda. Selalu patuhi [Coding Standards](./CODING_STANDARD.md).
3. Tulis unit tests yang sesuai untuk logika baru.
4. Pastikan semua tests lulus lokal: `cargo test --all`.
5. Format dan lint kode Anda: `cargo fmt && cargo clippy`.
6. Commit perubahan Anda dengan pesan terformat (Conventional Commits).
7. Push ke origin dan buka Pull Request ke branch `develop`.

---

## 4. Proses Code Review

Setiap Pull Request (PR) harus melalui review menyeluruh sebelum digabungkan:
- **Design Review**: Peninjauan arsitektur untuk memastikan perubahan tidak melanggar batasan modul atau pola desain domain.
- **Security Check**: Peninjauan kode untuk potensi celah keamanan (OWASP Top 10, memory leak, raw pointer usage).
- **Test Verification**: Memastikan test coverage di atas target (80% unit test coverage).
- **Performance Impact**: Evaluasi apakah perubahan memengaruhi CPU/RAM/bandwidth budget secara negatif.

Semua penemu bug, perbaikan dokumentasi, dan penambahan fitur sangat diapresiasi! Terima kasih telah berkontribusi membuat Remote Desktop Platform menjadi lebih baik.
