// ─────────────────────────────────────────────────────────────────────────────
// Pustaka bersama untuk agent dan viewer berbasis browser.
//
// Sengaja tanpa framework dan tanpa langkah build. Halaman-halaman ini adalah
// cikal bakal "Zero-Install Viewer" di PRD §17.2 — semakin sedikit yang harus
// diunduh dan dievaluasi sebelum layar muncul, semakin baik.
// ─────────────────────────────────────────────────────────────────────────────

const KUNCI_AKSES = 'aetherdesk.access';
const KUNCI_REFRESH = 'aetherdesk.refresh';

// ── Penyimpanan token ────────────────────────────────────────────────────────

export function simpanToken(akses, refresh) {
  sessionStorage.setItem(KUNCI_AKSES, akses);
  if (refresh) localStorage.setItem(KUNCI_REFRESH, refresh);
}

export const ambilAkses = () => sessionStorage.getItem(KUNCI_AKSES);
export const ambilRefresh = () => localStorage.getItem(KUNCI_REFRESH);

export function hapusToken() {
  sessionStorage.removeItem(KUNCI_AKSES);
  localStorage.removeItem(KUNCI_REFRESH);
}

/** Membaca klaim `exp` tanpa memverifikasi tanda tangan — cukup untuk
 *  memutuskan apakah token layak dicoba. Server tetap yang menentukan. */
function kedaluwarsa(jwt) {
  try {
    const p = JSON.parse(atob(jwt.split('.')[1].replace(/-/g, '+').replace(/_/g, '/')));
    // Beri margin 30 detik supaya token tidak mati di tengah permintaan.
    return !p.exp || p.exp * 1000 < Date.now() + 30_000;
  } catch { return true; }
}

/**
 * Benar bila ada sesi yang masih layak dipakai.
 *
 * Sebelumnya halaman hanya memeriksa "apakah ada string token", lalu
 * menyembunyikan form masuk. Token yang sudah kedaluwarsa membuat seluruh
 * permintaan menjawab 401 tanpa satu pun jalan kembali ke form masuk —
 * pengguna terkunci di halaman yang tampak normal.
 */
export function adaSesi() {
  const a = ambilAkses();
  if (a && !kedaluwarsa(a)) return true;
  return Boolean(ambilRefresh());
}

/** Dipanggil saat sesi benar-benar habis dan tidak bisa dipulihkan. */
let onSesiHabis = () => {};
export function saatSesiHabis(fn) { onSesiHabis = fn; }

// ── Klien REST ───────────────────────────────────────────────────────────────

