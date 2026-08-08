// ─────────────────────────────────────────────────────────────────────────────
// Uji signaling dua arah.
//
// Menjalankan alur nyata: agent dan viewer sama-sama terhubung, viewer meminta
// sesi, agent menerima tawaran, lalu SDP dan kandidat ICE dipertukarkan.
//
// Diuji juga properti keamanannya: pihak luar tidak boleh menyuntikkan pesan
// ke sesi milik orang lain, dan status perangkat harus langsung offline saat
// koneksi putus (temuan S-09).
//
// Pemakaian:
//   node scripts/signal-test.mjs                     # localhost
//   WS=wss://aetherdesk.masamune.my.id/ws node scripts/signal-test.mjs
// ─────────────────────────────────────────────────────────────────────────────

const API = process.env.API ?? 'http://127.0.0.1:8080';
const WS = process.env.WS ?? 'ws://127.0.0.1:8081/ws';

let lulus = 0, gagal = 0;
const ok = (m) => { console.log(`  \x1b[32m✓\x1b[0m ${m}`); lulus++; };
const no = (m) => { console.log(`  \x1b[31m✗\x1b[0m ${m}`); gagal++; };
const bab = (m) => console.log(`\n\x1b[1m${m}\x1b[0m`);

const tidur = (ms) => new Promise((r) => setTimeout(r, ms));

async function api(path, { method = 'GET', body, token } = {}) {
  const h = { 'Content-Type': 'application/json' };
  if (token) h.Authorization = `Bearer ${token}`;
  const r = await fetch(`${API}${path}`, { method, headers: h, body: body && JSON.stringify(body) });
  return { status: r.status, body: await r.json().catch(() => null) };
}

/** Membungkus WebSocket dengan antrean pesan supaya bisa ditunggu satu per satu. */
function klien(nama) {
  const ws = new WebSocket(WS);
  const antre = [];
  const menunggu = [];
  ws.addEventListener('message', (e) => {
    const m = JSON.parse(e.data);
    if (menunggu.length) menunggu.shift()(m);
    else antre.push(m);
  });
  return {
    nama,
    ws,
    siap: new Promise((res, rej) => {
      ws.addEventListener('open', res);
      ws.addEventListener('error', rej);
    }),
    kirim: (o) => ws.send(JSON.stringify(o)),
    /** Menunggu pesan berikutnya yang bukan PING. */
    async terima(batasMs = 5000) {
      const mulai = Date.now();
      for (;;) {
        let m;
        if (antre.length) m = antre.shift();
        else {
          m = await Promise.race([
            new Promise((r) => menunggu.push(r)),
            tidur(batasMs).then(() => null),
          ]);
        }
        if (m === null) throw new Error(`${nama}: tidak ada pesan dalam ${batasMs} ms`);
        if (m.type !== 'PING') return m;
        if (Date.now() - mulai > batasMs) throw new Error(`${nama}: hanya menerima PING`);
      }
    },
    tutup: () => ws.close(),
  };
}

console.log('\x1b[1mAetherDesk — uji signaling\x1b[0m');
console.log(`API: ${API}\nWS:  ${WS}`);

// ── Persiapan ────────────────────────────────────────────────────────────────
bab('1. Persiapan');
const slug = `sig-${Date.now()}`;
let r = await api('/api/v1/auth/bootstrap', {
  method: 'POST',
  body: { org_name: 'Signal Test', org_slug: slug, email: 's@ig.id', password: 'kata-sandi-uji-panjang', name: 'S' },
});
if (r.status !== 200) { console.error('bootstrap gagal', r); process.exit(1); }
ok('organisasi dibuat');

r = await api('/api/v1/auth/login', {
  method: 'POST',
  body: { org_slug: slug, email: 's@ig.id', password: 'kata-sandi-uji-panjang' },
});
const token = r.body.data.access_token;
ok('token diperoleh');

r = await api('/api/v1/devices', { method: 'POST', token, body: { os_type: 'Web', alias: 'Agent uji' } });
const deviceUuid = r.body.data.device_uuid;
ok(`perangkat terdaftar: ${r.body.data.device_id_tampil}`);

// ── Autentikasi WebSocket ────────────────────────────────────────────────────
bab('2. Autentikasi WebSocket');

const tanpaAuth = klien('tanpa-auth');
await tanpaAuth.siap;
tanpaAuth.kirim({ type: 'AUTH', payload: { token: 'token.palsu.sekali', device_uuid: null } });
let m = await tanpaAuth.terima();
m.type === 'ERROR' ? ok('token palsu ditolak') : no(`token palsu diterima: ${m.type}`);
tanpaAuth.tutup();

const agent = klien('agent');
await agent.siap;
agent.kirim({ type: 'AUTH', payload: { token, device_uuid: deviceUuid } });
m = await agent.terima();
m.type === 'AUTH_OK' && m.payload.role === 'agent' ? ok('agent terautentikasi') : no(`agent gagal: ${JSON.stringify(m)}`);

