# crates/armee-proto/src/

## Responsibility
Thin re-export layer over prost-generated `marengo.v1` module.

## Design
- `lib.rs` includes generated code via `include!` or `prost_build` output in `OUT_DIR`
- Public API mirrors proto message names

## Integration
- All Chappe publishers/subscribers use these types for wire compatibility
