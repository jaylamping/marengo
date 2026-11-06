import * as THREE from './node_modules/three/build/three.module.js';
import { RoomEnvironment } from './node_modules/three/examples/jsm/environments/RoomEnvironment.js';

// Robot frame (mm): X forward, Y left, Z up, Z measured from FLOOR.
// three frame: x = robotY, y = robotZ, z = robotX.
const q = new URLSearchParams(location.search);
const EXT = Number(q.get('ext') ?? 70);        // waist extension (mm)
const VIEW = q.get('view') ?? 'iso';
const XRAY = q.get('xray') === '1';
const FULL = q.get('full') === '1';
const OFFX = Number(q.get('offx') ?? 0);
const EXPLODE = q.get('explode') === '1';
const parts = [];
const DUMP = q.get('dump') === '1'; window.__grids = [];  // dump=1: export tagged loft grids for the split pipeline
const PALETTE = [0x4f9de0, 0x4f9de0, 0x63c08a, 0x63c08a, 0xe8a34a, 0xe8a34a, 0xb07fd6, 0xb07fd6, 0xd96a8a, 0xd96a8a, 0xe8d36a, 0xe8d36a, 0x56c7c7, 0x56c7c7];        // lateral offset for side-by-side

const P = (x, y, z) => new THREE.Vector3(y + OFFX, z, x);

// kinematics.md: pelvis bottom 740 from floor, waist (pelvis top) 855, frame 155..495 above pelvis bottom
const PELVIS0 = 740, WAIST = 855;
const F0 = PELVIS0 + 155 + EXT, F1 = PELVIS0 + 495 + EXT;
const ZS = PELVIS0 + 424.9 + EXT;   // shoulder pitch axis
const YSX = Number(q.get('ysx') ?? 0); // P1 lengthening per side (arm-thread option)
const FW = Number(q.get('fw') ?? 38);  // frame widening per side (longer left-right 2020 rails; arm thread 2026-11-06)
const FACE = 97.5 + FW;              // pitch RS03 output face |Y|
const YS = 170.49 + YSX + FW;             // pitch/roll joint, from left_arm.urdf

const scene = new THREE.Scene();
const W = 1600, H = 1200;
const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(window.devicePixelRatio); renderer.setSize(W, H);
renderer.toneMapping = THREE.ACESFilmicToneMapping; renderer.toneMappingExposure = 0.82;
renderer.shadowMap.enabled = true; renderer.shadowMap.type = THREE.PCFSoftShadowMap;
document.body.appendChild(renderer.domElement);
const pmrem = new THREE.PMREMGenerator(renderer);
scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
scene.background = new THREE.Color(0xc9ced6);

const white = new THREE.MeshPhysicalMaterial({ color: 0xeef0f3, roughness: 0.38, clearcoat: 0.6, clearcoatRoughness: 0.25,
  transparent: XRAY, opacity: XRAY ? 0.28 : 1, depthWrite: !XRAY, side: THREE.DoubleSide });
const dark = new THREE.MeshStandardMaterial({ color: 0x2b2e33, roughness: 0.45, metalness: 0.55 });
const alu = new THREE.MeshStandardMaterial({ color: 0x1c1d20, roughness: 0.5, metalness: 0.7 });
const seamMat = new THREE.MeshBasicMaterial({ color: 0x8d939b });
const ghost = new THREE.MeshStandardMaterial({ color: 0x8c929b, roughness: 0.8, transparent: true, opacity: 0.6 });
const accent = new THREE.MeshStandardMaterial({ color: 0x3aa8ff, emissive: 0x1b6fd1, emissiveIntensity: 0.9 });
const mount = new THREE.MeshStandardMaterial({ color: 0xff8a1f, roughness: 0.5 });
const root = new THREE.Group(); scene.add(root);
const add = (m) => { m.castShadow = true; m.receiveShadow = true; root.add(m); return m; };

