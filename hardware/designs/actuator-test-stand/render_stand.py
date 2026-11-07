#!/usr/bin/env python3
"""Assembled and exploded pictures of the RS03 test stand from ./out (run build_stand.py first).

Renders with three.js in headless Chromium (Playwright). Needs an unpacked three npm package:
  npm pack three@0.160.0 && tar xzf three-0.160.0.tgz   -> pass its 'package' folder as THREE_DIR (env or argv[1]).
"""
import http.server
import json
import os
import shutil
import socketserver
import sys
import tempfile
import threading
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"
P = "marengo_teststand_"
WHITE, GREY, DARK, TEAL, ORANGE, BLUE, GOLD = "#ecebe6", "#8d939b", "#4a4e54", "#2a9d8f", "#f08a24", "#3d6fb6", "#d4a017"
ANCHOR_X = 200.0
PARTS = [  # file, colour, exploded offset (stand frame, mm), legend label
    ("ref_rail_2020x420", DARK, (0, 0, -70), "bench rail 2020 x 420 (Joey's extrusion)"),
    (P + "upright_rs03", TEAL, (0, 0, 0), "upright, RS03"),
    ("ref_rs03_vendor", GREY, (0, -110, 0), "RS03 (vendor model)"),
    (P + "adapter_rs03", ORANGE, (0, 50, 0), "output adapter, RS03"),
    ("ref_lever_2020x300", DARK, (0, 100, 0), "lever 2020 x 300 (Joey's extrusion)"),
    (P + "clamp_cap", ORANGE, (0, 150, 0), "lever clamp cap"),
    (P + "load_block", BLUE, (0, 150, 0), "load block (M8x60 pin)"),
    (P + "anchor", BLUE, (0, 80, -70), "scale anchor (M8 pin)"),
    (P + "lock_post", GOLD, (0, 80, -70), "T3 lock post"),
]
VIEWS = [
    ("STAND_RS03_ASSEMBLED.png", False, [((-260, 520, 330), "front three-quarter, lever side"), ((-380, -480, 300), "rear three-quarter, actuator side")],
     "RS03 test stand, assembled. Lever shown horizontal; scale anchor at X 200, T3 lock post at r 150 (both slide on the rail)."),
    ("STAND_RS03_EXPLODED.png", True, [((-300, 620, 420), "exploded, lever side")],
     "RS03 test stand, exploded. Fasteners: M4x16 (8 housing + 6 output), M5 + T-nuts, 4x M5x30 + nuts, 2x M8x60."),
]

