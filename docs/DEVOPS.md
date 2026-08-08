# DevOps Architecture Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. CI/CD Pipeline Design

Alur otomatisasi pengujian, pemindaian keamanan, dan perilisan menggunakan **GitHub Actions**.

```
 Code Push
    │
    ▼
 [CI Pipeline] ──► Lint & Format ──► Security Scan ──► Unit Tests ──► Integration Tests
                                                                           │
                                                                   (Master Branch?)
                                                                           │
                                                                   ┌───────┴───────┐
                                                                  No              Yes
                                                                   │               │
                                                                   ▼               ▼
                                                              [PR Approved]   [Build & Release]
                                                                              • Sign Binaries
                                                                              • Docker Build
                                                                              • Helm Deploy
```

---

## 2. CI Pipeline Configuration (GitHub Actions Sample)

```yaml
# .github/workflows/ci.yml
name: CI Pipeline

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  lint-and-format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - name: Rustfmt
        run: cargo fmt --check
      - name: Clippy
        run: cargo clippy -- -D warnings

  security-scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run Cargo Audit (Dependency Scan)
        run: |
          cargo install cargo-audit
          cargo audit
      - name: Run Semgrep (SAST)
        run: |
          pip install semgrep
          semgrep scan --config auto

  unit-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run Tests
        run: cargo test --lib --all-features
```

---

## 3. Release & Code Signing Pipeline

Saat rilis baru dibuat:

1. **Compilation**: GitHub Runner mengompilasi biner untuk berbagai target platform:
   - Windows: `x86_64-pc-windows-msvc`
   - macOS: `x86_64-apple-darwin` dan `aarch64-apple-darwin` (Apple Silicon)
2. **Code Signing**:
   - **Windows**: Menggunakan sertifikat EV Code Signing (PFX) via tool `signtool.exe` untuk menghindari Windows SmartScreen warnings.
   - **macOS**: Menggunakan sertifikat Apple Developer ID via `codesign` dan proses `notarytool` (notarisasi Apple) agar biner dapat berjalan tanpa peringatan keamanan gatekeeper macOS.
3. **Distribution**:
   - Installer desktop diunggah ke CDN S3.
   - File manifest update (`latest.json`) diperbarui untuk memicu auto-update client.
   - Docker image API/Signal server didorong ke Amazon ECR / private registry.
