-- ═══════════════════════════════════════════════════════════════════════════
-- Identitas perangkat — kunci Ed25519 dan token enrolment
--
-- Menutup celah yang menghalangi sisa M1: agent tidak punya kredensial
-- miliknya sendiri.
--
-- Sampai sekarang `POST /api/v1/devices` mewajibkan JWT **pengguna**, dan
-- `AUTH` di Signal Server juga menerima token pengguna. Agent yang berjalan
-- tanpa pengawasan pada mesin orang lain tidak boleh menyimpan kredensial
-- pengguna — bila mesin itu dibongkar, yang bocor adalah akun manusianya
-- beserta seluruh perangkat organisasi, bukan satu perangkat.
--
-- Alurnya sekarang tiga langkah:
--
--   1. Pengguna menerbitkan token enrolment sekali pakai dari dashboard
--   2. Agent menukarnya dengan mendaftarkan kunci publik Ed25519 miliknya
--   3. Seterusnya agent membuktikan dirinya dengan menandatangani tantangan,
--      dan menerima JWT perangkat berumur pendek
--
-- Kunci privat tidak pernah meninggalkan mesin agent, dan server tidak pernah
-- memilikinya. Ini juga prasyarat ADR-008, yang mewajibkan SDP ditandatangani
-- device key — tanpa kunci publik tersimpan, tanda tangan itu tidak ada yang
-- bisa memverifikasi.
-- ═══════════════════════════════════════════════════════════════════════════

-- ── 1. Kunci publik perangkat ───────────────────────────────────────────────
--
-- Tabel `device_keys` sudah ada sejak migrasi 0001 dan dirancang persis untuk
-- keperluan ini: satu kunci aktif per perangkat, kunci lama tetap tersimpan
-- untuk audit, dengan `revoked_at` untuk pencabutan. Menambahkan kolom kunci
-- ke `devices` akan menduplikasinya sekaligus membuang kemampuan rotasi yang
-- sudah dirancang matang di sana.
--
-- Yang perlu disesuaikan hanya satu: kolom `certificate`.

-- Fase 0 belum punya CA, dan kunci Ed25519 mentah tanpa sertifikat adalah
-- keadaan yang sah — ADR-008 mensyaratkan tanda tangan kunci perangkat, bukan
-- rantai sertifikat. Memaksa kolom ini terisi hanya akan menghasilkan string
-- kosong yang berpura-pura menjadi sertifikat.
ALTER TABLE device_keys ALTER COLUMN certificate DROP NOT NULL;

COMMENT ON COLUMN device_keys.public_key IS
    'Kunci publik Ed25519, base64url tanpa padding (43 karakter).';
COMMENT ON COLUMN device_keys.certificate IS
    'Sertifikat perangkat. NULL selama Fase 0 — belum ada CA. '
    'Lihat ADR-008 dan SECURITY.md.';

-- Satu kunci publik tidak boleh dipakai dua perangkat sekaligus: bila terjadi,
-- dua mesin dapat saling menyamar dan jejak auditnya menjadi ambigu.
-- Dibatasi ke kunci yang masih aktif, supaya kunci lama yang sudah dirotasi
-- tidak menghalangi apa pun.
CREATE UNIQUE INDEX idx_device_keys_publik_unik
    ON device_keys (public_key) WHERE is_active AND revoked_at IS NULL;

-- `device_keys` tidak pernah ikut diberi RLS di migrasi 0001, padahal ia
-- memuat identitas perangkat. Selama tabelnya kosong hal itu tidak berakibat
-- apa-apa; begitu enrolment mengisinya, `aetherdesk_app` dapat membaca kunci
-- milik seluruh organisasi. Ditutup sekarang, sebelum ada baris pertama.
--
-- Policy-nya menumpang pada RLS `devices`: subquery di bawah ini pun tersaring
-- policy milik `devices`, jadi organisasi lain tidak akan pernah cocok.
ALTER TABLE device_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_keys FORCE  ROW LEVEL SECURITY;

CREATE POLICY org_isolation ON device_keys
    USING (EXISTS (
        SELECT 1 FROM devices d
        WHERE d.id = device_keys.device_id
          AND d.organization_id = current_org()
    ));

ALTER TABLE devices ADD COLUMN enrolled_at TIMESTAMPTZ;

COMMENT ON COLUMN devices.enrolled_at IS
    'Kapan perangkat menyelesaikan enrolment. NULL untuk perangkat berbasis '
    'browser yang didaftarkan lewat dashboard tanpa kunci.';

-- ── 2. Token enrolment ──────────────────────────────────────────────────────

