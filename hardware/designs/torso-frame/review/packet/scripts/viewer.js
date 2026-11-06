// Torso frame viewer. Torso coordinates: X lateral (robot left +), Y up, Z forward. Used by shoot.html (renders)
// and frame_viewer.html (interactive, data inlined). Query: view, explode, groups, ghost, data, labels.
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

const Q = new URLSearchParams(location.search);
const DATA = window.__FRAME_DATA || await (await fetch('parts.json')).json();
const VIEW = Q.get('view') || 'iso', EXPL = Number(Q.get('explode') || 0), SET = Q.get('data') || 'parts';
const GROUPS = Q.get('groups') ? Q.get('groups').split(',') : null, GHOST = Q.get('ghost') ? Q.get('ghost').split(',') : [];
const INTERACTIVE = !!window.__INTERACTIVE;

const W = innerWidth, H = innerHeight;
const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(devicePixelRatio); renderer.setSize(W, H); renderer.setClearColor(0xc9ced6);
document.body.appendChild(renderer.domElement);
const scene = new THREE.Scene();
scene.add(new THREE.HemisphereLight(0xffffff, 0x8a8f99, 1.1));
const sun = new THREE.DirectionalLight(0xffffff, 1.6); sun.position.set(300, 600, 500); scene.add(sun);
const fill = new THREE.DirectionalLight(0xffffff, 0.5); fill.position.set(-400, 200, -300); scene.add(fill);

