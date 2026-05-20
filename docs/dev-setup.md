# Development setup

## Rust (Armée workspace)

```bash
rustup toolchain install stable
cargo build --workspace
```

### `protoc`

Required to build `armee-proto` (prost codegen).

```bash
# macOS
brew install protobuf

# Debian/Ubuntu
sudo apt-get install -y protobuf-compiler
```

Verify: `protoc --version`

## Consul (TypeScript)

```bash
cd consul
npm install
npm run gen:proto   # writes src/gen/ from ../proto
```

### `buf`

Required for TypeScript protobuf codegen.

```bash
# macOS
brew install bufbuild/buf/buf

# Or: npm install -g @bufbuild/buf (also listed as consul devDependency)
```

Verify: `buf --version`

## Regenerating wire types

1. Edit files under [`proto/`](../proto/).
2. Rust: `cargo build -p armee-proto` (or any workspace build).
3. TypeScript: `cd consul && npm run gen:proto`.

Never commit hand-edits to generated output; `consul/src/gen/` is gitignored.
