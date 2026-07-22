# compound-auto-learn

Local BFF for Consul Compound Tests **Auto Learn**. Holds `CURSOR_API_KEY`, validates stage envelopes, and returns teach landmarks.

## Run

```bash
cd tools/compound-auto-learn
npm ci
export CURSOR_API_KEY=...
export AUTO_LEARN_TOKEN=...   # shared secret; same value in Consul VITE_AUTO_LEARN_TOKEN
npm start                     # http://127.0.0.1:8787/v1/auto-learn
```

## Consul env

```bash
# consul/.env.local
VITE_AUTO_LEARN_URL=http://127.0.0.1:8787
VITE_AUTO_LEARN_TOKEN=...     # must match AUTO_LEARN_TOKEN
```

## Security

- Binds `127.0.0.1` only.
- Requires `Authorization: Bearer $AUTO_LEARN_TOKEN`.
- CORS allows local Vite (`127.0.0.1:5173` / `localhost:5173`) only.
- Opt-in session logs are allowlisted summaries sent to Cursor when Consul attaches them.

## Tests

```bash
npm test
```
