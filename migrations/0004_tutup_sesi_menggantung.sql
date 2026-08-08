-- ═══════════════════════════════════════════════════════════════════════════
-- Menutup sesi yang menggantung sebelum siklus hidupnya diimplementasikan
--
-- `POST /api/v1/connect` menyisipkan baris berstatus `pending`, tetapi sampai
-- Signal Server diberi tanggung jawab memajukannya, tidak ada satu pun jalur
-- kode yang pernah mengubah status itu. Setiap sesi yang pernah dibuat — baik
-- yang berhasil tersambung maupun yang ditolak — tetap tercatat `pending`
-- dengan `ended_at` kosong.
--
-- Migrasi ini memperbaiki barisnya, bukan menghapusnya. Riwayat tetap utuh;
-- yang berubah hanya statusnya menjadi keadaan yang jujur.
--
-- Ambang 15 menit dipilih dari makna `pending` itu sendiri: status ini berarti
-- "menunggu persetujuan di perangkat tujuan". Permintaan yang belum disetujui
-- setelah seperempat jam tidak akan pernah disetujui — orangnya sudah pergi.
--
-- Percobaan pertama memakai ambang satu jam dan melaporkan `UPDATE 0`, karena
-- sesi yang hendak dibersihkan baru berumur belasan menit. Ambang yang terlalu
-- longgar membuat migrasi tampak berhasil padahal tidak menyentuh apa pun.
--
-- ⚠ JALANKAN SEBAGAI SUPERUSER:
--
--     sudo -u postgres psql -d aetherdesk -f 0004_tutup_sesi_menggantung.sql
--
-- `sessions` memakai FORCE ROW LEVEL SECURITY. Dijalankan sebagai role
-- `aetherdesk` tanpa menetapkan `aetherdesk.current_org`, policy menyaring
-- seluruh baris dan pernyataan di bawah melaporkan `UPDATE 0` — berhasil,
-- diam, dan tidak mengubah apa pun.
--
-- Ini kelas jebakan yang sama dengan yang membuat login selalu 401 pada
-- migrasi 0003: RLS tidak melempar galat, ia hanya menyembunyikan baris.
-- Setiap migrasi pemeliharaan yang harus menyentuh seluruh tenant wajib
-- dijalankan oleh role yang mem-bypass RLS.
-- ═══════════════════════════════════════════════════════════════════════════

UPDATE sessions
SET status   = 'disconnected',
    ended_at = started_at
WHERE ended_at IS NULL
  AND status = 'pending'
  AND started_at < now() - INTERVAL '15 minutes';

-- `ended_at` disamakan dengan `started_at`, bukan `now()`. Durasi sesi-sesi ini
-- memang tidak diketahui, dan mencatatnya sebagai nol jauh lebih jujur daripada
-- melaporkan durasi berjam-jam yang tidak pernah terjadi — angka palsu itu akan
-- mencemari setiap laporan pemakaian yang dihitung darinya.
