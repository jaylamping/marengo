const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch({ args: ['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader'] });
  const p = await b.newPage({ viewport: { width: 1600, height: 1200 }, deviceScaleFactor: Number(process.env.DPR || 1) });
  p.on('console', m => console.log('console:', m.text())); p.on('pageerror', e => console.log('ERR', e.message));
  for (const [name, qs] of JSON.parse(process.argv[2])) {
    await p.goto('http://localhost:8765/index.html?' + qs);
    await p.waitForFunction(() => window.__done, null, { timeout: 120000 });
    await p.locator('canvas').screenshot({ path: 'out/' + name + '.png' });
    console.log('ok', name);
  }
  await b.close();
})();
