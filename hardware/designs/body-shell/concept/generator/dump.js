// Export the tagged shell loft grids (robot frame, mm) for split/split_check.py. Needs the HTTP server on 8765.
const { chromium } = require('playwright'); const fs = require('fs');
(async () => {
  const b = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
  const p = await b.newPage({ viewport: { width: 400, height: 300 } });
  await p.goto('http://localhost:8765/index.html?dump=1&' + (process.argv[2] || ''));
  await p.waitForFunction(() => window.__done, null, { timeout: 120000 });
  const g = await p.evaluate(() => ({ grids: window.__grids, ZS: null }));
  fs.writeFileSync(process.argv[3] || 'out/grids.json', JSON.stringify(g)); console.log('grids', g.grids.length);
  await b.close();
})();
