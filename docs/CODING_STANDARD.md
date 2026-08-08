# Coding Standards & Architecture Guide

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft

---

## 1. Rust Code Standards

### 1.1 Formatting & Linting
- **rustfmt**: Semua kode wajib ter-format oleh `cargo fmt` sebelum commit.
- **clippy**: Semua peringatan `clippy` wajib diresolvasi. CI menolak build dengan `cargo clippy -- -D warnings`.

### 1.2 Naming Conventions

| Elemen | Konvensi | Contoh |
|---|---|---|
| Crates | `kebab-case` | `rdp-core`, `rdp-agent` |
| Modules | `snake_case` | `screen_capture`, `file_transfer` |
| Types/Structs | `PascalCase` | `DeviceInfo`, `SessionState` |
| Functions | `snake_case` | `handle_connection`, `encode_frame` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRY_COUNT`, `DEFAULT_BITRATE` |
| Enums | `PascalCase` (variants juga) | `PacketType::ScreenFrame` |
| Traits | `PascalCase` | `Encoder`, `CaptureSource` |

### 1.3 Error Handling
- Gunakan `thiserror` untuk mendefinisikan error types di library crates.
- Gunakan `color-eyre` di binary crates (agent, viewer) untuk rich error context.
- **Jangan gunakan `unwrap()`** di production code kecuali pada kasus yang benar-benar tidak mungkin gagal (dan berikan komentar mengapa).

---

## 2. Architecture Patterns

### 2.1 Hexagonal Architecture (Ports and Adapters)

```
                    ┌─────────────────────────────────┐
                    │         Domain Layer            │
                    │                                 │
                    │  • Entities (Device, Session)   │
                    │  • Use Cases (CreateSession)    │
  Inbound Ports     │  • Domain Services              │    Outbound Ports
  (Traits)          │                                 │    (Traits)
  ┌──────────┐      │                                 │      ┌──────────┐
  │ HTTP API │◄────►│                                 │◄────►│ Database │
  │ gRPC     │      │                                 │      │ Redis    │
  │ WebSocket│      │                                 │      │ NATS     │
  └──────────┘      └─────────────────────────────────┘      └──────────┘
   (Adapters)                                                 (Adapters)
```

- **Domain Layer** (pure Rust): Business logic, tidak depend pada framework.
- **Port** (trait): Interface yang mendefinisikan kontrak.
- **Adapter** (impl): Implementasi konkret (Axum handler, SQLx repository, Redis cache).

### 2.2 Repository Pattern

```rust
// Port (trait) — di domain layer
#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn find_by_id(&self, id: &DeviceId) -> Result<Option<Device>>;
    async fn save(&self, device: &Device) -> Result<()>;
    async fn delete(&self, id: &DeviceId) -> Result<()>;
}

// Adapter (impl) — di infrastructure layer
pub struct PgDeviceRepository {
    pool: PgPool,
}

#[async_trait]
impl DeviceRepository for PgDeviceRepository {
    async fn find_by_id(&self, id: &DeviceId) -> Result<Option<Device>> {
        sqlx::query_as!(Device, "SELECT * FROM devices WHERE device_id = $1", id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }
    // ...
}
```

### 2.3 CQRS — Evaluasi

**Keputusan: CQRS parsial, bukan penuh.**

- **Command side** (write): Melalui domain services → repository → PostgreSQL Primary.
- **Query side** (read): Melalui query handlers langsung → PostgreSQL Replica (read-optimized views).
- **Tidak menggunakan Event Sourcing penuh** — kompleksitasnya tidak sebanding untuk tahap awal. Cukup gunakan event-driven architecture via NATS untuk meng-broadcast domain events (misalnya: `SessionCreated`, `DeviceOffline`) ke subscriber yang membutuhkan.

### 2.4 SOLID Principles

| Prinsip | Penerapan |
|---|---|
| **S**ingle Responsibility | Setiap struct/module hanya bertanggung jawab atas satu concern |
| **O**pen-Closed | Gunakan trait untuk extensibility tanpa modifikasi kode existing |
| **L**iskov Substitution | Implementasi trait harus dapat saling menggantikan |
| **I**nterface Segregation | Trait kecil dan spesifik, bukan satu trait besar |
| **D**ependency Inversion | Domain layer depend pada trait (port), bukan implementasi konkret |

### 2.5 DDD (Domain-Driven Design)

- **Aggregate Root**: `Device`, `Session`, `Organization` — titik masuk utama untuk operasi domain.
- **Value Object**: `DeviceId`, `SessionId`, `UserId` — newtype wrappers dengan validasi.
- **Domain Event**: `DeviceRegistered`, `SessionStarted`, `FileTransferCompleted` — dikirim via NATS.

---

## 3. TypeScript / Vue 3 Standards (Web Dashboard)

### 3.1 TypeScript
- `strict: true` wajib di `tsconfig.json`.
- Tidak boleh menggunakan `any` — gunakan `unknown` dan type guards.
- Semua API response harus memiliki TypeScript interface/type.

### 3.2 Vue 3
- Gunakan **Composition API** (`<script setup>`) untuk semua komponen baru.
- State management via **Pinia** stores.
- Komponen diberi nama PascalCase: `DeviceListTable.vue`.

---

## 4. Git Conventions

### 4.1 Branch Naming
- `feature/short-description`
- `fix/issue-number-description`
- `release/v1.0.0`

### 4.2 Commit Messages
Format: Conventional Commits

```
feat(agent): add DXGI screen capture for Windows
fix(api): prevent race condition in session creation
docs(security): update encryption algorithm table
perf(encoder): reduce NVENC encode latency by 2ms
```

### 4.3 Pull Request Rules
- Minimal 1 reviewer approval.
- CI pipeline harus hijau (semua cek lulus).
- Squash merge ke `main`.
