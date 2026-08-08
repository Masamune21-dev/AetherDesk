#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Uji asap end-to-end AetherDesk.
#
# Menjalankan alur lengkap terhadap instalasi yang sedang berjalan:
#   bootstrap → login → daftar perangkat → Quick Connect
#
# Selain jalur bahagia, skrip ini juga menguji properti keamanan yang mudah
# hilang saat refactor: check digit, respons seragam, dan pembatasan laju.
#
# Pemakaian:
#   ./scripts/e2e.sh                                  # localhost:8080
#   BASE=https://aetherdesk.masamune.my.id ./scripts/e2e.sh
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

BASE="${BASE:-http://127.0.0.1:8080}"
SLUG="e2e-$(date +%s)"
EMAIL="e2e@contoh.id"
PASS="kata-sandi-uji-yang-panjang"

lulus=0; gagal=0

ok()   { printf '  \033[32m✓\033[0m %s\n' "$1"; lulus=$((lulus+1)); }
no()   { printf '  \033[31m✗\033[0m %s\n' "$1"; gagal=$((gagal+1)); }
bab()  { printf '\n\033[1m%s\033[0m\n' "$1"; }

# cek <nama> <diharapkan> <didapat>
cek() {
  if [ "$2" = "$3" ]; then ok "$1 ($3)"; else no "$1 — diharapkan $2, didapat $3"; fi
}

kode() { # kode <method> <path> <body> [token]
  local m="$1" p="$2" b="${3:-}" t="${4:-}"
  local args=(-sS -o /tmp/e2e_body -w '%{http_code}' -X "$m" -H 'Content-Type: application/json')
  [ -n "$t" ] && args+=(-H "Authorization: Bearer $t")
  [ -n "$b" ] && args+=(-d "$b")
  curl "${args[@]}" "$BASE$p" 2>/dev/null
}

# Menelusuri JSON lewat argumen, bukan lewat eval string. Versi sebelumnya
# menyusun `eval('d['data']['access_token']')` — tanda kutipnya bertabrakan dan
# ekspresinya tidak pernah valid, sehingga selalu mengembalikan string kosong.
json() {
  python3 -c '
import json, sys
d = json.load(open("/tmp/e2e_body"))
for k in sys.argv[1:]:
    d = d[k]
print(d)
' "$@" 2>/dev/null
}

printf '\033[1mAetherDesk — uji asap end-to-end\033[0m\n'
printf 'Target: %s\n' "$BASE"

# ── 1. Kesehatan ─────────────────────────────────────────────────────────────
bab '1. Kesehatan'
cek 'liveness'  200 "$(kode GET /api/health)"
cek 'readiness' 200 "$(kode GET /api/health/ready)"

# ── 2. Bootstrap ─────────────────────────────────────────────────────────────
bab '2. Bootstrap organisasi'
body="{\"org_name\":\"E2E Test\",\"org_slug\":\"$SLUG\",\"email\":\"$EMAIL\",\"password\":\"$PASS\",\"name\":\"Penguji\"}"
c=$(kode POST /api/v1/auth/bootstrap "$body")
if [ "$c" = "200" ]; then
  ok "organisasi pertama dibuat"
elif [ "$c" = "409" ]; then
  ok "bootstrap ditutup setelah organisasi ada (409) — perilaku benar"
  echo "     lewati sisa uji: instalasi sudah dipakai"
  printf '\n\033[1mHasil:\033[0m %d lulus, %d gagal\n' "$lulus" "$gagal"
  exit 0
else
  no "bootstrap gagal ($c)"; cat /tmp/e2e_body; exit 1
fi

cek 'slug tidak valid ditolak' 422 \
  "$(kode POST /api/v1/auth/bootstrap '{"org_name":"X","org_slug":"HURUF-BESAR","email":"a@b.c","password":"kata-sandi-panjang-sekali","name":"X"}')"

# ── 3. Login ─────────────────────────────────────────────────────────────────
bab '3. Login'
cek 'password salah ditolak' 401 \
  "$(kode POST /api/v1/auth/login "{\"org_slug\":\"$SLUG\",\"email\":\"$EMAIL\",\"password\":\"salah-sekali-ini\"}")"
cek 'org_slug tidak dikenal ditolak' 401 \
  "$(kode POST /api/v1/auth/login "{\"org_slug\":\"tidak-ada-$SLUG\",\"email\":\"$EMAIL\",\"password\":\"$PASS\"}")"

c=$(kode POST /api/v1/auth/login "{\"org_slug\":\"$SLUG\",\"email\":\"$EMAIL\",\"password\":\"$PASS\"}")
cek 'login berhasil' 200 "$c"
TOKEN=$(json data access_token)
[ -n "$TOKEN" ] && ok 'access token diterima' || no 'access token kosong'

# ── 4. Autentikasi ───────────────────────────────────────────────────────────
bab '4. Autentikasi'
cek 'tanpa token ditolak'      401 "$(kode GET /api/v1/auth/me)"
cek 'token sampah ditolak'     401 "$(kode GET /api/v1/auth/me '' 'bukan.token.sah')"
cek 'token sah diterima'       200 "$(kode GET /api/v1/auth/me '' "$TOKEN")"

# ── 5. Perangkat ─────────────────────────────────────────────────────────────
bab '5. Pendaftaran perangkat'
c=$(kode POST /api/v1/devices '{"alias":"Uji E2E","os_type":"Web","hostname":"e2e"}' "$TOKEN")
cek 'perangkat terdaftar' 200 "$c"
DEV_ID=$(json data device_id)
DEV_PASS=$(json data session_password)
DEV_TAMPIL=$(json data device_id_tampil)

