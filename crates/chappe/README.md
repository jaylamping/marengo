<p align="center">
  <img src="../../docs/portraits/chappe.jpg" alt="Claude Chappe" width="420"/>
</p>

# chappe

Chappe is Marengo's message bus.

It connects runtime components (control, safety, planner, vision) between Pi, Jetson, and diagnostics. Payloads are `Vec<u8>` binary protobuf from [`proto/`](../../proto/), encoded and decoded via [`armee-proto`](../armee-proto/) (`prost::Message::encode_to_vec` / `decode`). No JSON on the wire.

Operator gateway ([ADR 0008](../../docs/decisions/0008-chappe-webtransport-transport.md)): [`bins/marengo-gateway`](../../bins/marengo-gateway/) exposes HTTP CRUD and WebTransport to browsers (Consul). `marengo-pi` forwards publishes over a Unix socket (`MARENGO_CHAPPE_SOCKET`, default `/run/marengo/chappe.sock`).

Planned NATS/MQTT backends use the same bytes-on-the-wire contract.
