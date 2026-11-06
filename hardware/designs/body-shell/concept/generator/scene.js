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
const PALETTE = [0x4f9de0, 0x4f9de0, 0x63c08a, 0x63c08a, 0xe8a34a, 0xe8a34a, 0xb07fd6, 0xb07fd6, 0xd96a8a, 0xd96a8a, 0xe8d36a, 0xe8d36a, 0x56c7c7, 0x56c7c7];        // lateral offset for side-by-side

const P = (x, y, z) => new THREE.Vector3(y + OFFX, z, x);

// kinematics.md: pelvis bottom 740 from floor, waist (pelvis top) 855, frame 155..495 above pelvis bottom
const PELVIS0 = 740, WAIST = 855;
const F0 = PELVIS0 + 155 + EXT, F1 = PELVIS0 + 495 + EXT;
const ZS = PELVIS0 + 424.9 + EXT;   // shoulder pitch axis
const YS = 170.49;                   // pitch/roll joint, from left_arm.urdf

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
function interp(rows, z) { // monotone cubic (Fritsch-Carlson) per column: smooth, no overshoot
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
function ringPt(hw, df, db, n, th, x0 = 0) {
  const c = Math.cos(th), s = Math.sin(th);
  const y = hw * sgnpow(c, 2 / n);
  const x = (s >= 0 ? df : db) * sgnpow(s, 2 / n);
  return [x + x0, y];
}
function loft(rows, z0, z1, mat, nz = 90, nt = 128, x0 = 0, th0 = 0, th1 = 2 * Math.PI, tag = null, ex = [0, 0, 0]) {
  const pos = [], idx = [];
  for (let i = 0; i <= nz; i++) {
    const z = z0 + (z1 - z0) * i / nz; const [hw, df, db, n] = interp(rows, z);
    for (let j = 0; j <= nt; j++) { const th = th0 + (th1 - th0) * j / nt; const [x, y] = ringPt(hw, df, db, n, th, x0);
      const v = P(x, y, z); pos.push(v.x, v.y, v.z); }
  }
  for (let i = 0; i < nz; i++) for (let j = 0; j < nt; j++) {
    const a = i * (nt + 1) + j, b = a + nt + 1; idx.push(a, b, a + 1, b, b + 1, a + 1); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setIndex(idx); g.computeVertexNormals();
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
    const [x, y] = ringPt(hw + grow, df + grow, db + grow, n, th, x0); pts.push(P(x, y, z)); }
  add(new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(pts), 120, 0.9, 6), seamMat));
}