CREATE TABLE device_enrolment_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,

    -- SHA-256 dari token, bukan Argon2id.
    --
    -- Ini menyimpang dari password sesi di tabel `devices`, dan sengaja.
    -- Argon2id ada untuk melindungi rahasia beruang rendah yang dipilih
    -- manusia. Token ini 256 bit acak dari CSPRNG: menebaknya sudah mustahil
    -- terlepas dari fungsi hash yang dipakai, sehingga peregangan kunci hanya
    -- menambah ~60 ms pada setiap enrolment tanpa menambah keamanan apa pun.
    token_hash      BYTEA NOT NULL UNIQUE,

    -- Ditampilkan pada perangkat setelah enrolment, supaya operator dapat
    -- mengenali mesin yang baru didaftarkan tanpa harus menebak.
    alias           VARCHAR(255),

    created_by      UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at      TIMESTAMPTZ NOT NULL,

    -- Sekali pakai. Diisi lewat UPDATE ... WHERE used_at IS NULL RETURNING,
    -- yang menjadikan klaim token operasi atomik — dua agent yang berlomba
    -- dengan token sama hanya menghasilkan satu perangkat.
    used_at         TIMESTAMPTZ,
    device_uuid     UUID REFERENCES devices (id) ON DELETE SET NULL,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT enrolment_token_hash_sha256 CHECK (octet_length(token_hash) = 32),

    -- Token yang menunjuk sebuah perangkat wajib sudah ditandai terpakai.
    --
    -- Arah sebaliknya **tidak** dipaksakan, dan itu bukan kelalaian. Token
    -- diklaim lebih dulu, perangkatnya baru dibuat sesudahnya — di antara
    -- keduanya ada keadaan sah `used_at IS NOT NULL AND device_uuid IS NULL`.
    -- Versi pertama constraint ini menuliskan kesetaraan penuh dan membuat
    -- `claim_enrolment_token` mustahil dijalankan; ketahuan pada uji fungsional
    -- pertama.
    --
    -- Keadaan itu juga bukan sekadar transisi sesaat: bila pembuatan perangkat
    -- gagal setelah token diklaim, inilah keadaan akhir yang benar. Token
    -- sudah habis dan tidak boleh dapat dipakai ulang, justru karena
    -- percobaannya pernah terjadi.
    CONSTRAINT enrolment_perangkat_wajib_terpakai
        CHECK (device_uuid IS NULL OR used_at IS NOT NULL)
);

CREATE INDEX idx_enrolment_org ON device_enrolment_tokens (organization_id);
CREATE INDEX idx_enrolment_belum_terpakai
    ON device_enrolment_tokens (expires_at) WHERE used_at IS NULL;

ALTER TABLE device_enrolment_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE device_enrolment_tokens FORCE  ROW LEVEL SECURITY;

CREATE POLICY org_isolation ON device_enrolment_tokens
    USING (organization_id = current_org());

GRANT SELECT, INSERT, UPDATE, DELETE ON device_enrolment_tokens TO aetherdesk_app;

-- ── 3. Nonce autentikasi perangkat ──────────────────────────────────────────
--
-- Perlindungan replay untuk `POST /api/v1/devices/token` sebenarnya hidup di
-- Redis, tempat nonce kedaluwarsa sendiri. Tabel ini **bukan** duplikatnya —
-- ia hanya mencatat kegagalan, dan itu jejak yang berbeda sifatnya: tanda
-- tangan salah pada perangkat yang sah berarti seseorang sedang mencoba
-- menyamar, dan itu perlu terlihat setelah kejadiannya lewat.

CREATE TABLE device_auth_attempts (
    id            BIGSERIAL PRIMARY KEY,
    device_uuid   UUID,
    source_ip     INET NOT NULL,
    outcome       VARCHAR(30) NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT device_auth_outcome_valid CHECK (outcome IN (
        'ok', 'unknown_device', 'not_enrolled',
        'bad_signature', 'stale_timestamp', 'replayed_nonce'
    ))
);

CREATE INDEX idx_device_auth_gagal
    ON device_auth_attempts (created_at DESC) WHERE outcome <> 'ok';

GRANT SELECT, INSERT ON device_auth_attempts TO aetherdesk_app;
GRANT USAGE, SELECT ON SEQUENCE device_auth_attempts_id_seq TO aetherdesk_app;
-- Sejalan dengan audit_logs dan quick_connect_attempts: hanya boleh ditambah.
REVOKE UPDATE, DELETE ON device_auth_attempts FROM aetherdesk_app;

