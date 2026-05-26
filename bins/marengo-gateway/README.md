# marengo-gateway

Operator gateway for Consul: **HTTP CRUD** + **WebTransport** streaming of Chappe `Envelope` protobuf.

See [ADR 0008](../../docs/decisions/0008-chappe-webtransport-transport.md).

## Run (with marengo-pi on Pi)

1. `systemctl start marengo-gateway`
2. `MARENGO_CHAPPE_SOCKET=/run/marengo/chappe.sock marengo-pi`
3. SSH tunnel from Mac (see `docs/pi-commissioning.md`)
