# compound-auto-learn

Local BFF for Consul Compound Tests **Auto Learn**. Holds `CURSOR_API_KEY`, validates stage envelopes, and returns teach landmarks.

## Run

```bash
cd tools/compound-auto-learn
npm ci
export CURSOR_API_KEY=...
export AUTO_LEARN_TOKEN=...   # shared secret; enter this value in the Consul Auto Learn panel
npm start                     # http://127.0.0.1:8787/v1/auto-learn
```

## Consul env

```bash
# consul/.env.local
VITE_AUTO_LEARN_URL=http://127.0.0.1:8787
```

Enter `AUTO_LEARN_TOKEN` in the panel at runtime. It stays in the browser tab
until reload. Gateway and Auto Learn credentials are separate.

## Security

- Binds `127.0.0.1` only.
- Requires `Authorization: Bearer $AUTO_LEARN_TOKEN`.
- CORS allows local Vite only (`localhost` / `127.0.0.1`, ports 5173–5199).
- Opt-in session logs are allowlisted summaries sent to Cursor when Consul attaches them.

## Tests

```bash
npm test
```