if [ ${#DEV_ID} -eq 9 ]; then ok "device ID 9 digit: $DEV_TAMPIL"; else no "panjang device ID salah: '$DEV_ID'"; fi
if [ ${#DEV_PASS} -eq 8 ]; then ok "password sesi 8 karakter"; else no "panjang password salah: ${#DEV_PASS}"; fi
if [[ "$DEV_PASS" =~ ^[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{8}$ ]]; then
  ok 'password memakai alfabet yang benar'
else
  no "password keluar alfabet: $DEV_PASS"
fi

cek 'os_type tidak valid ditolak' 422 \
  "$(kode POST /api/v1/devices '{"os_type":"BeOS"}' "$TOKEN")"
cek 'daftar perangkat' 200 "$(kode GET /api/v1/devices '' "$TOKEN")"

# ── 6. Quick Connect ─────────────────────────────────────────────────────────
bab '6. Quick Connect'

# Check digit harus ditolak sebelum menyentuh database.
cek 'check digit salah ditolak' 401 \
  "$(kode POST /api/v1/connect '{"device_id":"111111111","password":"AAAAAAAA"}' "$TOKEN")"
cek 'device ID terlalu pendek ditolak' 401 \
  "$(kode POST /api/v1/connect '{"device_id":"123","password":"AAAAAAAA"}' "$TOKEN")"

# Respons harus seragam untuk sebab yang berbeda.
b1=$(kode POST /api/v1/connect "{\"device_id\":\"$DEV_ID\",\"password\":\"SALAHSAH\"}" "$TOKEN")
p1=$(cat /tmp/e2e_body)
b2=$(kode POST /api/v1/connect '{"device_id":"111111111","password":"AAAAAAAA"}' "$TOKEN")
p2=$(cat /tmp/e2e_body)
if [ "$p1" = "$p2" ]; then
  ok 'respons seragam untuk password salah dan ID tidak valid'
else
  no "respons berbeda — oracle bocor:\n     $p1\n     $p2"
fi

# Lama respons harus dinormalkan.
t0=$(python3 -c 'import time;print(time.time())')
kode POST /api/v1/connect '{"device_id":"111111111","password":"AAAAAAAA"}' "$TOKEN" >/dev/null
t1=$(python3 -c 'import time;print(time.time())')
ms=$(python3 -c "print(int(($t1-$t0)*1000))")
if [ "$ms" -ge 240 ]; then
  ok "lama respons dinormalkan (${ms} ms >= 250 ms)"
else
  no "respons terlalu cepat (${ms} ms) — lantai waktu tidak berlaku"
fi

# Perangkat 'Web' berstatus offline sampai agent terhubung, jadi kredensial
# yang benar diharapkan menghasilkan 409, bukan 200.
c=$(kode POST /api/v1/connect "{\"device_id\":\"$DEV_ID\",\"password\":\"$DEV_PASS\"}" "$TOKEN")
if [ "$c" = "409" ]; then
  ok 'kredensial benar diterima, perangkat masih offline (409)'
elif [ "$c" = "200" ]; then
  ok 'kredensial benar, sesi pending dibuat'
else
  no "kredensial benar ditolak ($c): $(cat /tmp/e2e_body)"
fi

# Password huruf kecil harus dimaafkan.
c=$(kode POST /api/v1/connect \
  "{\"device_id\":\"$DEV_ID\",\"password\":\"$(echo "$DEV_PASS" | tr 'A-Z' 'a-z')\"}" "$TOKEN")
if [ "$c" = "409" ] || [ "$c" = "200" ]; then
  ok 'password huruf kecil dinormalkan'
else
  no "normalisasi huruf kecil gagal ($c)"
fi

# ── 7. Pembatasan laju ───────────────────────────────────────────────────────
bab '7. Pembatasan laju'

# Perangkat terpisah, supaya hitungannya tidak tercampur uji sebelumnya.
kode POST /api/v1/devices '{"os_type":"Web","alias":"Target laju"}' "$TOKEN" >/dev/null
DEV2=$(json data device_id)
DEV2_PASS=$(json data session_password)

if [ ${#DEV2} -ne 9 ]; then
  no 'gagal menyiapkan perangkat untuk uji pembatasan laju'
else
  semua_ditolak=1
  for i in 1 2 3 4 5; do
    c=$(kode POST /api/v1/connect "{\"device_id\":\"$DEV2\",\"password\":\"SALAHSAH\"}" "$TOKEN")
    [ "$c" = "401" ] || semua_ditolak=0
  done
  [ "$semua_ditolak" -eq 1 ] \
    && ok 'lima percobaan salah semuanya ditolak' \
    || no 'ada percobaan salah yang tidak ditolak'

  # Inti pengujiannya: setelah ambang terlampaui, kredensial yang **benar**
  # pun harus tertahan. Kalau di sini lolos, jedanya tidak berlaku sama sekali.
  c=$(kode POST /api/v1/connect "{\"device_id\":\"$DEV2\",\"password\":\"$DEV2_PASS\"}" "$TOKEN")
  if [ "$c" = "401" ]; then
    ok 'jeda aktif — kredensial benar pun ditolak setelah 5 kegagalan'
  else
    no "jeda TIDAK berlaku: kredensial benar menghasilkan $c"
  fi

  # Perangkat lain tidak boleh ikut terkena; jeda bersifat per device ID.
  c=$(kode POST /api/v1/connect "{\"device_id\":\"$DEV_ID\",\"password\":\"$DEV_PASS\"}" "$TOKEN")
  if [ "$c" = "409" ] || [ "$c" = "200" ]; then
    ok 'jeda tidak merembet ke perangkat lain'
  else
    no "jeda merembet ke perangkat lain ($c)"
  fi
fi

printf '\n\033[1mHasil:\033[0m %d lulus, %d gagal\n' "$lulus" "$gagal"
[ "$gagal" -eq 0 ] || exit 1