-- ═══════════════════════════════════════════════════════════════════════════
-- 4. Fungsi lintas-tenant
--
-- Enrolment dan autentikasi perangkat sama-sama terjadi **sebelum** tenant
-- diketahui — agent hanya memegang token atau kunci, bukan sesi pengguna.
-- Sama persis dengan alasan `resolve_login` dan `resolve_quick_connect` ada.
-- Masing-masing dibuat sesempit mungkin dan tidak mengembalikan satu pun
-- kolom yang tidak dibutuhkan pemanggilnya.
-- ═══════════════════════════════════════════════════════════════════════════

-- ── Klaim token enrolment ───────────────────────────────────────────────────
-- Menandai terpakai dan mengembalikan tenant dalam satu pernyataan. Sekali
-- pakai ditegakkan oleh `used_at IS NULL` di klausa WHERE, bukan oleh
-- pemeriksaan terpisah yang bisa disusupi balapan.
CREATE OR REPLACE FUNCTION claim_enrolment_token(p_token_hash BYTEA)
RETURNS TABLE (
    token_id UUID,
    org_id   UUID,
    alias    VARCHAR(255)
)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
AS $$
    UPDATE device_enrolment_tokens
    SET used_at = now()
    WHERE token_hash = p_token_hash
      AND used_at IS NULL
      AND expires_at > now()
    RETURNING id, organization_id, device_enrolment_tokens.alias
$$;

-- ── Menautkan token ke perangkat yang dihasilkannya ─────────────────────────
-- Dipanggil setelah baris `devices` terbentuk. Dipisah dari fungsi di atas
-- karena perangkatnya memang belum ada saat token diklaim, dan
-- `enrolment_terpakai_punya_perangkat` menahan keduanya tetap konsisten.
CREATE OR REPLACE FUNCTION link_enrolment_token(p_token_id UUID, p_device_uuid UUID)
RETURNS VOID
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
AS $$
    UPDATE device_enrolment_tokens
    SET device_uuid = p_device_uuid
    WHERE id = p_token_id
$$;

-- ── Pendaftaran perangkat hasil enrolment ───────────────────────────────────
-- Tenant sudah ditentukan oleh token, bukan oleh pemanggil. Itu sebabnya
-- `p_org_id` aman di sini: nilainya berasal dari `claim_enrolment_token`,
-- tidak pernah dari badan request.
--
-- Dua tabel ditulis, jadi plpgsql — dan keduanya berada dalam satu transaksi
-- pemanggil, sehingga perangkat tanpa kunci tidak pernah sempat ada.
CREATE OR REPLACE FUNCTION enrol_device(
    p_org_id        UUID,
    p_device_id     CHAR(9),
    p_public_key    TEXT,
    p_alias         VARCHAR(255),
    p_os_type       VARCHAR(50),
    p_os_version    VARCHAR(100),
    p_hostname      VARCHAR(255),
    p_password_hash VARCHAR(255)
)
RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    v_device_uuid UUID;
BEGIN
    INSERT INTO devices
        (organization_id, device_id, alias, os_type, os_version, hostname,
         enrolled_at, session_password_hash, session_password_set_at)
    VALUES
        (p_org_id, p_device_id, p_alias, p_os_type, p_os_version, p_hostname,
         now(), p_password_hash, now())
    ON CONFLICT (device_id) DO NOTHING
    RETURNING id INTO v_device_uuid;

    -- Tabrakan device ID. Pemanggil akan mencoba ID lain.
    IF v_device_uuid IS NULL THEN
        RETURN NULL;
    END IF;

    -- `expires_at` sepuluh tahun ke depan, dan angkanya perlu dijelaskan:
    -- kolomnya NOT NULL sejak migrasi 0001, sementara agent tanpa pengawasan
    -- yang kuncinya kedaluwarsa sendiri akan menjadi mesin yang tidak lagi
    -- terjangkau tanpa ada yang menyadarinya. Mekanisme yang dimaksudkan untuk
    -- mengganti kunci adalah **rotasi** lewat `is_active` dan `revoked_at`,
    -- bukan kedaluwarsa diam-diam.
    INSERT INTO device_keys (device_id, public_key, certificate, expires_at)
    VALUES (v_device_uuid, p_public_key, NULL, now() + INTERVAL '10 years');

    RETURN v_device_uuid;
END $$;

