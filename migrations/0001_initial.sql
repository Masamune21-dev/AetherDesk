-- ═══════════════════════════════════════════════════════════════════════════
-- AetherDesk — migrasi awal
--
-- Skema ini adalah DATABASE.md dengan temuan review sudah diperbaiki sejak
-- awal, bukan ditambal belakangan. Setiap penyimpangan dari dokumen diberi
-- komentar beserta nomor temuannya.
-- ═══════════════════════════════════════════════════════════════════════════

-- ── Role runtime ────────────────────────────────────────────────────────────
-- Dipisahkan dari pemilik skema supaya Row-Level Security benar-benar berlaku.
-- Pemilik tabel melewati RLS kecuali dipaksa; memisahkan role membuat migrasi
-- tetap bisa berjalan sementara jalur runtime tetap terkurung. (Temuan T-07)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aetherdesk_app') THEN
        CREATE ROLE aetherdesk_app NOLOGIN;
    END IF;
END $$;

-- ═══════════════════════════════════════════════════════════════════════════
-- 1. ORGANISASI
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE organizations (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name               VARCHAR(255) NOT NULL,
    slug               VARCHAR(100) UNIQUE NOT NULL,
    license_key        VARCHAR(255),
    license_expires_at TIMESTAMPTZ,
    -- Optimistic concurrency control, diwajibkan SYSTEM_DESIGN.md §2 tetapi
    -- tidak pernah muncul di skema dokumen. (Temuan T-07)
    version            INTEGER NOT NULL DEFAULT 1,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_orgs_slug ON organizations (slug);

-- ═══════════════════════════════════════════════════════════════════════════
-- 2. PENGGUNA
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    email           VARCHAR(255) NOT NULL,
    password_hash   VARCHAR(255) NOT NULL,
    name            VARCHAR(255) NOT NULL,
    mfa_secret      VARCHAR(100),
    mfa_enabled     BOOLEAN NOT NULL DEFAULT FALSE,
    status          VARCHAR(50) NOT NULL DEFAULT 'active',
    version         INTEGER NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT users_status_valid CHECK (status IN ('active', 'suspended', 'pending')),

    -- Dokumen memakai UNIQUE global pada email, yang berarti satu orang tidak
    -- bisa menjadi anggota dua organisasi — persis skenario MSP di UC-03 dan
    -- persona Erik. Sekaligus menjadi oracle enumerasi lintas tenant.
    -- (Temuan T-05)
    CONSTRAINT users_email_unik_per_org UNIQUE (organization_id, email)
);

CREATE INDEX idx_users_org ON users (organization_id);
CREATE INDEX idx_users_email_lookup ON users (lower(email));

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. RBAC
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE roles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name            VARCHAR(100) NOT NULL,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, name)
);

CREATE TABLE permissions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code        VARCHAR(100) UNIQUE NOT NULL,
    description TEXT
);

