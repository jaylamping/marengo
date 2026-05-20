fn main() {
    marengo_support::init_tracing();

    let version = env!("CARGO_PKG_VERSION");
    tracing::info!(version, "probe: diagnostics scaffold");
}