// ---------- CHEST shell (torso-fixed, bolts to 2020 frame) ----------
// rows: [z, half-width Y, front depth +X, back depth -X, squareness n]
// frame is +-75 Y, +-70 X. Arm hangs at Y=170 with 15 deg inward roll -> keep side <= ~100 below shoulder.
const chest = [
  [F0 - 6, 81, 78, 76, 3.4],
  [F0 + 80, 83, 82, 78, 3.4],
  [F0 + 170, 94, 98, 84, 3.0],
  [F0 + 245, 104, 110, 88, 2.9],
  [ZS, 108, 106, 88, 3.1],
  [ZS + 36, 104, 92, 84, 3.4],
  [F1 + 2, 90, 76, 78, 3.4],
  [F1 + 26, 62, 56, 62, 2.8],
  [F1 + 44, 40, 40, 46, 2.4],
];
const SEAM_Z = F0 + 160;
loft(chest, SEAM_Z, F1 + 44, white, 80, 80, 4, 0, Math.PI, 'chest upper front', [110, 0, 60]);
loft(chest, SEAM_Z, F1 + 44, white, 80, 80, 4, Math.PI, 2 * Math.PI, 'chest upper back', [-110, 0, 60]);
loft(chest, F0 - 6, SEAM_Z, white, 50, 80, 4, 0, Math.PI, 'chest lower front', [110, 0, -40]);
loft(chest, F0 - 6, SEAM_Z, white, 50, 80, 4, Math.PI, 2 * Math.PI, 'chest lower back (service hatch)', [-110, 0, -40]);
if (!EXPLODE) { seamRing(chest, SEAM_Z, 4);
seamRing(chest, F0 - 6 + 0.5, 4);
seamVert(chest, 0, F0 - 6, F1 + 30, 4); seamVert(chest, Math.PI, F0 - 6, F1 + 30, 4); }
// raised chest shield (separate panel) on the front only
{
  const rows = chest.map(r => [r[0], r[1] + 6, r[2] + 7, r[3], r[4]]);
  const z0 = F0 + 165, z1 = ZS + 46; const nz = 70, nt = 90; const pos = [], idx = [];
  for (let i = 0; i <= nz; i++) { const z = z0 + (z1 - z0) * i / nz; const [hw, df, db, n] = interp(rows, z);
    const u = (z - z0) / (z1 - z0); const span = 0.16 + 0.10 * Math.pow(1 - u, 2); // shield narrows toward the bottom edge
    for (let j = 0; j <= nt; j++) { const th = Math.PI * (span + (1 - 2 * span) * j / nt); const [x, y] = ringPt(hw, df, db, n, th, 4);
      const v = P(x, y, z); pos.push(v.x, v.y, v.z); } }
  for (let i = 0; i < nz; i++) for (let j = 0; j < nt; j++) { const a = i * (nt + 1) + j, b = a + nt + 1; idx.push(a, b, a + 1, b, b + 1, a + 1); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setIndex(idx); g.computeVertexNormals();
  g.computeBoundingBox(); { const bb = g.boundingBox; parts.push({ tag: 'chest shield', Xdepth: +(bb.max.z - bb.min.z).toFixed(0), Ywidth: +(bb.max.x - bb.min.x).toFixed(0), Zheight: +(bb.max.y - bb.min.y).toFixed(0) }); }
  const sm = add(new THREE.Mesh(g, EXPLODE ? new THREE.MeshPhysicalMaterial({ color: 0xbfe0ff, roughness: 0.4, side: THREE.DoubleSide }) : white));
  if (EXPLODE) { sm.position.z += 230; sm.position.y += 40; }
  if (!EXPLODE) {
  // shadow-gap outline around the shield
  const edge = []; const rowsE = chest.map(r => [r[0], r[1] + 2, r[2] + 2.5, r[3], r[4]]);
  const spanAt = (z) => { const u = (z - z0) / (z1 - z0); return 0.16 + 0.10 * Math.pow(1 - u, 2); };
  const ptAt = (z, f) => { const [hw, df, db, n] = interp(rowsE, z); const sp = spanAt(z); const [x, y] = ringPt(hw, df, db, n, Math.PI * (sp + (1 - 2 * sp) * f), 4); return P(x, y, z); };
  for (let i = 0; i <= 40; i++) edge.push(ptAt(z0 + (z1 - z0) * i / 40, 0));
  for (let j = 1; j <= 40; j++) edge.push(ptAt(z1, j / 40));
  for (let i = 39; i >= 0; i--) edge.push(ptAt(z0 + (z1 - z0) * i / 40, 1));
  for (let j = 39; j >= 1; j--) edge.push(ptAt(z0, j / 40));
  add(new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(edge, true, 'centripetal'), 400, 2.2, 6, true), dark));
  }
}
// shoulder pitch pass-through bosses (dark ring = rotating P1 face at Y=97)
for (const s of [1, -1]) {
  const ring = new THREE.Mesh(new THREE.CylinderGeometry(52, 52, 26, 64), dark);
  ring.rotation.z = Math.PI / 2; ring.position.copy(P(0, s * 110, ZS)); add(ring);
}
// chest "core" light: subtle accent, original detail
const core = new THREE.Mesh(new THREE.CylinderGeometry(9, 9, 3, 48), accent);
core.rotation.x = Math.PI / 2; core.position.copy(P(113.5, 0, ZS - 40)); if (!XRAY) add(core);

