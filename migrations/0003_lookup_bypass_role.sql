-- ═══════════════════════════════════════════════════════════════════════════
-- Role pemilik fungsi lookup
--
-- Memperbaiki bug yang ditemukan uji end-to-end: login selalu 401 meskipun
-- kredensialnya benar.
--
-- Sebabnya: `FORCE ROW LEVEL SECURITY` berlaku pada pemilik tabel **termasuk
-- di dalam fungsi SECURITY DEFINER yang dimiliki role yang sama**. Fungsi
-- `resolve_login` berjalan sebagai `aetherdesk`, tetap terkena RLS, dan karena
-- `aetherdesk.current_org` memang belum diketahui pada tahap login, policy
-- menyaring seluruh baris. Fungsi mengembalikan nol baris, dan pemanggil
-- menyimpulkan kredensialnya salah.
--
-- `SECURITY DEFINER` **bukan** mekanisme bypass RLS. Yang mem-bypass RLS
-- adalah atribut `BYPASSRLS` pada role, atau status superuser.
--
-- Perbaikannya memakai role khusus, bukan `postgres`. Menjadikan superuser
-- sebagai pemilik fungsi SECURITY DEFINER berarti setiap cacat di dalamnya
-- berakibat kompromi total. Role di bawah ini hanya memiliki BYPASSRLS dan
-- tidak dapat login.
-- ═══════════════════════════════════════════════════════════════════════════

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aetherdesk_lookup') THEN
        CREATE ROLE aetherdesk_lookup NOLOGIN BYPASSRLS;
    ELSE
        ALTER ROLE aetherdesk_lookup BYPASSRLS;
    END IF;
END $$;

-- Role pemilik fungsi membutuhkan akses ke tabel yang dibacanya.
GRANT USAGE ON SCHEMA public TO aetherdesk_lookup;
GRANT SELECT ON organizations, users, devices TO aetherdesk_lookup;
GRANT INSERT ON quick_connect_attempts TO aetherdesk_lookup;

ALTER FUNCTION resolve_org_by_slug(TEXT)          OWNER TO aetherdesk_lookup;
ALTER FUNCTION resolve_login(TEXT, TEXT)          OWNER TO aetherdesk_lookup;
ALTER FUNCTION resolve_quick_connect(CHAR(9))     OWNER TO aetherdesk_lookup;
ALTER FUNCTION log_quick_connect_attempt(VARCHAR(9), INET, VARCHAR(20), UUID)
    OWNER TO aetherdesk_lookup;

-- Kepemilikan berpindah, jadi hak EXECUTE dipasang ulang.
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
        EXECUTE format('GRANT EXECUTE ON FUNCTION %s TO aetherdesk, aetherdesk_app', f);
    END LOOP;
END $$;
