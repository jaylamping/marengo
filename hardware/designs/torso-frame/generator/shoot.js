// Renders views listed on argv[2] as JSON [[name, query], ...] to out/<name>.png and out/<name>.json (labels, dims).
const { chromium } = require('playwright'); const fs = require('fs');
(async () => {
  const b = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
  const p = await b.newPage({ viewport: { width: Number(process.env.W || 1600), height: Number(process.env.H || 1200) }, deviceScaleFactor: Number(process.env.DPR || 2) });
  p.on('pageerror', e => console.log('ERR', e.message)); p.on('console', m => { if (m.type() === 'error') console.log('console:', m.text()); });
  fs.mkdirSync('out', { recursive: true });
  for (const [name, qs] of JSON.parse(process.argv[2])) {
    await p.goto('http://localhost:8766/shoot.html?' + qs);
    await p.waitForFunction(() => window.__done, null, { timeout: 180000 });
    await p.locator('canvas').screenshot({ path: 'out/' + name + '.png' });
    fs.writeFileSync('out/' + name + '.json', JSON.stringify(await p.evaluate(() => window.__meta)));
    console.log('ok', name);
  }
  await b.close();
})();