async function panggil(path, { method, body, token }) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = `Bearer ${token}`;
  const r = await fetch(`/api${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const teks = await r.text();
  let data = null;
  try { data = teks ? JSON.parse(teks) : null; } catch { /* respons bukan JSON */ }
  return { r, data };
}

/** Menukar refresh token dengan pasangan baru. Mengembalikan access token. */
export async function perbaruiSesi() {
  const rt = ambilRefresh();
  if (!rt) return null;

  const { r, data } = await panggil('/v1/auth/refresh', {
    method: 'POST',
    body: { refresh_token: rt },
  });

  if (!r.ok) {
    hapusToken();
    return null;
  }
  const d = data.data;
  simpanToken(d.access_token, d.refresh_token);
  return d.access_token;
}

/**
 * Pemanggil API dengan pemulihan sesi otomatis.
 *
 * Access token berumur 15 menit (ARCHITECTURE.md §6.2). Tanpa pembaruan
 * otomatis, agent yang berbagi layar berjam-jam akan mulai gagal di tengah
 * jalan tanpa alasan yang terlihat pengguna.
 */
export async function api(path, { method = 'GET', body, token, _ulang = false } = {}) {
  const akses = token ?? ambilAkses();
  const { r, data } = await panggil(path, { method, body, token: akses });

  if (r.status === 401 && !_ulang) {
    const baru = await perbaruiSesi();
    if (baru) return api(path, { method, body, token: baru, _ulang: true });
    onSesiHabis();
  }

  if (!r.ok) {
    const e = new Error(data?.error?.message ?? `HTTP ${r.status}`);
    e.status = r.status;
    e.code = data?.error?.code;
    throw e;
  }
  return data?.data ?? data;
}

/** Access token yang dijamin masih segar, untuk dipakai WebSocket. */
export async function aksesSegar() {
  const a = ambilAkses();
  if (a && !kedaluwarsa(a)) return a;
  const baru = await perbaruiSesi();
  if (!baru) onSesiHabis();
  return baru;
}

// ── Klien signaling ──────────────────────────────────────────────────────────

export class Signal extends EventTarget {
  constructor(token, deviceUuid = null) {
    super();
    this.token = token;
    this.deviceUuid = deviceUuid;
    this.ws = null;
    this.siap = false;
  }

  hubungkan() {
    return new Promise((resolve, reject) => {
      const skema = location.protocol === 'https:' ? 'wss://' : 'ws://';
      this.ws = new WebSocket(`${skema}${location.host}/ws`);

      this.ws.onopen = () => {
        this.kirim('AUTH', { token: this.token, device_uuid: this.deviceUuid });
      };

      this.ws.onmessage = (ev) => {
        let pesan;
        try { pesan = JSON.parse(ev.data); } catch { return; }

        if (pesan.type === 'AUTH_OK') {
          this.siap = true;
          resolve(pesan.payload);
          return;
        }
        if (pesan.type === 'ERROR' && !this.siap) {
          reject(new Error(pesan.payload?.message ?? 'autentikasi gagal'));
          return;
        }
        // PING dibalas otomatis; server memakainya untuk menjaga koneksi
        // tetap hidup melewati batas idle Cloudflare (~100 detik).
        if (pesan.type === 'PING') {
          this.kirim('PONG', null);
          return;
        }

        this.dispatchEvent(new CustomEvent(pesan.type, { detail: pesan.payload }));
      };

      this.ws.onerror = () => { if (!this.siap) reject(new Error('koneksi signaling gagal')); };
      this.ws.onclose = () => {
        this.siap = false;
        this.dispatchEvent(new CustomEvent('TERPUTUS'));
      };
    });
  }

  kirim(type, payload) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type, payload }));
    }
  }

  tutup() { this.ws?.close(); }
}

// ── WebRTC ───────────────────────────────────────────────────────────────────

/** Cadangan bila API tidak dapat dihubungi. STUN saja — cukup untuk jaringan
 *  yang ramah, tidak cukup untuk Symmetric NAT. */
const ICE_CADANGAN = [{ urls: 'stun:stun.l.google.com:19302' }];

let iceCache = null;

/**
 * Mengambil daftar ICE server beserta kredensial TURN berumur pendek.
 *
 * Kredensial sengaja tidak ditanam di klien: yang dikirim server hanyalah
 * pasangan HMAC yang kedaluwarsa dalam hitungan jam, sehingga bocornya konsol
 * atau HAR tidak berubah menjadi relay gratis bagi orang lain.
 */
export async function ambilIceServers() {
  if (iceCache && iceCache.expires_at * 1000 > Date.now() + 60_000) {
    return iceCache.ice_servers;
  }
  try {
    const d = await api('/v1/turn-credentials');
    iceCache = d;
    const punyaTurn = d.ice_servers.some((s) =>
      (Array.isArray(s.urls) ? s.urls : [s.urls]).some((u) => u.startsWith('turn:')));
    if (!punyaTurn) console.warn('TURN tidak tersedia — hanya STUN');
    return d.ice_servers;
  } catch (e) {
    console.warn('gagal mengambil kredensial ICE, memakai cadangan', e.message);
    return ICE_CADANGAN;
  }
}

export function buatPeer(iceServers = ICE_CADANGAN) {
  return new RTCPeerConnection({ iceServers, bundlePolicy: 'max-bundle' });
}

/**
 * Pembungkus RTCPeerConnection yang mengantre kandidat ICE.
 *
 * Trickle ICE mengirim kandidat segera setelah `setLocalDescription`, jauh
 * sebelum lawan sempat membuat peer connection-nya sendiri — apalagi sebelum
 * `setRemoteDescription` dipanggil. Kandidat yang tiba pada jeda itu akan
 * ditolak dan hilang tanpa jejak, dan koneksi berakhir `failed` meskipun
 * signaling-nya sempurna.
 *
 * Kelas ini menahan kandidat sampai remote description siap, lalu menyiramkan
 * seluruh antrean sekaligus.
 */
export class Kanal {
  /** Gunakan `await Kanal.buat({...})` supaya kredensial TURN sempat diambil. */
  static async buat(opsi) {
    const iceServers = await ambilIceServers();
    return new Kanal({ ...opsi, iceServers });
  }

  constructor({ onKandidat, onStatus, iceServers }) {
    this.pc = buatPeer(iceServers);
    this.remoteSiap = false;
    this.antre = [];
    this.kandidatLokal = [];

    this.pc.onicecandidate = (e) => {
      if (e.candidate) {
        this.kandidatLokal.push(e.candidate.type ?? '?');
        onKandidat?.(e.candidate.toJSON());
      }
    };
    this.pc.onconnectionstatechange = () => onStatus?.(this.pc.connectionState, this.diagnosa());
    this.pc.oniceconnectionstatechange = () =>
      onStatus?.(this.pc.connectionState, this.diagnosa());
  }

  async terapkanRemote(type, sdp) {
    await this.pc.setRemoteDescription({ type, sdp });
    this.remoteSiap = true;

    // Siram antrean. Kandidat basi wajar ditolak — jangan sampai satu
    // kegagalan menghentikan sisanya.
    for (const c of this.antre) {
      try { await this.pc.addIceCandidate(c); }
      catch (e) { console.warn('kandidat antrean ditolak', e.message); }
    }
    this.antre = [];
  }

  async tambahKandidat(c) {
    if (!c) return;
    if (!this.remoteSiap) { this.antre.push(c); return; }
    try { await this.pc.addIceCandidate(c); }
    catch (e) { console.warn('kandidat ditolak', e.message); }
  }

  /** Ringkasan untuk menjelaskan kegagalan, bukan sekadar melaporkannya. */
  diagnosa() {
    const jenis = [...new Set(this.kandidatLokal)];
    return {
      ice: this.pc.iceConnectionState,
      gathering: this.pc.iceGatheringState,
      koneksi: this.pc.connectionState,
      kandidat: jenis,
      antre: this.antre.length,
      // `host` saja berarti STUN tidak terjangkau; tanpa `srflx` koneksi
      // lintas-NAT mustahil.
      punyaSrflx: jenis.includes('srflx'),
      punyaRelay: jenis.includes('relay'),
    };
  }

  tutup() { try { this.pc.close(); } catch { /* sudah tertutup */ } }
}

/** Menjelaskan kegagalan koneksi dalam bahasa yang bisa ditindaklanjuti. */
export function jelaskanKegagalan(d) {
  if (!d) return 'Koneksi gagal.';
  if (d.kandidat.length === 0) {
    return 'Koneksi gagal: browser tidak menghasilkan satu pun kandidat ICE. '
         + 'Biasanya karena halaman tidak berjalan di konteks aman atau WebRTC diblokir.';
  }
  if (!d.punyaSrflx && !d.punyaRelay) {
    return 'Koneksi gagal: hanya kandidat host yang ditemukan, server STUN tidak terjangkau. '
         + 'Jaringan Anda kemungkinan memblokir UDP keluar ke port 19302.';
  }
  if (!d.punyaRelay) {
    return 'Koneksi P2P gagal meski STUN bekerja. Ini pola khas Symmetric NAT, '
         + 'dan menembusnya memerlukan server TURN yang belum dipasang pada Fase 0.';
  }
  return `Koneksi gagal (ICE: ${d.ice}).`;
}

// ── Utilitas tampilan ────────────────────────────────────────────────────────

export function $(sel) { return document.querySelector(sel); }

export function tampilkan(el, tampil = true) {
  el.style.display = tampil ? '' : 'none';
}

export function status(el, teks, jenis = 'info') {
  el.textContent = teks;
  el.className = `status ${jenis}`;
}

/** Memformat device ID sembilan digit menjadi `942 716 382`. */
export function kelompokkan(id) {
  const b = String(id).replace(/\D/g, '');
  return b.length === 9 ? `${b.slice(0, 3)} ${b.slice(3, 6)} ${b.slice(6)}` : id;
}

/** Statistik ringkas dari RTCPeerConnection, untuk overlay di VIEWER.md §2. */
export async function statistik(pc) {
  const s = await pc.getStats();
  const hasil = { rtt: null, bitrate: null, fps: null, lebar: null, tinggi: null, jenis: null };
  let masukVideo = null;

  s.forEach((r) => {
    if (r.type === 'candidate-pair' && r.state === 'succeeded') {
      hasil.rtt = r.currentRoundTripTime != null ? Math.round(r.currentRoundTripTime * 1000) : null;
    }
    if (r.type === 'inbound-rtp' && r.kind === 'video') masukVideo = r;
    if (r.type === 'local-candidate' && r.candidateType) hasil.jenis = r.candidateType;
  });

  if (masukVideo) {
    hasil.fps = masukVideo.framesPerSecond ?? null;
    hasil.lebar = masukVideo.frameWidth ?? null;
    hasil.tinggi = masukVideo.frameHeight ?? null;
    hasil._bytes = masukVideo.bytesReceived;
    hasil._ts = masukVideo.timestamp;
  }
  return hasil;
}
