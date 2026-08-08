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

// ── Medan aether ─────────────────────────────────────────────────────────────

/**
 * Muka gelombang sepusat yang merambat pelan dari satu titik asal.
 *
 * Ini satu-satunya gerakan di seluruh antarmuka, dan sengaja begitu. Alih-alih
 * hujan partikel yang dipakai hampir semua halaman teknologi, yang digambar di
 * sini adalah hal yang benar-benar dilakukan produk ini: sinyal yang merambat
 * melintasi jarak sampai akhirnya meredup.
 *
 * Dijaga agar murah — 24 fps, sepuluh muka gelombang, garis setipis rambut.
 * Saat pengguna meminta gerakan dikurangi, satu bingkai statis digambar lalu
 * animasinya tidak pernah dimulai.
 */
export function pasangMedan() {
  const kanvas = document.createElement('canvas');
  kanvas.id = 'medan';
  kanvas.setAttribute('aria-hidden', 'true');
  document.body.prepend(kanvas);

  const ctx = kanvas.getContext('2d', { alpha: true });
  const kurangiGerak = matchMedia('(prefers-reduced-motion: reduce)').matches;

  let w = 0, h = 0, asalX = 0, asalY = 0;
  const JUMLAH = 10;
  const JARAK = 190;      // jarak antar muka gelombang, piksel
  const KECEPATAN = 0.16; // piksel per milidetik — lambat dan tenang

  function ukur() {
    const dpr = Math.min(devicePixelRatio || 1, 2);
    w = innerWidth; h = innerHeight;
    kanvas.width = w * dpr;
    kanvas.height = h * dpr;
    kanvas.style.width = `${w}px`;
    kanvas.style.height = `${h}px`;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    // Titik asal di luar tepi kiri-atas: gelombang menyeberangi layar,
    // bukan memancar dari tengah seperti riak kolam.
    asalX = w * -0.12;
    asalY = h * 0.18;
  }

  function gambar(t) {
    ctx.clearRect(0, 0, w, h);
    const maks = Math.hypot(w - asalX, h - asalY);

    for (let i = 0; i < JUMLAH; i++) {
      const r = ((t * KECEPATAN + i * JARAK) % (maks + JARAK));
      if (r < 1) continue;

      // Meredup seiring jarak — energinya tersebar pada keliling yang membesar.
      const pudar = Math.max(0, 1 - r / maks);
      const alpha = pudar * pudar * 0.16;
      if (alpha < 0.004) continue;

      // Warna bergeser sepanjang lintasan: ungu di dekat sumber, sian di
      // tengah, kuning saat mendekati tujuan.
      const p = r / maks;
      const warna = p < 0.5
        ? `rgba(139, 123, 247, ${alpha})`
        : p < 0.8
          ? `rgba(76, 201, 240, ${alpha})`
          : `rgba(244, 162, 97, ${alpha})`;

      ctx.beginPath();
      ctx.arc(asalX, asalY, r, 0, Math.PI * 2);
      ctx.strokeStyle = warna;
      ctx.lineWidth = 1;
      ctx.stroke();
    }
  }

  ukur();
  addEventListener('resize', ukur, { passive: true });

  if (kurangiGerak) { gambar(2600); return; }

  let terakhir = 0;
  const INTERVAL = 1000 / 24;
  (function bingkai(t) {
    if (t - terakhir >= INTERVAL) { gambar(t); terakhir = t; }
    requestAnimationFrame(bingkai);
  })(0);
}

// ── Cangkang aplikasi ────────────────────────────────────────────────────────

/**
 * Menyisipkan header aplikasi di puncak halaman.
 *
 * Wordmark selalu menautkan ke beranda, sehingga setiap halaman punya jalan
 * pulang tanpa perlu menempelkan tombol lepas di masing-masing tata letak.
 *
 * Tandanya dua busur yang memancar — gagasan sinyal yang menyeberang jarak.
 * Ikon monitor akan menjadi pilihan paling mudah ditebak untuk kategori ini,
 * dan justru itu alasannya dihindari.
 */
export function pasangHeader(halamanAktif = '') {
  const tautan = [
    ['/', 'Beranda'],
    ['/agent', 'Agent'],
    ['/viewer', 'Viewer'],
    ['/dashboard', 'Dashboard'],
  ];

  const header = document.createElement('header');
  header.className = 'app-header';

  // Tanda: satu titik sumber dengan muka gelombang yang memancar dan meredup —
  // ringkasan dari keseluruhan gagasan desainnya. Warnanya mengikuti gradien
  // sinyal, jadi tandanya sendiri menunjukkan dua ujung sebuah koneksi.
  header.innerHTML = `
    <a class="wordmark" href="/" aria-label="AetherDesk — beranda">
      <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
        <defs>
          <linearGradient id="wm" x1="0" y1="20" x2="20" y2="0">
            <stop offset="0%" stop-color="#8b7bf7"/>
            <stop offset="55%" stop-color="#4cc9f0"/>
            <stop offset="100%" stop-color="#f4a261"/>
          </linearGradient>
        </defs>
        <circle cx="3.5" cy="16.5" r="2" fill="url(#wm)"/>
        <path d="M3.5 11.5a5 5 0 0 1 5 5" stroke="url(#wm)" stroke-width="1.7"
              stroke-linecap="round"/>
        <path d="M3.5 6.5a10 10 0 0 1 10 10" stroke="url(#wm)" stroke-width="1.7"
              stroke-linecap="round" opacity=".6"/>
        <path d="M3.5 1.5a15 15 0 0 1 15 15" stroke="url(#wm)" stroke-width="1.7"
              stroke-linecap="round" opacity=".28"/>
      </svg>
      AetherDesk
    </a>
    <nav class="app-nav">
      ${tautan.map(([h, t]) =>
        `<a href="${h}"${h === halamanAktif ? ' aria-current="page"' : ''}>${t}</a>`
      ).join('')}
    </nav>`;

  document.body.prepend(header);
  pasangMedan();
}

// ── Utilitas tampilan ────────────────────────────────────────────────────────

export function $(sel) { return document.querySelector(sel); }

/**
 * Menyalin teks ke papan klip dan memberi umpan balik pada tombolnya.
 *
 * Device ID dan kata sandi memang dibacakan lewat telepon, tetapi cukup sering
 * juga dikirim lewat chat — dan mengetik ulang delapan karakter acak adalah
 * cara paling mudah membuat kesalahan.
 */
export async function salin(teks, tombol) {
  const semula = tombol.textContent;
  try {
    await navigator.clipboard.writeText(teks);
    tombol.textContent = 'Tersalin';
  } catch {
    tombol.textContent = 'Gagal menyalin';
  }
  setTimeout(() => { tombol.textContent = semula; }, 1600);
}

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
