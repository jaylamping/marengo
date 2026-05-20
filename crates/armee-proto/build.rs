use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = Path::new("../../proto");
    let mut protos = Vec::new();

    for entry in std::fs::read_dir(proto_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("proto") {
            println!("cargo:rerun-if-changed={}", path.display());
            protos.push(path);
        }
    }

    prost_build::Config::new().compile_protos(&protos, &[proto_root])?;
    Ok(())
}
