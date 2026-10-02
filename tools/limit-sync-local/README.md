# Local Set Limits mirror

This optional laptop service mirrors taught hard and soft limits after Consul
receives a durable Pi acknowledgement. Run `just limit-sync-serve` from the
checkout to mirror. Start Consul on port 5173 with
`VITE_LIMIT_SYNC_URL=http://127.0.0.1:8790`, open Hardware, and paste the session
credential printed by the mirror terminal into **Local checkout mirror**.
Clear that field to disconnect. It lives only in tab memory until reload.

Each server start generates a new credential. A trusted operator may instead
provide `LIMIT_SYNC_TOKEN` in the server process environment. Do not put that
credential into a `VITE_*` variable or commit it. `LIMIT_SYNC_PORT` changes the
loopback port; `MARENGO_LIMIT_SYNC_BIN` selects a trusted local writer binary.

The listener binds `127.0.0.1`. POST requires `Authorization: Bearer <credential>`,
`Content-Type: application/json`, and an exact Origin of
`http://localhost:5173` or `http://127.0.0.1:5173`. Missing Origin is deliberately
refused, including non-browser clients; OPTIONS from an approved Origin allows
the credential header without invoking a writer.

Bodies are limited to 16 KiB with a five-second read deadline. Authenticated
session requests are limited to 30 per minute, and only one body/writer operation
is admitted at a time. The asynchronous writer has a five-second deadline and a
64 KiB combined output limit; timeout or excess output terminates it and waits
for process close before admitting another. Refusals/timeouts remain failures in
Consul even if the Pi already accepted its own write.

The CLI writes this checkout's `config/` and its configured URDF. It ignores
runtime config-directory overrides for this local operation. It mirrors the
supplied values; generation transactions and multi-file crash durability remain
separate work. The production Pi-hosted UI does not enable this laptop service.

Run `npm ci && npm test` here. The required repository gate also runs these
actual HTTP tests. They compile a harmless worker fixture and the real Rust CLI,
then use disposable checkout copies for real writes. No test opens CAN or
changes an installed robot tree.
