-- ═══════════════════════════════════════════════════════════════════════════
-- Alias perangkat dan kata sandi tetap
--
-- Dua kemampuan yang diminta agar AetherDesk terasa seperti aplikasi remote
-- desktop yang biasa dipakai orang: perangkat dapat diberi nama yang mudah
-- diingat, dan pemiliknya dapat menetapkan kata sandi sendiri untuk mesin yang
-- ia akses berulang kali.
--
-- Keduanya sengaja **tidak** menggantikan yang sudah ada.
--
-- ## Nomor perangkat tetap tidak dapat diubah
--
-- Permintaan awalnya "bisa ganti ID sendiri". Yang ditambahkan di sini adalah
-- alias, bukan penggantian nomor, dan alasannya bukan kemalasan:
--
-- 1. Nomor sembilan digit membawa check digit Damm (QUICK_CONNECT.md §2.3).
--    Nomor pilihan pengguna menghapus jaminan itu.
-- 2. `sessions.device_id_snapshot` menyimpan nomor apa adanya supaya riwayat
--    bertahan meski perangkatnya dihapus. Nomor yang berpindah tangan membuat
--    riwayat lama menunjuk mesin yang salah.
-- 3. Nomor yang dapat dipilih mengundang penyamaran — mengambil nomor yang
--    mirip milik mesin lain agar orang salah menyambung.
--
-- AnyDesk mengambil keputusan yang sama: nomornya tetap, yang dapat diubah
-- adalah aliasnya.
--
-- ## Dua kata sandi, bukan satu
--
-- Kata sandi sesi yang sudah ada bersifat acak 40 bit dan berotasi setelah
-- setiap sesi — tepat untuk bantuan sesaat yang dibacakan lewat telepon.
--
-- Kata sandi tetap dipilih manusia, dan manusia memilih kata sandi yang buruk.
-- Karena itu ia dipisah, bukan menggantikan: yang satu tetap acak dan berumur
-- pendek, yang lain dijaga panjang minimumnya dan dilindungi pembatasan laju
-- yang sudah berlaku (5 kegagalan menjeda 15 menit per perangkat).
-- ═══════════════════════════════════════════════════════════════════════════

-- ── 1. Alias ────────────────────────────────────────────────────────────────

ALTER TABLE devices ADD COLUMN handle VARCHAR(32);

COMMENT ON COLUMN devices.handle IS
    'Alias unik per organisasi, dapat dipakai Quick Connect menggantikan nomor. '
    'Berbeda dari kolom alias, yang sekadar nama tampilan dan boleh sama.';

-- Bentuknya dibatasi ketat, dan setiap batasan punya sebabnya:
--
--   huruf kecil saja  — supaya `PC-Kantor` dan `pc-kantor` tidak pernah menjadi
--                       dua perangkat berbeda yang membingungkan saat diketik
--   tanpa spasi       — alias dibacakan lewat telepon sama seperti nomor
--   tidak diawali '-' — menghindari alias yang tampak seperti argumen perintah
--   minimal 3         — alias satu huruf akan habis diperebutkan
ALTER TABLE devices ADD CONSTRAINT devices_handle_bentuk
    CHECK (handle IS NULL OR handle ~ '^[a-z0-9][a-z0-9_-]{2,31}$');

-- Angka sembilan digit dilarang menjadi alias. Tanpa ini, seseorang dapat
-- mengambil alias yang persis sama dengan nomor perangkat orang lain, dan
-- Quick Connect tidak akan pernah dapat memutuskan mana yang dimaksud.
ALTER TABLE devices ADD CONSTRAINT devices_handle_bukan_nomor
    CHECK (handle IS NULL OR handle !~ '^[0-9]{9}$');

CREATE UNIQUE INDEX idx_devices_handle
    ON devices (organization_id, handle) WHERE handle IS NOT NULL;

-- ── 2. Kata sandi tetap ─────────────────────────────────────────────────────

ALTER TABLE devices
    ADD COLUMN unattended_password_hash   VARCHAR(255),
    ADD COLUMN unattended_password_set_at TIMESTAMPTZ;

COMMENT ON COLUMN devices.unattended_password_hash IS
    'Argon2id atas kata sandi tetap pilihan pemilik perangkat. NULL berarti '
    'akses tanpa pengawasan belum dinyalakan, dan hanya kata sandi sesi yang '
    'berlaku.';

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. Resolusi Quick Connect yang menerima nomor maupun alias
--
-- Menggantikan `resolve_quick_connect`, yang hanya mengenal nomor sembilan
-- digit dan hanya mengembalikan satu hash. Fungsi lama dibiarkan hidup supaya
-- biner lama yang masih berjalan tidak tiba-tiba kehilangan jalur — migrasi
-- ini aditif, dan penggantian biner menyusul sesudahnya.
-- ═══════════════════════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION resolve_connect_key(p_kunci TEXT)
RETURNS TABLE (
    device_uuid       UUID,
    org_id            UUID,
    session_hash      VARCHAR(255),
    unattended_hash   VARCHAR(255),
    enabled           BOOLEAN,
    status            VARCHAR(50)
)
LANGUAGE sql
SECURITY DEFINER
STABLE
SET search_path = public
AS $$
    SELECT d.id, d.organization_id, d.session_password_hash,
           d.unattended_password_hash, d.quick_connect_enabled, d.status
    FROM devices d
    WHERE d.device_id = p_kunci
       OR d.handle = lower(p_kunci)
    -- Nomor didahulukan bila keduanya entah bagaimana cocok. Constraint
    -- `devices_handle_bukan_nomor` seharusnya membuat itu mustahil, tetapi
    -- urutan yang pasti lebih baik daripada urutan yang kebetulan.
    ORDER BY (d.device_id = p_kunci) DESC
    LIMIT 1
