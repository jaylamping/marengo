# Builds ../frame_viewer.html: single file, data inlined, three.js from a CDN, so it opens from disk or GitHub Pages.
import json
data = json.load(open('parts.json')); js = open('viewer.js').read()
html = f'''<!doctype html><html><head><meta charset="utf-8"><title>Marengo internal torso frame rev A</title>
<script type="importmap">{{"imports":{{"three":"https://unpkg.com/three@0.170.0/build/three.module.js","three/addons/":"https://unpkg.com/three@0.170.0/examples/jsm/"}}}}</script>
</head><body style="margin:0;overflow:hidden"><script>window.__FRAME_DATA={json.dumps(data)};window.__INTERACTIVE=true;</script>
<script type="module">{js}</script></body></html>'''
open('../frame_viewer.html', 'w').write(html); print('frame_viewer.html', len(html))