PAGE = """<!doctype html><html><head><style>body{margin:0;background:#fff;font-family:Helvetica,Arial,sans-serif}
#t{position:absolute;top:14px;left:24px;font-size:22px} .s{position:absolute;top:52px;font-size:18px;color:#333}
#lg{position:absolute;bottom:14px;left:24px;font-size:17px;display:flex;flex-wrap:wrap;gap:6px 22px;width:calc(100% - 48px)}
#lg span i{display:inline-block;width:16px;height:16px;margin-right:6px;vertical-align:-2px}</style>
<script type="importmap">{"imports":{"three":"./three/build/three.module.js","three/addons/":"./three/examples/jsm/"}}</script></head>
<body><div id="t"></div><div id="lg"></div><script type="module">
import * as THREE from 'three'; import {STLLoader} from 'three/addons/loaders/STLLoader.js';
const cfg = await (await fetch('cfg.json')).json();
const W = cfg.w, H = cfg.h, n = cfg.views.length;
document.getElementById('t').textContent = cfg.title;
document.getElementById('lg').innerHTML = cfg.parts.map(p => `<span><i style="background:${p.col}"></i>${p.label}</span>`).join('');
const r = new THREE.WebGLRenderer({antialias:true, preserveDrawingBuffer:true}); r.setPixelRatio(1); r.setSize(W, H);
r.setScissorTest(true); r.setClearColor(0xffffff); document.body.prepend(r.domElement);
const scene = new THREE.Scene(); scene.add(new THREE.HemisphereLight(0xffffff, 0x777777, 1.7));
const box = new THREE.Box3(); const ld = new STLLoader();
for (const p of cfg.parts) { const g = await ld.loadAsync(p.file); g.computeVertexNormals();
  const m = new THREE.Mesh(g, new THREE.MeshStandardMaterial({color:p.col, roughness:0.65, metalness:0.05, flatShading:false}));
  m.position.set(...p.off); scene.add(m); const e = new THREE.LineSegments(new THREE.EdgesGeometry(g, 35), new THREE.LineBasicMaterial({color:0x222222, transparent:true, opacity:0.35}));
  e.position.copy(m.position); scene.add(e); box.expandByObject(m); }
const c = box.getCenter(new THREE.Vector3()), rad = box.getSize(new THREE.Vector3()).length() / 2;
cfg.views.forEach((v, i) => { const cam = new THREE.PerspectiveCamera(26, (W / n) / (H - 150), 1, 10000); cam.up.set(0, 0, 1);
  const d = new THREE.Vector3(...v.dir).normalize(); cam.position.copy(c).addScaledVector(d, rad / Math.tan(13 * Math.PI / 180) * 1.02); cam.lookAt(c);
  const key = new THREE.DirectionalLight(0xffffff, 1.6); key.position.copy(cam.position).add(new THREE.Vector3(0, 0, 400)); scene.add(key);
  r.setViewport(i * W / n, 70, W / n, H - 150); r.setScissor(i * W / n, 70, W / n, H - 150); r.render(scene, cam); scene.remove(key);
  const s = document.createElement('div'); s.className = 's'; s.style.left = (i * W / n + 24) + 'px'; s.textContent = v.label; document.body.appendChild(s); });
window.done = true;
</script></body></html>"""


SHOT = """const {chromium} = require('playwright');
(async () => { const [url, out, w, h] = process.argv.slice(2);
  const b = await chromium.launch({args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader']});
  const pg = await b.newPage({viewport: {width: +w, height: +h}, deviceScaleFactor: 2});
  await pg.goto(url); await pg.waitForFunction('window.done === true', null, {timeout: 180000});
  await pg.screenshot({path: out}); await b.close(); })();"""


def main(three_dir):
    """Python serves the page; Node's preinstalled Playwright takes the screenshot."""
    import subprocess
    work = Path(tempfile.mkdtemp())
    shutil.copytree(three_dir, work / "three")
    for name, *_ in PARTS:
        shutil.copy(OUT / f"{name}.stl", work / f"{name}.stl")
    (work / "index.html").write_text(PAGE)
    (work / "shot.js").write_text(SHOT)
    handler = lambda *a, **k: http.server.SimpleHTTPRequestHandler(*a, directory=str(work), **k)
    httpd = socketserver.TCPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    port = httpd.server_address[1]
    for fname, exploded, views, title in VIEWS:
        w, h = 1200 * len(views), 1000
        parts = []
        for name, col, off, label in PARTS:
            o = list(off) if exploded else [0, 0, 0]
            if name.endswith("anchor"):
                o[0] += ANCHOR_X
            parts.append(dict(file=f"{name}.stl", col=col, off=o, label=label))
        (work / "cfg.json").write_text(json.dumps(dict(w=w, h=h, title=title, parts=parts, views=[dict(dir=d, label=l) for d, l in views])))
        subprocess.run(["node", str(work / "shot.js"), f"http://127.0.0.1:{port}/index.html", str(HERE / fname), str(w), str(h)], check=True,
                       env={**os.environ, "NODE_PATH": subprocess.run(["npm", "root", "-g"], capture_output=True, text=True).stdout.strip()})
        print("wrote", fname)
    httpd.shutdown()


if __name__ == "__main__":
    main(Path(sys.argv[1] if len(sys.argv) > 1 else os.environ["THREE_DIR"]))