-- ── Kunci publik perangkat ──────────────────────────────────────────────────
-- Hanya mengembalikan apa yang dibutuhkan untuk memverifikasi tanda tangan
-- dan menerbitkan token: kunci dan tenant. Tidak ada hash password, tidak ada
-- metadata perangkat.
--
-- Kunci yang dicabut maupun kedaluwarsa tidak ikut terbawa, sehingga
-- pencabutan berlaku seketika pada upaya autentikasi berikutnya — tanpa perlu
-- satu pun pemeriksaan tambahan di sisi aplikasi yang bisa lupa ditulis.
CREATE OR REPLACE FUNCTION resolve_device_key(p_device_uuid UUID)
RETURNS TABLE (
    org_id     UUID,
    public_key TEXT
)
LANGUAGE sql
SECURITY DEFINER
STABLE
SET search_path = public
AS $$
    SELECT d.organization_id, k.public_key
    FROM devices d
    LEFT JOIN device_keys k
           ON k.device_id = d.id
          AND k.is_active
          AND k.revoked_at IS NULL
          AND k.expires_at > now()
    WHERE d.id = p_device_uuid
$$;

-- ── Pencatatan upaya autentikasi perangkat ──────────────────────────────────
CREATE OR REPLACE FUNCTION log_device_auth_attempt(
    p_device_uuid UUID,
    p_source_ip   INET,
    p_outcome     VARCHAR(30)
)
RETURNS VOID
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
AS $$
    INSERT INTO device_auth_attempts (device_uuid, source_ip, outcome)
    VALUES (p_device_uuid, p_source_ip, p_outcome)
$$;

-- ── Heartbeat ───────────────────────────────────────────────────────────────
-- Agent memegang JWT perangkat, bukan sesi pengguna, jadi jalur ini pun tidak
-- punya `aetherdesk.current_org`. Tenant diambil dari klaim token yang sudah
-- diverifikasi dan ikut menjadi syarat WHERE — perangkat tidak dapat
-- memperbarui baris milik organisasi lain sekalipun UUID-nya ditebak benar.
--
-- **Kolom `status` sengaja tidak disentuh.** Kepemilikannya ada pada Signal
-- Server, yang menandai online saat WebSocket tersambung dan offline seketika
-- saat putus (butir 20 di worklog, temuan S-09). Bila heartbeat ikut menulis
-- 'online', agent yang WebSocket-nya baru saja putus akan menghidupkan kembali
-- statusnya pada detak berikutnya — persis bug "tampak online padahal tidak
-- terjangkau" yang sudah diperbaiki, kembali lewat pintu lain.
--
-- Yang dicatat di sini adalah keterjangkauan dan metadata, bukan kehadiran.
CREATE OR REPLACE FUNCTION device_heartbeat(
    p_device_uuid    UUID,
    p_org_id         UUID,
    p_os_version     VARCHAR(100),
    p_hostname       VARCHAR(255),
    p_client_version VARCHAR(50)
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
    SET last_heartbeat = now(),
        os_version     = COALESCE(p_os_version, os_version),
        hostname       = COALESCE(p_hostname, hostname),
        client_version = COALESCE(p_client_version, client_version),
        updated_at     = now()
    WHERE id = p_device_uuid
      AND organization_id = p_org_id;

    GET DIAGNOSTICS terpengaruh = ROW_COUNT;
    RETURN terpengaruh > 0;
END $$;

-- ═══════════════════════════════════════════════════════════════════════════
-- 5. Kepemilikan dan hak akses
--
-- Sama seperti migrasi 0003: SECURITY DEFINER saja tidak mem-bypass RLS.
-- Yang mem-bypass adalah atribut BYPASSRLS pada role pemilik fungsi.
-- ═══════════════════════════════════════════════════════════════════════════

GRANT SELECT, INSERT, UPDATE ON device_enrolment_tokens TO aetherdesk_lookup;
GRANT INSERT ON device_auth_attempts TO aetherdesk_lookup;
GRANT USAGE, SELECT ON SEQUENCE device_auth_attempts_id_seq TO aetherdesk_lookup;
GRANT INSERT, UPDATE ON devices TO aetherdesk_lookup;
GRANT SELECT, INSERT, UPDATE ON device_keys TO aetherdesk_lookup;

DO $$
DECLARE f TEXT;
BEGIN
    FOREACH f IN ARRAY ARRAY[
        'claim_enrolment_token(bytea)',
        'link_enrolment_token(uuid,uuid)',
        'enrol_device(uuid,character,text,character varying,character varying,character varying,character varying,character varying)',
        'resolve_device_key(uuid)',
        'log_device_auth_attempt(uuid,inet,character varying)',
        'device_heartbeat(uuid,uuid,character varying,character varying,character varying)'
    ]
    LOOP
        EXECUTE format('ALTER FUNCTION %s OWNER TO aetherdesk_lookup', f);
        EXECUTE format('REVOKE ALL ON FUNCTION %s FROM PUBLIC', f);
        EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO aetherdesk, aetherdesk_app', f);
    END LOOP;
END $$;
