# marengo-gateway

Operator gateway for Consul: HTTP CRUD plus WebTransport streaming of Chappe `Envelope` protobuf.

[ADR 0008](../../docs/decisions/0008-chappe-webtransport-transport.md).

## Run (with marengo-pi on Pi)

1. `systemctl start marengo-gateway`. HTTP `:8080`, HTTPS Consul UI `:8444`, WebTransport `:8443` on the Pi LAN (enabled on boot after `install-pi.sh`)
2. Open `https://marengo.local:8444` (accept the self-signed cert once)
3. `MARENGO_CHAPPE_SOCKET=/run/marengo/chappe.sock marengo-pi` for live telemetry
4. Local dev: `consul/.env.local` with `marengo.local` URLs (see `docs/pi-commissioning.md`)

```bash
# Local demo with static UI
cargo run -p marengo-gateway -- --demo \
  --https-listen 127.0.0.1:8444 --web-root consul/dist
```