// neck collar + head placeholder
const neck = new THREE.Mesh(new THREE.CylinderGeometry(30, 34, 50, 48), dark); neck.position.copy(P(0, 0, F1 + 50)); if (!EXPLODE) add(neck);
const headRows = [[F1 + 70, 40, 52, 52, 2.2], [F1 + 105, 62, 78, 76, 2.4], [F1 + 190, 66, 84, 80, 2.6], [F1 + 250, 54, 68, 68, 2.4], [F1 + 274, 36, 46, 46, 2.2], [F1 + 284, 4, 4, 4, 2]];
if (!EXPLODE) loft(headRows, F1 + 70, F1 + 284, white, 60, 96, 6);
if (!EXPLODE) { // visor: thin dark strip following head front
  const pos = [], idx = []; const nz = 8, nt = 48;
  for (let i = 0; i <= nz; i++) { const z = F1 + 150 + 26 * i / nz; const [hw, df, db, n] = interp(headRows, z);
    for (let j = 0; j <= nt; j++) { const th = Math.PI * (0.18 + 0.64 * j / nt); const [x, y] = ringPt(hw + 1.2, df + 1.2, db, n, th, 6);
      const v = P(x, y, z); pos.push(v.x, v.y, v.z); } }
  for (let i = 0; i < nz; i++) for (let j = 0; j < nt; j++) { const a = i * (nt + 1) + j, b = a + nt + 1; idx.push(a, b, a + 1, b, b + 1, a + 1); }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setIndex(idx); g.computeVertexNormals();
  add(new THREE.Mesh(g, new THREE.MeshPhysicalMaterial({ color: 0x15171a, roughness: 0.08, metalness: 0.3, clearcoat: 1, side: THREE.DoubleSide })));
}

// ---------- ABDOMEN (longer stomach): segmented bands over waist bay + extension ----------
const abTop = F0 - 10, abBot = WAIST + 8;
const nb = EXT >= 50 ? 3 : 2; const gap = 5; const bh = (abTop - abBot - gap * (nb - 1)) / nb;
for (let b = 0; b < nb; b++) {
  const zt = abTop - b * (bh + gap), zb = zt - bh;
  const t = b / Math.max(1, nb - 1);
  const hw = 80 - 6 * t, df = 80 - 4 * t, db = 76 - 4 * t;
  const rows = [[zb, hw - 4, df - 4, db - 4, 3.2], [zb + 6, hw, df, db, 3.2], [zt - 6, hw, df, db, 3.2], [zt, hw - 4, df - 4, db - 4, 3.2]];
  loft(rows, zb, zt, white, 16, 64, 2, 0, Math.PI, `abdomen band ${b + 1} front`, [90, 0, -40 - 30 * b]);
  loft(rows, zb, zt, white, 16, 64, 2, Math.PI, 2 * Math.PI, `abdomen band ${b + 1} back`, [-90, 0, -40 - 30 * b]);
}
const spine = new THREE.Mesh(new THREE.CylinderGeometry(58, 58, abTop - abBot + 10, 48), dark);
spine.position.copy(P(0, 0, (abTop + abBot) / 2)); add(spine);

// ---------- PELVIS placeholder (draft 215 x 260 x 115) ----------
const pelvis = [[PELVIS0 - 20, 118, 98, 98, 3], [PELVIS0 + 30, 130, 108, 104, 3.2], [WAIST - 6, 112, 96, 92, 3.2], [WAIST, 100, 86, 84, 3]];
if (!EXPLODE) loft(pelvis, PELVIS0 - 20, WAIST, FULL ? white : ghost, 30, 128, 4);

