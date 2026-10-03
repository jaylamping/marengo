//! Every protobuf module the build script compiles must be reachable from the
//! crate root (L-armee-proto-02). `build.rs` compiles all of `proto/`, but
//! only `marengo.v1` was `include!`d — a new package would build yet stay
//! silently unreachable. This test scans the build-script output directory and
//! requires each generated module to be named in `src/lib.rs`.
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use std::path::PathBuf;

#[test]
fn every_compiled_proto_module_is_included() {
    let out_dir = PathBuf::from(env!("OUT_DIR"));
    let lib_rs =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("armee-proto src/lib.rs is readable");
    let mut unmentioned = Vec::new();
    let entries = std::fs::read_dir(&out_dir).expect("OUT_DIR is readable");
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("utf8 stem");
        if !lib_rs.contains(stem) {
            unmentioned.push(stem.to_string());
        }
    }
    assert!(
        unmentioned.is_empty(),
        "compiled proto modules unreachable from lib.rs (add include!): {unmentioned:?}"
    );
}