// ---------- lofted superellipse shell ----------
function sgnpow(v, e) { return Math.sign(v) * Math.pow(Math.abs(v), e); }
function interp(rows, z) {
  if (rows.lin) { const n0 = rows.length; if (z <= rows[0][0]) return rows[0].slice(1); if (z >= rows[n0 - 1][0]) return rows[n0 - 1].slice(1);
    let i0 = 0; while (z > rows[i0 + 1][0]) i0++; const t = (z - rows[i0][0]) / (rows[i0 + 1][0] - rows[i0][0]); return rows[i0].slice(1).map((v, k) => v + (rows[i0 + 1][k + 1] - v) * t); }
  return interpCubic(rows, z);
}
function interpCubic(rows, z) { // monotone cubic (Fritsch-Carlson) per column: smooth, no overshoot
  const n = rows.length; if (z <= rows[0][0]) return rows[0].slice(1); if (z >= rows[n - 1][0]) return rows[n - 1].slice(1);
  let i = 0; while (z > rows[i + 1][0]) i++;
  const out = [];
  for (let k = 1; k < rows[0].length; k++) {
    const xs = rows.map(r => r[0]), ys = rows.map(r => r[k]);
    const d = []; for (let j = 0; j < n - 1; j++) d.push((ys[j + 1] - ys[j]) / (xs[j + 1] - xs[j]));
    const m = [d[0]]; for (let j = 1; j < n - 1; j++) m.push(d[j - 1] * d[j] <= 0 ? 0 : (d[j - 1] + d[j]) / 2); m.push(d[n - 2]);
    for (let j = 0; j < n - 1; j++) { if (d[j] === 0) { m[j] = 0; m[j + 1] = 0; continue; }
      const a2 = m[j] / d[j], b2 = m[j + 1] / d[j], h = a2 * a2 + b2 * b2; if (h > 9) { const t = 3 / Math.sqrt(h); m[j] = t * a2 * d[j]; m[j + 1] = t * b2 * d[j]; } }
    const hh = xs[i + 1] - xs[i], t = (z - xs[i]) / hh, t2 = t * t, t3 = t2 * t;
    out.push((2 * t3 - 3 * t2 + 1) * ys[i] + (t3 - 2 * t2 + t) * hh * m[i] + (-2 * t3 + 3 * t2) * ys[i + 1] + (t3 - t2) * hh * m[i + 1]);
  }
  return out;
}
// Faceted section (Joey 2026-11-06: less boxy/soft, more masculine): flat pec planes meeting at a sternum
// crease, chamfered corners, flat back with shoulder-blade planes. th: 0 = +Y side, pi/2 = front, pi = -Y, 3pi/2 = back.
const QF = [[0, 1], [0.35, 1], [0.82, 0.86], [1.0, 0.42], [0.94, 0]];
const QB = [[0, 1], [-0.42, 1], [-0.88, 0.8], [-1.0, 0.34], [-1.0, 0]];
let FACET = false;
function facetPt(hw, df, db, th, x0) {
  let t = ((th % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI); const q = Math.floor(t / (Math.PI / 2)); let f = (t - q * Math.PI / 2) / (Math.PI / 2);
  const front = q === 0 || q === 1; const Q = front ? QF : QB; const d = front ? df : db;
  if (q === 1 || q === 3) f = 1 - f; // mirror toward the other side
  const sy = (q === 0 || q === 3) ? 1 : -1;
  const k = Math.min(3, Math.floor(f * 4)); const u = f * 4 - k;
  const ax = Q[k][0] + (Q[k + 1][0] - Q[k][0]) * u, ay = Q[k][1] + (Q[k + 1][1] - Q[k][1]) * u;
  return [ax * d + x0, sy * ay * hw];
}
function ringPt(hw, df, db, n, th, x0 = 0) {
  if (FACET) return facetPt(hw, df, db, th, x0);
  const c = Math.cos(th), s = Math.sin(th);
  const y = hw * sgnpow(c, 2 / n);
  const x = (s >= 0 ? df : db) * sgnpow(s, 2 / n);
  return [x + x0, y];
}
let CLAMP = null; let SEAM_CLAMP = null;
function loft(rows, z0, z1, mat, nz = 90, nt = 128, x0 = 0, th0 = 0, th1 = 2 * Math.PI, tag = null, ex = [0, 0, 0]) {
  const pos = [], idx = [];
  for (let i = 0; i <= nz; i++) {
    const z = z0 + (z1 - z0) * i / nz; const [hw, df, db, n] = interp(rows, z);
    for (let j = 0; j <= nt; j++) { const th = th0 + (th1 - th0) * j / nt; let [x, y] = ringPt(hw, df, db, n, th, x0); let zz = z;
      if (CLAMP) [x, y, zz] = CLAMP(x, y, z);
      const v = P(x, y, zz); pos.push(v.x, v.y, v.z); }
  }
  for (let i = 0; i < nz; i++) for (let j = 0; j < nt; j++) {
    const a = i * (nt + 1) + j, b = a + nt + 1; idx.push(a, b, a + 1, b, b + 1, a + 1); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setIndex(idx); g.computeVertexNormals();
  if (tag && DUMP) { const rp = []; for (let k = 0; k < pos.length; k += 3) rp.push(+pos[k + 2].toFixed(3), +(pos[k] - OFFX).toFixed(3), +pos[k + 1].toFixed(3));
    window.__grids.push({ tag, nz, nt, pts: rp }); }   // robot frame X, Y, Z (Z from floor)
  let m2 = mat;
  if (tag) { g.computeBoundingBox(); const bb = g.boundingBox;
    parts.push({ tag, Xdepth: +(bb.max.z - bb.min.z).toFixed(0), Ywidth: +(bb.max.x - bb.min.x).toFixed(0), Zheight: +(bb.max.y - bb.min.y).toFixed(0) });
    if (EXPLODE) { m2 = mat.clone(); m2.color = new THREE.Color(PALETTE[(parts.length - 1) % PALETTE.length]); m2.side = THREE.DoubleSide; } }
  const mesh = add(new THREE.Mesh(g, m2));
  if (EXPLODE) mesh.position.add(P(ex[0], ex[1], ex[2]).sub(P(0, 0, 0)));
  return mesh;
}
function seamRing(rows, z, x0 = 0, grow = 0.8) {
  const pts = []; const [hw, df, db, n] = interp(rows, z);
  for (let j = 0; j <= 128; j++) { const [x, y] = ringPt(hw + grow, df + grow, db + grow, n, 2 * Math.PI * j / 128, x0); pts.push(P(x, y, z)); }
  add(new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts, true), 256, 0.9, 6, true), seamMat));
}
function seamVert(rows, th, za, zb, x0 = 0, grow = 0.8) {
  const pts = [];
  for (let i = 0; i <= 60; i++) { const z = za + (zb - za) * i / 60; const [hw, df, db, n] = interp(rows, z);
    let [x, y] = ringPt(hw + grow, df + grow, db + grow, n, th, x0); let zz = z; if (SEAM_CLAMP) [x, y, zz] = SEAM_CLAMP(x, y, z); pts.push(P(x, y, zz)); }
  add(new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts), 120, 0.9, 6), seamMat));
}