function roundedShape(pts, radii) {
  const n = pts.length, s = new THREE.Shape();
  const P = pts.map(p => new THREE.Vector2(p[0], p[1]));
  const segs = [];
  for (let i = 0; i < n; i++) {
    const a = P[(i + n - 1) % n], b = P[i], c = P[(i + 1) % n], r = radii ? radii[i] : 0;
    const d1 = a.clone().sub(b).normalize(), d2 = c.clone().sub(b).normalize();
    const ang = Math.acos(THREE.MathUtils.clamp(d1.dot(d2), -1, 1));
    const t = Math.min(r / Math.tan(ang / 2), b.distanceTo(a) / 2 - 0.01, b.distanceTo(c) / 2 - 0.01);
    segs.push([b.clone().add(d1.clone().multiplyScalar(t)), b, b.clone().add(d2.clone().multiplyScalar(t))]);
  }
  s.moveTo(segs[0][0].x, segs[0][0].y);
  for (let i = 0; i < n; i++) { const [p, b, q] = segs[i]; s.lineTo(p.x, p.y); s.quadraticCurveTo(b.x, b.y, q.x, q.y); }
  s.closePath(); return s;
}
function geomFor(p) {
  if (p.type === 'box' || p.type === 'multibox') {
    const boxes = p.type === 'box' ? [[p.lo, p.hi]] : p.boxes; const gs = [];
    for (const [lo, hi] of boxes) {
      const g = new THREE.BoxGeometry(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);
      g.translate((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2); gs.push(g);
    }
    return gs;
  }
  if (p.type === 'cyl') {
    const g = new THREE.CylinderGeometry(p.r, p.r, p.h, 48);
    if (p.axis === 'x') g.rotateZ(Math.PI / 2); else if (p.axis === 'z') g.rotateX(Math.PI / 2);
    g.translate(p.c[0], p.c[1], p.c[2]); return [g];
  }
  if (p.type === 'poly') {
    const sh = roundedShape(p.pts, p.radii);
    for (const [x, y, r] of (p.holes || [])) { const h = new THREE.Path(); h.absarc(x, y, r, 0, Math.PI * 2, true); sh.holes.push(h); }
    const g = new THREE.ExtrudeGeometry(sh, { depth: p.t, bevelEnabled: false, curveSegments: 12 }); g.translate(0, 0, p.z0); return [g];
  }
}
const root = new THREE.Group(); scene.add(root);
const items = [];
const REGION = Q.get('region') ? Q.get('region').split(',').map(Number) : null;
function partBox(p) { const b = new THREE.Box3(); for (const g of geomFor(p)) { g.computeBoundingBox(); b.union(g.boundingBox); } return b; }
for (const p of DATA[SET]) {
  if (GROUPS && !GROUPS.includes(p.group) && !GHOST.includes(p.group)) continue;
  if (REGION) { const b = partBox(p); if (b.max.x < REGION[0] || b.min.x > REGION[1] || b.max.y < REGION[2] || b.min.y > REGION[3] || b.max.z < REGION[4] || b.min.z > REGION[5]) continue; }
  const ghost = GHOST.includes(p.group) && !(GROUPS && GROUPS.includes(p.group));
  const op = ghost ? 0.12 : p.opacity;
  const mat = new THREE.MeshStandardMaterial({ color: p.color, roughness: 0.55, metalness: 0.15, transparent: op < 1, opacity: op, depthWrite: op >= 1 });
  const grp = new THREE.Group();
  for (const g of geomFor(p)) {
    const m = new THREE.Mesh(g, mat); grp.add(m);
    if (!ghost) { const e = new THREE.LineSegments(new THREE.EdgesGeometry(g, 25), new THREE.LineBasicMaterial({ color: 0x1e2126, transparent: op < 1, opacity: Math.min(1, op + 0.2) })); grp.add(e); }
  }
  grp.position.set(p.explode[0] * EXPL, p.explode[1] * EXPL, p.explode[2] * EXPL);
  grp.userData = p; root.add(grp); items.push(grp);
}
// camera
const dirs = { iso: [1, 0.75, 1.35], iso_rear: [-1, 0.75, -1.35], iso_right: [-1, 0.75, 1.35], front: [0, 0, 1], back: [0, 0, -1], side: [1, 0, 0], top: [0, 1, 0.0001] };
const d = new THREE.Vector3(...dirs[VIEW]).normalize();
const cam = new THREE.OrthographicCamera(-1, 1, 1, -1, -5000, 5000);
cam.up.set(0, 1, 0); if (VIEW === 'top') cam.up.set(0, 0, -1);
const bb = new THREE.Box3(); for (const g of items) bb.expandByObject(g);
const ctr = Q.get('center') ? new THREE.Vector3(...Q.get('center').split(',').map(Number)) : bb.getCenter(new THREE.Vector3());
cam.position.copy(ctr).add(d.clone().multiplyScalar(2000)); cam.lookAt(ctr); cam.updateMatrixWorld();
function fit(margin) {
  const inv = cam.matrixWorldInverse; let minx = 1e9, maxx = -1e9, miny = 1e9, maxy = -1e9;
  const v = new THREE.Vector3();
  for (const g of items) { const b = new THREE.Box3().setFromObject(g); for (let i = 0; i < 8; i++) { v.set(i & 1 ? b.max.x : b.min.x, i & 2 ? b.max.y : b.min.y, i & 4 ? b.max.z : b.min.z).applyMatrix4(inv); minx = Math.min(minx, v.x); maxx = Math.max(maxx, v.x); miny = Math.min(miny, v.y); maxy = Math.max(maxy, v.y); } }
  const cw = (maxx - minx) * (1 + margin), ch = (maxy - miny) * (1 + margin), asp = W / H;
  let hw = cw / 2, hh = ch / 2; if (hw / hh > asp) hh = hw / asp; else hw = hh * asp;
  const cx = (minx + maxx) / 2, cy = (miny + maxy) / 2;
  cam.left = cx - hw; cam.right = cx + hw; cam.top = cy + hh; cam.bottom = cy - hh; cam.updateProjectionMatrix();
}
fit(Number(Q.get('margin') || 0.16));
const SCALE = Number(Q.get('scale') || 0); if (SCALE) { const hw = W / SCALE / 2, hh = H / SCALE / 2, cx = (cam.left + cam.right) / 2, cy = (cam.top + cam.bottom) / 2; cam.left = cx - hw; cam.right = cx + hw; cam.top = cy + hh; cam.bottom = cy - hh; cam.updateProjectionMatrix(); }
function proj(p) { const v = new THREE.Vector3(...p).project(cam); return [(v.x + 1) / 2 * W, (1 - v.y) / 2 * H]; }
function render() { renderer.render(scene, cam); }
render();
// labels and dimensions for the composer
const labels = [];
for (const g of items) { const p = g.userData; if (!p.balloon) continue; const b = new THREE.Box3().setFromObject(g); const c = b.getCenter(new THREE.Vector3());
  labels.push({ item: p.item, id: p.id, name: p.name, tag: p.tag || null, xy: proj([c.x, c.y, c.z]), bbox: [proj([b.min.x, b.min.y, b.min.z]), proj([b.max.x, b.max.y, b.max.z])] }); }
const dims = [];
for (const [p1, p2, ax, off, text] of (DATA.dims[VIEW] || [])) { const o = [0, 0, 0]; o['xyz'.indexOf(ax)] = off; const a = p1.map((v, i) => v + o[i]), b = p2.map((v, i) => v + o[i]);
  dims.push({ text, p1: proj(p1), p2: proj(p2), a: proj(a), b: proj(b) }); }
const ppm = Math.hypot(...proj([ctr.x + 100, ctr.y, ctr.z]).map((v, i) => v - proj([ctr.x, ctr.y, ctr.z])[i]));
window.__meta = { labels, dims, px_per_mm: ppm, view: VIEW, explode: EXPL };
window.__done = true;
if (INTERACTIVE) {
  const ctl = new OrbitControls(cam, renderer.domElement); ctl.addEventListener('change', render);
  const ui = document.createElement('div'); ui.style.cssText = 'position:fixed;left:12px;top:12px;font:14px system-ui;background:#fff;padding:8px 10px;border-radius:6px;opacity:.9';
  ui.innerHTML = 'Explode <input id=ex type=range min=0 max=1.4 step=0.02 value="' + EXPL + '" style="width:220px"> <span id=exv></span><br><span id=nm style="color:#333"></span>';
  document.body.appendChild(ui);
  const ex = ui.querySelector('#ex'); ex.oninput = () => { for (const g of items) g.position.set(g.userData.explode[0] * ex.value, g.userData.explode[1] * ex.value, g.userData.explode[2] * ex.value); ui.querySelector('#exv').textContent = Number(ex.value).toFixed(2); render(); };
  const ray = new THREE.Raycaster(), mouse = new THREE.Vector2();
  renderer.domElement.addEventListener('mousemove', e => { mouse.set(e.clientX / W * 2 - 1, -(e.clientY / H) * 2 + 1); ray.setFromCamera(mouse, cam); const hit = ray.intersectObjects(items, true)[0]; ui.querySelector('#nm').textContent = hit ? ('[' + hit.object.parent.userData.item + '] ' + hit.object.parent.userData.name) : ''; });
  addEventListener('resize', () => { location.reload(); });
}
