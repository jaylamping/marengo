<p align="center">
  <img src="../../docs/portraits/chappe.jpg" alt="Claude Chappe" width="420"/>
</p>

# chappe

**Chappe** — Marengo’s message bus.

Connects runtime components (control, safety, planner, vision) between Pi, Jetson, and diagnostics. Payloads are **`Vec<u8>` binary protobuf** from [`proto/`](../../proto/), encoded/decoded via [`armee-proto`](../armee-proto/) (`prost::Message::encode_to_vec` / `decode`). JSON is not used on the wire.

Planned NATS/MQTT backends use the same bytes-on-the-wire contract.
