# proto

**Source of truth** for all inter-service wire types on [Chappe](../crates/chappe/).

- Edit `.proto` files under `marengo/v1/` (package `marengo.v1`) — never hand-edit generated Rust (`armee-proto`) or TypeScript (`consul/src/gen/`).
- Rust: [prost](https://github.com/tokio-rs/prost) via `crates/armee-proto/build.rs`
- TypeScript: [Buf](https://buf.build) + `@bufbuild/protobuf` from `consul/` (`npm run gen:proto`)
- On the wire: **binary protobuf** (`encode_to_vec` / `decode`), not JSON

## Prerequisites

- [`protoc`](https://grpc.io/docs/protoc-installation/) — required for `cargo build -p armee-proto`
- [`buf`](https://buf.build/docs/installation) — required for `consul` codegen (`npm run gen:proto`)