const viewer = klien('viewer');
await viewer.siap;
viewer.kirim({ type: 'AUTH', payload: { token, device_uuid: null } });
m = await viewer.terima();
m.type === 'AUTH_OK' && m.payload.role === 'viewer' ? ok('viewer terautentikasi') : no(`viewer gagal: ${JSON.stringify(m)}`);

// ── Presence ─────────────────────────────────────────────────────────────────
bab('3. Presence (temuan S-09)');
await tidur(400);
r = await api('/api/v1/devices', { token });
let dev = r.body.data.find((d) => d.device_uuid === deviceUuid);
dev?.status === 'online' ? ok('perangkat online saat agent terhubung') : no(`status: ${dev?.status}`);

// ── Alur sesi ────────────────────────────────────────────────────────────────
bab('4. Alur sesi');
const sessionId = crypto.randomUUID();
viewer.kirim({ type: 'SESSION_REQUEST', payload: { session_id: sessionId, device_uuid: deviceUuid } });

m = await agent.terima();
m.type === 'SESSION_OFFER' && m.payload.session_id === sessionId
  ? ok('agent menerima tawaran sesi')
  : no(`agent tidak menerima tawaran: ${JSON.stringify(m)}`);

agent.kirim({ type: 'SESSION_ACCEPT', payload: { session_id: sessionId } });
m = await viewer.terima();
m.type === 'SESSION_ACCEPTED' ? ok('viewer menerima persetujuan') : no(`gagal: ${JSON.stringify(m)}`);

const sdpAsli = 'v=0\r\no=- 4611731400430051336 2 IN IP4 127.0.0.1\r\na=ice-ufrag:abcd\r\n';
agent.kirim({ type: 'SDP_OFFER', payload: { session_id: sessionId, sdp: sdpAsli } });
m = await viewer.terima();
if (m.type === 'SDP_OFFER' && m.payload.sdp === sdpAsli) {
  ok('SDP diteruskan byte-per-byte tanpa diubah');
} else {
  no(`SDP berubah atau tidak sampai: ${JSON.stringify(m).slice(0, 120)}`);
}

viewer.kirim({ type: 'SDP_ANSWER', payload: { session_id: sessionId, sdp: 'v=0\r\njawaban\r\n' } });
m = await agent.terima();
m.type === 'SDP_ANSWER' ? ok('SDP answer sampai ke agent') : no(`gagal: ${m.type}`);

const kandidat = { candidate: 'candidate:1 1 udp 2130706431 192.0.2.1 54321 typ host', sdpMid: '0' };
viewer.kirim({ type: 'ICE_CANDIDATE', payload: { session_id: sessionId, candidate: kandidat } });
m = await agent.terima();
m.type === 'ICE_CANDIDATE' && m.payload.candidate.candidate === kandidat.candidate
  ? ok('kandidat ICE diteruskan utuh')
  : no(`kandidat berubah: ${JSON.stringify(m).slice(0, 120)}`);

// ── Isolasi sesi ─────────────────────────────────────────────────────────────
bab('5. Isolasi sesi');
const penyusup = klien('penyusup');
await penyusup.siap;
penyusup.kirim({ type: 'AUTH', payload: { token, device_uuid: null } });
await penyusup.terima();

penyusup.kirim({ type: 'SDP_OFFER', payload: { session_id: sessionId, sdp: 'v=0\r\njahat\r\n' } });
m = await penyusup.terima();
m.type === 'ERROR' && m.payload.code === 'NOT_A_PARTICIPANT'
  ? ok('pihak luar ditolak menyuntik SDP ke sesi orang lain')
  : no(`pembajakan sesi TIDAK dicegah: ${JSON.stringify(m)}`);

penyusup.kirim({ type: 'SESSION_END', payload: { session_id: sessionId } });
m = await penyusup.terima();
m.type === 'ERROR' ? ok('pihak luar tidak bisa mengakhiri sesi orang lain') : no(`gagal: ${m.type}`);
penyusup.tutup();

// ── Akhiri sesi ──────────────────────────────────────────────────────────────
bab('6. Akhiri sesi');
viewer.kirim({ type: 'SESSION_END', payload: { session_id: sessionId } });
m = await agent.terima();
m.type === 'SESSION_END' ? ok('agent diberi tahu sesi berakhir') : no(`gagal: ${m.type}`);

// ── Offline seketika ─────────────────────────────────────────────────────────
bab('7. Offline seketika (temuan S-09)');
agent.tutup();
await tidur(700);
r = await api('/api/v1/devices', { token });
dev = r.body.data.find((d) => d.device_uuid === deviceUuid);
if (dev?.status === 'offline') {
  ok('offline dalam < 1 detik, bukan menunggu TTL 90 detik');
} else {
  no(`masih berstatus '${dev?.status}' setelah agent putus`);
}

viewer.tutup();

console.log(`\n\x1b[1mHasil:\x1b[0m ${lulus} lulus, ${gagal} gagal`);
process.exit(gagal === 0 ? 0 : 1);
