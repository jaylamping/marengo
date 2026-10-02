# consul/src/lib/

## Responsibility
Chappe/gateway client library and shared utilities.

## Design
- `chappe-client.ts`: `connectChappeStream`, `dispatchEnvelope`, `fetchGatewayHealth`, `postEnableCommand`
- WebTransport with certificate hash pinning; HTTP length-prefixed stream fallback
- Protobuf decode via `@bufbuild/protobuf`

## Flow
`connectChappeStream` → try WebTransport → fallback HTTP SSE-style stream → return unsubscribe cleanup fn

## Integration
- Called by dashboard components and Zustand store init effects

- `runtime-credentials.ts`: tab-memory credential state; shared capability-specific headers.
- `chappe-transport.ts`: bounded typed admission before connected, public-first subscription without read credentials.