// ---------- ARMS ----------
function arm(side) {
  const g = new THREE.Group(); root.add(g);
  const pivot = P(0, side * YS, ZS);
  // deltoid cap: squared superellipsoid riding on roll actuator (moves with arm)
  const del = [[ZS - 72, 48, 56, 56, 3.2], [ZS - 44, 60, 66, 66, 4.2], [ZS + 22, 64, 68, 68, 4.2], [ZS + 50, 58, 62, 62, 3.6], [ZS + 64, 40, 48, 48, 3.0], [ZS + 68, 16, 20, 20, 2.4]];
  const m = loft(del, ZS - 72, ZS + 68, white, 60, 128, 0, 0, 2 * Math.PI, side > 0 ? 'left deltoid cap (on arm)' : 'right deltoid cap (on arm)', [0, side * 120, 60]); m.geometry.translate(side * (YS + 6), 0, 0);
  if (EXPLODE) return;
  // inner face visible joint ring
  // upper arm: closed tube 215 mm, elbow RS02 dark, forearm 195
  const bend = 15 * Math.PI / 180;
  const ua = [[ZS - 215 + 35, 39, 40, 40, 2.2], [ZS - 120, 44, 45, 45, 2.2], [ZS - 70, 46, 47, 47, 2.3]];
  const u = loft(ua, ZS - 215 + 30, ZS - 68, white, 40, 72, 0); u.geometry.translate(side * YS, 0, 0);
  const elbow = new THREE.Mesh(new THREE.CylinderGeometry(39.5, 39.5, 74, 48), dark);
  elbow.rotation.z = Math.PI / 2; elbow.position.copy(P(0, side * YS, ZS - 215)); add(elbow);
  // forearm, rotated forward by default elbow bend about elbow axis (Y)
  const fa = [[-195 + 20, 31, 29, 29, 2.2], [-120, 38, 36, 36, 2.2], [-40, 41, 41, 41, 2.3], [-18, 37, 37, 37, 2.3]];
  const fm = loft(fa, -195 + 20, -18, white, 40, 72, 0);
  fm.geometry.translate(-OFFX, 0, 0); // build at origin in three-x then position
  const fg = new THREE.Group(); fg.add(fm); fg.position.set(side * YS + OFFX, ZS - 215, 0); fg.rotation.x = -bend;
  root.remove(fm); fg.add(fm); root.add(fg);
  const hand = new THREE.Mesh(new THREE.BoxGeometry(56, 90, 30), dark); hand.position.set(0, -195 - 40, 0); fg.add(hand);
  [m, u, elbow, fm, hand].forEach(o => { o.castShadow = true; });
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
  for (const x of [-60, 60]) for (const y of [-65, 65]) bar(x, y, (F0 + F1) / 2, 20, 20, 340);
  for (const z of [F0 + 10, F1 - 10]) { for (const x of [-60, 60]) bar(x, 0, z, 20, 110, 20); for (const y of [-65, 65]) bar(0, y, z, 100, 20, 20); }
  for (const s of [1, -1]) { const a = new THREE.Mesh(new THREE.CylinderGeometry(49.75, 49.75, 56.6, 48), dark); a.rotation.z = Math.PI / 2; a.position.copy(P(0, s * (97 - 28.3), ZS)); add(a); }
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

const tgtY = FULL ? 780 : (ZS - 120);
let cam;
const aspect = W / H;
const ortho = (h) => { const c = new THREE.OrthographicCamera(-h * aspect / 2, h * aspect / 2, h / 2, -h / 2, 1, 10000); return c; };
const span = FULL ? Number(q.get('span') ?? 1700) : 900;
if (VIEW === 'front') { cam = ortho(span); cam.position.set(Number(q.get('cx') ?? 0), tgtY, 3000); }
else if (VIEW === 'side') { cam = ortho(span); cam.position.set(3000, tgtY, 0); }
else if (VIEW === 'back') { cam = ortho(span); cam.position.set(0, tgtY, -3000); }
else { cam = new THREE.PerspectiveCamera(24, aspect, 10, 20000); const d = FULL ? 4600 : 2300;
  const az = Number(q.get('az') ?? 35) * Math.PI / 180, el = Number(q.get('el') ?? 12) * Math.PI / 180;
  cam.position.set(d * Math.sin(az) * Math.cos(el), tgtY + d * Math.sin(el), d * Math.cos(az) * Math.cos(el)); }
cam.lookAt(Number(q.get('cx') ?? 0), tgtY, 0);
renderer.render(scene, cam);
console.log('PARTS ' + JSON.stringify(parts));
window.__done = true;
