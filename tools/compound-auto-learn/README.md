# compound-auto-learn

Pi-first BFF for Consul Compound Tests **Auto Learn**. Holds `CURSOR_API_KEY`, validates stage envelopes, and returns teach landmarks.

Primary path: **Consul → marengo-gateway → loopback BFF on the Pi** (`127.0.0.1:8787`). Laptop Consul → `:8787` direct is no longer the primary setup.

## Production (Pi)

```bash
cd /opt/marengo/tools/compound-auto-learn   # or synced package path
npm ci
npm run build                              # emits dist/server.js (esbuild)
npm ci --omit=dev                          # runtime deps only (@cursor/sdk, zod)
export CURSOR_API_KEY=...
export AUTO_LEARN_TOKEN=...                # shared secret with gateway/Consul
# optional; defaults to /opt/marengo/var/auto-learn-cwd when creatable
export MARENGO_AUTO_LEARN_CWD=/opt/marengo/var/auto-learn-cwd
npm start                                  # node dist/server.js → 127.0.0.1:8787
```

Health (no auth, no secrets):

```bash
curl -s http://127.0.0.1:8787/health
# {"ok":true,"cursorKeyPresent":true}
```

Auto Learn:

```bash
curl -s http://127.0.0.1:8787/v1/auto-learn \
  -H "Authorization: Bearer $AUTO_LEARN_TOKEN" \
  -H "content-type: application/json" \
  -d @request.json
```

## Local development

```bash
cd tools/compound-auto-learn
npm ci
export CURSOR_API_KEY=...
export AUTO_LEARN_TOKEN=...
npm run dev                 # tsx src/server.ts
# or: npm run build && npm start
```

SDK cwd falls back to package `schema-cwd/` when `/opt/marengo/var/auto-learn-cwd` is not creatable (typical laptop).

## Consul / gateway

On the Pi, **marengo-gateway** proxies `POST /v1/auto-learn` to this loopback BFF.
Consul uses `${Chappe HTTP base}/v1/auto-learn` with header `x-marengo-auto-learn-token`
(matching `MARENGO_AUTO_LEARN_OPERATOR_TOKEN` on the Pi). Never bake that token into production www.

```bash
# consul/.env.local (Vite dev only)
VITE_AUTO_LEARN_OPERATOR_TOKEN=...   # = MARENGO_AUTO_LEARN_OPERATOR_TOKEN on Pi
```

Pi www: paste the same operator token in the Auto Learn panel (sessionStorage).

Internal gateway→BFF auth uses `AUTO_LEARN_TOKEN` (Bearer) — not exposed to the browser.

## Security

- Binds `127.0.0.1` only.
- Loopback `POST /v1/auto-learn` requires `Authorization: Bearer $AUTO_LEARN_TOKEN`.
- `GET /health` is unauthenticated; reports `cursorKeyPresent` as a boolean only (never the key).
- Opt-in session logs are allowlisted summaries sent to Cursor when Consul attaches them.

## Spike (SDK smoke)

```bash
CURSOR_API_KEY=... node scripts/spike-agent-prompt.mjs
# PASS / FAIL; non-zero exit on failure
```

## Tests

```bash
npm test
npm run typecheck
```
