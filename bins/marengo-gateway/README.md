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

## Runtime access

[ADR0033](../../docs/decisions/0033-gateway-runtime-access.md) describes the shared
HTTP/HTTPS/WebTransport policy. Configure trusted credentials in the gateway's
startup environment (`scripts/env.example`), then enter the matching credential
in Consul's **Robot access** sheet. Scoped roles support control, calibration,
configuration, management and logs/audit; an operator credential covers all roles.
The historical `MARENGO_GATEWAY_LOG_TOKEN` retains administrator compatibility.
Credentials are captured once and comparisons do not log their values.

Protected requests require `Authorization: Bearer ...` (legacy
`x-marengo-log-token` remains supported). Browser origins must match the local
Vite origins, the configured robot HTTPS listener or an explicit
`MARENGO_GATEWAY_ALLOWED_ORIGINS` entry. Authenticated CLI requests may omit Origin.
Public health/ordinary telemetry are available without credentials. Sensitive
mixed subscriptions require the read capability. WebTransport receives a bounded
`GatewaySubscribe` with its runtime credential and sends typed admission before
envelopes; every stream includes runtime-connection invalidation.

Consul credentials stay in tab memory until reload; clearing a credential removes
it. Read credential changes reconnect telemetry and retire old producer facts.
Builds never fetch credentials from the Pi. `npm run build:qualified` supplies
11 disposable markers, checks every emitted asset and runs in CI/deploy builds.
Authorization is an additional request gate: attestation, joint allowlists,
rates, runtime authority and Davout's motor gates still apply. Software fixture
results do not qualify a deployment or physical movement.