// ---------- CHEST shell (torso-fixed, bolts to 2020 frame) ----------
// rows: [z, half-width Y, front depth +X, back depth -X, squareness n]
// frame is +-75 Y, +-70 X. Arm hangs at Y=170 with 15 deg inward roll -> keep side <= ~100 below shoulder.
// Joey 2026-11-06 (in-between): from the shoulder point the side curves gently in, steeper below ~110 mm;
// inboard of the hanging arm for 5 deg inward roll. Earlier: side runs almost straight down to ~65 mm below the
// black P1 part (bottom at ZS-46), then tapers to the waist. Wall tracks the shoulder width (YSX).
const WALL = 124 + YSX + FW;
// arm-thread gap check (TORSO_WIDE_VARIANT.md): shoulder-point half-width <= 155 at fw=38 for 5 mm at 5 deg inward roll
const TOPW = WALL - Number(q.get('topcap') ?? 0);   // 7 = arm-thread cap (155 at fw=38); Joey kept the baseline
const WX = Number(q.get('wx') ?? 10); // waist widening per side (Joey 2026-11-06: 'widen the waist a bit')
const TAPER_Z = ZS - 46 - 65;
// Manufacturing thread 2026-11-06: frame_clearance.py found the back too shallow for the 2020 frame (rear corners
// through the wall at the frame bottom, <1 mm at the gusset corners, <8 mm across the back). bd deepens the back
// everywhere, bdlo adds more toward the frame bottom. Joey approved 2026-11-06: defaults bd=10 bdlo=10 nlo=1 wlo=4; pass 0s for the old baseline.
const BD = Number(q.get('bd') ?? 10), BDLO = Number(q.get('bdlo') ?? 10);
const bdz = (z) => BD + BDLO * (1 - Math.min(1, Math.max(0, (z - (F0 - 6)) / 170)));
const chest = [
  [F0 - 6, 79 + WX, 80, 76, 3.6],
  [ZS - 200, WALL - 60 + 0.6 * WX, 92, 80, 3.6],
  [ZS - 160, WALL - 46, 100, 84, 3.8],
  [ZS - 111, WALL - 23, 106, 86, 4.0],
  [ZS - 55, TOPW - 3, 108, 88, 4.4],
  [ZS + 10, TOPW - 1, 108, 88, 4.8],
  // Joey 2026-11-06: rounded human shoulder top. Quarter-round cap (R ~40) at the shoulder point,
  // then a sloped trapezius line up to the neck.
  [F1 - 15, TOPW, 104, 88, 4.6],   // shoulder point
  [F1 + 2, TOPW - 4, 100, 87, 4.4],
  [F1 + 13, TOPW - 12, 95, 85, 4.1],
  [F1 + 21, TOPW - 24, 88, 82, 3.8],
  [F1 + 27, TOPW - 40, 80, 78, 3.5],
  [F1 + 34, 96, 68, 70, 3.1],
  [F1 + 41, 68, 55, 58, 2.7],
  [F1 + 48, 42, 44, 46, 2.4],
];
const NLO = Number(q.get('nlo') ?? 1.0), WLO = Number(q.get('wlo') ?? 4);   // squarer / wider section at the frame bottom (clears the lower-box corners)
for (const r of chest) { r[3] += bdz(r[0]); const lo = 1 - Math.min(1, Math.max(0, (r[0] - (F0 - 6)) / 170)); r[4] += NLO * lo; r[1] += WLO * lo; r[2] += WLO * 0.5 * lo; }
// shoulder socket: inside radius SOCK_R of the pitch axis the shell side steps in to the RS03 face (Y = +-97),
// so the rotating P1 and roll RS03 (sweep radius ~57 mm) never touch it
const RIM = Number(q.get('rim') ?? 7);
const SOCK_R = Number(q.get('sock') ?? 78);   // arm thread 2026-11-06: r76 min for full pitch with >=3 mm, r78 for margin
const sstep = (a, b, v) => { const t = Math.min(1, Math.max(0, (v - a) / (b - a))); return t * t * (3 - 2 * t); };
// male chest plate: two pec masses with a sternum groove and a crisp lower edge
const PEC_Z = ZS - 30;
const pec = (x, y, z) => { if (x <= 0) return [x, y, z]; const ay = Math.abs(y); const dz = z - PEC_Z;
  const g = sstep(3, 26, ay) * (1 - sstep(0.62 * WALL, 0.86 * WALL, ay));
  const v = dz >= 0 ? 1 - sstep(10, 105, dz) : 1 - sstep(38, 50, -dz);
  const w = sstep(30, 85, x);
  // Joey 2026-11-06: front of the chest rolls gently back toward each shoulder (plan view), like a human
  // pec wrapping into the deltoid. Proportional in x so the side wall is untouched.
  const rz = sstep(ZS - 170, ZS - 90, z) * (1 - sstep(F1 + 20, F1 + 45, z));
  const roll = 1 - 0.3 * sstep(0.45 * WALL, 0.98 * WALL, ay) * rz;
  return [(x + 15 * g * v * w) * roll, y, z]; };
