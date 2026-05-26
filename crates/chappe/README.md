<p align="center">
  <img src="../../docs/portraits/chappe.jpg" alt="Claude Chappe" width="420"/>
</p>

# chappe

**Chappe** — Marengo’s message bus.

Connects runtime components (control, safety, planner, vision) between Pi, Jetson, and diagnostics. Payloads are **`Vec<u8>` binary protobuf** from [`proto/`](../../proto/), encoded/decoded via [`armee-proto`](../armee-proto/) (`prost::Message::encode_to_vec` / `decode`). JSON is not used on the wire.

**Operator gateway (ADR 0008):** [`bins/marengo-gateway`](../../bins/marengo-gateway/) exposes HTTP CRUD + WebTransport to browsers (Consul). `marengo-pi` forwards publishes over a Unix socket (`MARENGO_CHAPPE_SOCKET`, default `/run/marengo/chappe.sock`).

Planned NATS/MQTT backends use the same bytes-on-the-wire contract.
