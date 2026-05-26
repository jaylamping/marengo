# marengo-gateway

Operator gateway for Consul: **HTTP CRUD** + **WebTransport** streaming of Chappe `Envelope` protobuf.

See [ADR 0008](../../docs/decisions/0008-chappe-webtransport-transport.md).

## Run (with marengo-pi on Pi)

1. `systemctl start marengo-gateway` (HTTP `:8080`, WebTransport `:8443` on the Pi LAN)
2. `MARENGO_CHAPPE_SOCKET=/run/marengo/chappe.sock marengo-pi`
3. Consul on Mac: `consul/.env.local` with `marengo.local` URLs (see `docs/pi-commissioning.md`)
