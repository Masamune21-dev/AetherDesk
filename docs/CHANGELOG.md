# Changelog

## Remote Desktop Platform

Semua perubahan penting pada proyek ini akan didokumentasikan di file ini. Format changelog didasarkan pada [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) dan proyek ini mematuhi [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Desain arsitektur dasar dan dokumentasi engineering (PRD, Architecture, System Design, Network, dll).
- Rancangan skema database relasional PostgreSQL untuk multi-tenancy.
- Definisi spesifikasi protokol biner kustom untuk komunikasi media & kontrol.
- Rencana integrasi hardware encoder (NVENC, SCKit, VideoToolbox).

---

## [0.1.0] - 2026-08-07

### Added
- Initial commit dari draft spesifikasi dan dokumen desain sistem Remote Desktop Platform.
- Framework arsitektur Modular Monolith berbasis Rust workspace.
- Rencana implementasi UI/UX Viewer berbasis Tauri + Vue 3.
- Skema deployment Cloud Native Kubernetes.
- Integrasi pipeline DevOps dan Code Signing.