const socket = (x, y, z) => { const r = Math.hypot(x, z - ZS); if (Math.abs(y) <= FACE) return [x, y, z];
  const t = Math.min(1, Math.max(0, (r - SOCK_R) / RIM));
  const k = RIM > 7 ? Math.sqrt(1 - (1 - t) * (1 - t)) : t * t * (3 - 2 * t);   // RIM > 7: rolled quarter-round lip into the cavity
  return [x, Math.sign(y) * (FACE + (Math.abs(y) - FACE) * k), z]; };
const SEAM_Z = F0 + 160;
CLAMP = (x, y, z) => { const p = pec(x, y, z); return socket(p[0], p[1], p[2]); }; SEAM_CLAMP = CLAMP;
loft(chest, SEAM_Z, F1 + 48, white, 240, 200, 4, 0, Math.PI, 'chest upper front', [110, 0, 60]);
loft(chest, SEAM_Z, F1 + 48, white, 240, 200, 4, Math.PI, 2 * Math.PI, 'chest upper back', [-110, 0, 60]);
loft(chest, F0 - 6, SEAM_Z, white, 50, 80, 4, 0, Math.PI, 'chest lower front', [110, 0, -40]);
loft(chest, F0 - 6, SEAM_Z, white, 50, 80, 4, Math.PI, 2 * Math.PI, 'chest lower back (service hatch)', [-110, 0, -40]);
CLAMP = null;
if (!EXPLODE) { seamRing(chest, SEAM_Z, 4);
seamRing(chest, F0 - 6 + 0.5, 4);
seamVert(chest, 0, F0 - 6, F1 + 40, 4); seamVert(chest, Math.PI, F0 - 6, F1 + 40, 4); }
// shoulder: shell side ends flush with the pitch RS03 face (Y = +-97). A printed collar clamps under the
// RS03 outer-ring bolts; the rotating output and everything outboard stays exposed.
const act = new THREE.MeshStandardMaterial({ color: 0x5b6068, roughness: 0.35, metalness: 0.75 });
const actDark = new THREE.MeshStandardMaterial({ color: 0x25282d, roughness: 0.4, metalness: 0.6 });
const P1mat = new THREE.MeshStandardMaterial({ color: 0x30343a, roughness: 0.55, metalness: 0.2 });
for (const s of [1, -1]) {
  const collar = new THREE.Mesh(new THREE.RingGeometry(43, 57, 72), new THREE.MeshPhysicalMaterial({ color: 0xeef0f3, roughness: 0.38, clearcoat: 0.6, side: THREE.DoubleSide }));
  const cg = new THREE.Group(); cg.add(collar);
  const lip = new THREE.Mesh(new THREE.CylinderGeometry(57, 57, 4, 72, 1, true), collar.material); lip.rotation.x = Math.PI / 2; lip.position.z = -2; cg.add(lip);
  for (let k = 0; k < 8; k++) { const t = (k + 0.5) * Math.PI / 4; const bh = new THREE.Mesh(new THREE.CylinderGeometry(3.6, 3.6, 3, 16), actDark);
    bh.rotation.x = Math.PI / 2; bh.position.set(49 * Math.cos(t), 49 * Math.sin(t), 1.5); cg.add(bh); }
  cg.rotation.y = s * Math.PI / 2; cg.position.copy(P(0, s * FACE, ZS)); add(cg); cg.traverse(o => { o.castShadow = true; });
  if (EXPLODE) cg.position.x += s * 70;
  if (!EXPLODE) parts.length < 99 && (s > 0) && parts.push({ tag: 'shoulder collar x2', Xdepth: 114, Ywidth: 6, Zheight: 114 });
  if (EXPLODE) continue;
  // rotating output flange + P1 + roll RS03 + P2/upper-arm-yaw RS02, all exposed
  const out = new THREE.Mesh(new THREE.CylinderGeometry(42, 42, 6, 64), actDark); out.rotation.z = Math.PI / 2; out.position.copy(P(0, s * (FACE + 2.5), ZS)); add(out);
  const p1 = new THREE.Mesh(new THREE.CylinderGeometry(46, 46, 20, 64), P1mat); p1.rotation.z = Math.PI / 2; p1.position.copy(P(0, s * (FACE + 15.5), ZS)); add(p1);
  const p1b = new THREE.Mesh(new THREE.BoxGeometry(YS - 156.5 - FW, 92, 64), P1mat); p1b.position.copy(P(-2, s * (117 + FW + YS - 40) / 2, ZS)); add(p1b);
  const roll = new THREE.Mesh(new THREE.CylinderGeometry(49.75, 49.75, 56.6, 64), act); roll.rotation.x = Math.PI / 2; roll.position.copy(P(-4, s * YS, ZS)); add(roll);
  for (const xf of [-32.6, 24.6]) { const rr = new THREE.Mesh(new THREE.TorusGeometry(46, 2.2, 10, 64), actDark); rr.position.copy(P(xf, s * YS, ZS)); add(rr); }
  const rface = new THREE.Mesh(new THREE.CylinderGeometry(22, 22, 4, 48), actDark); rface.rotation.x = Math.PI / 2; rface.position.copy(P(26, s * YS, ZS)); add(rface);
  const p2 = new THREE.Mesh(new THREE.BoxGeometry(70, 30, 86), P1mat); p2.position.copy(P(0, s * YS, ZS - 62)); add(p2);
  const yaw = new THREE.Mesh(new THREE.CylinderGeometry(39.25, 39.25, 45.5, 64), act); yaw.position.copy(P(0, s * YS, ZS - 100)); add(yaw);
  const yr = new THREE.Mesh(new THREE.TorusGeometry(37, 2, 10, 64), actDark); yr.rotation.x = Math.PI / 2; yr.position.copy(P(0, s * YS, ZS - 122)); add(yr);
}
// ---- details from Joey's references: socket bezel, dark yoke panel, side lattice vents ----
const darkGloss = new THREE.MeshPhysicalMaterial({ color: 0x1d2024, roughness: 0.25, metalness: 0.2, clearcoat: 0.8, side: THREE.DoubleSide });
function patch(rows, z0, z1, thA, thB, grow, mat, x0 = 4, clamp = null) {
  const nz = 60, nt = 60, pos = [], idx = [];
  for (let i = 0; i <= nz; i++) { const z = z0 + (z1 - z0) * i / nz; const [hw, df, db, n] = interp(rows, z);
    for (let j = 0; j <= nt; j++) { const th = thA(z) + (thB(z) - thA(z)) * j / nt; let [x, y] = ringPt(hw + grow, df + grow, db + grow, n, th, x0); let zz = z;
      if (clamp) [x, y, zz] = clamp(x, y, z); const v = P(x, y, zz); pos.push(v.x, v.y, v.z); } }
  for (let i = 0; i < nz; i++) for (let j = 0; j < nt; j++) { const a2 = i * (nt + 1) + j, b2 = a2 + nt + 1; idx.push(a2, b2, a2 + 1, b2, b2 + 1, a2 + 1); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setIndex(idx); g.computeVertexNormals();
  return add(new THREE.Mesh(g, mat));
}
if (!EXPLODE && !XRAY) {
  // socket bezel: raised white ring around each shoulder opening + dark shadow gap
  // (bezel rings dropped with the r 93 socket: they stood proud of the shell top)
  for (const sd of (SOCK_R < 80 ? [1, -1] : [])) {
    const bz = new THREE.Mesh(new THREE.TorusGeometry(SOCK_R + 3, 3.2, 14, 96), white); bz.rotation.y = Math.PI / 2; bz.position.copy(P(0, sd * 116, ZS)); add(bz);
    const gap = new THREE.Mesh(new THREE.TorusGeometry(SOCK_R - 1, 1.6, 10, 96), darkGloss); gap.rotation.y = Math.PI / 2; gap.position.copy(P(0, sd * 108, ZS)); add(gap);
  }
  // side lattice vents on the lower chest: dark opening + white diamond ribs
  for (const sd of [0, Math.PI]) {
    const zA = F0 + 25, zB = F0 + 140, w = 0.15 * Math.PI;
    patch(chest, zA, zB, () => sd - w, () => sd + w, 0.8, darkGloss);
    const rib = (f0, f1) => { const pts = []; const ua = Math.max(0, Math.min((0 - f0) / (f1 - f0), (1 - f0) / (f1 - f0))), ub = Math.min(1, Math.max((0 - f0) / (f1 - f0), (1 - f0) / (f1 - f0))); if (ub - ua < 0.05) return;
      for (let i = 0; i <= 30; i++) { const u = ua + (ub - ua) * i / 30; const z = zA + (zB - zA) * u; const f = f0 + (f1 - f0) * u;
      const [hw, df, db, n] = interp(chest, z); const [x, y] = ringPt(hw + 1.8, df + 1.8, db + 1.8, n, sd - w + 2 * w * f, 4); pts.push(P(x, y, z)); }
      add(new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts), 40, 2.4, 8), white)); };
    for (let k = -3; k <= 3; k++) { rib(k / 3, k / 3 + 1); rib(k / 3 + 1, k / 3); }
    // frame the vent with a rim
    for (const f of [0, 1]) { const pts = []; for (let i = 0; i <= 30; i++) { const z = zA + (zB - zA) * i / 30; const [hw, df, db, n] = interp(chest, z);
      const [x, y] = ringPt(hw + 1.8, df + 1.8, db + 1.8, n, sd - w + 2 * w * f, 4); pts.push(P(x, y, z)); }
      add(new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts), 40, 2.6, 8), white)); }
  }
}
// chest "core" light: subtle accent, original detail
const core = new THREE.Mesh(new THREE.CylinderGeometry(9, 9, 3, 48), accent);


