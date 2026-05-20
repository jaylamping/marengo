use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let version = env!("CARGO_PKG_VERSION");
    tracing::info!(version, "probe: diagnostics scaffold");
    println!("probe {version}: diagnostics scaffold");
}
