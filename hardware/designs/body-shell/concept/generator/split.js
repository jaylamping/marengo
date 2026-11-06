// Manufacturing renders: real split pieces from split_check.py (out/pieces/*.stl), frame, hardpoints, doors, airflow.
// Robot frame (mm): X forward, Y left, Z up from floor. three frame: x = robotY, y = robotZ, z = robotX.
import * as THREE from 'three';
import { RoomEnvironment } from './node_modules/three/examples/jsm/environments/RoomEnvironment.js';
import { STLLoader } from './node_modules/three/examples/jsm/loaders/STLLoader.js';
const q = new URLSearchParams(location.search);
const MODE = q.get('mode') ?? 'assembled';   // assembled | explode | mounts | plates | air
const VIEW = q.get('view') ?? 'iso';
const P = (x, y, z) => new THREE.Vector3(y, z, x);
const ZS = 740 + 424.9 + 70, F0 = 740 + 155 + 70, F1 = 740 + 495 + 70, ZB = ZS - 71.24, ZG = ZS - 139.86, FW = 38, FACE = 135.5;
const W = 1600, H = 1200;
const scene = new THREE.Scene(); scene.background = new THREE.Color(0xc9ced6);
const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
renderer.setSize(W, H); renderer.toneMapping = THREE.ACESFilmicToneMapping; renderer.toneMappingExposure = 0.85;
renderer.shadowMap.enabled = true; document.body.appendChild(renderer.domElement);
scene.environment = new THREE.PMREMGenerator(renderer).fromScene(new RoomEnvironment(), 0.04).texture;
const root = new THREE.Group(); scene.add(root);
const add = (m) => { m.castShadow = m.receiveShadow = true; root.add(m); return m; };
const XR = MODE === 'mounts' || MODE === 'air';
const COL = { U1: 0xeef0f3, U2: 0xe4e7ec, U3: 0xeef0f3, U4: 0xe4e7ec, D1: 0x4f9de0, D2: 0x63c08a, D3: 0xb07fd6, V1: 0xe8a34a, C: 0xd96a8a, A1: 0x34383e, A2: 0x34383e, A3: 0x34383e };
const SPLITCOL = { L: 0, R: 1 };
const mat = (c, o = 1) => new THREE.MeshPhysicalMaterial({ color: c, roughness: 0.4, clearcoat: 0.5, side: THREE.DoubleSide,
  transparent: o < 1, opacity: o, depthWrite: o >= 1 });
const alu = new THREE.MeshStandardMaterial({ color: 0x1c1d20, roughness: 0.5, metalness: 0.7 });
const hp = new THREE.MeshStandardMaterial({ color: 0xff8a1f, roughness: 0.5 });
const dark = new THREE.MeshStandardMaterial({ color: 0x5b6068, roughness: 0.35, metalness: 0.75 });

const [rep, mounts] = await Promise.all([fetch('out/split_report.json').then(r => r.json()), fetch('mounts.json').then(r => r.json())]);
const loader = new STLLoader();
const load = (f) => new Promise((res) => loader.load(f, res));

