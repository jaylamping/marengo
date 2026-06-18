# proto

Wire-type definitions for all inter-service messages on [Chappe](../crates/chappe/).

- Edit `.proto` files under `marengo/v1/` (package `marengo.v1`). Never hand-edit generated Rust (`armee-proto`) or TypeScript (`consul/src/gen/`).
- Rust: [prost](https://github.com/tokio-rs/prost) via `crates/armee-proto/build.rs`
- TypeScript: [Buf](https://buf.build) + `@bufbuild/protobuf` from `consul/` (`npm run gen:proto`)
- On the wire: binary protobuf (`encode_to_vec` / `decode`), not JSON

## Prerequisites

- [`protoc`](https://grpc.io/docs/protoc-installation/) for `cargo build -p armee-proto`
- [`buf`](https://buf.build/docs/installation) for `consul` codegen (`npm run gen:proto`)
