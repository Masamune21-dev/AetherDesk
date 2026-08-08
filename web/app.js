// ─────────────────────────────────────────────────────────────────────────────
// Pustaka bersama untuk agent dan viewer berbasis browser.
//
// Sengaja tanpa framework dan tanpa langkah build. Halaman-halaman ini adalah
// cikal bakal "Zero-Install Viewer" di PRD §17.2 — semakin sedikit yang harus
// diunduh dan dievaluasi sebelum layar muncul, semakin baik.
// ─────────────────────────────────────────────────────────────────────────────

export const KUNCI_TOKEN = 'aetherdesk.token';

// ── Klien REST ───────────────────────────────────────────────────────────────

export async function api(path, { method = 'GET', body, token } = {}) {
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

  if (!r.ok) {
    const e = new Error(data?.error?.message ?? `HTTP ${r.status}`);
    e.status = r.status;
    e.code = data?.error?.code;
    throw e;
  }
  return data?.data ?? data;
}

export const simpanToken = (t) => sessionStorage.setItem(KUNCI_TOKEN, t);
export const ambilToken = () => sessionStorage.getItem(KUNCI_TOKEN);
export const hapusToken = () => sessionStorage.removeItem(KUNCI_TOKEN);

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

/**
 * Fase 0 memakai STUN publik saja. Konsekuensinya dicatat terbuka di
 * DEPLOYMENT_PLAN.md §7: koneksi gagal bagi pengguna di belakang Symmetric NAT,
 * secara industri sekitar 10-20% kasus. TURN menunggu keputusan soal port
 * forwarding UDP dan paparan IP origin.
 */
export const ICE_SERVERS = [
  { urls: 'stun:stun.l.google.com:19302' },
  { urls: 'stun:stun1.l.google.com:19302' },
];

export function buatPeer() {
  return new RTCPeerConnection({ iceServers: ICE_SERVERS, bundlePolicy: 'max-bundle' });
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
  constructor({ onKandidat, onStatus }) {
    this.pc = buatPeer();
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
