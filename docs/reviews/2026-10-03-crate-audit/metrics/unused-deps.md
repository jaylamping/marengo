# Unused dependencies (`cargo machete`)

```
Analyzing dependencies of crates in this directory...
cargo-machete found the following unused dependencies in this directory:
talleyrand -- ./crates/talleyrand/Cargo.toml:
	armee-kinematics
	thiserror
	tracing
fouche -- ./crates/fouche/Cargo.toml:
	thiserror
	tracing
marengo-host-metrics -- ./crates/marengo-host-metrics/Cargo.toml:
	thiserror
marengo-store -- ./crates/marengo-store/Cargo.toml:
	tracing
marengo-jetson -- ./bins/marengo-jetson/Cargo.toml:
	chappe
	fouche
	talleyrand
	tokio
wave-demo -- ./bins/wave-demo/Cargo.toml:
	berthier
marengo-gateway -- ./bins/marengo-gateway/Cargo.toml:
	marengo-support
	prost
	quinn
	thiserror
marengo-limit-sync -- ./bins/marengo-limit-sync/Cargo.toml:
	anyhow
marengo-pi -- ./bins/marengo-pi/Cargo.toml:
	marengo-homing
	marengo-support
marengo-log-cli -- ./bins/marengo-log-cli/Cargo.toml:
	tracing

If you believe cargo-machete has detected an unused dependency incorrectly,
you can add the dependency to the list of dependencies to ignore in the
`[package.metadata.cargo-machete]` section of the appropriate Cargo.toml.
For example:

[package.metadata.cargo-machete]
ignored = ["prost"]

You can also try running it with the `--with-metadata` flag for better accuracy,
though this may modify your Cargo.lock files.

Done!

```
