// Head concept line-up (2026-11-06, thread "Design the robot's head"). ?head=1..20 builds one concept on the body.
// Original designs only: parametric mock-ups to judge style and size, not parts.
// Local frame is the robot frame (X forward, Y left, Z up) with z measured from H0 (top of the neck).
export const HEAD_NAMES = {
  1: 'Sentinel', 2: 'Crested arrow', 3: 'Cyclops lens', 4: 'Twin optics', 5: 'Scanner bar', 6: 'Brow guard', 7: 'Chevron',
  8: 'Listener', 9: 'Fins', 10: 'Screen face', 11: 'Grille knight', 12: 'Hunter', 13: 'Sensor stack', 14: 'Owl',
  15: 'Companion', 16: 'Crown', 17: 'Talking jaw', 18: 'Prism', 19: 'Hood', 20: 'Stereo brow',
};

export function buildHead(id, c) {
  const { THREE, loft, patch, add, P, interp, white, dark, setFacet, setClamp } = c;
  const H0 = c.F1 + 66;
  const S = c.scale;
  const PI = Math.PI, FR = PI / 2;
  const R = (rows) => rows.map(r => [H0 + r[0] * S, r[1] * S, r[2] * S, r[3] * S, r[4]]);
  const visor = new THREE.MeshPhysicalMaterial({ color: 0x0d0f12, roughness: 0.06, metalness: 0.45, clearcoat: 1, clearcoatRoughness: 0.04, side: THREE.DoubleSide });
  const matte = new THREE.MeshStandardMaterial({ color: 0x24272c, roughness: 0.6, metalness: 0.35, side: THREE.DoubleSide });
  const metal = new THREE.MeshStandardMaterial({ color: 0x6a7079, roughness: 0.3, metalness: 0.85 });
  const glow = (col, k = 2.2) => new THREE.MeshStandardMaterial({ color: col, emissive: col, emissiveIntensity: k, side: THREE.DoubleSide });
  const BLUE = 0x46b4ff, AMBER = 0xffa028, CYAN = 0x7af2ff, RED = 0xff3b30;
  const z = (v) => H0 + v * S;
  const body = (rows, mat = white, x0 = 0, th0 = 0, th1 = 2 * PI) => { const r = R(rows); return loft(r, r[0][0], r[r.length - 1][0], mat, 90, 160, x0, th0, th1); };
  // surface band on a lofted head: theta 0 = +Y side, pi/2 = front; a/b may be numbers or functions of z (absolute)
  const band = (rows, z0, z1, a, b, grow, mat, x0 = 0) => patch(R(rows), z(z0), z(z1), typeof a === 'function' ? a : () => a, typeof b === 'function' ? b : () => b, grow, mat, x0);
  // rounded pill on the surface: centre theta thc, centre z zc, half-height hz (mm), half-width w (rad)
  const pill = (rows, zc, hz, thc, w, grow, mat, p = 4) => { const f = (zz) => w * Math.pow(Math.max(0, 1 - Math.abs((zz - z(zc)) / (hz * S)) ** p), 1 / p); band(rows, zc - hz, zc + hz, (zz) => thc - f(zz), (zz) => thc + f(zz), grow, mat); };
  const eyes = (rows, zc, hz, ca, w, mat, grow = 1.6, p = 4) => { pill(rows, zc, hz, FR - ca, w, grow, mat, p); pill(rows, zc, hz, FR + ca, w, grow, mat, p); };
  const sym = (rows, z0, z1, ca, w, grow, mat, x0 = 0) => { band(rows, z0, z1, FR - ca - w, FR - ca + w, grow, mat, x0); band(rows, z0, z1, FR + ca - w, FR + ca + w, grow, mat, x0); };
  const box = (x, y, zz, sx, sy, sz, mat, rx = 0, ry = 0, rz = 0) => { const m = new THREE.Mesh(new THREE.BoxGeometry(sy * S, sz * S, sx * S), mat);
    m.position.copy(P(x * S, y * S, z(zz))); m.rotation.set(ry, rz, rx, 'ZYX'); return add(m); };   // rx roll about X, ry pitch about Y, rz yaw about Z
  const cylX = (x, y, zz, r, len, mat, seg = 48) => { const m = new THREE.Mesh(new THREE.CylinderGeometry(r * S, r * S, len * S, seg), mat); m.rotation.x = PI / 2; m.position.copy(P(x * S, y * S, z(zz))); return add(m); };
  const cylY = (x, y, zz, r, len, mat, seg = 48) => { const m = new THREE.Mesh(new THREE.CylinderGeometry(r * S, r * S, len * S, seg), mat); m.rotation.z = PI / 2; m.position.copy(P(x * S, y * S, z(zz))); return add(m); };
  const cylZ = (x, y, zz, r, len, mat, seg = 48) => { const m = new THREE.Mesh(new THREE.CylinderGeometry(r * S, r * S, len * S, seg), mat); m.position.copy(P(x * S, y * S, z(zz))); return add(m); };
  const torX = (x, y, zz, r, t, mat) => { const m = new THREE.Mesh(new THREE.TorusGeometry(r * S, t * S, 12, 64), mat); m.position.copy(P(x * S, y * S, z(zz))); return add(m); };
  const torY = (x, y, zz, r, t, mat) => { const m = new THREE.Mesh(new THREE.TorusGeometry(r * S, t * S, 12, 64), mat); m.rotation.y = PI / 2; m.position.copy(P(x * S, y * S, z(zz))); return add(m); };
  const sph = (x, y, zz, r, mat, sx = 1, sy = 1, sz = 1) => { const m = new THREE.Mesh(new THREE.SphereGeometry(r * S, 32, 20), mat); m.scale.set(sy, sz, sx); m.position.copy(P(x * S, y * S, z(zz))); return add(m); };
  // fin in the X-Z plane (outline points [x, z] in mm), thickness t along Y, tilted out by roll
  const fin = (pts, y, t, mat, roll = 0) => { const sh = new THREE.Shape(); pts.forEach(([px, pz], i) => i ? sh.lineTo(px * S, pz * S) : sh.moveTo(px * S, pz * S));
    const g = new THREE.ExtrudeGeometry(sh, { depth: t * S, bevelEnabled: true, bevelThickness: 1, bevelSize: 1, bevelSegments: 2 });
    const m = new THREE.Mesh(g, mat); const grp = new THREE.Group(); grp.add(m); m.rotation.y = -PI / 2; m.position.x = t * S / 2;  // shape x -> robot X (three z), shape y -> Z
    grp.position.copy(P(0, y * S, H0)); grp.rotation.z = roll; return add(grp); };
  // rows: [z, half-width Y, front depth, back depth, squareness]
  const STD = [[8, 42, 52, 52, 3.0], [22, 54, 68, 64, 3.6], [55, 60, 80, 74, 3.8], [100, 66, 84, 80, 3.8], [145, 66, 82, 82, 3.6], [175, 60, 76, 78, 3.4], [192, 46, 60, 64, 3.0], [200, 26, 34, 38, 2.6], [204, 4, 4, 4, 2]];
  const ROUND = [[8, 40, 48, 50, 2.4], [26, 54, 64, 64, 2.6], [70, 64, 78, 76, 2.7], [115, 68, 82, 80, 2.7], [158, 64, 78, 80, 2.6], [188, 48, 58, 62, 2.4], [204, 22, 26, 28, 2.2], [208, 3, 3, 3, 2]];
  const SQ = [[8, 44, 54, 54, 3.4], [24, 56, 70, 66, 4.2], [70, 62, 80, 76, 4.6], [130, 64, 82, 80, 4.8], [176, 63, 80, 80, 4.6], [196, 56, 70, 72, 3.6], [205, 40, 50, 52, 3.0], [209, 6, 6, 6, 2]];

  switch (id) {
    case 1: { // Sentinel: smooth helmet, wraparound tinted visor, eyes glowing behind it, ear pods with mic grilles
      body(STD);
      band(STD, 100, 152, FR - 0.40 * PI, FR + 0.40 * PI, 1.0, visor);
      eyes(STD, 125, 7, 0.10 * PI, 0.045 * PI, glow(BLUE, 1.6), 1.6);
      for (const s of [1, -1]) { cylY(-4, s * 64, 112, 24, 12, white); cylY(-4, s * 70, 112, 17, 2, matte); torY(-4, s * 70.5, 112, 20, 1.6, glow(BLUE, 1.2)); }
      break; }
    case 2: { // Ridge: faceted helmet, T-visor, central crest
      setFacet(true); body(STD);
      const nv = (zz) => 0.30 * PI - 0.22 * PI * Math.max(0, (z(126) - zz) / (z(126) - z(104)));
      band(STD, 104, 142, (zz) => FR - nv(zz), (zz) => FR + nv(zz), 1.2, visor);
      eyes(STD, 131, 5, 0.12 * PI, 0.04 * PI, glow(AMBER, 1.8), 1.8);
      setFacet(false);
      fin([[-90, 140], [-70, 196], [20, 212], [70, 194], [86, 168], [60, 172], [0, 186], [-60, 172]], 0, 10, white);
      break; }
    case 3: { // Cyclops lens: egg head, one big camera eye in a dark bezel with a light ring
      body(ROUND);
      band(ROUND, 70, 172, FR - 0.30 * PI, FR + 0.30 * PI, 0.8, matte);
      cylX(76, 0, 124, 32, 18, matte); cylX(84, 0, 124, 24, 10, visor); torX(89, 0, 124, 28, 2.2, glow(BLUE, 2.2));
      cylX(90, 0, 124, 9, 2, glow(CYAN, 1.2));
      for (const s of [1, -1]) cylX(72, s * 38, 84, 4, 6, matte);
      break; }
    case 4: { // Twin optics: two real camera eyes in barrels under an armoured brow
      body(STD);
      band(STD, 74, 152, FR - 0.30 * PI, FR + 0.30 * PI, 0.8, matte);
      box(84, 0, 156, 34, 128, 16, white, 0, 0.18);
      for (const s of [1, -1]) { cylX(84, s * 27, 128, 16, 26, metal); cylX(97, s * 27, 128, 12, 2, visor); torX(98, s * 27, 128, 8, 1.4, glow(BLUE, 2.4));
        box(70, s * 50, 92, 40, 22, 60, white, s * 0.25, 0, 0); }
      break; }
    case 5: { // Scanner bar: dark face, one horizontal light bar that sweeps
      body(SQ);
      band(SQ, 58, 176, FR - 0.32 * PI, FR + 0.32 * PI, 0.8, visor);
      band(SQ, 124, 132, FR - 0.28 * PI, FR + 0.28 * PI, 1.6, glow(0x2a6cff, 1.2));
      band(SQ, 123, 133, FR - 0.035 * PI, FR + 0.035 * PI, 2.2, glow(CYAN, 2.6));
      box(-30, 0, 203, 120, 22, 10, matte);
      break; }
    case 6: { // Brow guard: heavy overhanging brow, deep-set eyes, square jaw and chin plate
      const J = [[8, 44, 58, 52, 3.4], [24, 58, 74, 66, 3.8], [60, 62, 82, 76, 3.8], [110, 66, 84, 82, 3.6], [160, 64, 80, 82, 3.4], [196, 54, 66, 72, 3.0], [216, 32, 40, 46, 2.5], [224, 4, 4, 4, 2]];
      setFacet(true); body(J); setFacet(false);
      band(J, 108, 150, FR - 0.30 * PI, FR + 0.30 * PI, 0.8, matte);
      box(86, 0, 154, 40, 132, 20, white, 0, 0.12);
      for (const s of [1, -1]) sph(82, s * 25, 132, 7, glow(AMBER, 2.4), 0.5, 1.4, 0.7);
      box(80, 0, 40, 26, 64, 34, white);
      for (const s of [1, -1]) for (let k = 0; k < 3; k++) box(60 - k * 4, s * 58, 64 + k * 10, 26, 4, 4, matte);
      break; }
    case 7: { // Chevron: V-shaped visor split by a centre seam, slanted eyes
      body(STD);
      const a = (zz) => 0.06 * PI + 0.28 * PI * Math.min(1, Math.max(0, (zz - z(86)) / (z(150) - z(86))));
      band(STD, 86, 152, (zz) => FR - a(zz), (zz) => FR + a(zz), 1.0, visor);
      band(STD, 86, 152, FR - 0.012 * PI, FR + 0.012 * PI, 2.0, white);
      const t = (zz) => (zz - z(122)) / (z(140) - z(122));
      for (const s of [1, -1]) band(STD, 122, 140, (zz) => FR + s * (0.07 + 0.08 * t(zz)) * PI - 0.05 * PI, (zz) => FR + s * (0.07 + 0.08 * t(zz)) * PI + 0.05 * PI, 1.7, glow(BLUE, 1.8));
      break; }
    case 8: { // Listener: compact round head, big side ear arrays with status rings, oval visor
      body(ROUND);
      const e = (zz) => 0.32 * PI * Math.sqrt(Math.max(0, 1 - ((zz - z(126)) / (28 * S)) ** 2));
      band(ROUND, 98, 154, (zz) => FR - e(zz), (zz) => FR + e(zz), 1.0, visor);
      eyes(ROUND, 126, 6, 0.09 * PI, 0.04 * PI, glow(CYAN, 1.6), 1.6);
      for (const s of [1, -1]) { cylY(-4, s * 66, 120, 38, 16, white); cylY(-4, s * 74.5, 120, 29, 2, matte); torY(-4, s * 75, 120, 33, 2, glow(BLUE, 1.8));
        for (let k = 0; k < 6; k++) { const t = k * PI / 3; cylY(-4 + 16 * Math.cos(t), s * 75.6, 120 + 16 * Math.sin(t), 3, 1.5, visor, 16); } cylY(-4, s * 75.6, 120, 3, 1.5, visor, 16); }
      break; }
    case 9: { // Fins: swept fins at the back of the head that move to show mood
      body(STD);
      band(STD, 112, 146, FR - 0.36 * PI, FR + 0.36 * PI, 1.0, visor);
      eyes(STD, 128, 6, 0.10 * PI, 0.05 * PI, glow(BLUE, 1.6), 1.6);
      for (const s of [1, -1]) { fin([[-20, 140], [-120, 236], [-86, 190], [-60, 150]], s * 56, 6, white, -s * 0.22);
        fin([[-40, 104], [-126, 166], [-100, 132], [-70, 98]], s * 60, 5, matte, -s * 0.18); }
      break; }
    case 10: { // Screen face: a dark display panel framed by the helmet, drawn eyes
      body(STD);
      band(STD, 66, 172, FR - 0.33 * PI, FR + 0.33 * PI, 1.0, visor);
      const rr = (zz) => 0.05 * PI * Math.sqrt(Math.max(0, 1 - ((zz - z(128)) / (16 * S)) ** 6));
      for (const s of [1, -1]) band(STD, 112, 144, (zz) => FR + s * 0.11 * PI - rr(zz), (zz) => FR + s * 0.11 * PI + rr(zz), 1.6, glow(CYAN, 1.8));
      band(STD, 90, 94, FR - 0.07 * PI, FR + 0.07 * PI, 1.6, glow(CYAN, 1.4));
      break; }
    case 11: { // Grille knight: vertical slot grille over a dark face, eye slot, low crest
      setFacet(true); body(SQ); setFacet(false);
      band(SQ, 50, 150, FR - 0.30 * PI, FR + 0.30 * PI, 0.8, visor);
      band(SQ, 126, 138, FR - 0.24 * PI, FR + 0.24 * PI, 1.4, glow(BLUE, 1.0));
      for (let k = -4; k <= 4; k++) band(SQ, 52, 118, FR + k * 0.062 * PI - 0.012 * PI, FR + k * 0.062 * PI + 0.012 * PI, 2.6, white);
      fin([[-80, 180], [-50, 212], [40, 214], [76, 192], [30, 200], [-40, 196]], 0, 12, white);
      break; }
    case 12: { // Hunter: low swept wedge head, slanted eye slits
      const HW = [[8, 40, 56, 52, 3.0], [24, 52, 74, 70, 3.2], [60, 58, 86, 88, 3.4], [110, 62, 90, 100, 3.6], [150, 60, 80, 104, 3.4], [180, 50, 60, 96, 3.0], [198, 34, 40, 74, 2.6], [206, 6, 6, 10, 2]];
      setFacet(true); body(HW);
      band(HW, 50, 116, FR - 0.30 * PI, FR + 0.30 * PI, 0.8, matte);
      const t = (zz) => (zz - z(118)) / (z(138) - z(118));
      for (const s of [1, -1]) band(HW, 118, 138, (zz) => FR + s * (0.05 + 0.12 * t(zz)) * PI - 0.06 * PI, (zz) => FR + s * (0.05 + 0.12 * t(zz)) * PI + 0.06 * PI, 1.8, glow(RED, 1.8));
      setFacet(false);
      for (const s of [1, -1]) for (let k = 0; k < 3; k++) box(30 - k * 12, s * 60, 80, 6, 4, 30, matte, 0, 0, s * 0.3);
      break; }
    case 13: { // Sensor stack: squared head, exposed depth-camera bar, lidar puck on top
      body(SQ);
      band(SQ, 60, 176, FR - 0.30 * PI, FR + 0.30 * PI, 0.8, matte);
      box(86, 0, 140, 22, 104, 28, visor);
      for (const y of [-38, 38]) { cylX(97.5, y, 140, 9, 2, metal); cylX(98.6, y, 140, 6, 1, visor); }
      cylX(97.5, 0, 140, 5, 2, glow(RED, 0.9)); cylX(97.5, 16, 140, 4, 2, metal);
      eyes(SQ, 104, 4, 0.09 * PI, 0.05 * PI, glow(BLUE, 1.8), 1.6);
      cylZ(-8, 0, 218, 34, 22, matte); cylZ(-8, 0, 220, 34.6, 8, visor); cylZ(-8, 0, 230, 30, 2, metal);
      break; }
    case 14: { // Owl: wide round head, facial disc, two big lens eyes with amber irises
      const O = [[8, 42, 48, 50, 2.6], [26, 58, 62, 66, 2.8], [80, 70, 74, 78, 3.0], [130, 72, 78, 80, 3.2], [176, 66, 72, 76, 3.0], [206, 48, 52, 58, 2.6], [220, 20, 22, 24, 2.2], [224, 3, 3, 3, 2]];
      body(O);
      band(O, 70, 170, FR - 0.36 * PI, FR + 0.36 * PI, 0.6, matte);
      for (const s of [1, -1]) { cylX(72, s * 32, 126, 24, 14, metal); cylX(78, s * 32, 126, 20, 4, visor); torX(80.5, s * 32, 126, 13, 2.2, glow(AMBER, 2.4)); cylX(81, s * 32, 126, 5, 1, glow(0xfff1d0, 1.6)); }
      box(78, 0, 104, 22, 10, 30, white, 0, -0.3);
      break; }
    case 15: { // Companion: small soft head, simple tall oval eyes
      const C = ROUND.map(r => [r[0] * 0.94, r[1] * 0.96, r[2] * 0.96, r[3] * 0.96, r[4]]);
      body(C);
      const e = (zz) => 0.30 * PI * Math.sqrt(Math.max(0, 1 - ((zz - z(118)) / (40 * S)) ** 2));
      band(C, 78, 158, (zz) => FR - e(zz), (zz) => FR + e(zz), 1.0, visor);
      const o = (zz) => 0.035 * PI * Math.sqrt(Math.max(0, 1 - ((zz - z(122)) / (14 * S)) ** 2));
      for (const s of [1, -1]) band(C, 108, 136, (zz) => FR + s * 0.1 * PI - o(zz), (zz) => FR + s * 0.1 * PI + o(zz), 1.6, glow(CYAN, 1.8));
      break; }
    case 16: { // Crown: ring of small cameras around the forehead for all-round vision, main visor below
      body(STD);
      band(STD, 150, 168, 0, 2 * PI, 0.8, matte);
      for (let k = 0; k < 10; k++) { const t = k * 2 * PI / 10 + FR; band(STD, 155, 163, t - 0.025 * PI, t + 0.025 * PI, 1.6, glow(BLUE, 1.6)); }
      band(STD, 104, 146, FR - 0.34 * PI, FR + 0.34 * PI, 1.0, visor);
      eyes(STD, 124, 6, 0.10 * PI, 0.05 * PI, glow(BLUE, 1.4), 1.6);
      break; }
    case 17: { // Talking jaw: separate jaw plate under the visor, mouth light that moves with speech
      const U = [[86, 60, 80, 74, 3.8], [100, 66, 84, 80, 3.8], [145, 66, 82, 82, 3.6], [175, 60, 76, 78, 3.4], [192, 46, 60, 64, 3.0], [200, 26, 34, 38, 2.6], [204, 4, 4, 4, 2]];
      const Jw = [[10, 38, 58, 40, 3.2], [24, 50, 72, 50, 3.4], [60, 56, 80, 60, 3.6], [82, 58, 80, 64, 3.6]];
      body(U); body(Jw);
      body([[76, 54, 70, 70, 3], [92, 58, 76, 76, 3]], matte);
      band(U, 112, 150, FR - 0.36 * PI, FR + 0.36 * PI, 1.0, visor);
      eyes(U, 128, 6, 0.10 * PI, 0.05 * PI, glow(BLUE, 1.6), 1.6);
      band(Jw, 42, 50, FR - 0.13 * PI, FR + 0.13 * PI, 1.4, glow(BLUE, 1.6));
      break; }
    case 18: { // Prism: hard low-poly facets, one flat slanted visor plane
      const PR = [[8, 44, 52, 52, 3], [30, 58, 74, 68, 3], [90, 66, 86, 80, 3], [150, 64, 82, 80, 3], [196, 50, 62, 70, 3], [220, 22, 26, 34, 3], [226, 4, 4, 4, 2]];
      setFacet(true); body(PR);
      band(PR, 112, 148, FR - 0.24 * PI, FR + 0.24 * PI, 1.0, visor);
      eyes(PR, 129, 5, 0.09 * PI, 0.05 * PI, glow(BLUE, 1.8), 1.6);
      band(PR, 150, 196, FR - 0.008 * PI, FR + 0.008 * PI, 1.4, glow(BLUE, 0.8));
      setFacet(false);
      break; }
    case 19: { // Hood: protective cowl over a recessed dark face, eyes glowing inside
      const IN = [[8, 38, 46, 48, 2.8], [30, 50, 60, 64, 3.0], [90, 56, 66, 74, 3.2], [150, 56, 64, 74, 3.0], [196, 44, 50, 60, 2.6], [212, 20, 24, 30, 2.2]];
      const HD = [[60, 64, 84, 84, 3.2], [110, 70, 96, 88, 3.4], [160, 70, 96, 88, 3.4], [200, 60, 84, 78, 3.0], [222, 36, 52, 52, 2.6], [232, 4, 6, 6, 2]];
      body(IN, matte);
      body(HD, white, 0, FR + 0.30 * PI, FR - 0.30 * PI + 2 * PI);
      body([[198, 60, 84, 78, 3.0], [222, 36, 52, 52, 2.6], [232, 4, 6, 6, 2]], white);
      eyes(IN, 129, 5, 0.10 * PI, 0.05 * PI, glow(AMBER, 2.4), 1.6);
      band(IN, 60, 68, FR - 0.08 * PI, FR + 0.08 * PI, 1.4, glow(AMBER, 0.8));
      break; }
    case 20: { // Stereo brow: honest tech, visible stereo camera pair above a wide visor, chin mic grille
      setFacet(true); body(STD); setFacet(false);
      band(STD, 100, 146, FR - 0.38 * PI, FR + 0.38 * PI, 1.0, visor);
      eyes(STD, 122, 6, 0.10 * PI, 0.05 * PI, glow(BLUE, 1.5), 1.6);
      box(84, 0, 160, 16, 96, 18, visor, 0, 0.2);
      for (const y of [-32, 32]) { cylX(92.5, y, 160.5, 6.5, 2, metal); cylX(93.4, y, 160.5, 4.4, 1, visor); }
      cylX(92.5, 0, 160.5, 3, 2, glow(RED, 0.8));
      for (let k = -2; k <= 2; k++) for (let j = 0; j < 2; j++) cylX(78 - j * 2, k * 8, 46 - j * 8, 2, 4, visor, 12);
      break; }
  }
}