// neck collar + head placeholder
const neck = new THREE.Mesh(new THREE.CylinderGeometry(30, 34, 50, 48), dark); neck.position.copy(P(0, 0, F1 + 58)); if (!EXPLODE) add(neck);
const headRows = [[F1 + 70, 50, 60, 58, 3.4], [F1 + 96, 60, 76, 72, 3.6], [F1 + 190, 64, 82, 80, 3.6], [F1 + 252, 56, 70, 72, 3.2], [F1 + 276, 34, 44, 46, 2.6], [F1 + 284, 4, 4, 4, 2]];
const SHOW_HEAD = false;
if (!EXPLODE && SHOW_HEAD) loft(headRows, F1 + 70, F1 + 284, white, 60, 96, 6);
if (!EXPLODE && SHOW_HEAD) { // visor: thin dark strip following head front
  const pos = [], idx = []; const nz = 8, nt = 48;
  for (let i = 0; i <= nz; i++) { const z = F1 + 150 + 26 * i / nz; const [hw, df, db, n] = interp(headRows, z);
    for (let j = 0; j <= nt; j++) { const th = Math.PI * (0.18 + 0.64 * j / nt); const [x, y] = ringPt(hw + 1.2, df + 1.2, db, n, th, 6);
      const v = P(x, y, z); pos.push(v.x, v.y, v.z); } }
  for (let i = 0; i < nz; i++) for (let j = 0; j < nt; j++) { const a = i * (nt + 1) + j, b = a + nt + 1; idx.push(a, b, a + 1, b, b + 1, a + 1); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setIndex(idx); g.computeVertexNormals();
  add(new THREE.Mesh(g, new THREE.MeshPhysicalMaterial({ color: 0x15171a, roughness: 0.08, metalness: 0.3, clearcoat: 1, side: THREE.DoubleSide })));
}

// ---------- ABDOMEN (longer stomach): segmented bands over waist bay + extension ----------
const abMatDef = 1; const abdMat = new THREE.MeshPhysicalMaterial({ color: 0x23262b, roughness: 0.4, metalness: 0.3, clearcoat: 0.5 });
const abTop = F0 - 8, abBot = WAIST + 8;
const nb = EXT >= 50 ? 3 : 2; const gap = 5; const bh = (abTop - abBot - gap * (nb - 1)) / nb;
for (let b = 0; b < nb; b++) {
  const zt = abTop - b * (bh + gap), zb = zt - bh;
  const t = b / Math.max(1, nb - 1);
  const hw = 68 + WX - 3 * t, df = 66 - 2 * t, db = 64 - 2 * t;
  const rows = [[zb, hw - 4, df - 4, db - 4, 3.2], [zb + 6, hw, df, db, 3.2], [zt - 6, hw, df, db, 3.2], [zt, hw - 4, df - 4, db - 4, 3.2]];
  loft(rows, zb, zt, EXPLODE ? white : abdMat, 16, 64, 2, 0, Math.PI, `abdomen band ${b + 1} front`, [90, 0, -40 - 30 * b]);
  loft(rows, zb, zt, EXPLODE ? white : abdMat, 16, 64, 2, Math.PI, 2 * Math.PI, `abdomen band ${b + 1} back`, [-90, 0, -40 - 30 * b]);
}
const spine = new THREE.Mesh(new THREE.CylinderGeometry(54, 54, abTop - abBot + 10, 48), alu);
spine.position.copy(P(0, 0, (abTop + abBot) / 2)); add(spine);

// ---------- PELVIS placeholder (draft 215 x 260 x 115) ----------
const pelvis = [[PELVIS0 - 20, 118, 98, 98, 3], [PELVIS0 + 30, 130, 108, 104, 3.2], [WAIST - 6, 112, 96, 92, 3.2], [WAIST, 100, 86, 84, 3]];
if (!EXPLODE) loft(pelvis, PELVIS0 - 20, WAIST, FULL ? white : ghost, 30, 128, 4);

// ---------- ARMS ----------
function arm(side) {
  const g = new THREE.Group(); root.add(g);
  const pivot = P(0, side * YS, ZS);
  if (EXPLODE) return;
  // inner face visible joint ring
  // upper arm: closed tube 215 mm, elbow RS02 dark, forearm 195
  const bend = 15 * Math.PI / 180;
  const ua = [[ZS - 215 + 35, 39, 40, 40, 2.2], [ZS - 160, 43, 44, 44, 2.2], [ZS - 126, 42, 43, 43, 2.3]];
  const u = loft(ua, ZS - 215 + 30, ZS - 124, white, 40, 72, 0); u.geometry.translate(side * YS, 0, 0);
  const elbow = new THREE.Mesh(new THREE.CylinderGeometry(39.5, 39.5, 74, 48), dark);
  elbow.rotation.z = Math.PI / 2; elbow.position.copy(P(0, side * YS, ZS - 215)); add(elbow);
  // forearm, rotated forward by default elbow bend about elbow axis (Y)
  const fa = [[-195 + 20, 31, 29, 29, 2.2], [-120, 38, 36, 36, 2.2], [-40, 41, 41, 41, 2.3], [-18, 37, 37, 37, 2.3]];
  const fm = loft(fa, -195 + 20, -18, white, 40, 72, 0);
  fm.geometry.translate(-OFFX, 0, 0); // build at origin in three-x then position
  const fg = new THREE.Group(); fg.add(fm); fg.position.set(side * YS + OFFX, ZS - 215, 0); fg.rotation.x = -bend;
  root.remove(fm); fg.add(fm); root.add(fg);
  const hand = new THREE.Mesh(new THREE.BoxGeometry(56, 90, 30), dark); hand.position.set(0, -195 - 40, 0); fg.add(hand);
  [u, elbow, fm, hand].forEach(o => { o.castShadow = true; });
}
arm(1); arm(-1);

// ---------- LEGS (ghost proportion only) ----------
if (FULL) {
  for (const s of [1, -1]) {
    const thigh = new THREE.Mesh(new THREE.CapsuleGeometry(52, 260, 8, 24), ghost); thigh.position.copy(P(0, s * 105, 740 - 170)); add(thigh);
    const shin = new THREE.Mesh(new THREE.CapsuleGeometry(44, 270, 8, 24), ghost); shin.position.copy(P(0, s * 105, 405 - 175)); add(shin);
    const foot = new THREE.Mesh(new THREE.BoxGeometry(100, 40, 230), ghost); foot.position.copy(P(35, s * 105, 20)); add(foot);
  }
  const fl = new THREE.Mesh(new THREE.PlaneGeometry(4000, 4000), new THREE.ShadowMaterial({ opacity: 0.18 }));
  fl.rotation.x = -Math.PI / 2; fl.receiveShadow = true; scene.add(fl);
}

// ---------- X-RAY: 2020 frame, pitch actuators, mount points ----------
if (XRAY || EXPLODE) {
  const bar = (x, y, z, sx, sy, sz) => { const b = new THREE.Mesh(new THREE.BoxGeometry(sy, sz, sx), alu); b.position.copy(P(x, y, z)); add(b); };
  // two-box frame (arm thread, torso-stepped de5a006): wide upper box from ZS-71.24 to F1, existing lower box below,
  // 3 mm aluminium gusset plates front and back (outer face X +-73) bridging the step down to ZS-139.86
  const ZB = ZS - 71.24, ZG = ZS - 139.86;
  for (const x of [-60, 60]) for (const y of [-65, 65]) bar(x, y, (F0 + ZB) / 2, 20, 20, ZB - F0);
  for (const x of [-60, 60]) for (const y of [-65 - FW, 65 + FW]) bar(x, y, (ZB + F1) / 2, 20, 20, F1 - ZB);
  for (const x of [-60, 60]) { bar(x, 0, F0 + 10, 20, 110, 20); bar(x, 0, F1 - 10, 20, 110 + 2 * FW, 20); bar(x, 0, ZB + 10, 20, 110 + 2 * FW, 20); }
  for (const y of [-65, 65]) bar(0, y, F0 + 10, 100, 20, 20);
  for (const y of [-65 - FW, 65 + FW]) { bar(0, y, F1 - 10, 100, 20, 20); bar(0, y, ZB + 10, 100, 20, 20); }
  const gshape = new THREE.Shape(); const gw = 75 + FW;
  gshape.moveTo(-gw, F1); gshape.lineTo(gw, F1); gshape.lineTo(gw, ZB); gshape.lineTo(75, ZG); gshape.lineTo(-75, ZG); gshape.lineTo(-gw, ZB); gshape.closePath();
  const ghole = new THREE.Path(); ghole.moveTo(-gw + 20, F1 - 20); ghole.lineTo(gw - 20, F1 - 20); ghole.lineTo(gw - 20, ZB + 20); ghole.lineTo(-gw + 20, ZB + 20); ghole.closePath(); gshape.holes.push(ghole);
  for (const sx of [1, -1]) { const g = new THREE.Mesh(new THREE.ExtrudeGeometry(gshape, { depth: 3, bevelEnabled: false }), alu);
    g.position.set(OFFX, 0, sx > 0 ? 70 : -73); add(g); }
  for (const s of [1, -1]) { const a = new THREE.Mesh(new THREE.CylinderGeometry(49.75, 49.75, 56.6, 48), dark); a.rotation.z = Math.PI / 2; a.position.copy(P(0, s * (FACE - 0.5 - 28.3), ZS)); add(a); }
  // proposed mount bosses: printed standoffs into T-nuts on the outer slots
  const zs = [F0 + 40, F0 + 160, F1 - 40];
  for (const z of zs) for (const x of [-70, 70]) for (const y of [-55, 55]) {
    const b = new THREE.Mesh(new THREE.SphereGeometry(6, 16, 12), mount); b.position.copy(P(x + Math.sign(x) * 6, y, z)); add(b); }
}

// ---------- lights & camera ----------
const key = new THREE.DirectionalLight(0xffffff, 2.6); key.position.set(1400, 1900, 1100); key.castShadow = true;
key.shadow.mapSize.set(2048, 2048); Object.assign(key.shadow.camera, { left: -1200, right: 1200, top: 2000, bottom: -200, near: 100, far: 6000 });
scene.add(key);
const rim = new THREE.DirectionalLight(0xcfe3ff, 1.2); rim.position.set(-1500, 1500, -1200); scene.add(rim);
scene.add(new THREE.HemisphereLight(0xffffff, 0x7a808a, 0.25));

const tgtY = q.get('ty') ? ZS + Number(q.get('ty')) : (FULL ? 780 : (ZS - 120));
let cam;
const aspect = W / H;
const ortho = (h) => { const c = new THREE.OrthographicCamera(-h * aspect / 2, h * aspect / 2, h / 2, -h / 2, 1, 10000); return c; };
const span = FULL ? Number(q.get('span') ?? 1700) : 900;
if (VIEW === 'front') { cam = ortho(span); cam.position.set(Number(q.get('cx') ?? 0), tgtY, 3000); }
else if (VIEW === 'side') { cam = ortho(span); cam.position.set(3000, tgtY, 0); }
else if (VIEW === 'back') { cam = ortho(span); cam.position.set(0, tgtY, -3000); }
else { cam = new THREE.PerspectiveCamera(24, aspect, 10, 20000); const d = Number(q.get('d') ?? (FULL ? 4600 : 2300));
  const az = Number(q.get('az') ?? 35) * Math.PI / 180, el = Number(q.get('el') ?? 12) * Math.PI / 180;
  cam.position.set(d * Math.sin(az) * Math.cos(el), tgtY + d * Math.sin(el), d * Math.cos(az) * Math.cos(el)); }
const cx = Number(q.get('cx') ?? 0); if (q.get('cx') && cam.isPerspectiveCamera) cam.position.x += cx; cam.lookAt(cx, tgtY, 0);
renderer.render(scene, cam);
console.log('PARTS ' + JSON.stringify(parts));
window.__done = true;