$$;

-- ── 4. Swalayan perangkat ───────────────────────────────────────────────────
--
-- Perangkat mengubah aliasnya dan kata sandinya sendiri, memakai token
-- perangkat — bukan sesi pengguna. Inilah yang membuat aplikasi Windows dapat
-- menampilkan dan mengubah keduanya tanpa pemiliknya perlu membuka dashboard.
--
-- Tenant selalu ikut menjadi syarat, sehingga perangkat tidak pernah dapat
-- menyentuh baris milik organisasi lain sekalipun UUID-nya ditebak benar.

CREATE OR REPLACE FUNCTION set_device_handle(
    p_device_uuid UUID,
    p_org_id      UUID,
    p_handle      VARCHAR(32)
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    terpengaruh INTEGER;
BEGIN
    UPDATE devices
    SET handle = p_handle, updated_at = now()
    WHERE id = p_device_uuid AND organization_id = p_org_id;

    GET DIAGNOSTICS terpengaruh = ROW_COUNT;
    RETURN terpengaruh > 0;
END $$;

CREATE OR REPLACE FUNCTION set_device_passwords(
    p_device_uuid     UUID,
    p_org_id          UUID,
    p_session_hash    VARCHAR(255),
    p_unattended_hash VARCHAR(255),
    -- Membedakan "jangan sentuh" dari "hapus". NULL saja tidak cukup: kata
    -- sandi tetap memang boleh dikosongkan untuk mematikan akses tanpa
    -- pengawasan, dan itu perintah yang sah, bukan ketiadaan perintah.
    p_ubah_session    BOOLEAN,
    p_ubah_unattended BOOLEAN
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    terpengaruh INTEGER;
BEGIN
    UPDATE devices
    SET session_password_hash =
            CASE WHEN p_ubah_session THEN p_session_hash ELSE session_password_hash END,
        session_password_set_at =
            CASE WHEN p_ubah_session THEN now() ELSE session_password_set_at END,
        unattended_password_hash =
            CASE WHEN p_ubah_unattended THEN p_unattended_hash ELSE unattended_password_hash END,
        unattended_password_set_at =
            CASE WHEN p_ubah_unattended THEN
                     CASE WHEN p_unattended_hash IS NULL THEN NULL ELSE now() END
                 ELSE unattended_password_set_at END,
        updated_at = now()
    WHERE id = p_device_uuid AND organization_id = p_org_id;

    GET DIAGNOSTICS terpengaruh = ROW_COUNT;
    RETURN terpengaruh > 0;
END $$;

-- Ringkasan yang ditampilkan aplikasi Windows pada jendelanya.
CREATE OR REPLACE FUNCTION device_self(p_device_uuid UUID, p_org_id UUID)
RETURNS TABLE (
    device_id        CHAR(9),
    handle           VARCHAR(32),
    alias            VARCHAR(255),
    org_slug         VARCHAR(100),
    org_name         VARCHAR(255),
    punya_sandi_tetap BOOLEAN,
    status           VARCHAR(50)
)
LANGUAGE sql
SECURITY DEFINER
STABLE
SET search_path = public
AS $$
    SELECT d.device_id, d.handle, d.alias, o.slug, o.name,
           d.unattended_password_hash IS NOT NULL, d.status
    FROM devices d
    JOIN organizations o ON o.id = d.organization_id
    WHERE d.id = p_device_uuid AND d.organization_id = p_org_id
$$;

-- ── 5. Kepemilikan dan hak akses ────────────────────────────────────────────

DO $$
DECLARE f TEXT;
BEGIN
    FOREACH f IN ARRAY ARRAY[
        'resolve_connect_key(text)',
        'set_device_handle(uuid,uuid,character varying)',
        'set_device_passwords(uuid,uuid,character varying,character varying,boolean,boolean)',
        'device_self(uuid,uuid)'
    ]
    LOOP
        EXECUTE format('ALTER FUNCTION %s OWNER TO aetherdesk_lookup', f);
        EXECUTE format('REVOKE ALL ON FUNCTION %s FROM PUBLIC', f);
        EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO aetherdesk, aetherdesk_app', f);
    END LOOP;
END $$;
