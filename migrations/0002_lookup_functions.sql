-- ═══════════════════════════════════════════════════════════════════════════
-- Fungsi lookup lintas-tenant
--
-- Konsekuensi langsung dari dua perbaikan sebelumnya:
--
-- 1. T-05 mengubah email menjadi unik **per organisasi**, bukan global. Maka
--    `email + password` saja tidak lagi cukup untuk login — dua organisasi
--    boleh punya `erik@msp.id` yang berbeda orang. Login sekarang wajib
--    menyertakan `org_slug`.
--
-- 2. T-07 mengaktifkan FORCE ROW LEVEL SECURITY. Akibatnya query apa pun
--    harus tahu tenant-nya lebih dulu — padahal saat login dan saat Quick
--    Connect, tenant justru **belum** diketahui.
--
-- Jalan keluarnya adalah beberapa fungsi SECURITY DEFINER yang sangat sempit:
-- masing-masing hanya mengembalikan kolom minimum yang dibutuhkan untuk
-- menentukan tenant, dan tidak satu pun mengembalikan data pengguna lain.
-- ═══════════════════════════════════════════════════════════════════════════

-- ── Slug organisasi → id ────────────────────────────────────────────────────
-- Tidak membocorkan apa pun yang belum publik: slug memang muncul di URL.
CREATE OR REPLACE FUNCTION resolve_org_by_slug(p_slug TEXT)
RETURNS UUID
LANGUAGE sql
SECURITY DEFINER
STABLE
SET search_path = public
AS $$
    SELECT id FROM organizations WHERE slug = p_slug
$$;

-- ── Kredensial login ────────────────────────────────────────────────────────
-- Mengembalikan hash, bukan password. Pemanggil tetap wajib memverifikasi.
CREATE OR REPLACE FUNCTION resolve_login(p_org_slug TEXT, p_email TEXT)
RETURNS TABLE (
    user_id       UUID,
    org_id        UUID,
    password_hash VARCHAR(255),
    status        VARCHAR(50),
    mfa_enabled   BOOLEAN
)
LANGUAGE sql
SECURITY DEFINER
STABLE
SET search_path = public
AS $$
    SELECT u.id, u.organization_id, u.password_hash, u.status, u.mfa_enabled
    FROM users u
    JOIN organizations o ON o.id = u.organization_id
    WHERE o.slug = p_org_slug
      AND lower(u.email) = lower(p_email)
$$;

-- ── Device ID → tenant + kredensial sesi ────────────────────────────────────
-- Dipakai alur Quick Connect, yang menurut definisinya belum tahu tenant.
CREATE OR REPLACE FUNCTION resolve_quick_connect(p_device_id CHAR(9))
RETURNS TABLE (
    device_uuid   UUID,
    org_id        UUID,
    password_hash VARCHAR(255),
    enabled       BOOLEAN,
    status        VARCHAR(50)
)
LANGUAGE sql
SECURITY DEFINER
STABLE
SET search_path = public
AS $$
    SELECT d.id, d.organization_id, d.session_password_hash,
           d.quick_connect_enabled, d.status
    FROM devices d
    WHERE d.device_id = p_device_id
$$;

-- ── Pencatatan upaya Quick Connect ──────────────────────────────────────────
-- Upaya harus tercatat meskipun device ID-nya tidak pernah ada — justru baris
-- itulah sinyal pemindaian ruang ID (QUICK_CONNECT.md §5, §8).
CREATE OR REPLACE FUNCTION log_quick_connect_attempt(
    p_device_id_input VARCHAR(9),
    p_source_ip       INET,
    p_outcome         VARCHAR(20),
    p_viewer_user_id  UUID DEFAULT NULL
)
RETURNS VOID
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
AS $$
    INSERT INTO quick_connect_attempts
        (device_id_input, source_ip, outcome, viewer_user_id)
    VALUES (p_device_id_input, p_source_ip, p_outcome, p_viewer_user_id)
$$;

-- ── Hak akses ───────────────────────────────────────────────────────────────
-- Fungsi dicabut dari PUBLIC lalu diberikan hanya ke role yang membutuhkan.
DO $$
DECLARE f TEXT;
BEGIN
    FOREACH f IN ARRAY ARRAY[
        'resolve_org_by_slug(text)',
        'resolve_login(text,text)',
        'resolve_quick_connect(character)',
        'log_quick_connect_attempt(character varying,inet,character varying,uuid)'
    ]
    LOOP
        EXECUTE format('REVOKE ALL ON FUNCTION %s FROM PUBLIC', f);
        EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO aetherdesk_app', f);
    END LOOP;
END $$;