CREATE TABLE role_permissions (
    role_id       UUID NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions (id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE user_roles (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);

-- ═══════════════════════════════════════════════════════════════════════════
-- 4. GRUP PERANGKAT
-- ═══════════════════════════════════════════════════════════════════════════
-- Tabel ini muncul di ERD DATABASE.md dan direferensikan devices.group_id,
-- tetapi tidak pernah punya DDL. (Temuan T-07)

CREATE TABLE groups (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    parent_id       UUID REFERENCES groups (id) ON DELETE SET NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    version         INTEGER NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, name)
);

CREATE INDEX idx_groups_org ON groups (organization_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- 5. PERANGKAT
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE devices (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    -- Dokumen menuliskan group_id tanpa foreign key sama sekali. (Temuan T-07)
    group_id        UUID REFERENCES groups (id) ON DELETE SET NULL,

    -- Device ID sembilan digit dengan check digit Damm; lihat QUICK_CONNECT.md.
    device_id       CHAR(9) UNIQUE NOT NULL,
    alias           VARCHAR(255),

    os_type         VARCHAR(50) NOT NULL,
    os_version      VARCHAR(100),
    hostname        VARCHAR(255),
    client_version  VARCHAR(50),
    cpu_model       VARCHAR(255),
    ram_total_mb    BIGINT,
    gpu_model       VARCHAR(255),

    -- Wake-on-LAN mustahil dibentuk tanpa MAC, dan kolomnya tidak pernah ada
    -- di dokumen meskipun UC-06 dan SYNC.md mengandalkannya. (Temuan T-01)
    mac_address     MACADDR,
    last_known_ip   INET,

    -- Quick Connect (QUICK_CONNECT.md §6)
    quick_connect_enabled   BOOLEAN NOT NULL DEFAULT TRUE,
    session_password_hash   VARCHAR(255),
    session_password_set_at TIMESTAMPTZ,

    status          VARCHAR(50) NOT NULL DEFAULT 'offline',
    last_heartbeat  TIMESTAMPTZ,
    version         INTEGER NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT devices_status_valid CHECK (status IN ('online', 'offline', 'busy')),
    CONSTRAINT devices_id_sembilan_digit CHECK (device_id ~ '^[0-9]{9}$')
);

CREATE INDEX idx_devices_lookup     ON devices (device_id);
CREATE INDEX idx_devices_org_status ON devices (organization_id, status);
CREATE INDEX idx_devices_group      ON devices (group_id) WHERE group_id IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════════════
-- 6. KUNCI PERANGKAT
-- ═══════════════════════════════════════════════════════════════════════════
-- Dokumen memakai device_id sebagai primary key, yang berarti satu kunci per
-- perangkat dan rotasi mustahil dilakukan tanpa periode tumpang tindih.

CREATE TABLE device_keys (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    device_id   UUID NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    public_key  TEXT NOT NULL,
    certificate TEXT NOT NULL,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Hanya satu kunci aktif per perangkat pada satu waktu, tetapi kunci lama
-- tetap tersimpan untuk verifikasi dan audit.
CREATE UNIQUE INDEX idx_device_keys_satu_aktif
    ON device_keys (device_id) WHERE is_active;
CREATE INDEX idx_device_keys_device ON device_keys (device_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- 7. SESI
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE sessions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,

    -- Dokumen memakai ON DELETE RESTRICT di sini sementara devices memakai
    -- CASCADE dari organizations. Akibatnya organisasi yang punya satu saja
    -- sesi historis tidak akan pernah bisa dihapus. Identitas didenormalisasi
    -- supaya riwayat bertahan tanpa memblokir penghapusan. (Temuan T-06)
    device_uuid     UUID REFERENCES devices (id) ON DELETE SET NULL,
    viewer_user_id  UUID REFERENCES users (id) ON DELETE SET NULL,
    device_id_snapshot   CHAR(9)      NOT NULL,
    viewer_email_snapshot VARCHAR(255) NOT NULL,

    connect_method  VARCHAR(20) NOT NULL DEFAULT 'quick_connect',
    status          VARCHAR(50) NOT NULL,
    recording_path  VARCHAR(512),
    started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at        TIMESTAMPTZ,

    CONSTRAINT sessions_status_valid
        CHECK (status IN ('pending', 'active', 'terminated', 'disconnected')),
    CONSTRAINT sessions_method_valid
        CHECK (connect_method IN ('quick_connect', 'unattended', 'api'))
);

CREATE INDEX idx_sessions_aktif ON sessions (organization_id) WHERE status = 'active';
CREATE INDEX idx_sessions_device ON sessions (device_uuid, started_at DESC);

-- ═══════════════════════════════════════════════════════════════════════════
-- 8. TABEL TERPARTISI
-- ═══════════════════════════════════════════════════════════════════════════
-- Dokumen mendeklarasikan id UUID NOT NULL tanpa primary key sama sekali.
-- PostgreSQL mengharuskan kolom partisi menjadi bagian dari setiap constraint
-- unik, jadi primary key di sini komposit. (Temuan T-06)

CREATE TABLE connection_logs (
    id                   UUID NOT NULL DEFAULT gen_random_uuid(),
    organization_id      UUID NOT NULL,
    session_id           UUID NOT NULL,
    device_uuid          UUID,
    viewer_user_id       UUID,
    connection_type      VARCHAR(50) NOT NULL,
    relay_server_id      VARCHAR(100),
    duration_seconds     INTEGER,
    bytes_transferred    BIGINT,
    avg_latency_ms       INTEGER,
    avg_fps              INTEGER,
    disconnection_reason VARCHAR(255),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_conn_logs_device ON connection_logs (device_uuid, created_at DESC);

CREATE TABLE audit_logs (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL,
    user_id         UUID,
    -- VARCHAR(45) diganti INET: validasi, normalisasi, dan operator subnet
    -- yang justru dibutuhkan saat menganalisis audit. (Temuan R-08)
    ip_address      INET NOT NULL,
    action          VARCHAR(100) NOT NULL,
    payload         JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_audit_org_action ON audit_logs (organization_id, action, created_at DESC);

CREATE TABLE quick_connect_attempts (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    device_id_input VARCHAR(9) NOT NULL,
    source_ip       INET NOT NULL,
    outcome         VARCHAR(20) NOT NULL,
    viewer_user_id  UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, created_at),
    CONSTRAINT qc_outcome_valid CHECK (outcome IN
        ('accepted', 'bad_password', 'unknown_id', 'throttled', 'rejected_by_user'))
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_qc_device ON quick_connect_attempts (device_id_input, created_at DESC);
CREATE INDEX idx_qc_ip     ON quick_connect_attempts (source_ip, created_at DESC);

-- ── Partisi DEFAULT ─────────────────────────────────────────────────────────
-- DATABASE.md §3 menyerahkan pembuatan partisi kepada cron bulanan tanpa
-- jaring pengaman. Bila cron terlewat, seluruh INSERT gagal dan audit trail
-- berhenti diam-diam. Partisi DEFAULT membuat data tetap tertampung.
CREATE TABLE connection_logs_default       PARTITION OF connection_logs       DEFAULT;
CREATE TABLE audit_logs_default            PARTITION OF audit_logs            DEFAULT;
CREATE TABLE quick_connect_attempts_default PARTITION OF quick_connect_attempts DEFAULT;

-- ═══════════════════════════════════════════════════════════════════════════
-- 9. AUDIT LOG BENAR-BENAR IMMUTABLE
-- ═══════════════════════════════════════════════════════════════════════════
-- Dokumen menyebut audit log immutable tanpa satu pun mekanisme yang
-- menegakkannya. Untuk bukti SOC 2, sifat itu harus ditegakkan. (Temuan T-08)

CREATE OR REPLACE FUNCTION tolak_perubahan_audit()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'audit_logs bersifat append-only: % ditolak', TG_OP
        USING ERRCODE = 'insufficient_privilege';
END $$;

CREATE TRIGGER audit_logs_append_only
    BEFORE UPDATE OR DELETE ON audit_logs
    FOR EACH STATEMENT EXECUTE FUNCTION tolak_perubahan_audit();

-- ═══════════════════════════════════════════════════════════════════════════
-- 10. TRIGGER updated_at
-- ═══════════════════════════════════════════════════════════════════════════
-- Dokumen mendeklarasikan DEFAULT CURRENT_TIMESTAMP pada updated_at, yang
-- hanya berlaku saat INSERT dan tidak pernah berubah saat UPDATE.

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.updated_at := now();
    -- Menaikkan version sekaligus, supaya OCC tidak bergantung pada disiplin
    -- setiap pemanggil.
    IF TG_TABLE_NAME IN ('organizations', 'users', 'devices', 'groups') THEN
        NEW.version := OLD.version + 1;
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER trg_orgs_updated    BEFORE UPDATE ON organizations
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_users_updated   BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_devices_updated BEFORE UPDATE ON devices
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_groups_updated  BEFORE UPDATE ON groups
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ═══════════════════════════════════════════════════════════════════════════
-- 11. ROW-LEVEL SECURITY
-- ═══════════════════════════════════════════════════════════════════════════
-- ADR-006 menjadikan RLS sebagai alasan utama memilih PostgreSQL, tetapi
-- DATABASE.md tidak pernah mendefinisikan satu pun policy. (Temuan T-07)
--
-- Runtime menetapkan tenant aktif sekali per transaksi:
--     SET LOCAL aetherdesk.current_org = '<uuid>';

CREATE OR REPLACE FUNCTION current_org()
RETURNS UUID
LANGUAGE sql
STABLE
AS $$
    SELECT nullif(current_setting('aetherdesk.current_org', true), '')::UUID
$$;

DO $$
DECLARE t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY['organizations','users','roles','groups','devices','sessions']
    LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
    END LOOP;
END $$;

CREATE POLICY org_isolation ON organizations
    USING (id = current_org());

CREATE POLICY org_isolation ON users     USING (organization_id = current_org());
CREATE POLICY org_isolation ON roles     USING (organization_id = current_org());
CREATE POLICY org_isolation ON groups    USING (organization_id = current_org());
CREATE POLICY org_isolation ON devices   USING (organization_id = current_org());
CREATE POLICY org_isolation ON sessions  USING (organization_id = current_org());

-- ═══════════════════════════════════════════════════════════════════════════
-- 12. HAK AKSES
-- ═══════════════════════════════════════════════════════════════════════════

GRANT USAGE ON SCHEMA public TO aetherdesk_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO aetherdesk_app;

-- Audit log: hanya boleh ditambah, tidak pernah diubah atau dihapus.
REVOKE UPDATE, DELETE ON audit_logs FROM aetherdesk_app;
REVOKE UPDATE, DELETE ON quick_connect_attempts FROM aetherdesk_app;

-- ═══════════════════════════════════════════════════════════════════════════
-- 13. DATA AWAL
-- ═══════════════════════════════════════════════════════════════════════════

INSERT INTO permissions (code, description) VALUES
    ('device.read',      'Melihat perangkat dan metadatanya'),
    ('device.write',     'Mengubah pengaturan perangkat'),
    ('device.delete',    'Menghapus pendaftaran perangkat'),
    ('session.create',   'Memulai sesi remote'),
    ('session.terminate','Mengakhiri sesi yang sedang berjalan'),
    ('session.recording.view', 'Memutar rekaman sesi'),
    ('user.read',        'Melihat pengguna'),
    ('user.write',       'Membuat dan mengubah pengguna'),
    ('audit.read',       'Membaca audit log'),
    ('org.manage',       'Mengelola pengaturan organisasi');
