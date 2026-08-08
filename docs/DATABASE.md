# Database Design Document

## Remote Desktop Platform

**Versi:** 1.0.0
**Tanggal:** 2026-08-07
**Status:** Draft
**Pemilik:** Database Administrator / System Architect

---

## 1. Skema Relasional Database (ERD)

Desain database PostgreSQL ini menggunakan UUID sebagai primary key untuk keamanan dan kemudahan migrasi data di lingkungan terdistribusi. Indeks B-Tree ditempatkan secara strategis pada foreign keys dan kolom pencarian.

```
       ┌──────────────────────┐               ┌──────────────────────┐
       │    organizations     │◄─────────────┤        users         │
       │                      │1            *│                      │
       └──────────┬───────────┘               └──────────┬───────────┘
                  │1                                     │1
                  │                                      │
                  │*                                     │*
       ┌──────────▼───────────┐               ┌──────────▼───────────┐
       │       groups         │◄─────────────┤     user_roles       │
       │                      │1            *│                      │
       └──────────┬───────────┘               └──────────────────────┘
                  │1
                  │
                  │*
       ┌──────────▼───────────┐               ┌──────────────────────┐
       │       devices        │◄─────────────┤     device_keys      │
       │                      │1            1│                      │
       └──────────┬───────────┘               └──────────────────────┘
                  │1
                  ├──────────────────────────────┐
                  │*                             │*
       ┌──────────▼───────────┐       ┌──────────▼───────────┐
       │       sessions       │       │    connection_logs   │
       │                      │1     *│  (Partitioned Table) │
       └──────────┬───────────┘       └──────────────────────┘
                  │1
                  ├──────────────────────────────┐
                  │*                             │*
       ┌──────────▼───────────┐       ┌──────────▼───────────┐
       │    file_transfers    │       │  clipboard_history   │
       │                      │       │                      │
       └──────────────────────┘       └──────────────────────┘
```

---

## 2. Struktur Tabel Database

### 2.1 organizations

Tabel utama untuk mendukung multi-tenancy.

```sql
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) UNIQUE NOT NULL,
    license_key VARCHAR(255),
    license_expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_orgs_slug ON organizations(slug);
```

### 2.2 users

Informasi kredensial dan data profil user.

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    mfa_secret VARCHAR(100),
    mfa_enabled BOOLEAN DEFAULT FALSE,
    status VARCHAR(50) DEFAULT 'active', -- active, suspended, pending
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_users_org ON users(organization_id);
```

### 2.3 roles & permissions (RBAC)

Struktur kontrol akses berbasis peran (Role-Based Access Control).

```sql
CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(organization_id, name)
);

CREATE TABLE permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(100) UNIQUE NOT NULL,
    description TEXT
);

CREATE TABLE role_permissions (
    role_id UUID REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE user_roles (
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);
```

### 2.4 devices

Metadata hardware dan status agent.

```sql
CREATE TABLE devices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    group_id UUID, -- References groups(id)
    device_id VARCHAR(100) UNIQUE NOT NULL, -- Target ID untuk koneksi (eg: 123-456-789)
    alias VARCHAR(255),
    os_type VARCHAR(50) NOT NULL, -- Windows, macOS, Linux
    os_version VARCHAR(100),
    hostname VARCHAR(255),
    client_version VARCHAR(50),
    cpu_model VARCHAR(255),
    ram_total_mb BIGINT,
    gpu_model VARCHAR(255),
    status VARCHAR(50) DEFAULT 'offline', -- online, offline, busy
    last_heartbeat TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_devices_id ON devices(device_id);
CREATE INDEX idx_devices_org_status ON devices(organization_id, status);
```

### 2.5 device_keys

Menyimpan public key kriptografi perangkat untuk mTLS.

```sql
CREATE TABLE device_keys (
    device_id UUID PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    public_key TEXT NOT NULL, -- PEM encoded Ed25519 public key
    certificate TEXT NOT NULL, -- Signed X.509 device certificate
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
```

### 2.6 sessions

Sesi aktif jarak jauh.

```sql
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id VARCHAR(100) UNIQUE NOT NULL, -- Token sesi komunikasi
    device_id UUID REFERENCES devices(id) ON DELETE RESTRICT,
    viewer_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    status VARCHAR(50) NOT NULL, -- active, terminated, disconnected
    recording_path VARCHAR(512),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    ended_at TIMESTAMP WITH TIME ZONE
);
CREATE INDEX idx_sessions_active ON sessions(status) WHERE status = 'active';
```

### 2.7 connection_logs (Partitioned Table)

Log riwayat koneksi. Dipakai untuk analisis jaringan dan audit.

```sql
CREATE TABLE connection_logs (
    id UUID NOT NULL,
    session_id VARCHAR(100) NOT NULL,
    device_id UUID NOT NULL,
    viewer_id UUID NOT NULL,
    connection_type VARCHAR(50) NOT NULL, -- P2P, TURN
    relay_server_id VARCHAR(100),
    duration_seconds INT,
    bytes_transferred BIGINT,
    avg_latency_ms INT,
    avg_fps INT,
    disconnection_reason VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_conn_logs_device ON connection_logs(device_id, created_at);
```

### 2.8 audit_logs (Partitioned Table)

Log audit immutable untuk kebutuhan regulasi SOC2.

```sql
CREATE TABLE audit_logs (
    id UUID NOT NULL,
    organization_id UUID NOT NULL,
    user_id UUID,
    ip_address VARCHAR(45) NOT NULL,
    action VARCHAR(100) NOT NULL, -- user.login, device.delete, permission.grant
    payload JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_audit_org_action ON audit_logs(organization_id, action, created_at);
```

---

## 3. Strategi Partisi Tabel PostgreSQL

Tabel `connection_logs` dan `audit_logs` akan tumbuh sangat cepat pada skala enterprise. Oleh karena itu, kita menerapkan **Range Partitioning** berbasis kolom `created_at` secara bulanan.

### Contoh Implementasi Partisi Otomatis (Prosedur Bulanan)

```sql
-- Membuat partisi untuk Agustus 2026
CREATE TABLE connection_logs_y2026m08 PARTITION OF connection_logs
    FOR VALUES FROM ('2026-08-01 00:00:00+00') TO ('2026-09-01 00:00:00+00');

-- Membuat partisi untuk September 2026
CREATE TABLE connection_logs_y2026m09 PARTITION OF connection_logs
    FOR VALUES FROM ('2026-09-01 00:00:00+00') TO ('2026-10-01 00:00:00+00');
```

Sebuah cron job atau task worker di service backend bertugas menjalankan perintah DDL ini secara berkala setiap bulan, 15 hari sebelum bulan baru dimulai.