function frame() {
  const bar = (x, y, z, sx, sy, sz) => add(new THREE.Mesh(new THREE.BoxGeometry(sy, sz, sx), alu)).position.copy(P(x, y, z));
  for (const x of [-60, 60]) for (const y of [-65, 65]) bar(x, y, (F0 + ZB) / 2, 20, 20, ZB - F0);
  for (const x of [-60, 60]) for (const y of [-65 - FW, 65 + FW]) bar(x, y, (ZB + F1) / 2, 20, 20, F1 - ZB);
  for (const x of [-60, 60]) { bar(x, 0, F0 + 10, 20, 110, 20); bar(x, 0, F1 - 10, 20, 110 + 2 * FW, 20); bar(x, 0, ZB + 10, 20, 110 + 2 * FW, 20); }
  for (const y of [-65, 65]) bar(0, y, F0 + 10, 100, 20, 20);
  for (const y of [-65 - FW, 65 + FW]) { bar(0, y, F1 - 10, 100, 20, 20); bar(0, y, ZB + 10, 100, 20, 20); }
  for (const sx of [1, -1]) for (const sy of [1, -1]) {           // gusset plates, outline from torso-stepped/README.md
    const s = new THREE.Shape(); const pts = [[55, ZG], [75, ZG], [113, ZG + 50], [113, ZS], [93, ZS], [93, ZB + 20], [55, ZB + 20]];
    s.moveTo(sy * pts[0][0], pts[0][1]); for (const p of pts.slice(1)) s.lineTo(sy * p[0], p[1]); s.closePath();
    const g = add(new THREE.Mesh(new THREE.ExtrudeGeometry(s, { depth: 3, bevelEnabled: false }), alu)); g.position.set(0, 0, sx > 0 ? 70 : -73); }
  for (const s of [1, -1]) { const a = add(new THREE.Mesh(new THREE.CylinderGeometry(49.75, 49.75, 56.6, 48), dark)); a.rotation.z = Math.PI / 2; a.position.copy(P(0, s * (FACE - 29), ZS)); }
  // compute shelf + Pi 5 stand-in, PDB stand-in, waist RS03
  add(new THREE.Mesh(new THREE.BoxGeometry(186, 3, 120), new THREE.MeshStandardMaterial({ color: 0x6b4fa0 }))).position.copy(P(0, 0, ZB + 21.5));
  add(new THREE.Mesh(new THREE.BoxGeometry(85, 20, 56), new THREE.MeshStandardMaterial({ color: 0x2e8b57 }))).position.copy(P(-20, 0, ZB + 33));
  add(new THREE.Mesh(new THREE.BoxGeometry(90, 70, 12), new THREE.MeshStandardMaterial({ color: 0xb8860b }))).position.copy(P(-52, 0, F0 + 80));
  const wy = add(new THREE.Mesh(new THREE.CylinderGeometry(49.75, 49.75, 56.6, 48), dark)); wy.position.copy(P(0, 0, F0 - 40));
}
function hardpoints() {
  for (const h of mounts.hardpoints) for (const p of h.pts) {
    if (h.id === 'C') continue;
    const f = h.face; let g;
    if (f.includes('X')) g = new THREE.BoxGeometry(24, 24, 10); else if (f.includes('Y')) g = new THREE.BoxGeometry(10, 24, 24); else g = new THREE.BoxGeometry(24, 10, 24);
    const m = add(new THREE.Mesh(g, hp)); const o = f.includes('X') ? [Math.sign(p[0]) * 5, 0, 0] : f.includes('Y') ? [0, Math.sign(p[1]) * 5, 0] : [0, 0, -5];
    m.position.copy(P(p[0] + o[0], p[1] + o[1], p[2] + o[2]));
  }
}
const unitOf = (r) => r.unit.split(' ')[0];
async function pieces(opacity = 1, explode = false) {
  for (const r of rep.pieces) {
    const g = await load(`out/pieces/${r.name}.stl`); g.computeVertexNormals();
    const u = unitOf(r); let c = COL[u] ?? 0xffffff;
    if (r.kind === 'panel' && r.name.endsWith('_R')) c = new THREE.Color(c).offsetHSL(0, 0, -0.07).getHex();
    const m = new THREE.Mesh(g, mat(c, r.kind === 'panel' || r.kind === 'band' ? opacity : Math.max(opacity, 0.9)));
    // robot -> three axes
    m.matrixAutoUpdate = false;
    const M = new THREE.Matrix4().set(0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1);
    if (explode) {
      g.computeBoundingBox(); const c0 = new THREE.Vector3(); g.boundingBox.getCenter(c0);
      const rad = new THREE.Vector3(c0.x - 4, c0.y, 0); rad.normalize();
      const k = { door: 120, vent: 100, collar: 90, panel: 55, band: 45 }[r.kind];
      let dz = (c0.z - 1150) * 0.35 + (r.kind === 'door' ? 30 : 0);
      if (r.name.startsWith('hatch')) { dz += 40; }
      M.premultiply(new THREE.Matrix4().makeTranslation(rad.y * k, dz, rad.x * k));
      if (r.kind === 'collar') M.premultiply(new THREE.Matrix4().makeTranslation(Math.sign(c0.y) * 40, 0, Math.sign(c0.x) * 20));
    }
    m.matrix.copy(M); add(m);
  }
}
function arrow(from, to, col) {
  const d = new THREE.Vector3().subVectors(to, from); const L = d.length();
  const a = new THREE.ArrowHelper(d.clone().normalize(), from, L, col, 22, 14); a.line.material.linewidth = 3; root.add(a);
  const t = new THREE.Mesh(new THREE.CylinderGeometry(3.5, 3.5, L - 20, 10), new THREE.MeshBasicMaterial({ color: col }));
  t.position.copy(from.clone().add(d.clone().multiplyScalar((L - 20) / 2 / L))); t.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), d.clone().normalize()); root.add(t);
}
async function plates() {
  // every print in its chosen orientation inside a 256 mm P2S volume
  const list = rep.pieces;
  const cols = 6; let k = 0;
  for (const r of list) {
    const g = await load(`out/pieces/${r.name}_print.stl`);
    const gx = (k % cols) * 300, gz = Math.floor(k / cols) * 300; k++;
    const m = new THREE.Mesh(g, mat(COL[unitOf(r)] ?? 0xffffff)); m.rotation.x = -Math.PI / 2; m.updateMatrixWorld();
    const bb = new THREE.Box3().setFromObject(m); const c = new THREE.Vector3(); bb.getCenter(c);
    m.position.set(gx - c.x, -bb.min.y, gz - c.z); add(m);
    const box = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(256, 256, 256)), new THREE.LineBasicMaterial({ color: 0x30343a }));
    box.position.set(gx, 128, gz); root.add(box);
    const bed = new THREE.Mesh(new THREE.BoxGeometry(256, 2, 256), new THREE.MeshStandardMaterial({ color: 0x3a3f46 })); bed.position.set(gx, -1, gz); add(bed);
  }
  window.__plates = list.map(r => r.name);
}
if (MODE === 'plates') await plates();
else {
  if (XR) { frame(); hardpoints(); }
  await pieces(XR ? 0.22 : 1, MODE === 'explode');
  if (MODE === 'explode') { frame(); hardpoints(); }
  if (MODE === 'air') {
    const blue = 0x2f7de1, red = 0xe0452f;
    for (const s of [1, -1]) {
      arrow(P(-30, s * 210, F0 + 70), P(-10, s * 120, F0 + 85), blue);                 // side vent intake
      arrow(P(0, s * 60, F0 + 100), P(-20, s * 40, ZB), 0xb07fd6);                    // rising through the chest
      arrow(P(-40, s * 30, ZB + 60), P(-170, s * 30, ZB + 95), red);                   // out of the compute door louvres
      arrow(P(0, s * 110, F1 - 5), P(-10, s * 40, F1 + 70), red);                      // neck ring exhaust
    }
    arrow(P(140, 0, F0 - 70), P(50, 0, F0 - 50), blue);                                // abdomen band gaps
  }
}
const key = new THREE.DirectionalLight(0xffffff, 2.4); key.position.set(1400, 1900, 1100); key.castShadow = true;
key.shadow.mapSize.set(2048, 2048); Object.assign(key.shadow.camera, { left: -2400, right: 2400, top: 2400, bottom: -2400, near: 100, far: 8000 }); scene.add(key);
scene.add(new THREE.DirectionalLight(0xcfe3ff, 1.1).translateX(-1500).translateY(1500).translateZ(-1200));
scene.add(new THREE.HemisphereLight(0xffffff, 0x7a808a, 0.35));
const aspect = W / H; let cam;
const ortho = (h) => new THREE.OrthographicCamera(-h * aspect / 2, h * aspect / 2, h / 2, -h / 2, 1, 20000);
const ty = MODE === 'plates' ? 0 : (ZS - 100);
if (MODE === 'plates') { cam = ortho(Number(q.get('span') ?? 1750)); const gx = 750, gz = 450; cam.position.set(gx + 1300, 2200, gz + 2600); cam.lookAt(gx, 60, gz); }
else if (VIEW === 'front') { cam = ortho(Number(q.get('span') ?? 800)); cam.position.set(0, ty, 3000); cam.lookAt(0, ty, 0); }
else if (VIEW === 'side') { cam = ortho(Number(q.get('span') ?? 800)); cam.position.set(3000, ty, 0); cam.lookAt(0, ty, 0); }
else if (VIEW === 'back') { cam = ortho(Number(q.get('span') ?? 800)); cam.position.set(0, ty, -3000); cam.lookAt(0, ty, 0); }
else { cam = new THREE.PerspectiveCamera(24, aspect, 10, 30000); const d = Number(q.get('d') ?? 1500);
  const az = Number(q.get('az') ?? 35) * Math.PI / 180, el = Number(q.get('el') ?? 14) * Math.PI / 180;
  cam.position.set(d * Math.sin(az) * Math.cos(el), ty + d * Math.sin(el), d * Math.cos(az) * Math.cos(el)); cam.lookAt(0, ty, 0); }
renderer.render(scene, cam);
window.__done = true;
