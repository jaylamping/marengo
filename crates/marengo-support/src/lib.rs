//! Shared helpers for Marengo binaries (tracing, etc.).

use tracing_subscriber::filter::{EnvFilter, LevelFilter};

fn env_filter(directives: Option<&str>) -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .parse_lossy(directives.unwrap_or_default())
}

/// Initialize `tracing` with `RUST_LOG` / `.env` filter (call once from `main`).
pub fn init_tracing() {
    let directives = std::env::var("RUST_LOG").ok();
    tracing_subscriber::fmt()
        .with_env_filter(env_filter(directives.as_deref()))
        .init();
}

#[cfg(test)]
mod tests {
    use super::env_filter;

    #[test]
    fn unset_filter_defaults_to_info_and_explicit_directives_override_it() {
        assert_eq!(env_filter(None).to_string(), "info");
        assert_eq!(env_filter(Some("warn")).to_string(), "warn");
    }
}
