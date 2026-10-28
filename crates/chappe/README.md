<p align="center">
  <img src="../../docs/portraits/chappe.jpg" alt="Claude Chappe" width="420"/>
</p>

# chappe

Chappe is Marengo's message bus.

It connects runtime components (control, safety, diagnostics) between Pi and tools. Payloads are `Vec<u8>` binary protobuf from [`proto/`](../../proto/), encoded and decoded via [`armee-proto`](../armee-proto/) (`prost::Message::encode_to_vec` / `decode`). No JSON on the wire.

Operator gateway ([ADR 0008](../../docs/decisions/0008-chappe-webtransport-transport.md)): [`bins/marengo-gateway`](../../bins/marengo-gateway/) is the Unix IPC listener (`/run/marengo/chappe.sock`) and exposes HTTP CRUD and WebTransport to browsers (Consul). `marengo-pi` connects as a client when `MARENGO_CHAPPE_SOCKET` is set, publishes telemetry and receives allowlisted command topics.

The IPC is Unix-only. There is no network transport between machines today; the earlier `chappe::transport` module was removed in the 2026-10-03 crate audit, and a Pi ↔ Jetson bridge is future work ([ADR 0014](../../docs/decisions/0014-jetson-perception-semantic-motion.md)). Any new transport keeps the same bytes-on-the-wire contract.
